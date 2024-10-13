% ATTENTION: This file was automatically generated using cargo xtask.
% Do not manually edit this file!

# CLI Reference

This document contains the help content for the `warcat` command-line program.

## `warcat`

WARC archive tool

**Usage:** `warcat [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `export` — Decodes a WARC file to messages in a easier-to-process format such as JSON
* `import` — Encodes a WARC file from messages in a format of the `export` subcommand
* `list` — Provides a listing of the WARC records
* `get` — Returns a single WARC record
* `extract` — Extracts resources for casual viewing of the WARC contents
* `verify` — Perform specification and integrity checks on WARC files
* `self` — Self-installer and uninstaller

###### **Options:**

* `-q`, `--quiet` — Disable any progress messages.

   Does not affect logging.
* `--log-level <LOG_LEVEL>` — Filter log messages by level

  Default value: `off`

  Possible values: `trace`, `debug`, `info`, `warn`, `error`, `off`

* `--log-file <LOG_FILE>` — Write log messages to the given file instead of standard error
* `--log-json` — Write log messages as JSON sequences instead of a console logging format



## `warcat export`

Decodes a WARC file to messages in a easier-to-process format such as JSON

**Usage:** `warcat export [OPTIONS]`

###### **Options:**

* `--input <INPUT>` — Path to a WARC file

  Default value: `-`
* `--compression <COMPRESSION>` — Specify the compression format of the input WARC file

  Default value: `auto`

  Possible values:
  - `auto`:
    Automatically detect the format by the filename extension
  - `none`:
    No compression
  - `gzip`:
    Gzip format

* `--output <OUTPUT>` — Path for the output messages

  Default value: `-`
* `--format <FORMAT>` — Format for the output messages

  Default value: `json-seq`

  Possible values:
  - `json-seq`:
    JSON sequences (RFC 7464)
  - `jsonl`:
    JSON Lines
  - `cbor-seq`:
    CBOR sequences (RFC 8742)

* `--no-block` — Do not output block messages
* `--extract` — Output extract messages



## `warcat import`

Encodes a WARC file from messages in a format of the `export` subcommand

**Usage:** `warcat import [OPTIONS]`

###### **Options:**

* `--input <INPUT>` — Path to the input messages

  Default value: `-`
* `--format <FORMAT>` — Format for the input messages

  Default value: `json-seq`

  Possible values:
  - `json-seq`:
    JSON sequences (RFC 7464)
  - `jsonl`:
    JSON Lines
  - `cbor-seq`:
    CBOR sequences (RFC 8742)

* `--output <OUTPUT>` — Path of the output WARC file

  Default value: `-`
* `--compression <COMPRESSION>` — Compression format of the output WARC file

  Default value: `auto`

  Possible values:
  - `auto`:
    Automatically detect the format by the filename extension
  - `none`:
    No compression
  - `gzip`:
    Gzip format

* `--compression-level <COMPRESSION_LEVEL>` — Level of compression for the output

  Default value: `high`

  Possible values:
  - `balanced`:
    A balance between compression ratio and resource consumption
  - `high`:
    Use a high level of resources to achieve a better compression ratio
  - `low`:
    Fast and low resource usage, but lower compression ratio




## `warcat list`

Provides a listing of the WARC records

**Usage:** `warcat list [OPTIONS]`

###### **Options:**

* `--input <INPUT>` — Path of the WARC file

  Default value: `-`
* `--compression <COMPRESSION>` — Compression format of the input WARC file

  Default value: `auto`

  Possible values:
  - `auto`:
    Automatically detect the format by the filename extension
  - `none`:
    No compression
  - `gzip`:
    Gzip format

* `--output <OUTPUT>` — Path to output listings

  Default value: `-`
* `--format <FORMAT>` — Format of the output

  Default value: `json-seq`

  Possible values:
  - `json-seq`:
    JSON sequences (RFC 7464)
  - `jsonl`:
    JSON Lines
  - `cbor-seq`:
    CBOR sequences (RFC 8742)
  - `csv`:
    Comma separated values

* `--field <FIELD>` — Fields to include in the listing.

   The option accepts names of fields that occur in a WARC header.

   The pseudo-name `:position` represents the position in the file. `:file` represents the path of the file.

  Default value: `:position,WARC-Record-ID,WARC-Type,Content-Type,WARC-Target-URI`



## `warcat get`

Returns a single WARC record

**Usage:** `warcat get <COMMAND>`

###### **Subcommands:**

* `export` — Output export messages
* `extract` — Extract a resource



## `warcat get export`

Output export messages

**Usage:** `warcat get export [OPTIONS] --position <POSITION> --id <ID>`

###### **Options:**

* `--input <INPUT>` — Path of the WARC file

  Default value: `-`
* `--compression <COMPRESSION>` — Compression format of the input WARC file

  Default value: `auto`

  Possible values:
  - `auto`:
    Automatically detect the format by the filename extension
  - `none`:
    No compression
  - `gzip`:
    Gzip format

* `--position <POSITION>` — Position where the record is located in the input WARC file
* `--id <ID>` — The ID of the record to extract
* `--output <OUTPUT>` — Path for the output messages

  Default value: `-`
* `--format <FORMAT>` — Format for the output messages

  Default value: `json-seq`

  Possible values:
  - `json-seq`:
    JSON sequences (RFC 7464)
  - `jsonl`:
    JSON Lines
  - `cbor-seq`:
    CBOR sequences (RFC 8742)

* `--no-block` — Do not output block messages
* `--extract` — Output extract messages



## `warcat get extract`

Extract a resource

**Usage:** `warcat get extract [OPTIONS] --position <POSITION> --id <ID>`

###### **Options:**

* `--input <INPUT>`

  Default value: `-`
* `--compression <COMPRESSION>` — Compression format of the input WARC file

  Default value: `auto`

  Possible values:
  - `auto`:
    Automatically detect the format by the filename extension
  - `none`:
    No compression
  - `gzip`:
    Gzip format

* `--position <POSITION>` — Position where the record is located in the input WARC file
* `--id <ID>` — The ID of the record to extract
* `--output <OUTPUT>` — Path for the output file

  Default value: `-`



## `warcat extract`

Extracts resources for casual viewing of the WARC contents.

Files are extracted to a directory structure similar to the archived URL.

This operation does not automatically permit offline viewing of archived websites; no content conversion or link-rewriting is performed.

**Usage:** `warcat extract [OPTIONS]`

###### **Options:**

* `--input <INPUT>` — Path to the WARC file

  Default value: `-`
* `--compression <COMPRESSION>` — Compression format of the input WARC file

  Default value: `auto`

  Possible values:
  - `auto`:
    Automatically detect the format by the filename extension
  - `none`:
    No compression
  - `gzip`:
    Gzip format

* `--output <OUTPUT>` — Path to the output directory

  Default value: `./`
* `--continue-on-error` — Whether to ignore errors
* `--include <INCLUDE>` — Select only records with a field.

   Rule format is "NAME" or "NAME:VALUE".
* `--include-pattern <INCLUDE_PATTERN>` — Select only records matching a regular expression.

   Rule format is "NAME:VALUEPATTERN".
* `--exclude <EXCLUDE>` — Do not select records with a field.

   Rule format is "NAME" or "NAME:VALUE".
* `--exclude-pattern <EXCLUDE_PATTERN>` — Do not select records matching a regular expression.

   Rule format is "NAME:VALUEPATTERN".



## `warcat verify`

Perform specification and integrity checks on WARC files

**Usage:** `warcat verify [OPTIONS] [DATABASE]`

###### **Arguments:**

* `<DATABASE>` — Database filename for storing temporary intermediate data

###### **Options:**

* `--input <INPUT>` — Path to the WARC file

  Default value: `-`
* `--compression <COMPRESSION>` — Compression format of the input WARC file

  Default value: `auto`

  Possible values:
  - `auto`:
    Automatically detect the format by the filename extension
  - `none`:
    No compression
  - `gzip`:
    Gzip format

* `--output <OUTPUT>` — Path to output problems

  Default value: `-`
* `--format <FORMAT>` — Format of the output

  Default value: `json-seq`

  Possible values:
  - `json-seq`:
    JSON sequences (RFC 7464)
  - `jsonl`:
    JSON Lines
  - `cbor-seq`:
    CBOR sequences (RFC 8742)
  - `csv`:
    Comma separated values

* `--exclude-check <EXCLUDE_CHECK>` — Do not perform check

  Possible values: `mandatory-fields`, `known-record-type`, `content-type`, `concurrent-to`, `block-digest`, `payload-digest`, `ip-address`, `refers-to`, `refers-to-target-uri`, `refers-to-date`, `target-uri`, `truncated`, `warcinfo-id`, `filename`, `profile`, `segment`




## `warcat self`

Self-installer and uninstaller

**Usage:** `warcat self <COMMAND>`

###### **Subcommands:**

* `install` — Launch the interactive self-installer
* `uninstall` — Launch the interactive uninstaller



## `warcat self install`

Launch the interactive self-installer

**Usage:** `warcat self install [OPTIONS]`

###### **Options:**

* `--quiet` — Install automatically without user interaction



## `warcat self uninstall`

Launch the interactive uninstaller

**Usage:** `warcat self uninstall [OPTIONS]`

###### **Options:**

* `--quiet` — Uninstall automatically without user interaction




