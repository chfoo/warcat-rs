import json
import base64
import io
import os

RECORD_SEPARATOR = b"\x1e"


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

    def __init__(self, crc32c: int):
        self.crc32c = crc32c

    def deserialize(crc32c: int):
        return BlockEnd(crc32c)

    def serialize(self) -> dict:
        return {"BlockEnd": {"crc32c": self.crc32c}}


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

    stream.write(RECORD_SEPARATOR)
    stream.write(data)
    stream.write(b"\n")


def decode(stream: io.BufferedIOBase):
    buffer = bytearray()

    for line in stream.readlines():
        buffer.extend(line)

        if RECORD_SEPARATOR not in buffer:
            continue

        if RECORD_SEPARATOR in buffer:
            segment, _, buffer = buffer.partition(RECORD_SEPARATOR)
            segment = segment.decode("utf8")

        if len(segment.strip()) == 0:
            continue

        yield json.loads(segment, object_hook=message_object_hook)

    segment = buffer.decode("utf8")

    if len(segment.strip()):
        yield json.loads(segment, object_hook=message_object_hook)
