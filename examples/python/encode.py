import subprocess
import json

import message


def main():
    with subprocess.Popen(
        ["cargo", "run", "--", "import", "--compression=none"],
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

        block_chunk = message.BlockChunk(b"Hello world!")
        message.encode(process.stdin, block_chunk)

        block_end = message.BlockEnd(2073618257)
        message.encode(process.stdin, block_end)


if __name__ == "__main__":
    main()
