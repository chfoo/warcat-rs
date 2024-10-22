use std::io::{Cursor, Read};

use warcat::{
    compress::Dictionary, io::LogicalPosition, verify::Verifier, warc::{DecStateHeader, Decoder, DecoderConfig}
};

mod warc_generator;

#[tracing_test::traced_test]
#[test]
fn test_decode_gzip() {
    let input = warc_generator::generate_warc_gzip();
    dbg!(input.len());

    let mut config = DecoderConfig::default();
    config.decompressor.format = warcat::compress::Format::Gzip;

    let decoder = Decoder::new(Cursor::new(input), config).unwrap();

    check_decode(decoder);
}

#[cfg(feature = "zstd")]
#[tracing_test::traced_test]
#[test]
fn test_decode_zst() {
    let input = warc_generator::generate_warc_zst(false);
    dbg!(input.len());

    let mut config = DecoderConfig::default();
    config.decompressor.format = warcat::compress::Format::Zstandard;
    config.decompressor.dictionary = Dictionary::WarcZstd(Vec::new());

    let decoder = Decoder::new(Cursor::new(input), config).unwrap();

    check_decode(decoder);
}

#[cfg(feature = "zstd")]
#[tracing_test::traced_test]
#[test]
fn test_decode_zst_compressed_dict() {
    let input = warc_generator::generate_warc_zst(true);
    dbg!(input.len());

    let mut config = DecoderConfig::default();
    config.decompressor.format = warcat::compress::Format::Zstandard;
    config.decompressor.dictionary = Dictionary::WarcZstd(Vec::new());

    let decoder = Decoder::new(Cursor::new(input), config).unwrap();

    check_decode(decoder);
}

fn check_decode(mut decoder: Decoder<DecStateHeader, Cursor<Vec<u8>>>) {
    let mut verifier = Verifier::new();
    let mut count = 0;

    while decoder.has_next_record().unwrap() {
        dbg!(count);
        dbg!(decoder.logical_position());
        dbg!(&decoder.get_ref().position());

        let (header, mut block_decoder) = decoder.read_header().unwrap();

        verifier.begin_record(&header).unwrap();

        let mut buf = [0u8; 4096];
        loop {
            let read_len = block_decoder.read(&mut buf).unwrap();

            if read_len == 0 {
                break;
            }

            verifier.block_data(&buf[0..read_len]);
        }

        verifier.end_record();
        decoder = block_decoder.finish_block().unwrap();

        if !verifier.problems().is_empty() {
            println!("{:?}", verifier.problems());
        }
        assert!(verifier.problems().is_empty());

        count += 1;
    }

    decoder.into_inner();

    println!("{:?}", verifier.problems());
    assert!(verifier.problems().is_empty());
}
