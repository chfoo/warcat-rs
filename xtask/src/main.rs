use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod doc;
mod gh;
mod license;
mod package;

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
    PackageBin {
        target: String,
    },
    DownloadArtifacts {
        #[arg(long, short)]
        access_token: PathBuf,
        #[arg(long, short)]
        workflow_id: String,
    },
    GenLicense,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::BuildDoc => crate::doc::build_doc(),
        Command::GenCliDoc => crate::doc::gen_cli_doc(),
        Command::PackageBin { target } => crate::package::package_bin(&target),
        Command::DownloadArtifacts {
            access_token,
            workflow_id,
        } => crate::gh::download_artifacts(&access_token, &workflow_id),
        Command::GenLicense => crate::license::generate_license_file(),
    }
}
