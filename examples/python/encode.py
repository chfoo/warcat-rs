# Example on how to write WARC files.
import subprocess
import zlib

import message


def main():
    # Launch the warcat program. The options provided will tell it to read
    # JSON as a line from standard in.
    # Ensure you have warcat on the search path or adjust the path as needed.
    with subprocess.Popen(
        [
            "warcat",
            "import",
            "--compression=none",
            "--format=jsonl",
        ],
        stdin=subprocess.PIPE,
    ) as process:
        # Write a record header with the given header fields.
        # Note: this header is not valid; it is simply a concise demonstration.
        header = message.Header(
            "WARC/1.1",
            [
                ("WARC-Record-Type", "resource"),
                ("Content-Length", "12"),
            ],
        )
        message.encode(process.stdin, header)

        # Write the record block data.
        checksum = 0

        data = b"Hello world!"
        checksum = zlib.crc32(data, checksum)

        block_chunk = message.BlockChunk(data)
        message.encode(process.stdin, block_chunk)

        # Write the end of the block message.
        block_end = message.BlockEnd(checksum)
        message.encode(process.stdin, block_end)

        # Finish writing the file.
        message.encode(process.stdin, message.EndOfFile())


if __name__ == "__main__":
    main()
