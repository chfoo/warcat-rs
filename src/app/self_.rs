use regex::Regex;
use takecrate::{
    inst::{InstallConfig, PackageManifest},
    manifest::AppId,
};

use super::arg::{SelfCommand, SelfSubcommand};

pub fn self_(args: &SelfCommand) -> anyhow::Result<()> {
    match &args.command {
        SelfSubcommand::Install { quiet } => {
            if *quiet {
                install_quiet()
            } else {
                install_interactive()
            }
        }
        SelfSubcommand::Uninstall { quiet } => {
            if *quiet {
                uninstall_quiet()
            } else {
                uninstall_interactive()
            }
        }
    }
}

pub fn is_installer() -> bool {
    if std::env::args().len() > 1 {
        return false;
    }

    let name = std::env::current_exe().unwrap_or_default();
    let name = name.to_string_lossy();
    let name = name.strip_suffix(std::env::consts::EXE_SUFFIX).unwrap_or(&name);
    let pattern = Regex::new(r"(?-ui:[. _-]installer)$").unwrap();
    pattern.is_match(name)
}

pub fn install_interactive() -> anyhow::Result<()> {
    let manifest = package_manifest()?;
    takecrate::install_interactive(&manifest)?;
    Ok(())
}

pub fn install_quiet() -> anyhow::Result<()> {
    let manifest = package_manifest()?;
    let config = InstallConfig::new_user()?;
    takecrate::install(&manifest, &config)?;
    Ok(())
}

pub fn uninstall_interactive() -> anyhow::Result<()> {
    let app_id = app_id();
    takecrate::uninstall_interactive(&app_id)?;
    Ok(())
}

pub fn uninstall_quiet() -> anyhow::Result<()> {
    let app_id = app_id();
    takecrate::uninstall(&app_id)?;
    Ok(())
}

fn app_id() -> AppId {
    AppId::new("io.github.chfoo.warcat-rs").unwrap()
}

fn package_manifest() -> anyhow::Result<PackageManifest> {
    let mut manifest = PackageManifest::new(&app_id())
        .with_interactive_uninstall_args(&["self", "uninstall"])
        .with_quiet_uninstall_args(&["self", "uninstall", "--quiet"])
        .with_self_exe_renamed(format!("warcat{}", std::env::consts::EXE_SUFFIX))?;

    manifest.app_metadata.display_name = "Warcat".to_string();
    manifest.app_metadata.display_version = clap::crate_version!().to_string();

    Ok(manifest)
}
