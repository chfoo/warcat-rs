use std::{fs::File, io::Write, path::Path, str::FromStr, sync::Mutex};

use tracing_subscriber::{layer::SubscriberExt, Layer};

use super::progress::global_progress_bar;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Off,
}

impl Level {
    fn as_level_filter(&self) -> tracing_subscriber::filter::LevelFilter {
        match self {
            Self::Trace => tracing_subscriber::filter::LevelFilter::TRACE,
            Self::Debug => tracing_subscriber::filter::LevelFilter::DEBUG,
            Self::Info => tracing_subscriber::filter::LevelFilter::INFO,
            Self::Warn => tracing_subscriber::filter::LevelFilter::WARN,
            Self::Error => tracing_subscriber::filter::LevelFilter::ERROR,
            Self::Off => tracing_subscriber::filter::LevelFilter::OFF,
        }
    }
}

impl Default for Level {
    fn default() -> Self {
        Self::Off
    }
}

impl FromStr for Level {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            "off" => Ok(Self::Off),
            _ => Err(()),
        }
    }
}

struct ProgressBarMutexWriter<W: Write> {
    dest: W,
}

impl<W: Write> ProgressBarMutexWriter<W> {
    fn new(dest: W) -> Self {
        Self { dest }
    }
}

impl<W: Write> Write for ProgressBarMutexWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        global_progress_bar().suspend(|| self.dest.write(buf))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        global_progress_bar().suspend(|| self.dest.flush())
    }
}

pub fn set_up_logging(level: Level, file: Option<&Path>, json: bool) -> std::io::Result<()> {
    let file_sub = if let Some(path) = file {
        let writer = File::options().create(true).append(true).open(path)?;
        Some(tracing_subscriber::fmt::layer().with_writer(Mutex::new(writer)))
    } else {
        None
    };

    let stderr_sub = if file.is_none() {
        let writer = ProgressBarMutexWriter::new(std::io::stderr());
        Some(tracing_subscriber::fmt::layer().with_writer(Mutex::new(writer)))
    } else {
        None
    };

    let json_sub = if json {
        Some(tracing_subscriber::fmt::layer().json())
    } else {
        None
    };

    let sub = tracing_subscriber::Registry::default();
    let sub = sub.with(file_sub.with_filter(level.as_level_filter()));
    let sub = sub.with(stderr_sub.with_filter(level.as_level_filter()));
    let sub = sub.with(json_sub);
    tracing::subscriber::set_global_default(sub).unwrap();

    tracing::debug!("logging configured");

    Ok(())
}
