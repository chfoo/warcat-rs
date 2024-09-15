use std::process::ExitCode;

use clap::Parser;

use self::arg::Args;
use self::arg::Command;

// pub access for generating CLI documentation using xtask
#[doc(hidden)]
pub mod arg;

mod export;
mod format;
mod import;
mod io;
mod logging;
mod model;
mod progress;

pub fn run() -> ExitCode {
    match run_impl() {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(?error);
            eprintln!("{:#}", error);
            ExitCode::FAILURE
        }
    }
}

fn run_impl() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.quiet {
        self::progress::disable_global_progress_bar();
    }

    self::logging::set_up_logging(args.log_level, args.log_file.as_deref(), args.log_json)?;

    match args.command {
        Command::Export(args) => {
            self::export::export(&args)?;
        }
        Command::Import(args) => {
            self::import::import(&args)?;
        }
    }

    self::progress::global_progress_bar().println("Done.")?;

    Ok(())
}
