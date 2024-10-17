use std::io::Write;

use rand::{Rng, RngCore};
use rand_xoshiro::{rand_core::SeedableRng, Xoshiro256PlusPlus};
use warcat::{
    digest::{AlgorithmName, Digest, Hasher},
    header::WarcHeader,
    warc::{Encoder, EncoderConfig},
};

pub fn generate_warc_gzip() -> Vec<u8> {
    let mut output = Vec::new();

    let config = EncoderConfig {
        compression: warcat::compress::Format::Gzip,
        ..Default::default()
    };
    let mut encoder = Encoder::new(&mut output, config);

    for round in 0..100 {
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(round);

        let length: u64 = rng.gen_range(100 + round * 123..200 + round * 123);

        let mut data: Vec<u8> = vec![0; length as usize];
        rng.fill_bytes(&mut data);

        let mut hasher = Hasher::new(AlgorithmName::Sha1);
        hasher.update(&data);
        let digest = Digest::new(AlgorithmName::Sha1, hasher.finish());

        let mut header = WarcHeader::new(length, "resource");
        header
            .fields
            .insert("WARC-Block-Digest".to_string(), digest.to_string());
        header.fields.insert(
            "WARC-Target-URI".to_string(),
            "urn:example:test".to_string(),
        );

        let mut block_encoder = encoder.write_header(&header).unwrap();
        block_encoder.write_all(&data).unwrap();
        encoder = block_encoder.finish_block().unwrap();
    }

    encoder.finish().unwrap();

    output
}
