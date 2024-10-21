using System.Diagnostics;
using System.IO.Hashing;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using WarcatExample;

// Example on how to read WARC files.

// JSON options:
// Use snake_case for names.
var options = new JsonSerializerOptions
{
    PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
    DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
};

// Launch the warcat program. The options provided will tell it to write
// JSON as a line to standard out.
// Ensure you have warcat on the search path or adjust the path as needed.
using (var process = new Process())
{
    process.StartInfo.FileName = "warcat";
    process.StartInfo.ArgumentList.Add("export");
    process.StartInfo.ArgumentList.Add("--input=example.warc");
    process.StartInfo.ArgumentList.Add("--format=jsonl");
    process.StartInfo.RedirectStandardOutput = true;
    process.Start();

    while (true)
    {
        var line = process.StandardOutput.ReadLine();

        if (line == null)
        {
            break;
        }

        // Decode each message
        var message = JsonSerializer.Deserialize<Message>(line, options)!;

        if (message.Header != null)
        {
            // We decoded the start of the record.
            foreach (var field in message.Header.Fields)
            {
                Console.WriteLine($"{field[0]}:{field[1]}");
            }
        }
        else if (message.BlockChunk != null)
        {
            // We decoded the body of the record.
            Console.WriteLine($"{message.BlockChunk.Data.Length}");
        }
        else if (message.EndOfFile != null)
        {
            // The end of the record was reached.
            Console.WriteLine("---");
        }
    }
}

// Example on how to write WARC files.

// Launch the warcat program. The options provided will tell it to read
// JSON as a line from standard in.
// Ensure you have warcat on the search path or adjust the path as needed.
using (var process = new Process())
{
    process.StartInfo.FileName = "warcat";
    process.StartInfo.ArgumentList.Add("import");
    process.StartInfo.ArgumentList.Add("--compression=none");
    process.StartInfo.ArgumentList.Add("--format=jsonl");
    process.StartInfo.RedirectStandardInput = true;
    process.Start();

    // Write a record header with the given header fields.
    // Note: this header is not valid; it is simply a concise demonstration.

    var header = new Message()
    {
        Header = new Header()
        {
            Version = "WARC/1.1",
            Fields = [
                ["WARC-Record-Type", "resource"],
                ["Content-Length", "12"],
            ]
        }
    };
    process.StandardInput.WriteLine(JsonSerializer.Serialize(header, options));

    // Write the record block data.
    var hasher = new XxHash3();

    var data = Encoding.UTF8.GetBytes("Hello world!");
    hasher.Append(data);

    var block_chunk = new Message()
    {
        BlockChunk = new BlockChunk()
        {
            Data = data
        }
    };
    process.StandardInput.WriteLine(JsonSerializer.Serialize(block_chunk, options));

    // Write the end of the block message.
    var block_end = new Message()
    {
        BlockEnd = new BlockEnd()
        {
            Xxh3 = hasher.GetCurrentHashAsUInt64()
        }
    };
    process.StandardInput.WriteLine(JsonSerializer.Serialize(block_end, options));

    // Finish writing the file.
    process.StandardInput.WriteLine(JsonSerializer.Serialize(new Message() { EndOfFile = new EndOfFile() }, options));
}