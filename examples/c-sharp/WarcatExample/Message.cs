using System.Text.Json.Serialization;

namespace WarcatExample;

public class Message
{
    [JsonPropertyName("Metadata")]
    public Metadata? Metadata { get; set; }
    [JsonPropertyName("Header")]
    public Header? Header { get; set; }
    [JsonPropertyName("BlockChunk")]
    public BlockChunk? BlockChunk { get; set; }
    [JsonPropertyName("BlockEnd")]
    public BlockEnd? BlockEnd { get; set; }
    [JsonPropertyName("ExtractMetadata")]
    public ExtractMetadata? ExtractMetadata { get; set; }
    [JsonPropertyName("ExtractChunk")]
    public ExtractChunk? ExtractChunk { get; set; }
    [JsonPropertyName("ExtractEnd")]
    public ExtractEnd? ExtractEnd { get; set; }
    [JsonPropertyName("EndOfFile")]
    public EndOfFile? EndOfFile { get; set; }
}

public class Metadata
{
    public required string File { get; set; }
    public required ulong Position { get; set; }
}

public class Header
{
    public required string Version { get; set; }
    public required List<string[]> Fields { get; set; }
}

public class BlockChunk
{
    public required byte[] Data { get; set; }
}

public class BlockEnd
{
    public uint? Crc32 { get; set; }
    public uint? Crc32c { get; set; }
    public ulong? Xxh3 { get; set; }
}

public class ExtractMetadata
{
    public required bool HasContent { get; set; }
    public required List<string> FilePathComponents { get; set; }
    public required bool IsTruncated { get; set; }
}

public class ExtractChunk
{
    public required byte[] Data { get; set; }
}

public class ExtractEnd
{
    public uint? Crc32 { get; set; }
    public uint? Crc32c { get; set; }
    public ulong? Xxh3 { get; set; }
}

public class EndOfFile { }