use std::io::{BufRead, Read, Write};

const IO_BUFFER_LENGTH: usize = 4096;

pub trait LogicalPosition {
    fn logical_position(&self) -> u64;
}

pub struct BufferReader<R: Read> {
    reader: R,
    buffer: Vec<u8>,
    buffer_position: usize,
    logical_position: u64,
}

impl<R: Read> BufferReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            buffer_position: 0,
            logical_position: 0,
        }
    }

    pub fn get_ref(&self) -> &R {
        &self.reader
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    pub fn into_inner(self) -> R {
        self.reader
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer[self.buffer_position..]
    }

    pub fn fill_buffer(&mut self) -> std::io::Result<usize> {
        let original_len = self.buffer.len();
        self.buffer.resize(original_len + IO_BUFFER_LENGTH, 0);

        let range = original_len..;

        match self.reader.read(&mut self.buffer[range]) {
            Ok(read_len) => {
                self.buffer.truncate(original_len + read_len);
                Ok(read_len)
            }
            Err(error) => {
                self.buffer.truncate(original_len);
                Err(error)
            }
        }
    }

    pub fn fill_buffer_if_empty(&mut self) -> std::io::Result<usize> {
        if self.buffer.is_empty() {
            self.fill_buffer()
        } else {
            Ok(0)
        }
    }

    fn compact_buffer(&mut self) {
        self.buffer.drain(..self.buffer_position);
        self.buffer_position = 0;
    }

    fn read_using_buffer(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        self.fill_buffer_if_empty()?;

        let range = self.buffer_position..self.buffer.len().min(self.buffer_position + buf.len());
        let write_len = range.len();

        buf.write_all(&self.buffer[range])?;
        self.buffer_position += write_len;

        self.clean_up_buffer();

        Ok(write_len)
    }

    fn clean_up_buffer(&mut self) {
        if self.buffer_position >= self.buffer.len() {
            self.buffer.clear();
            self.buffer_position = 0;
        } else if self.buffer_position > IO_BUFFER_LENGTH {
            self.compact_buffer();
        }
    }
}

impl<R: Read> Read for BufferReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read_len = if buf.len() >= IO_BUFFER_LENGTH && self.buffer.is_empty() {
            self.reader.read(buf)
        } else {
            self.read_using_buffer(buf)
        }?;

        self.logical_position += read_len as u64;
        Ok(read_len)
    }
}

impl<R: Read> BufRead for BufferReader<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.fill_buffer_if_empty()?;

        Ok(self.buffer())
    }

    fn consume(&mut self, amt: usize) {
        self.buffer_position += amt;
        self.logical_position += amt as u64;
        self.clean_up_buffer();
    }
}

impl<R: Read> LogicalPosition for BufferReader<R> {
    fn logical_position(&self) -> u64 {
        self.logical_position
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_buffer_reader() {
        let mut source = Vec::new();
        let data_len = 50000;

        for i in 0..data_len {
            source.push(i as u8);
        }

        let mut r = BufferReader::new(Cursor::new(source));
        let mut actual = Vec::new();
        let mut remain_len = data_len;
        let mut buf = Vec::new();

        for buf_size in [10, 2000, 4000, 4096, 4096, 5000].iter().cycle() {
            if remain_len == 0 {
                break;
            }
            let read_len = (*buf_size).min(remain_len);
            buf.resize(read_len, 0);
            r.read_exact(&mut buf).unwrap();

            actual.extend_from_slice(&buf);
            remain_len -= read_len;
        }

        let source = r.into_inner().into_inner();

        assert_eq!(source, actual);
    }

    #[test]
    fn test_buffer_reader_until() {
        let mut source = Vec::new();
        let data_len = 10000;

        for i in 0..data_len {
            if i == 5000 {
                source.push(b'\n');
            } else {
                source.push(0);
            }
        }

        let mut r = BufferReader::new(Cursor::new(source));
        let mut buf = Vec::new();
        r.read_until(b'\n', &mut buf).unwrap();

        assert_eq!(buf.len(), 5001);
    }
}
