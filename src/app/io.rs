use std::{
    fs::File,
    io::{Read, Seek, Stdin, Stdout, Write},
    path::Path,
};

use crate::error::{GeneralError, ProtocolError, ProtocolErrorKind};

#[derive(Debug)]
pub enum ProgramInput {
    File(File),
    Stdin(Stdin),
}

impl ProgramInput {
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let path = path.as_ref();

        if path.to_str() == Some("-") {
            Ok(Self::Stdin(std::io::stdin()))
        } else {
            let file = File::options().read(true).open(path)?;
            Ok(Self::File(file))
        }
    }

    pub fn seek_absolute(&mut self, position: u64) -> Result<(), GeneralError> {
        match self {
            ProgramInput::File(file) => {
                file.seek(std::io::SeekFrom::Start(position))?;
                Ok(())
            }
            ProgramInput::Stdin(_stdin) => {
                Err(ProtocolError::new(ProtocolErrorKind::IoNotSeekable).into())
            }
        }
    }
}

impl Read for ProgramInput {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            ProgramInput::File(r) => r.read(buf),
            ProgramInput::Stdin(r) => r.read(buf),
        }
    }
}

#[derive(Debug)]
pub enum ProgramOutput {
    File(File),
    Stdout(Stdout),
}

impl ProgramOutput {
    pub fn open<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let path = path.as_ref();

        if path.to_str() == Some("-") {
            Ok(Self::Stdout(std::io::stdout()))
        } else {
            let file = File::options()
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)?;
            Ok(Self::File(file))
        }
    }
}

impl Write for ProgramOutput {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            ProgramOutput::File(w) => w.write(buf),
            ProgramOutput::Stdout(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            ProgramOutput::File(w) => w.flush(),
            ProgramOutput::Stdout(w) => w.flush(),
        }
    }
}
