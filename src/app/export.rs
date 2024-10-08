use crate::{
    app::{
        common::ReaderEvent,
        model::{self, WarcMessage},
    },
    dataseq::SeqWriter,
    digest::{AlgorithmName, MultiHasher},
};

use super::{arg::ExportCommand, common::ReaderPipeline};

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
        let mut writer = SeqWriter::new(output, seq_format);
        let mut hasher = MultiHasher::new(&[
            AlgorithmName::Crc32,
            AlgorithmName::Crc32c,
            AlgorithmName::Xxh3,
        ]);

        ReaderPipeline::new(
            |event| match event {
                ReaderEvent::Header {
                    header,
                    record_boundary_position,
                } => {
                    let message = WarcMessage::Metadata(model::Metadata {
                        file: input_path.to_path_buf(),
                        position: record_boundary_position,
                    });
                    writer.put(message)?;

                    let message = WarcMessage::Header(model::Header {
                        version: header.version,
                        fields: Vec::from_iter(header.fields),
                    });
                    writer.put(message)?;

                    Ok(())
                }
                ReaderEvent::Block { data } => {
                    if data.is_empty() {
                        let checksum_map = hasher.finish_u64();
                        let message = WarcMessage::BlockEnd(model::BlockEnd {
                            crc32: Some(checksum_map[&AlgorithmName::Crc32] as u32),
                            crc32c: Some(checksum_map[&AlgorithmName::Crc32c] as u32),
                            xxh3: Some(checksum_map[&AlgorithmName::Xxh3]),
                        });
                        writer.put(message)?;
                    } else {
                        let message = WarcMessage::BlockChunk(model::BlockChunk {
                            data: data.to_vec(),
                        });
                        hasher.update(data);
                        writer.put(message)?;
                    }

                    Ok(())
                }
            },
            input,
            compression_format,
            file_len,
        )?
        .run()?;

        tracing::info!("closed file");
    }

    Ok(())
}
