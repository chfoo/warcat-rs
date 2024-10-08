import subprocess
import zlib

import message


def main():
    with subprocess.Popen(
        ["cargo", "run", "--", "import", "--compression=none", "--format=jsonl"],
        stdin=subprocess.PIPE,
    ) as process:
        header = message.Header(
            "WARC/1.1",
            [
                ("WARC-Record-Type", "resource"),
                ("Content-Length", "12"),
            ],
        )
        message.encode(process.stdin, header)

        checksum = 0

        data = b"Hello world!"
        checksum = zlib.crc32(data, checksum)

        block_chunk = message.BlockChunk(data)
        message.encode(process.stdin, block_chunk)

        block_end = message.BlockEnd(checksum)
        message.encode(process.stdin, block_end)


if __name__ == "__main__":
    main()
