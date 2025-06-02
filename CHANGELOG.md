# Changelog

## 0.3.4 (2025-06-02)

* Fixed: WARC header incorrectly rejected as invalid (#8).
* Fixed: Unexpected end of file error during decompression of the last record.
* Changed: `--id` is now optional for `get export` and `get extract` (#7).

### Library

* Added: PushDecompressor::write_eof()
* Changed: PushDecoderEvent and PushDecoder::write_eof()

## 0.3.3 (2025-05-26)

* Fixed: parse error for HTTP responses without a space after the status code (#3).
* Fixed: wrong record boundaries listed for uncompressed WARC files (#4).
* Added: new errors for unknown headers or unexpected compressed files (#5).

## 0.3.2 (2024-11-14)

* Fixed: application named with version isn't detected as installer on macOS/Linux.

## 0.3.1 (2024-10-22)

* Fixed: memory error reading ".warc.zst" files with compressed dictionaries.
* Fixed: corrupted data reading and file offsets for highly compressed ".warc.zst" files.
* Fixed: exclude-check from verify command not respected.
* Fixed: ANSI codes written to log files.
* Fixed: corrupted decoding Chunk-Transfer Encoding in cases where data aligns within a boundary.

## 0.3.0 (2024-10-20)

* Fixed: false positive Payload Digest problem during verify for "revisit" records.
* Added: Get command for exporting/extracting single records.
* Added: Record-at-time compression check to verify.
* Added: Zstandard (.warc.zst) support.

### Library

* Changed: `compress`: structs now take a configuration, renamed function for reading concatenated members
* Added `warc::PushDecoder`.

## 0.2.0 (2024-10-12)

* Fixed: HTTP decoder (and Extract command) incorrectly truncated data with Content-Length.
* Fixed: Verify functionality: block and payload digest checks were not functional.
* Added: filter options for Extract command.
* Added: extract option for Export command.
* Changed: Made the EndOfFile message explicit for the Export and Import commands.

## 0.1.0 (2024-10-11)

* First release.
