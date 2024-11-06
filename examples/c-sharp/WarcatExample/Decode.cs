// Example on how to read WARC files.
using System.Diagnostics;
using System.Text.Json;

namespace WarcatExample;

class Decode
{
    public static void Run()
    {
        var options = Message.Options();

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
    }
}
