use std::io::Read;

use anyhow::Context;
use indicatif::ProgressBar;

use crate::{
    app::io::{ProgramInput, ProgramOutput},
    compress::Format,
    dataseq::{SeqFormat, SeqWriter},
    io::LogicalPosition,
    read::{Reader, ReaderConfig},
};

use super::{
    arg::ExportCommand,
    model::{self, WarcMessage},
};

pub fn export(args: &ExportCommand) -> anyhow::Result<()> {
    let input_path = &args.input;
    let output_path = &args.output;

    let span = tracing::info_span!("export file", path = ?input_path);
    let _span_guard = span.enter();

    let input = ProgramInput::open(input_path).context("opening input file failed")?;
    let output = ProgramOutput::open(output_path).context("opening output file failed")?;

    tracing::info!("opened file");

    let compression_format = args.compression.try_into_native(input_path)?;
    let seq_format = args.format.into();
    let file_len = std::fs::metadata(input_path).map(|m| m.len()).ok();

    let mut exporter = Exporter::new(input, output, compression_format, seq_format, file_len)?;
    exporter.run()?;

    Ok(())
}

pub struct Exporter {
    progress_bar: ProgressBar,
    input: Option<Reader<crate::read::StateHeader, ProgramInput>>,
    output: SeqWriter<ProgramOutput>,
    buf: Vec<u8>,
}

impl Exporter {
    fn new(
        input: ProgramInput,
        output: ProgramOutput,
        compression_format: Format,
        seq_format: SeqFormat,
        file_len: Option<u64>,
    ) -> anyhow::Result<Exporter> {
        let progress_bar = super::progress::make_bytes_progress_bar(file_len);

        let config = ReaderConfig { compression_format };
        let reader = Reader::new(input, config)?;
        let writer = SeqWriter::new(output, seq_format);

        Ok(Exporter {
            progress_bar,
            input: Some(reader),
            output: writer,
            buf: Vec::new(),
        })
    }

    fn run(&mut self) -> anyhow::Result<()> {
        super::progress::global_progress_bar().add(self.progress_bar.clone());

        loop {
            let reader = self.export_header()?;
            let mut reader = self.export_block(reader)?;

            if !reader.has_next_record()? {
                break;
            }

            self.input = Some(reader);
        }

        tracing::info!("closing file");
        self.progress_bar.finish();
        super::progress::global_progress_bar().remove(&self.progress_bar);

        Ok(())
    }

    fn write_message(&mut self, message: &WarcMessage) -> anyhow::Result<()> {
        self.output.put(message)?;
        Ok(())
    }

    fn export_header(&mut self) -> anyhow::Result<Reader<crate::read::StateBlock, ProgramInput>> {
        let reader = self.input.take().unwrap();

        let message = model::WarcMessage::ExportMetadata(model::ExportMetadata {
            file: "".into(),
            position: reader.record_boundary_position(),
        });
        self.progress_bar.set_position(reader.logical_position());
        self.write_message(&message)?;

        let (header, reader) = reader.read_header().context("invalid WARC header")?;

        let record_id = header
            .fields
            .get("WARC-Record-ID")
            .map(|s| s.as_str())
            .unwrap_or_default();
        self.progress_bar
            .println(format!("Processing record {}", record_id));

        let message = model::WarcMessage::Header(model::Header {
            version: header.version,
            fields: Vec::from_iter(header.fields),
        });
        self.write_message(&message)?;

        Ok(reader)
    }

    fn export_block(
        &mut self,
        mut reader: Reader<crate::read::StateBlock, ProgramInput>,
    ) -> anyhow::Result<Reader<crate::read::StateHeader, ProgramInput>> {
        let mut crc32c = 0u32;

        loop {
            self.buf.resize(4096, 0);

            let read_length = reader.read(&mut self.buf)?;
            self.buf.truncate(read_length);

            if read_length == 0 {
                break;
            }

            crc32c = crc32c::crc32c_append(crc32c, &self.buf);

            let message = model::WarcMessage::BlockChunk(model::BlockChunk {
                data: self.buf.clone(),
            });
            self.write_message(&message)?;
            self.progress_bar.set_position(reader.logical_position());
        }

        let message = model::WarcMessage::BlockEnd(model::BlockEnd { crc32c });
        self.write_message(&message)?;

        Ok(reader.finish_block()?)
    }
}
