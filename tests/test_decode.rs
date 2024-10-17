use std::io::{Cursor, Read};

use warcat::{verify::Verifier, warc::{Decoder, DecoderConfig}};

mod warc_generator;

#[tracing_test::traced_test]
#[test]
fn test_decode_gzip() {
    let input = warc_generator::generate_warc_gzip();

    let config = DecoderConfig {
        compression_format: warcat::compress::Format::Gzip,
    };
    let mut decoder = Decoder::new(Cursor::new(&input), config).unwrap();
    let mut verifier = Verifier::new();

    while decoder.has_next_record().unwrap() {
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
    }

    decoder.into_inner();

    println!("{:?}", verifier.problems());
    assert!(verifier.problems().is_empty());
}
