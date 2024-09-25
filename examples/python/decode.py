import subprocess
import json

import message


def main():
    with subprocess.Popen(
        ["cargo", "run", "--", "export", "--input=examples/example.warc"],
        stdout=subprocess.PIPE,
    ) as process:
        for msg in message.decode(process.stdout):
            if isinstance(msg, message.Header):
                print(msg.fields)
            elif isinstance(msg, message.BlockChunk):
                print(len(msg.data))
            elif isinstance(msg, message.BlockEnd):
                print("---")



if __name__ == "__main__":
    main()
