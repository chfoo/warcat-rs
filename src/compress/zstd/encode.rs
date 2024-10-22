use std::io::Write;

use zstd::stream::write::Encoder as ZstdEncoderImpl;

use crate::compress::Dictionary;

use super::WARC_DICT_FRAME;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WarcDictionaryState {
    None,
    PendingFrameWrite,
    Ok,
}

pub struct ZstdEncoder<W: Write> {
    level: i32,
    dictionary: Dictionary,
    warc_dict_state: WarcDictionaryState,
    encoder_impl: Option<ZstdEncoderImpl<'static, W>>,
}

impl<W: Write> ZstdEncoder<W> {
    pub fn new(dest: W, level: i32, dictionary: Dictionary) -> std::io::Result<Self> {
        let warc_dict_state = match &dictionary {
            Dictionary::None => WarcDictionaryState::None,
            Dictionary::Zstd(_vec) => WarcDictionaryState::None,
            Dictionary::WarcZstd(_vec) => WarcDictionaryState::PendingFrameWrite,
        };
        let mut encoder_impl = match &dictionary {
            Dictionary::None => ZstdEncoderImpl::new(dest, level)?,
            Dictionary::Zstd(vec) => ZstdEncoderImpl::with_dictionary(dest, level, vec)?,
            Dictionary::WarcZstd(vec) => ZstdEncoderImpl::with_dictionary(dest, level, vec)?,
        };
        Self::config_encoder(&mut encoder_impl)?;
        Ok(Self {
            level,
            dictionary,
            warc_dict_state,
            encoder_impl: Some(encoder_impl),
        })
    }

    fn config_encoder(encoder: &mut ZstdEncoderImpl<'static, W>) -> std::io::Result<()> {
        encoder.include_checksum(true)?;
        Ok(())
    }

    pub fn get_ref(&self) -> &W {
        self.encoder_impl.as_ref().unwrap().get_ref()
    }

    pub fn get_mut(&mut self) -> &mut W {
        self.encoder_impl.as_mut().unwrap().get_mut()
    }

    fn write_warc_dictionary(&mut self) -> std::io::Result<()> {
        if let Dictionary::WarcZstd(data) = &self.dictionary {
            let dest = self.encoder_impl.as_mut().unwrap().get_mut();
            dest.write_all(&WARC_DICT_FRAME.to_le_bytes())?;
            dest.write_all(&(data.len() as u32).to_le_bytes())?;
            dest.write_all(data)?;
        }

        Ok(())
    }

    pub fn finish(self) -> std::io::Result<W> {
        self.encoder_impl.unwrap().finish()
    }

    pub fn start_new_frame(&mut self) -> std::io::Result<()> {
        // FIXME: We should be reusing the zstd context but the API is a bit difficult.

        let dest = self.encoder_impl.take().unwrap().finish()?;

        let mut encoder_impl = match &self.dictionary {
            Dictionary::None => ZstdEncoderImpl::new(dest, self.level)?,
            Dictionary::Zstd(vec) => ZstdEncoderImpl::with_dictionary(dest, self.level, vec)?,
            Dictionary::WarcZstd(vec) => ZstdEncoderImpl::with_dictionary(dest, self.level, vec)?,
        };
        Self::config_encoder(&mut encoder_impl)?;

        self.encoder_impl = Some(encoder_impl);

        Ok(())
    }
}

impl<W: Write> Write for ZstdEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.warc_dict_state == WarcDictionaryState::PendingFrameWrite {
            self.warc_dict_state = WarcDictionaryState::Ok;

            self.write_warc_dictionary()?;
        }

        self.encoder_impl.as_mut().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.encoder_impl.as_mut().unwrap().flush()
    }
}
