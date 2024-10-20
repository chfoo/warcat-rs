# Changelog

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