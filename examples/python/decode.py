# Example on how to read WARC files.
import subprocess

import message


def main():
    # Launch the warcat program. The options provided will tell it to write
    # JSON as a line to standard out.
    # In your code, do not use "cargo",
    # use ["warcat", "export", ... ]
    with subprocess.Popen(
        [
            "cargo",
            "run",
            "--",
            "export",
            "--input=examples/example.warc",
            "--format=jsonl",
        ],
        stdout=subprocess.PIPE,
    ) as process:
        # Decode each message by using our helper module.
        for msg in message.decode(process.stdout):
            if isinstance(msg, message.Header):
                # We decoded the start of the record.
                print(msg.fields)
            elif isinstance(msg, message.BlockChunk):
                # We decoded the body of the record.
                print(len(msg.data))
            elif isinstance(msg, message.BlockEnd):
                # The end of the record was reached.
                print("---")


if __name__ == "__main__":
    main()
