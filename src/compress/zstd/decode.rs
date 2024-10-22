use std::{
    collections::VecDeque,
    io::{Read, Write},
};

use zstd::{
    stream::raw::{Decoder as ZstdFrameDecoder, Operation},
    zstd_safe::{InBuffer, OutBuffer},
};

use crate::compress::Dictionary;

use super::{BULK_BUFFER_LENGTH, WARC_DICT_FRAME, ZSTD_FRAME};

const BUFFER_LENGTH: usize = crate::io::IO_BUFFER_LENGTH;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PushDecoderState {
    FileHeader,
    FrameHeader,
    DictionaryFrameData,
    SkippableFrameData,
    ZstdFrame,
}

pub struct ZstdPushDecoder<W: Write> {
    state: PushDecoderState,
    dictionary: Dictionary,
    output: W,
    frame_decoder: ZstdFrameDecoder<'static>,
    magic_number: u32,
    data_length: u32,
    data_current: u32,
    buf: Vec<u8>,
    frame_decoder_buf: Vec<u8>,
}

impl<W: Write> ZstdPushDecoder<W> {
    pub fn new(output: W, dictionary: Dictionary) -> std::io::Result<Self> {
        let decoder_impl = match &dictionary {
            Dictionary::Zstd(vec) => ZstdFrameDecoder::with_dictionary(vec)?,
            _ => ZstdFrameDecoder::new()?,
        };

        Ok(Self {
            output,
            frame_decoder: decoder_impl,
            dictionary,
            state: PushDecoderState::FileHeader,
            magic_number: 0,
            data_length: 0,
            data_current: 0,
            buf: Vec::new(),
            frame_decoder_buf: vec![0u8; BUFFER_LENGTH],
        })
    }

    pub fn get_ref(&self) -> &W {
        &self.output
    }

    pub fn get_mut(&mut self) -> &mut W {
        &mut self.output
    }

    pub fn into_inner(self) -> W {
        self.output
    }

    fn read_magic_bytes(&mut self, buf: &[u8]) -> Result<usize, usize> {
        if self.buf.is_empty() && buf.len() >= 8 {
            self.magic_number = u32::from_le_bytes(buf[0..4].try_into().unwrap());
            self.data_length = u32::from_le_bytes(buf[4..8].try_into().unwrap());
            self.data_current = 0;

            tracing::trace!(self.magic_number, self.data_length, "magic bytes");

            Ok(8)
        } else if self.buf.len() + buf.len() >= 8 {
            let remain_length = 8 - self.buf.len();

            self.buf.extend_from_slice(&buf[0..remain_length]);

            self.magic_number = u32::from_le_bytes(self.buf[0..4].try_into().unwrap());
            self.data_length = u32::from_le_bytes(self.buf[4..8].try_into().unwrap());
            self.data_current = 0;

            tracing::trace!(self.magic_number, self.data_length, "magic bytes (buf)");

            Ok(remain_length)
        } else {
            self.buf.extend_from_slice(buf);
            Err(buf.len())
        }
    }

    fn process_file_header(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.process_header(buf, true)
    }

    fn process_frame_header(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.process_header(buf, false)
    }

    fn process_header(&mut self, buf: &[u8], enable_dict: bool) -> std::io::Result<usize> {
        match self.read_magic_bytes(buf) {
            Ok(bytes_read) => {
                if enable_dict
                    && self.magic_number == WARC_DICT_FRAME
                    && self.dictionary.is_warc_zstd()
                {
                    tracing::trace!("{:?} -> DictionaryFrameData", self.state);
                    self.state = PushDecoderState::DictionaryFrameData;
                } else if super::is_skippable_frame(self.magic_number) {
                    tracing::trace!("{:?} -> SkippableFrameData", self.state);
                    self.state = PushDecoderState::SkippableFrameData;
                } else {
                    tracing::trace!("{:?} -> ZstdFrame", self.state);
                    self.state = PushDecoderState::ZstdFrame;

                    if !self.buf.is_empty() {
                        self.process_zstd_frame(None)?;
                    } else {
                        self.process_zstd_frame(Some(&buf[0..8]))?;
                    }
                }

                self.buf.clear();

                Ok(bytes_read)
            }
            Err(bytes_read) => Ok(bytes_read),
        }
    }

    fn process_dictionary_frame_data(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let (data, bytes_read) = self.read_skippable_frame(buf)?;

        self.dictionary
            .as_warc_zstd_mut()
            .unwrap()
            .extend_from_slice(data);

        if self.data_current == self.data_length {
            if self
                .dictionary
                .as_warc_zstd()
                .unwrap()
                .starts_with(&(ZSTD_FRAME.to_le_bytes()))
            {
                let decomp_dict = zstd::bulk::decompress(
                    self.dictionary.as_warc_zstd().unwrap(),
                    BULK_BUFFER_LENGTH,
                )?;

                tracing::trace!(
                    dict_len = decomp_dict.len(),
                    "read dictionary frame (compressed)"
                );

                self.frame_decoder = ZstdFrameDecoder::with_dictionary(&decomp_dict)?;
                self.dictionary = Dictionary::WarcZstd(decomp_dict);
            } else {
                let dict = self.dictionary.as_warc_zstd().unwrap();

                tracing::trace!(dict_len = dict.len(), "read dictionary frame");

                self.frame_decoder = ZstdFrameDecoder::with_dictionary(dict)?;
            }

            self.reset_for_next_frame()?;
        }

        Ok(bytes_read)
    }

    fn process_skippable_frame_data(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let (_data, bytes_read) = self.read_skippable_frame(buf)?;

        if self.data_current == self.data_length {
            self.reset_for_next_frame()?;
        }

        Ok(bytes_read)
    }

    fn read_skippable_frame<'a>(&mut self, buf: &'a [u8]) -> std::io::Result<(&'a [u8], usize)> {
        assert!(self.data_current <= self.data_length);

        let remain_length = self.data_length - self.data_current;
        let remain_length = remain_length.min(buf.len().try_into().unwrap_or(u32::MAX));
        let bytes_read = remain_length as usize;

        let data = &buf[0..bytes_read];

        self.data_current += remain_length;

        Ok((data, bytes_read))
    }

    fn process_zstd_frame(&mut self, buf: Option<&[u8]>) -> std::io::Result<usize> {
        let mut input_buf = InBuffer::around(buf.unwrap_or_else(|| &self.buf));

        loop {
            let mut output_buf = OutBuffer::around(&mut self.frame_decoder_buf);
            let next_input_len_hint = self.frame_decoder.run(&mut input_buf, &mut output_buf)?;
            let decoded_len = output_buf.pos();

            self.output
                .write_all(&self.frame_decoder_buf[0..decoded_len])?;

            tracing::trace!(
                in_len = input_buf.pos(),
                out_len = decoded_len,
                next_input_len_hint,
                "process zstd frame"
            );

            if next_input_len_hint == 0 {
                tracing::trace!("ZstdFrame -> FrameHeader");
                self.state = PushDecoderState::FrameHeader;

                break;
            } else if decoded_len == 0 || input_buf.pos() == input_buf.src.len() {
                break;
            }
        }

        Ok(input_buf.pos())
    }

    fn reset_for_next_frame(&mut self) -> std::io::Result<()> {
        tracing::trace!("reset for next frame: {:?} -> FrameHeader", self.state);
        self.state = PushDecoderState::FrameHeader;

        self.frame_decoder.reinit()?;

        Ok(())
    }

    pub fn start_next_frame(&mut self) -> std::io::Result<()> {
        self.reset_for_next_frame()?;
        Ok(())
    }
}

impl<W: Write> Write for ZstdPushDecoder<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        tracing::trace!(?self.state, buf_len = buf.len(), "push decoder write");

        match self.state {
            PushDecoderState::FileHeader => self.process_file_header(buf),
            PushDecoderState::FrameHeader => self.process_frame_header(buf),
            PushDecoderState::DictionaryFrameData => self.process_dictionary_frame_data(buf),
            PushDecoderState::SkippableFrameData => self.process_skippable_frame_data(buf),
            PushDecoderState::ZstdFrame => self.process_zstd_frame(Some(buf)),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.output.flush()
    }
}

pub struct ZstdDecoder<R: Read> {
    input: R,
    push_decoder: ZstdPushDecoder<VecDeque<u8>>,
    buf: Vec<u8>,
}

impl<R: Read> ZstdDecoder<R> {
    pub fn new(input: R, dictionary: Dictionary) -> std::io::Result<Self> {
        Ok(Self {
            input,
            push_decoder: ZstdPushDecoder::new(VecDeque::new(), dictionary)?,
            buf: Vec::new(),
        })
    }

    fn fill_decoder(&mut self) -> std::io::Result<()> {
        tracing::trace!("fill decoder");

        while self.push_decoder.get_ref().is_empty() {
            self.buf.resize(BUFFER_LENGTH, 0);
            let source_read_len = self.input.read(&mut self.buf)?;
            self.buf.truncate(source_read_len);

            tracing::trace!(source_read_len, "fill decoder");

            if source_read_len == 0 {
                // End of input file
                break;
            }

            let decode_write_len = self.push_decoder.write(&self.buf)?;

            tracing::trace!(decode_write_len, "fill decoder");

            if decode_write_len == 0 {
                // End of zstd frame
                break;
            }

            self.buf.drain(0..decode_write_len);
        }

        Ok(())
    }

    pub fn get_ref(&self) -> &R {
        &self.input
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.input
    }

    pub fn into_inner(self) -> R {
        self.input
    }

    pub fn start_next_frame(&mut self) -> std::io::Result<()> {
        self.push_decoder.start_next_frame()
    }
}

impl<R: Read> Read for ZstdDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.fill_decoder()?;

        self.push_decoder.get_mut().read(buf)
    }
}
