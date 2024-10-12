# Integration to other programs

Integration to other programs is done through standard input and output using the `export` and `import` commands.

For reading WARC files, the `export` command will format the data into messages such as JSON which your program can ingest and process. Likewise for writing WARC files, the `import` command accepts messages from your program.

For working examples, [see here](https://github.com/chfoo/warcat-rs/tree/main/examples).

The format of the messages is documented in the next section.

## Overview

In order to integrate with your programming language of choice, your language's library must be able to launch other programs and communicate using standard in or standard out.

This section will use pseudocode to explain an overview on how to read a WARC file.

To begin reading, run the warcat program with options to export and output in JSON Lines:

```
process <- run_process("warcat", "export", "--input", "example.warc.gz", "--format=jsonl")
```

Next, get the record header by reading lines containing JSON:

```
metadata_line <- process.stdout.read_line()
metadata <- decode_json(metadata_line)

print("Reading file " + metadata["Metadata"]["file"])

header_line <- process.stdout.read_line()
header <- decode_json(header_line)

header_fields <- header["Header"]["fields"]

for each field <- header_fields do
    name = field[0]
    value = field[1]

    print("Header name: " + name + " value: " + value)
end for
```

Next, get the record block data:

```
message_line <- process.stdout.read_line()
message <- decode_json(message_line)

loop do
    if message.has_key("BlockEnd") then
        break loop
    end if

    block_chunk <- message
    b64_data <- block_chunk["BlockChunk"]["data"]
    data <- decode_base64(b64_data)

    print("Read " + data.length() + " bytes")
end loop
```

Once you have read the end of the record, repeat the steps for each record until the end of file message is reached:

```
message_line <- process.stdout.read_line()
message <- decode_json(message_line)
is_end_of_file <- message.has_key("EndOfFile")
```
