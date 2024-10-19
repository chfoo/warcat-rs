use std::io::Write;

use indicatif::ProgressBar;

use crate::{
    compress::{CompressorConfig, Format, Level},
    dataseq::{SeqFormat, SeqReader},
    digest::{AlgorithmName, MultiHasher},
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
    multi_hasher: MultiHasher,
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
            compressor: CompressorConfig {
                format: compression,
                level: compression_level,
                ..Default::default()
            },
        };
        let output = Encoder::new(output, config);

        Ok(Self {
            progress_bar,
            input: SeqReader::new(BufferReader::new(input), seq_format),
            state: State::Header(output),
            multi_hasher: MultiHasher::new(&[
                AlgorithmName::Crc32,
                AlgorithmName::Crc32c,
                AlgorithmName::Xxh3,
            ]),
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
                WarcMessage::EndOfFile(_) => self.process_eof(writer),
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
        self.multi_hasher.update(&chunk.data);

        self.state = State::Block(writer);

        Ok(())
    }

    fn process_block_end(
        &mut self,
        writer: Encoder<EncStateBlock, ProgramOutput>,
        end: super::model::BlockEnd,
    ) -> anyhow::Result<()> {
        let checksum_map = self.multi_hasher.finish_u64();

        if let Some(expect) = end.crc32 {
            let actual = checksum_map[&AlgorithmName::Crc32] as u32;

            if expect != actual {
                anyhow::bail!("CRC32 mismatch: expect {}, actual {}", expect, actual)
            }
        } else if let Some(expect) = end.crc32c {
            let actual = checksum_map[&AlgorithmName::Crc32c] as u32;

            if expect != actual {
                anyhow::bail!("CRC32C mismatch: expect {}, actual {}", expect, actual)
            }
        } else if let Some(expect) = end.xxh3 {
            let actual = checksum_map[&AlgorithmName::Xxh3];

            if expect != actual {
                anyhow::bail!("Xxhash3 mismatch: expect {}, actual {}", expect, actual)
            }
        } else {
            anyhow::bail!("no checksum provided");
        }

        self.state = State::Header(writer.finish_block()?);

        Ok(())
    }
}
