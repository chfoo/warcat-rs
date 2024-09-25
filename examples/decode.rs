//! Example showing how to decode a WARC file by records.
use std::{fs::File, io::Read};

use warcat::warc::{Decoder, DecoderConfig};

fn main() -> anyhow::Result<()> {
    // Source file
    let mut warc_file = File::open("examples/example.warc")?;

    // Configure the compression format if needed, otherwise use default
    let config = DecoderConfig::default();

    // Create a new WARC decoder
    let mut decoder = Decoder::new(&mut warc_file, config)?;

    loop {
        // Check for end of file
        if !decoder.has_next_record()? {
            break;
        }

        // Get the header of the WARC record and a decoder for the
        // block part of a record. Note that `read_header()` consumes the
        // decoder and returns another decoder with a different type. This
        // is known as the typestate pattern.
        let (header, mut block_decoder) = decoder.read_header()?;
        println!("Header: {:?}", header);

        // Reading the block is like reading a file
        let mut buf = Vec::new();
        block_decoder.read_to_end(&mut buf)?;
        println!("Block len: {}", buf.len());

        // Get a header decoder. Again, this is the typestate pattern.
        decoder = block_decoder.finish_block()?;
    }

    // Get the inner reader if needed
    let _file = decoder.into_inner();

    Ok(())
}
