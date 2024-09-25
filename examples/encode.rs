//! Example on how to encode a WARC file by records
//! use std::io::Write;

use std::io::Write;

use warcat::{
    header::WarcHeader,
    warc::{Encoder, EncoderConfig},
};

fn main() -> anyhow::Result<()> {
    // For this example, our file is just a in-memory buffer
    let mut warc_file = Vec::new();

    // Configure the compression format if needed, otherwise use default
    let config = EncoderConfig::default();

    // Create a new WARC encoder
    let mut encoder = Encoder::new(&mut warc_file, config);

    // Write a header of a WARC record and return a block encoder.
    // Note that `write_header()` consumes the encoder and returns a
    // decoder of a different type. This is known as the typestate pattern.
    let header = WarcHeader::new(12, "Resource");
    let mut block_encoder = encoder.write_header(&header)?;

    // Write the block like a file.
    block_encoder.write_all(b"Hello world!")?;

    // Get a header encoder. Again, this is the typestate pattern.
    encoder = block_encoder.finish_block()?;

    // Get the inner writer if needed
    let _file = encoder.finish()?;

    println!("Wrote {} bytes", warc_file.len());

    Ok(())
}
