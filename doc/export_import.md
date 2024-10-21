# Export/import format

This section describes the message format used during the export and import commands.

## Message types

### Metadata

The metadata message is provided during only the export command. It is produced at a start of a WARC record.

map:

* `Metadata` - map
  * `file` - string: The input filename of the WARC.
  * `position` - integer: The position in the WARC file where the record is located. For compressed files, this position is only valid if the file was compressed by concatenating compressed streams.

Example:

```json
{
    "Metadata": {
        "file": "./my_file.warc.gz",
        "position": 123
    }
}
```

### Header

The header message is provided for both export and import commands. It is produced when a header from a WARC record has been read.

map:

* `Header` - map
  * `version` - string: The WARC version string such as "WARC/1.1"
  * `fields` - array[[string, string]]: Name-value pairs.

```json
{
    "Header": {
        "version": "WARC/1.1",
        "fields": [
            ["WARC-Record-Type": "metadata"],
            ["Content-Length": "123"]
        ]
    }
}
```

### Block chunk

The block chunk message is provided for export and import commands. It is produced when a segment of a block from a WARC record has been read.

map:

* `BlockChunk` - map
  * `data` - bytes: A segment of block data. For JSON, this is a string in base64 standard (with padding) encoding.

```json
{
    "BlockChunk": {
        "data": "Zm9vYmFy"
    }
}
```

### Block end

The block chunk message is provided for export and import commands. It is produced at the end of reading a block and WARC record.

map:

* `BlockEnd` - map
  * `crc32` - integer (optional, unsigned 32-bit): CRC32 (ITU-T V.42) checksum of the block data.
  * `crc32c` - integer (optional, unsigned 32-bit): CRC32C checksum of the block data.
  * `xxh3` - integer (optional, unsigned 64-bit): XxHash XXH3 checksum of the block data.

The checksum is used to ensure that processing of messages was properly implemented.

When importing, it is required that at least one of the fields "crc32", "crc32c", or "xxh3" be provided. When exporting, all fields will be filled.

```json
{
    "BlockEnd": {
        "crc32c": 123456
    }
}
```

### Extract metadata

The extract metadata message is provided during only the export command with the extract option.

map:

* `ExtractMetadata` - map
  * `has_content` - boolean: Whether data can be extracted from this record.
  * `file_path_components` - array\[string\]: A safe filename for writing to disk.
  * `is_truncated` - bool: As recorded in the header field, whether the content is truncated.

Example:

```json
{
    "ExtractMetadata": {
        "has_content": true,
        "file_path_components": ["http", "www.example.com", "index.html"],
        "is_truncated": false
    }
}
```

### Extract chunk

The extract chunk message is provided for export command with the extract option. It is produced when content can be extracted from a segment of block data.

map:

* `ExtractChunk` - map
  * `data` - bytes: A segment of block data. For JSON, this is a string in base64 standard (with padding) encoding.

```json
{
    "ExtractChunk": {
        "data": "Zm9vYmFy"
    }
}
```

### Extract end

The extract end message is provided for export command with extract option. It is produced at the end of extracting a block record.

map:

* `ExtractEnd` - map
  * `crc32` - integer (optional, unsigned 32-bit): CRC32 (ITU-T V.42) checksum of the extracted content.
  * `crc32c` - integer (optional, unsigned 32-bit): CRC32C checksum of the extracted content.
  * `xxh3`a - integer (optional, unsigned 64-bit): XxHash XXH3 checksum of the extracted content.

```json
{
    "ExtractEnd": {
        "crc32c": 123456
    }
}
```

### End of file

The end of file message indicates the output stream is ending and no other messages will be sent.

map:

* `EndOfFile` - map

```json
{
    "EndOfFile": {}
}
```

## Message flows

During the export command, every record consists of:

* 1 `Metadata`
* 1 `Header`
* 0 or more `BlockChunk`
* 1 `BlockEnd`

During the import command, every record consists of:

* 1 `Header`
* 0 or more `BlockChunk`
* 1 `BlockEnd`

After all records are processed, the `EndOfFile` message is sent.