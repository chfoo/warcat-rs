# Integration to other programs

Integration to other programs is done through standard input and output using the `export` and `import` commands.

For reading WARC files, the `export` command will format the data into messages such as JSON which your program can ingest and process. Likewise for writing WARC files, the `import` command accepts messages from your program.

For examples, [see here](https://github.com/chfoo/warcat-rs/tree/main/examples).

The format of the messages is documented in the next section.

