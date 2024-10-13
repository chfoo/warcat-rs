# Usage examples

## Extract everything

Extract resources from a WARC file as much as possible:

```sh
warcat extract --input my_warc_file.warc.gz --output my_output_folder
```

## Extract a single item

First locate where the item is within the WARC file:

```sh
warcat list --input my_warc_file.warc.gz --format csv
```

For the purposes of this example, we'll use this hypothetical listing:

```csv
45678,<urn:example:abcdef>,response,application/http; msgtype=response,https://example.com/index.html
```

Then provide the position and ID to the `get extract` command:

```sh
warcat get extract --input my_warc_file.warc.gz --position 45678 --id "<urn:example:abcdef>" --output index.html
```
