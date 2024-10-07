//! WARC related hashing functions.

use std::{fmt::Display, str::FromStr};

use data_encoding::{BASE32, BASE32_NOPAD, HEXLOWER, HEXLOWER_PERMISSIVE};
use digest::Digest as _;

use crate::error::{ProtocolError, ProtocolErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Algorithm {
    Md5,
    Sha1,
    Sha256,
    Sha512,
}

impl Algorithm {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
        }
    }

    pub fn output_len(&self) -> usize {
        match self {
            Self::Md5 => 16,
            Self::Sha1 => 20,
            Self::Sha256 => 32,
            Self::Sha512 => 64,
        }
    }
}

impl Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Algorithm {
    type Err = ProtocolError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = crate::util::to_ascii_lowercase_cow(s);
        let s = remove_compatibility_label(&s);
        match s {
            "md5" => Ok(Self::Md5),
            "sha1" => Ok(Self::Sha1),
            "sha256" => Ok(Self::Sha256),
            "sha512" => Ok(Self::Sha512),

            _ => Err(ProtocolError::new(ProtocolErrorKind::UnsupportedDigest)),
        }
    }
}

pub struct Digest {
    algorithm: Algorithm,
    value: Vec<u8>,
}

impl Digest {
    pub fn new(algorithm: Algorithm, value: Vec<u8>) -> Self {
        Self { algorithm, value }
    }

    pub fn algorithm(&self) -> Algorithm {
        self.algorithm
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

impl FromStr for Digest {
    type Err = ProtocolError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (label, encoded) = s.split_once(":").unwrap_or((s, ""));
        let algorithm: Algorithm = label.parse()?;
        let value = decode_value(algorithm.output_len(), encoded)?;

        Ok(Self { algorithm, value })
    }
}

impl Display for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.algorithm {
            Algorithm::Sha1 => write!(
                f,
                "{}:{}",
                self.algorithm.as_str(),
                BASE32.encode(&self.value)
            ),
            _ => write!(
                f,
                "{}:{}",
                self.algorithm.as_str(),
                HEXLOWER.encode(&self.value)
            ),
        }
    }
}

fn remove_compatibility_label(label: &str) -> &str {
    match label {
        "sha-1" => "sha1",
        "sha-224" => "sha224",
        "sha-256" => "sha256",
        "sha-384" => "sha384",
        "sha-512" => "sha512",
        _ => label,
    }
}

fn decode_value(expected_len: usize, value: &str) -> Result<Vec<u8>, ProtocolError> {
    let nopad_value = value.trim_end_matches('=');

    let b32_len = BASE32_NOPAD
        .decode_len(nopad_value.len())
        .unwrap_or_default();
    let hex_len = HEXLOWER_PERMISSIVE
        .decode_len(nopad_value.len())
        .unwrap_or_default();

    let result = {
        if expected_len == b32_len && expected_len == hex_len {
            if value.ends_with('=') {
                let input = crate::util::to_ascii_uppercase_cow(nopad_value);
                BASE32_NOPAD.decode(input.as_bytes())
            } else {
                HEXLOWER_PERMISSIVE.decode(value.as_bytes())
            }
        } else if expected_len == b32_len {
            let input = crate::util::to_ascii_uppercase_cow(nopad_value);
            BASE32_NOPAD.decode(input.as_bytes())
        } else {
            HEXLOWER_PERMISSIVE.decode(value.as_bytes())
        }
    };

    result.map_err(|error| {
        ProtocolError::new(ProtocolErrorKind::InvalidBaseEncodedValue).with_source(error)
    })
}

enum HasherImpl {
    Md5(md5::Md5),
    Sha1(sha1::Sha1),
    Sha256(sha2::Sha256),
    Sha512(sha2::Sha512),
}

impl HasherImpl {
    fn update(&mut self, data: &[u8]) {
        match self {
            Self::Md5(digest) => digest.update(data),
            Self::Sha1(digest) => digest.update(data),
            Self::Sha256(digest) => digest.update(data),
            Self::Sha512(digest) => digest.update(data),
        }
    }

    fn finish(self) -> Vec<u8> {
        match self {
            Self::Md5(digest) => digest.finalize().to_vec(),
            Self::Sha1(digest) => digest.finalize().to_vec(),
            Self::Sha256(digest) => digest.finalize().to_vec(),
            Self::Sha512(digest) => digest.finalize().to_vec(),
        }
    }
}

pub struct Hasher {
    algorithm: Algorithm,
    inner: HasherImpl,
}

impl Hasher {
    pub fn new(algorithm: Algorithm) -> Self {
        let inner = Self::make_impl(algorithm);

        Self { algorithm, inner }
    }

    fn make_impl(algorithm: Algorithm) -> HasherImpl {
        match &algorithm {
            Algorithm::Md5 => HasherImpl::Md5(md5::Md5::new()),
            Algorithm::Sha1 => HasherImpl::Sha1(sha1::Sha1::new()),
            Algorithm::Sha256 => HasherImpl::Sha256(sha2::Sha256::new()),
            Algorithm::Sha512 => HasherImpl::Sha512(sha2::Sha512::new()),
        }
    }
    pub fn algorithm(&self) -> Algorithm {
        self.algorithm
    }

    pub fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    pub fn finish(&mut self) -> Vec<u8> {
        let inner = std::mem::replace(&mut self.inner, Self::make_impl(self.algorithm));

        inner.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_md5() {
        let digest = Digest::from_str("md5:b1946ac92492d2347c6235b4d2611184").unwrap();
        assert_eq!(digest.algorithm, Algorithm::Md5);
        assert_eq!(
            &digest.value,
            b"\xb1\x94j\xc9$\x92\xd24|b5\xb4\xd2a\x11\x84"
        );

        let digest = Digest::from_str("MD5:WGKGVSJESLJDI7DCGW2NEYIRQQ======").unwrap();
        assert_eq!(digest.algorithm, Algorithm::Md5);
        assert_eq!(
            &digest.value,
            b"\xb1\x94j\xc9$\x92\xd24|b5\xb4\xd2a\x11\x84"
        );

        let digest = Digest::from_str("md5:wgkgvsjesljdi7dcgw2neyirqq").unwrap();
        assert_eq!(digest.algorithm, Algorithm::Md5);
        assert_eq!(
            &digest.value,
            b"\xb1\x94j\xc9$\x92\xd24|b5\xb4\xd2a\x11\x84"
        );
    }

    #[test]
    fn test_parse_sha1() {
        let digest = Digest::from_str("Sha-1:VL2MMHO4YXUKFWV63YHTWSBM3GXKSQ2N").unwrap();
        assert_eq!(digest.algorithm, Algorithm::Sha1);
        assert_eq!(
            &digest.value,
            b"\xaa\xf4\xc6\x1d\xdc\xc5\xe8\xa2\xda\xbe\xde\x0f;H,\xd9\xae\xa9CM"
        );

        let digest = Digest::from_str("sha1:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d").unwrap();
        assert_eq!(digest.algorithm, Algorithm::Sha1);
        assert_eq!(
            &digest.value,
            b"\xaa\xf4\xc6\x1d\xdc\xc5\xe8\xa2\xda\xbe\xde\x0f;H,\xd9\xae\xa9CM"
        );
    }

    #[test]
    fn test_to_string() {
        let digest = Digest::new(
            Algorithm::Sha1,
            b"\xaa\xf4\xc6\x1d\xdc\xc5\xe8\xa2\xda\xbe\xde\x0f;H,\xd9\xae\xa9CM".to_vec(),
        );

        assert_eq!(digest.to_string(), "sha1:VL2MMHO4YXUKFWV63YHTWSBM3GXKSQ2N");
    }

    #[test]
    fn test_hash_sha1() {
        let mut hasher = Hasher::new(Algorithm::Sha1);

        hasher.update("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".as_bytes());

        let output = hasher.finish();

        assert_eq!(
            &output,
            b"\x84\x98>D\x1c;\xd2n\xba\xaeJ\xa1\xf9Q)\xe5\xe5Fp\xf1"
        )
    }
}
