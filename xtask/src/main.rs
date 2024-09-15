use clap::{Parser, Subcommand};

mod doc;

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    BuildDoc,
    GenCliDoc,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::BuildDoc => crate::doc::build_doc(),
        Command::GenCliDoc => crate::doc::gen_cli_doc(),
    }
}
