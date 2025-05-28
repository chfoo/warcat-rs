use std::io::{Read, Seek, Write};

use crate::{
    app::export::Exporter,
    compress::{Dictionary, Format},
    dataseq::SeqWriter,
    error::{ProtocolError, ProtocolErrorKind},
    extract::WarcExtractor,
    header::fields::FieldsExt,
    warc::{Decoder, DecoderConfig},
};

use super::arg::{GetCommand, GetExportSubcommand, GetExtractSubcommand, GetSubcommand};

pub fn get(args: &GetCommand) -> anyhow::Result<()> {
    match &args.subcommand {
        GetSubcommand::Export(sub_args) => export(sub_args),
        GetSubcommand::Extract(sub_args) => extract(sub_args),
    }
}

// FIXME: refactor the copypaste boilerplate

fn export(args: &GetExportSubcommand) -> anyhow::Result<()> {
    let input_path = &args.input;
    let output_path = &args.output;
    let span = tracing::info_span!("export", path = ?input_path);
    let _span_guard = span.enter();

    let input = super::common::open_input(input_path)?;
    let output = super::common::open_output(output_path)?;

    tracing::info!("opened file");

    let compression_format = args.compression.try_into_native(input_path)?;
    let seq_format = args.format.into();
    let writer = SeqWriter::new(output, seq_format);

    let mut exporter = Exporter::new(input_path, writer, args.no_block, args.extract);

    let mut config = DecoderConfig::default();
    config.decompressor.format = compression_format;
    config.decompressor.dictionary = get_dictionary(compression_format);

    let mut decoder = Decoder::new(input, config)?;

    if args.position != 0 {
        decoder.prepare_for_seek()?;
        decoder
            .get_mut()
            .seek(std::io::SeekFrom::Start(args.position))?;
    }

    let (header, mut decoder) = decoder.read_header()?;

    let record_id = header.fields.get_or_default("WARC-Record-ID");

    if !args.id.as_ref().is_none_or(|id| id == record_id) {
        return Err(ProtocolError::new(ProtocolErrorKind::NotFound).into());
    }

    let progress_bar = super::progress::make_bytes_progress_bar(Some(header.content_length()?));
    super::progress::global_progress_bar().add(progress_bar.clone());

    exporter.process_header(&header, decoder.record_boundary_position())?;

    let mut buf = Vec::with_capacity(8192);

    loop {
        buf.resize(8192, 0);

        let bytes_read = decoder.read(&mut buf)?;

        if bytes_read == 0 {
            break;
        }

        progress_bar.inc(bytes_read as u64);
        buf.truncate(bytes_read);
        exporter.process_block(&buf)?;
    }

    decoder.finish_block()?;
    exporter.finish()?;

    tracing::info!("closed file");

    progress_bar.finish();
    super::progress::global_progress_bar().remove(&progress_bar);

    Ok(())
}

fn extract(args: &GetExtractSubcommand) -> anyhow::Result<()> {
    let input_path = &args.input;
    let output_path = &args.output;
    let span = tracing::info_span!("export", path = ?input_path);
    let _span_guard = span.enter();

    let input = super::common::open_input(input_path)?;
    let mut output = super::common::open_output(output_path)?;

    tracing::info!("opened file");

    let compression_format = args.compression.try_into_native(input_path)?;

    let mut extractor = WarcExtractor::new();

    let mut config = DecoderConfig::default();
    config.decompressor.format = compression_format;
    config.decompressor.dictionary = get_dictionary(compression_format);

    let mut decoder = Decoder::new(input, config)?;

    if args.position != 0 {
        decoder.prepare_for_seek()?;
        decoder
            .get_mut()
            .seek(std::io::SeekFrom::Start(args.position))?;
    }

    let (header, mut decoder) = decoder.read_header()?;

    let record_id = header.fields.get_or_default("WARC-Record-ID");

    if !args.id.as_ref().is_none_or(|id| id == record_id) {
        return Err(ProtocolError::new(ProtocolErrorKind::NotFound).into());
    }

    let progress_bar = super::progress::make_bytes_progress_bar(Some(header.content_length()?));
    super::progress::global_progress_bar().add(progress_bar.clone());

    extractor.read_header(&header)?;

    if !extractor.has_content() {
        return Err(ProtocolError::new(ProtocolErrorKind::NoContent).into());
    }

    let mut buf = Vec::with_capacity(8192);

    loop {
        buf.resize(8192, 0);

        let bytes_read = decoder.read(&mut buf)?;

        if bytes_read == 0 {
            break;
        }

        progress_bar.inc(bytes_read as u64);
        buf.truncate(bytes_read);
        extractor.extract_data(&buf, &mut output)?;
    }

    decoder.finish_block()?;
    output.flush()?;

    tracing::info!("closed file");

    progress_bar.finish();
    super::progress::global_progress_bar().remove(&progress_bar);

    Ok(())
}

fn get_dictionary(format: Format) -> Dictionary {
    #[cfg(feature = "zstd")]
    if format == Format::Zstandard {
        return Dictionary::WarcZstd(Vec::new());
    }

    Dictionary::None
}
