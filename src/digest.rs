//! WARC related hashing functions.

use std::{collections::HashMap, fmt::Display, str::FromStr};

use data_encoding::{BASE32, BASE32_NOPAD, HEXLOWER, HEXLOWER_PERMISSIVE};
use digest::Digest as _;

use crate::error::{ProtocolError, ProtocolErrorKind};

/// Name of a standardized hashing algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AlgorithmName {
    Crc32,
    Crc32c,
    Xxh3,
    Md5,
    Sha1,
    Sha256,
    Sha512,
    Sha3_256,
    Sha3_512,
    Blake2s,
    Blake3,
}

impl AlgorithmName {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Crc32 => "crc32",
            Self::Crc32c => "crc32c",
            Self::Xxh3 => "xxh3",
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
            Self::Sha3_256 => "sha3-256",
            Self::Sha3_512 => "sha3-512",
            Self::Blake2s => "blake2s",
            Self::Blake3 => "blake3",
        }
    }

    pub fn output_len(&self) -> usize {
        match self {
            Self::Crc32 => 4,
            Self::Crc32c => 4,
            Self::Xxh3 => 8,
            Self::Md5 => 16,
            Self::Sha1 => 20,
            Self::Sha256 => 32,
            Self::Sha512 => 64,
            Self::Sha3_256 => 32,
            Self::Sha3_512 => 64,
            Self::Blake2s => 32,
            Self::Blake3 => 32,
        }
    }
}

impl Display for AlgorithmName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AlgorithmName {
    type Err = ProtocolError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = crate::util::to_ascii_lowercase_cow(s);
        let s = remove_compatibility_label(&s);
        match s {
            "crc32" => Ok(Self::Crc32),
            "crc32c" => Ok(Self::Crc32c),
            "xxh3" => Ok(Self::Xxh3),
            "md5" => Ok(Self::Md5),
            "sha1" => Ok(Self::Sha1),
            "sha256" => Ok(Self::Sha256),
            "sha512" => Ok(Self::Sha512),
            "sha3-256" => Ok(Self::Sha3_256),
            "sha3-512" => Ok(Self::Sha3_512),
            "blake2s" => Ok(Self::Blake2s),
            "blake3" => Ok(Self::Blake3),

            _ => Err(ProtocolError::new(ProtocolErrorKind::UnsupportedDigest)),
        }
    }
}

/// Data structure for a hash digest value and the algorithm that produced it.
///
/// Corresponds to the format in the WARC-Block-Digest field.
#[derive(Debug, Clone)]
pub struct Digest {
    algorithm: AlgorithmName,
    value: Vec<u8>,
}

impl Digest {
    pub fn new(algorithm: AlgorithmName, value: Vec<u8>) -> Self {
        Self { algorithm, value }
    }

    pub fn algorithm(&self) -> AlgorithmName {
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
        let algorithm: AlgorithmName = label.parse()?;
        let value = decode_value(algorithm.output_len(), encoded)?;

        Ok(Self { algorithm, value })
    }
}

impl Display for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.algorithm {
            AlgorithmName::Sha1 => write!(
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

#[allow(clippy::large_enum_variant)]
enum HasherImpl {
    Crc32(crc32fast::Hasher),
    Crc32c(u32),
    Xxh3(xxhash_rust::xxh3::Xxh3),
    Md5(md5::Md5),
    Sha1(sha1::Sha1),
    Sha256(sha2::Sha256),
    Sha512(sha2::Sha512),
    Sha3_256(sha3::Sha3_256),
    Sha3_512(sha3::Sha3_512),
    Blake2s(blake2::Blake2s256),
    Blake3(blake3::Hasher),
}

impl HasherImpl {
    fn update(&mut self, data: &[u8]) {
        match self {
            Self::Crc32(hasher) => hasher.update(data),
            Self::Crc32c(checksum) => *checksum = crc32c::crc32c_append(*checksum, data),
            Self::Xxh3(hasher) => hasher.update(data),
            Self::Md5(digest) => digest.update(data),
            Self::Sha1(digest) => digest.update(data),
            Self::Sha256(digest) => digest.update(data),
            Self::Sha512(digest) => digest.update(data),
            Self::Sha3_256(digest) => digest.update(data),
            Self::Sha3_512(digest) => digest.update(data),
            Self::Blake2s(digest) => digest.update(data),
            Self::Blake3(digest) => digest::Digest::update(digest, data),
        }
    }

    fn finish(self) -> Vec<u8> {
        match self {
            Self::Crc32(hasher) => hasher.finalize().to_le_bytes().to_vec(),
            Self::Crc32c(checksum) => checksum.to_le_bytes().to_vec(),
            Self::Xxh3(hasher) => hasher.digest().to_be_bytes().to_vec(),
            Self::Md5(digest) => digest.finalize().to_vec(),
            Self::Sha1(digest) => digest.finalize().to_vec(),
            Self::Sha256(digest) => digest.finalize().to_vec(),
            Self::Sha512(digest) => digest.finalize().to_vec(),
            Self::Sha3_256(digest) => digest.finalize().to_vec(),
            Self::Sha3_512(digest) => digest.finalize().to_vec(),
            Self::Blake2s(digest) => digest.finalize().to_vec(),
            Self::Blake3(digest) => digest.finalize().to_vec(),
        }
    }

    fn finish_u64(self) -> Option<u64> {
        match self {
            Self::Crc32(hasher) => Some(hasher.finalize() as u64),
            Self::Crc32c(checksum) => Some(checksum as u64),
            Self::Xxh3(hasher) => Some(hasher.digest()),
            _ => None,
        }
    }
}

/// Hashing function abstraction.
pub struct Hasher {
    algorithm: AlgorithmName,
    inner: HasherImpl,
}

impl Hasher {
    pub fn new(algorithm: AlgorithmName) -> Self {
        let inner = Self::make_impl(algorithm);

        Self { algorithm, inner }
    }

    fn make_impl(algorithm: AlgorithmName) -> HasherImpl {
        match &algorithm {
            AlgorithmName::Crc32 => HasherImpl::Crc32(crc32fast::Hasher::new()),
            AlgorithmName::Crc32c => HasherImpl::Crc32c(0),
            AlgorithmName::Xxh3 => HasherImpl::Xxh3(xxhash_rust::xxh3::Xxh3::new()),
            AlgorithmName::Md5 => HasherImpl::Md5(md5::Md5::new()),
            AlgorithmName::Sha1 => HasherImpl::Sha1(sha1::Sha1::new()),
            AlgorithmName::Sha256 => HasherImpl::Sha256(sha2::Sha256::new()),
            AlgorithmName::Sha512 => HasherImpl::Sha512(sha2::Sha512::new()),
            AlgorithmName::Sha3_256 => HasherImpl::Sha3_256(sha3::Sha3_256::new()),
            AlgorithmName::Sha3_512 => HasherImpl::Sha3_512(sha3::Sha3_512::new()),
            AlgorithmName::Blake2s => HasherImpl::Blake2s(blake2::Blake2s::new()),
            AlgorithmName::Blake3 => HasherImpl::Blake3(blake3::Hasher::new()),
        }
    }
    pub fn algorithm(&self) -> AlgorithmName {
        self.algorithm
    }

    pub fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    pub fn finish(&mut self) -> Vec<u8> {
        let inner = std::mem::replace(&mut self.inner, Self::make_impl(self.algorithm));

        inner.finish()
    }

    pub fn finish_u64(&mut self) -> Option<u64> {
        let inner = std::mem::replace(&mut self.inner, Self::make_impl(self.algorithm));

        inner.finish_u64()
    }
}

/// Computes multiple hashes at once.
pub struct MultiHasher {
    inner: HashMap<AlgorithmName, Hasher>,
}

impl MultiHasher {
    pub fn new(algorithms: &[AlgorithmName]) -> Self {
        let mut inner = HashMap::new();

        for &algorithm in algorithms {
            inner.insert(algorithm, Hasher::new(algorithm));
        }

        Self { inner }
    }

    pub fn update(&mut self, data: &[u8]) {
        for hasher in &mut self.inner.values_mut() {
            hasher.update(data);
        }
    }

    pub fn finish(&mut self) -> HashMap<AlgorithmName, Vec<u8>> {
        let mut map = HashMap::new();

        for (&algorithm, hasher) in &mut self.inner {
            map.insert(algorithm, hasher.finish());
        }

        map
    }

    pub fn finish_u64(&mut self) -> HashMap<AlgorithmName, u64> {
        let mut map = HashMap::new();

        for (&algorithm, hasher) in &mut self.inner {
            if let Some(value) = hasher.finish_u64() {
                map.insert(algorithm, value);
            }
        }

        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_md5() {
        let digest = Digest::from_str("md5:b1946ac92492d2347c6235b4d2611184").unwrap();
        assert_eq!(digest.algorithm, AlgorithmName::Md5);
        assert_eq!(
            &digest.value,
            b"\xb1\x94j\xc9$\x92\xd24|b5\xb4\xd2a\x11\x84"
        );

        let digest = Digest::from_str("MD5:WGKGVSJESLJDI7DCGW2NEYIRQQ======").unwrap();
        assert_eq!(digest.algorithm, AlgorithmName::Md5);
        assert_eq!(
            &digest.value,
            b"\xb1\x94j\xc9$\x92\xd24|b5\xb4\xd2a\x11\x84"
        );

        let digest = Digest::from_str("md5:wgkgvsjesljdi7dcgw2neyirqq").unwrap();
        assert_eq!(digest.algorithm, AlgorithmName::Md5);
        assert_eq!(
            &digest.value,
            b"\xb1\x94j\xc9$\x92\xd24|b5\xb4\xd2a\x11\x84"
        );
    }

    #[test]
    fn test_parse_sha1() {
        let digest = Digest::from_str("Sha-1:VL2MMHO4YXUKFWV63YHTWSBM3GXKSQ2N").unwrap();
        assert_eq!(digest.algorithm, AlgorithmName::Sha1);
        assert_eq!(
            &digest.value,
            b"\xaa\xf4\xc6\x1d\xdc\xc5\xe8\xa2\xda\xbe\xde\x0f;H,\xd9\xae\xa9CM"
        );

        let digest = Digest::from_str("sha1:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d").unwrap();
        assert_eq!(digest.algorithm, AlgorithmName::Sha1);
        assert_eq!(
            &digest.value,
            b"\xaa\xf4\xc6\x1d\xdc\xc5\xe8\xa2\xda\xbe\xde\x0f;H,\xd9\xae\xa9CM"
        );
    }

    #[test]
    fn test_to_string() {
        let digest = Digest::new(
            AlgorithmName::Sha1,
            b"\xaa\xf4\xc6\x1d\xdc\xc5\xe8\xa2\xda\xbe\xde\x0f;H,\xd9\xae\xa9CM".to_vec(),
        );

        assert_eq!(digest.to_string(), "sha1:VL2MMHO4YXUKFWV63YHTWSBM3GXKSQ2N");
    }

    #[test]
    fn test_hash_sha1() {
        let mut hasher = Hasher::new(AlgorithmName::Sha1);

        hasher.update("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".as_bytes());

        let output = hasher.finish();

        assert_eq!(
            &output,
            b"\x84\x98>D\x1c;\xd2n\xba\xaeJ\xa1\xf9Q)\xe5\xe5Fp\xf1"
        )
    }
}
