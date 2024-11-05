// Example on how to write WARC files.
using System.Diagnostics;
using System.IO.Hashing;
using System.Text;
using System.Text.Json;

namespace WarcatExample;

class Encode
{
    public static void Run()
    {
        var options = Message.Options();

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
    }
}
