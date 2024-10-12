use std::process::ExitCode;

use clap::Parser;

use self::arg::Args;
use self::arg::Command;

mod arg;
mod common;
mod dump_help;
mod export;
mod extract;
mod filter;
mod format;
mod import;
mod io;
mod list;
mod logging;
mod model;
mod progress;
mod self_;
mod verify;

pub fn run() -> ExitCode {
    match run_impl() {
        Ok(exit_code) => exit_code,
        Err(error) => {
            tracing::error!(?error);
            eprintln!("{:#}", error);
            ExitCode::FAILURE
        }
    }
}

fn run_impl() -> anyhow::Result<ExitCode> {
    if self::self_::is_installer() {
        self::self_::install_interactive()?;
        return Ok(ExitCode::SUCCESS);
    }

    let args = Args::parse();

    if args.quiet {
        self::progress::disable_global_progress_bar();
    }

    self::logging::set_up_logging(args.log_level, args.log_file.as_deref(), args.log_json)?;

    let exit_code = match args.command {
        Command::Export(args) => {
            self::export::export(&args)?;
            ExitCode::SUCCESS
        }
        Command::Import(args) => {
            self::import::import(&args)?;
            ExitCode::SUCCESS
        }
        Command::List(args) => {
            self::list::list(&args)?;
            ExitCode::SUCCESS
        }
        Command::Extract(args) => {
            self::extract::extract(&args)?;
            ExitCode::SUCCESS
        }
        Command::Verify(args) => self::verify::verify(&args)?,
        Command::Self_(args) => {
            self::self_::self_(&args)?;
            ExitCode::SUCCESS
        }
        Command::DumpHelp => {
            self::dump_help::dump_help()?;
            ExitCode::SUCCESS
        }
    };

    self::progress::global_progress_bar().println("Done.")?;

    Ok(exit_code)
}
