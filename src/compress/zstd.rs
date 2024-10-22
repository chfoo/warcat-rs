use std::io::Read;

#[cfg(feature = "zstd")]
pub(crate) use decode::{ZstdDecoder, ZstdPushDecoder};
#[cfg(feature = "zstd")]
pub(crate) use encode::ZstdEncoder;

#[cfg(feature = "zstd")]
mod decode;
#[cfg(feature = "zstd")]
mod encode;

const WARC_DICT_FRAME: u32 = 0x184D2A5D;
const ZSTD_FRAME: u32 = 0xFD2FB528;
const BULK_BUFFER_LENGTH: usize = 16 * 1024 * 1024;

pub fn is_skippable_frame(magic_number: u32) -> bool {
    (0x184D2A50..=0x184D2A5F).contains(&magic_number)
}

pub fn extract_warc_zst_dictionary<R: Read>(
    mut input: R,
) -> Result<Vec<u8>, WarcZstDictExtractError> {
    let mut buf = [0u8; 8];

    input.read_exact(&mut buf)?;

    let magic_number = u32::from_le_bytes(buf[0..4].try_into().unwrap());
    let length = u32::from_le_bytes(buf[4..8].try_into().unwrap());

    if length > BULK_BUFFER_LENGTH as u32 {
        return Err(WarcZstDictExtractError::TooLarge);
    }

    if magic_number != WARC_DICT_FRAME {
        return Err(WarcZstDictExtractError::NotDict);
    }

    let mut buf = vec![0u8; length as usize];
    input.read_exact(&mut buf)?;

    if buf.starts_with(&ZSTD_FRAME.to_le_bytes()) {
        #[cfg(feature = "zstd")]
        {
            let buf2 = zstd::bulk::decompress(&buf, BULK_BUFFER_LENGTH)?;

            Ok(buf2)
        }
        #[cfg(not(feature = "zstd"))]
        {
            Err(std::io::Error::other(
                "failed to read compressed .warc.zst dictionary: zstd feature is not enabled",
            ))
        }
    } else {
        Ok(buf)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WarcZstDictExtractError {
    #[error("dictionary too large")]
    TooLarge,
    #[error("not a .warc.zst dictionary")]
    NotDict,
    #[error(transparent)]
    Other(#[from] std::io::Error),
}
