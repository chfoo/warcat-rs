use std::path::{Path, PathBuf};

use crate::{
    app::{
        common::ReaderEvent,
        model::{self, WarcMessage},
    },
    dataseq::SeqWriter,
    digest::{AlgorithmName, MultiHasher},
    extract::WarcExtractor,
    header::WarcHeader,
};

use super::{
    arg::ExportCommand,
    common::ReaderPipeline,
    io::ProgramOutput,
    model::{EndOfFile, ExtractChunk, ExtractEnd, ExtractMetadata},
};

pub fn export(args: &ExportCommand) -> anyhow::Result<()> {
    let output_path = &args.output;
    let seq_format = args.format.into();

    for input_path in &args.input {
        let span = tracing::info_span!("export", path = ?input_path);
        let _span_guard = span.enter();

        let input = super::common::open_input(input_path)?;
        let output = super::common::open_output(output_path)?;

        tracing::info!("opened file");

        let compression_format = args.compression.try_into_native(input_path)?;
        let file_len = std::fs::metadata(input_path).map(|m| m.len()).ok();
        let writer = SeqWriter::new(output, seq_format);

        let mut exporter = Exporter::new(input_path, writer, args.no_block, args.extract);

        ReaderPipeline::new(
            |event| match event {
                ReaderEvent::Header {
                    header,
                    record_boundary_position,
                } => exporter.process_header(&header, record_boundary_position),
                ReaderEvent::Block { data } => exporter.process_block(data),
            },
            input,
            compression_format,
            file_len,
        )?
        .run()?;

        exporter.finish()?;

        tracing::info!("closed file");
    }

    Ok(())
}

struct Exporter {
    input_path: PathBuf,
    writer: SeqWriter<ProgramOutput>,
    hasher: MultiHasher,
    no_block: bool,
    extractor: Option<WarcExtractor>,
    extract_hasher: MultiHasher,
    buf: Vec<u8>,
}

impl Exporter {
    fn new(
        input_path: &Path,
        writer: SeqWriter<ProgramOutput>,
        no_block: bool,
        extract: bool,
    ) -> Self {
        let hasher = MultiHasher::new(&[
            AlgorithmName::Crc32,
            AlgorithmName::Crc32c,
            AlgorithmName::Xxh3,
        ]);
        let extract_hasher = MultiHasher::new(&[
            AlgorithmName::Crc32,
            AlgorithmName::Crc32c,
            AlgorithmName::Xxh3,
        ]);

        let extractor = if extract {
            Some(WarcExtractor::new())
        } else {
            None
        };

        Self {
            input_path: input_path.to_path_buf(),
            writer,
            hasher,
            no_block,
            extractor,
            extract_hasher,
            buf: Vec::new(),
        }
    }

    fn process_header(
        &mut self,
        header: &WarcHeader,
        record_boundary_position: u64,
    ) -> anyhow::Result<()> {
        let message = WarcMessage::Metadata(model::Metadata {
            file: self.input_path.to_path_buf(),
            position: record_boundary_position,
        });
        self.writer.put(message)?;

        let message = WarcMessage::Header(model::Header {
            version: header.version.clone(),
            fields: header
                .fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        });
        self.writer.put(message)?;

        self.message_extract_header(header)?;

        Ok(())
    }

    fn message_extract_header(&mut self, header: &WarcHeader) -> anyhow::Result<()> {
        if let Some(extractor) = &mut self.extractor {
            extractor.read_header(header)?;

            let message = WarcMessage::ExtractMetadata(ExtractMetadata {
                has_content: extractor.has_content(),
                file_path_components: extractor.file_path_components(),
                is_truncated: extractor.is_truncated(),
            });
            self.writer.put(message)?;
        }

        Ok(())
    }

    fn process_block(&mut self, data: &[u8]) -> anyhow::Result<()> {
        if !self.no_block {
            self.message_block_chunk(data)?;
        }

        self.message_extract_chunk(data)?;

        Ok(())
    }

    fn message_block_chunk(&mut self, data: &[u8]) -> anyhow::Result<()> {
        if data.is_empty() {
            let checksum_map = self.hasher.finish_u64();
            let message = WarcMessage::BlockEnd(model::BlockEnd {
                crc32: Some(checksum_map[&AlgorithmName::Crc32] as u32),
                crc32c: Some(checksum_map[&AlgorithmName::Crc32c] as u32),
                xxh3: Some(checksum_map[&AlgorithmName::Xxh3]),
            });
            self.writer.put(message)?;
        } else {
            let message = WarcMessage::BlockChunk(model::BlockChunk {
                data: data.to_vec(),
            });
            self.hasher.update(data);
            self.writer.put(message)?;
        }

        Ok(())
    }

    fn message_extract_chunk(&mut self, data: &[u8]) -> anyhow::Result<()> {
        if let Some(extractor) = &mut self.extractor {
            if !extractor.has_content() {
                return Ok(());
            }

            if data.is_empty() {
                let checksum_map = self.extract_hasher.finish_u64();
                let message = WarcMessage::ExtractEnd(ExtractEnd {
                    crc32: Some(checksum_map[&AlgorithmName::Crc32] as u32),
                    crc32c: Some(checksum_map[&AlgorithmName::Crc32c] as u32),
                    xxh3: Some(checksum_map[&AlgorithmName::Xxh3]),
                });
                self.writer.put(message)?;
            } else {
                extractor.extract_data(data, &mut self.buf)?;

                let message = WarcMessage::ExtractChunk(ExtractChunk {
                    data: self.buf.clone(),
                });
                self.extract_hasher.update(&self.buf);
                self.writer.put(message)?;

                self.buf.clear();
            }
        }

        Ok(())
    }

    fn finish(&mut self) -> anyhow::Result<()> {
        self.writer.put(WarcMessage::EndOfFile(EndOfFile {}))?;

        Ok(())
    }
}
