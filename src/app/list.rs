use crate::{app::common::ReaderEvent, dataseq::SeqWriter};

use super::{arg::ListCommand, common::ReaderPipeline};

pub fn list(args: &ListCommand) -> anyhow::Result<()> {
    let input_path = &args.input;
    let output_path = &args.output;

    let span = tracing::info_span!("list file", path = ?input_path);
    let _span_guard = span.enter();

    let input = super::common::open_input(input_path)?;
    let output = super::common::open_output(output_path)?;

    tracing::info!("opened file");

    let compression_format = args.compression.try_into_native(input_path)?;
    let file_len = std::fs::metadata(input_path).map(|m| m.len()).ok();
    let seq_format = args.format.into();
    let mut writer = SeqWriter::new(output, seq_format);

    ReaderPipeline::new(
        |event| match event {
            ReaderEvent::Header {
                header,
                record_boundary_position,
            } => {
                let mut values = Vec::new();

                for name in &args.field {
                    if name == ":position" {
                        values.push(serde_json::Value::Number(record_boundary_position.into()));
                    } else {
                        let value = header.fields.get(name).cloned().unwrap_or_default();
                        values.push(serde_json::Value::String(value));
                    }
                }

                writer.put(values)?;

                Ok(())
            }
            ReaderEvent::Block { data: _ } => Ok(()),
        },
        input,
        compression_format,
        file_len,
    )?
    .run()?;

    tracing::info!("closed file");

    Ok(())
}
