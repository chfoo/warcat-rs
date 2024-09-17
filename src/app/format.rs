use std::path::Path;

use crate::compress::Format;

pub fn filename_compression_format(path: &Path) -> Option<Format> {
    if let Some(filename) = path.file_name() {
        let filename = filename.to_string_lossy().to_ascii_lowercase();

        if filename.ends_with(".warc") {
            return Some(Format::Identity);
        }
        if filename.ends_with(".warc.gz") {
            return Some(Format::Gzip);
        }
        #[cfg(feature = "zstd")]
        if filename.ends_with(".warc.zstd") {
            return Some(Format::Zstandard);
        }
    }

    None
}
