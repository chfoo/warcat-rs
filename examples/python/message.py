import json
import base64
import io


class Metadata:
    file: str
    position: int

    def __init__(self, file: str, position: int):
        self.file = file
        self.position = position

    def deserialize(file: str, position: str):
        return Metadata(file, int(position))

    def serialize(self) -> dict:
        return {
            "Metadata": {
                "file": self.file,
                "position": self.position,
            }
        }


class Header:
    version: str
    fields: list

    def __init__(self, version: str, fields: list):
        self.version = version
        self.fields = fields

    def deserialize(version: str, fields: list):
        return Header(version, fields)

    def serialize(self) -> dict:
        return {
            "Header": {
                "version": self.version,
                "fields": self.fields,
            }
        }


class BlockChunk:
    data: bytes

    def __init__(self, data: bytes):
        self.data = data

    def deserialize(data: str):
        return BlockChunk(base64.b64decode(data))

    def serialize(self) -> dict:
        return {"BlockChunk": {"data": base64.b64encode(self.data).decode("utf8")}}


class BlockEnd:
    crc32c: int

    def __init__(self, crc32: int = None, crc32c: int = None, xxh3: int = None):
        self.crc32 = crc32
        self.crc32c = crc32c
        self.xxh3 = xxh3

    def deserialize(crc32: int = None, crc32c: int = None, xxh3: int = None):
        return BlockEnd(crc32, crc32c, xxh3)

    def serialize(self) -> dict:
        return {
            "BlockEnd": {"crc32": self.crc32, "crc32c": self.crc32c, "xxh3": self.xxh3}
        }


MESSAGE_TABLE = {
    "Metadata": Metadata,
    "Header": Header,
    "BlockChunk": BlockChunk,
    "BlockEnd": BlockEnd,
}


class MessageEncoder(json.JSONEncoder):
    def default(self, o):
        if hasattr(o, "serialize"):
            return o.serialize()

        return super().default(o)


def message_object_hook(obj: dict):
    for k, v in MESSAGE_TABLE.items():
        if k in obj:
            return MESSAGE_TABLE[k].deserialize(**obj[k])

    return obj


def encode(stream: io.BufferedIOBase, message):
    data = MessageEncoder().encode(message).encode("utf8")

    stream.write(data)
    stream.write(b"\n")


def decode(stream: io.BufferedIOBase):
    for line in stream.readlines():
        segment = line.decode("utf8")

        yield json.loads(segment, object_hook=message_object_hook)
