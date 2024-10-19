use std::{io::Read, path::Path};

use anyhow::Context;
use indicatif::ProgressBar;

use crate::{
    compress::{Dictionary, Format},
    header::WarcHeader,
    io::LogicalPosition,
    warc::{DecStateBlock, DecStateHeader, Decoder, DecoderConfig},
};

use super::io::{ProgramInput, ProgramOutput};

const BUFFER_LENGTH: usize = crate::io::IO_BUFFER_LENGTH;

pub fn open_input(path: &Path) -> anyhow::Result<ProgramInput> {
    ProgramInput::open(path).context("opening input file failed")
}

pub fn open_output(path: &Path) -> anyhow::Result<ProgramOutput> {
    ProgramOutput::open(path).context("opening output file failed")
}

pub enum ReaderEvent<'a> {
    Header {
        header: WarcHeader,
        record_boundary_position: u64,
    },
    Block {
        data: &'a [u8],
    },
}

#[derive(Debug)]
enum ReaderState {
    None,
    Header(Decoder<DecStateHeader, ProgramInput>),
    Block(Decoder<DecStateBlock, ProgramInput>),
}

impl ReaderState {
    fn take(&mut self) -> Self {
        std::mem::replace(self, Self::None)
    }

    #[allow(clippy::result_large_err)]
    fn try_into_header(self) -> Result<Decoder<DecStateHeader, ProgramInput>, Self> {
        if let Self::Header(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    #[allow(clippy::result_large_err)]
    fn try_into_block(self) -> Result<Decoder<DecStateBlock, ProgramInput>, Self> {
        if let Self::Block(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }
}

pub struct ReaderPipeline<C>
where
    C: FnMut(ReaderEvent) -> anyhow::Result<()>,
{
    progress_bar: ProgressBar,
    state: ReaderState,
    buf: Vec<u8>,
    callback: C,
    pub has_record_at_time_compression_fault: bool,
}

impl<C> ReaderPipeline<C>
where
    C: FnMut(ReaderEvent) -> anyhow::Result<()>,
{
    pub fn new(
        callback: C,
        input: ProgramInput,
        compression_format: Format,
        file_len: Option<u64>,
    ) -> anyhow::Result<Self> {
        let progress_bar = super::progress::make_bytes_progress_bar(file_len);

        let mut config = DecoderConfig::default();
        config.decompressor.format = compression_format;
        config.decompressor.dictionary = Dictionary::WarcZstd(Vec::new());

        let reader = Decoder::new(input, config)?;

        Ok(Self {
            progress_bar,
            state: ReaderState::Header(reader),
            buf: Vec::new(),
            callback,
            has_record_at_time_compression_fault: false,
        })
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        super::progress::global_progress_bar().add(self.progress_bar.clone());

        loop {
            self.process_header()?;
            self.process_block()?;

            let mut reader = self.state.take().try_into_header().unwrap();
            let has_more = reader.has_next_record()?;
            self.state = ReaderState::Header(reader);

            if !has_more {
                break;
            }
        }

        self.progress_bar.finish();
        super::progress::global_progress_bar().remove(&self.progress_bar);

        Ok(())
    }

    fn process_header(&mut self) -> anyhow::Result<()> {
        let reader = self.state.take().try_into_header().unwrap();

        self.has_record_at_time_compression_fault = reader.has_record_at_time_compression_fault();

        let (header, reader) = reader.read_header().context("invalid WARC header")?;

        let record_id = header
            .fields
            .get("WARC-Record-ID")
            .map(|s| s.as_str())
            .unwrap_or_default();
        self.progress_bar
            .set_message(format!("Processing record {}", record_id));
        self.progress_bar.set_position(reader.logical_position());

        (self.callback)(ReaderEvent::Header {
            header,
            record_boundary_position: reader.record_boundary_position(),
        })?;

        self.state = ReaderState::Block(reader);

        Ok(())
    }

    fn process_block(&mut self) -> anyhow::Result<()> {
        let mut reader = self.state.take().try_into_block().unwrap();

        loop {
            self.buf.resize(BUFFER_LENGTH, 0);

            let read_length = reader.read(&mut self.buf)?;
            self.buf.truncate(read_length);

            if read_length == 0 {
                break;
            }

            self.progress_bar.set_position(reader.logical_position());

            (self.callback)(ReaderEvent::Block { data: &self.buf })?;
        }

        (self.callback)(ReaderEvent::Block { data: &[] })?;

        self.state = ReaderState::Header(reader.finish_block()?);

        Ok(())
    }
}
