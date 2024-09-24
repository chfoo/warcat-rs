use std::io::Write;

use indicatif::ProgressBar;

use crate::{
    compress::{Format, Level},
    dataseq::{SeqFormat, SeqReader},
    header::WarcHeader,
    io::{BufferReader, LogicalPosition},
    warc::{EncStateBlock, EncStateHeader, Encoder, EncoderConfig},
};

use super::{
    arg::ImportCommand,
    io::{ProgramInput, ProgramOutput},
    model::WarcMessage,
};

pub fn import(args: &ImportCommand) -> anyhow::Result<()> {
    let output_path = &args.output;
    let seq_format = args.format.into();
    let format = args.compression.try_into_native(output_path)?;
    let level = args.compression_level.into();

    for input_path in &args.input {
        let span = tracing::info_span!("import", path = ?input_path);
        let _span_guard = span.enter();

        let input = super::common::open_input(input_path)?;
        let output = super::common::open_output(output_path)?;

        tracing::info!("opened file");

        let file_len = std::fs::metadata(input_path).map(|m| m.len()).ok();

        Importer::new(input, output, seq_format, (format, level), file_len)?.run()?;

        tracing::info!("closed file");
    }

    Ok(())
}

enum State {
    None,
    Header(Encoder<EncStateHeader, ProgramOutput>),
    Block(Encoder<EncStateBlock, ProgramOutput>),
    Done,
}

impl State {
    fn take(&mut self) -> Self {
        std::mem::replace(self, Self::None)
    }
}

struct Importer {
    progress_bar: ProgressBar,
    input: SeqReader<BufferReader<ProgramInput>>,
    state: State,
    crc32c: u32,
}

impl Importer {
    fn new(
        input: ProgramInput,
        output: ProgramOutput,
        seq_format: SeqFormat,
        (compression, compression_level): (Format, Level),
        file_len: Option<u64>,
    ) -> anyhow::Result<Self> {
        let progress_bar = super::progress::make_bytes_progress_bar(file_len);
        let config = EncoderConfig {
            compression,
            compression_level,
        };
        let output = Encoder::new(output, config);

        Ok(Self {
            progress_bar,
            input: SeqReader::new(BufferReader::new(input), seq_format),
            state: State::Header(output),
            crc32c: 0,
        })
    }

    fn run(&mut self) -> anyhow::Result<()> {
        super::progress::global_progress_bar().add(self.progress_bar.clone());

        loop {
            let message = self.input.get()?;

            if let Some(message) = message {
                self.process_message(message)?;
                self.progress_bar
                    .set_position(self.input.get_ref().logical_position());

                debug_assert!(!matches!(self.state, State::None));
            } else {
                break;
            }

            if matches!(self.state, State::Done) {
                break;
            }
        }

        self.progress_bar.finish();
        super::progress::global_progress_bar().remove(&self.progress_bar);

        Ok(())
    }

    fn process_message(&mut self, message: WarcMessage) -> anyhow::Result<()> {
        let state = self.state.take();

        match state {
            State::Header(writer) => match message {
                WarcMessage::Header(header) => self.process_header(writer, header),
                WarcMessage::EndOfFile => self.process_eof(writer),
                WarcMessage::Metadata(_) => {
                    self.state = State::Header(writer);
                    Ok(())
                }
                _ => anyhow::bail!("invalid state: expected header"),
            },
            State::Block(writer) => match message {
                WarcMessage::BlockChunk(chunk) => self.process_block(writer, chunk),
                WarcMessage::BlockEnd(end) => self.process_block_end(writer, end),
                _ => anyhow::bail!("invalid state: expected block"),
            },
            _ => unreachable!(),
        }
    }

    fn process_header(
        &mut self,
        writer: Encoder<EncStateHeader, ProgramOutput>,
        header: super::model::Header,
    ) -> anyhow::Result<()> {
        let mut warc_header = WarcHeader::empty();
        warc_header.version = header.version;
        warc_header.fields.extend(header.fields);

        let writer = writer.write_header(&warc_header)?;

        let record_id = warc_header
            .fields
            .get("WARC-Record-ID")
            .map(|s| s.as_str())
            .unwrap_or_default();
        self.progress_bar
            .println(format!("Processing record {}", record_id));

        self.state = State::Block(writer);

        Ok(())
    }

    fn process_eof(
        &mut self,
        writer: Encoder<EncStateHeader, ProgramOutput>,
    ) -> anyhow::Result<()> {
        writer.finish()?;
        self.state = State::Done;
        Ok(())
    }

    fn process_block(
        &mut self,
        mut writer: Encoder<EncStateBlock, ProgramOutput>,
        chunk: super::model::BlockChunk,
    ) -> anyhow::Result<()> {
        writer.write_all(&chunk.data)?;
        self.crc32c = crc32c::crc32c_append(self.crc32c, &chunk.data);

        self.state = State::Block(writer);

        Ok(())
    }

    fn process_block_end(
        &mut self,
        writer: Encoder<EncStateBlock, ProgramOutput>,
        end: super::model::BlockEnd,
    ) -> anyhow::Result<()> {
        if end.crc32c != self.crc32c {
            anyhow::bail!(
                "CRC32C mismatch: expect {}, actual {}",
                end.crc32c,
                self.crc32c
            )
        }

        self.crc32c = 0;
        self.state = State::Header(writer.finish_block()?);

        Ok(())
    }
}
