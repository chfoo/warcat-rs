use std::io::{Cursor, Read, Write};

use warcat::{
    compress::Dictionary,
    io::LogicalPosition,
    verify::Verifier,
    warc::{Decoder, DecoderConfig, PushDecoder, PushDecoderEvent},
};

mod warc_generator;

#[tracing_test::traced_test]
#[test]
fn test_decode_gzip() {
    let (input, offsets) = warc_generator::generate_warc_gzip();
    dbg!(input.len());

    let mut config = DecoderConfig::default();
    config.decompressor.format = warcat::compress::Format::Gzip;

    check_push_decoder(input.clone(), config.clone(), offsets);
    check_decoder(input, config);
}

#[cfg(feature = "zstd")]
#[tracing_test::traced_test]
#[test]
fn test_decode_zst() {
    let (input, offsets) = warc_generator::generate_warc_zst(false);
    dbg!(input.len());

    let mut config = DecoderConfig::default();
    config.decompressor.format = warcat::compress::Format::Zstandard;
    config.decompressor.dictionary = Dictionary::WarcZstd(Vec::new());

    check_push_decoder(input.clone(), config.clone(), offsets);
    check_decoder(input, config);
}

#[cfg(feature = "zstd")]
#[tracing_test::traced_test]
#[test]
fn test_decode_zst_compressed_dict() {
    let (input, offsets) = warc_generator::generate_warc_zst(true);
    dbg!(input.len());

    let mut config = DecoderConfig::default();
    config.decompressor.format = warcat::compress::Format::Zstandard;
    config.decompressor.dictionary = Dictionary::WarcZstd(Vec::new());

    check_push_decoder(input, config, offsets);
}

fn check_push_decoder(input: Vec<u8>, config: DecoderConfig, mut offsets: Vec<u64>) {
    let mut decoder = PushDecoder::new(config).unwrap();
    let mut verifier = Verifier::new();
    let mut input = Cursor::new(input);

    // dbg!(&offsets);

    loop {
        match decoder.get_event().unwrap() {
            PushDecoderEvent::Ready | PushDecoderEvent::WantData => {
                let mut buf = vec![0; 4096];
                let len = input.read(&mut buf).unwrap();
                buf.truncate(len);
                decoder.write_all(&buf).unwrap();

                if len == 0 {
                    decoder.write_eof().unwrap();
                    break;
                }
            }

            PushDecoderEvent::Continue => {}
            PushDecoderEvent::Header { header } => {
                assert_eq!(decoder.record_boundary_position(), offsets[0]);
                offsets.drain(0..1);
                verifier.begin_record(&header).unwrap();
            }
            PushDecoderEvent::BlockData { data } => {
                verifier.block_data(data);
            }
            PushDecoderEvent::EndRecord => {
                verifier.end_record();
            }
            PushDecoderEvent::Finished => {
                break;
            }
        }
    }
}

fn check_decoder(input: Vec<u8>, config: DecoderConfig) {
    let mut decoder = Decoder::new(Cursor::new(input), config).unwrap();
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
