use std::{
    io::{Cursor, Write},
    path::PathBuf,
};

use tempfile::NamedTempFile;

use crate::{
    app::common::{ReaderEvent, ReaderPipeline},
    error::GeneralError,
    extract::{WarcExtractor, FILENAME_CONFLICT_MARKER},
    header::WarcHeader,
};

use super::{arg::ExtractCommand, filter::FieldFilter};

// FIXME: continuation records not yet implemented.

pub fn extract(args: &ExtractCommand) -> anyhow::Result<()> {
    let output_dir = &args.output;

    if !output_dir.is_dir() {
        anyhow::bail!("not a directory: {:?}", output_dir)
    }

    let mut filter = FieldFilter::new();

    for rule in &args.include {
        filter.add_include(rule);
    }
    for rule in &args.include_pattern {
        filter.add_include_pattern(rule)?;
    }
    for rule in &args.exclude {
        filter.add_exclude(rule);
    }
    for rule in &args.exclude_pattern {
        filter.add_exclude_pattern(rule)?;
    }

    for input_path in &args.input {
        let span = tracing::info_span!("extract", path = ?input_path);
        let _span_guard = span.enter();

        let input = super::common::open_input(input_path)?;

        tracing::info!("opened file");

        let compression_format = args.compression.try_into_native(input_path)?;
        let file_len = std::fs::metadata(input_path).map(|m| m.len()).ok();

        let mut extractor = Extractor::new(output_dir, filter.clone());

        ReaderPipeline::new(
            |event| match event {
                ReaderEvent::Header {
                    header,
                    record_boundary_position: _,
                } => {
                    let result = extractor.process_header(&header);

                    if args.continue_on_error {
                        if let Err(error) = result {
                            let error = anyhow::anyhow!(error);
                            tracing::error!(?error, "error processing record header");
                        }
                    } else {
                        result?;
                    }

                    Ok(())
                }
                ReaderEvent::Block { data } => {
                    let result = extractor.process_data(data);

                    if args.continue_on_error {
                        if let Err(error) = result {
                            let error = anyhow::anyhow!(error);
                            tracing::error!(?error, "error processing record block");
                        }
                    } else {
                        result?;
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

struct Extractor {
    extractor: WarcExtractor,
    file: Option<NamedTempFile>,
    buf: Vec<u8>,
    hasher: xxhash_rust::xxh3::Xxh3Default,
    output_dir: PathBuf,
    filter: FieldFilter,
}

impl Extractor {
    fn new<P: Into<PathBuf>>(output_dir: P, filter: FieldFilter) -> Self {
        Self {
            output_dir: output_dir.into(),
            filter,
            extractor: WarcExtractor::new(),
            buf: Vec::new(),
            hasher: xxhash_rust::xxh3::Xxh3Default::new(),
            file: None,
        }
    }

    fn process_header(&mut self, header: &WarcHeader) -> anyhow::Result<()> {
        self.extractor.reset();

        if !self.filter.is_allow(header) {
            return Ok(());
        }

        self.extractor.read_header(header)?;

        if self.extractor.has_content() {
            self.file = Some(
                tempfile::Builder::new()
                    .prefix("extract-")
                    .suffix(".incomplete.tmp")
                    .tempfile_in(&self.output_dir)?,
            );
        }

        Ok(())
    }

    fn process_data(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.write_extracted_data(data)?;
        self.finish_processing_data(data)?;

        Ok(())
    }

    fn write_extracted_data(&mut self, data: &[u8]) -> Result<(), GeneralError> {
        if let Some(writer) = &mut self.file {
            self.extractor.extract_data(data, &mut self.buf)?;
            self.hasher.update(&self.buf);
            std::io::copy(&mut Cursor::new(&self.buf), writer)?;
            self.buf.clear();
        }

        Ok(())
    }

    fn finish_processing_data(&mut self, data: &[u8]) -> std::io::Result<()> {
        if self.file.is_some() && data.is_empty() {
            let digest = self.hasher.digest();
            self.hasher.reset();

            let file = self.file.take().unwrap();

            let target_path = self.create_target_path(digest);

            if !target_path.exists() {
                std::fs::create_dir_all(target_path.parent().unwrap())?;
                let (mut file, temp_path) = file.keep()?;
                file.flush()?;
                std::fs::rename(temp_path, &target_path)?;

                tracing::info!(path = ?target_path, "extracted file");
            }
        }

        Ok(())
    }

    fn create_target_path(&self, conflict_id: u64) -> PathBuf {
        let mut target_path = self.output_dir.clone();
        let components = self.extractor.file_path_components();

        let mut iter = components.iter().peekable();

        while let Some(component) = iter.next() {
            let is_last_component = iter.peek().is_none();

            if is_last_component {
                let mut base_filename = component.to_string();

                if self.extractor.is_truncated() {
                    base_filename.push(FILENAME_CONFLICT_MARKER);
                    base_filename.push_str("truncated");
                }

                target_path.push(&base_filename);

                if target_path.exists() {
                    // File or directory already exists, append a unique ID to the name.
                    target_path.pop();
                    target_path.push(format!(
                        "{}{}{:016x}",
                        base_filename, FILENAME_CONFLICT_MARKER, conflict_id
                    ));
                }
            } else {
                target_path.push(component);

                if target_path.is_file() {
                    // File exists in place of directory component, append ".d"-style to the name
                    target_path.pop();
                    target_path.push(format!("{}{}d", component, FILENAME_CONFLICT_MARKER));
                }
            }
        }

        target_path
    }
}
