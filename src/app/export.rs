use crate::{
    app::{
        common::ReaderEvent,
        model::{self, WarcMessage},
    },
    dataseq::SeqWriter,
};

use super::{arg::ExportCommand, common::ReaderPipeline};

pub fn export(args: &ExportCommand) -> anyhow::Result<()> {
    let input_path = &args.input;
    let output_path = &args.output;

    let span = tracing::info_span!("export file", path = ?input_path);
    let _span_guard = span.enter();

    let input = super::common::open_input(input_path)?;
    let output = super::common::open_output(output_path)?;

    tracing::info!("opened file");

    let compression_format = args.compression.try_into_native(input_path)?;
    let file_len = std::fs::metadata(input_path).map(|m| m.len()).ok();
    let seq_format = args.format.into();
    let mut writer = SeqWriter::new(output, seq_format);
    let mut checksum = 0u32;

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
                    let message = WarcMessage::BlockEnd(model::BlockEnd { crc32c: checksum });
                    writer.put(message)?;
                    checksum = 0;
                } else {
                    let message = WarcMessage::BlockChunk(model::BlockChunk {
                        data: data.to_vec(),
                    });
                    checksum = crc32c::crc32c_append(checksum, data);
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

    Ok(())
}
