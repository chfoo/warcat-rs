use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod digest;
mod doc;
#[cfg(feature = "bloat")]
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
    /// Convenience command to build Sphinx HTML user guide.
    BuildDoc,
    /// Generate CLI reference to doc directory.
    GenCliDoc,
    /// Package a built release binary along with supporting files for distribution.
    PackageBin { target: String },
    /// Download the artifacts from GitHub Actions containing the packages.
    DownloadArtifacts {
        #[arg(long, short)]
        access_token: PathBuf,
        #[arg(long, short)]
        workflow_id: String,
    },
    /// Output a hash of the packages.
    Digests {
        #[arg(long)]
        minisign_secret_key: Option<PathBuf>,
    },
    /// Generate the license file of dependencies.
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
        } => {
            #[cfg(feature = "bloat")]
            {
                crate::gh::download_artifacts(&access_token, &workflow_id)
            }
            #[cfg(not(feature = "bloat"))]
            {
                let _ = access_token;
                let _ = workflow_id;
                unimplemented!("feature 'bloat' required")
            }
        }
        Command::Digests {
            minisign_secret_key,
        } => crate::digest::compute_digests(minisign_secret_key.as_deref()),
        Command::GenLicense => crate::license::generate_license_file(),
    }
}
