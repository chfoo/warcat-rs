use std::{
    env::consts::EXE_SUFFIX,
    path::{Path, PathBuf},
    process::Command,
};

pub fn package_bin(target_triple: &str) -> anyhow::Result<()> {
    let packager = Packager::new(target_triple.to_string());

    match std::env::consts::OS {
        "windows" => packager.package_zip(),
        "macos" => packager.package_tar("tgz"),
        "linux" => packager.package_tar("tar.gz"),
        _ => unimplemented!(),
    }
}

struct Packager {
    target_triple: String,
}

impl Packager {
    fn new(target_triple: String) -> Self {
        Self { target_triple }
    }

    fn package_zip(&self) -> anyhow::Result<()> {
        let staging_dir = self.prepare_staging_dir()?;
        let output_dir = self.prepare_output_dir()?;
        let package_name = self.build_package_name()?;
        let output_file = output_dir.join(format!("{}.zip", package_name));

        eprintln!("Creating archive {:?} of {:?}", output_file, staging_dir);
        let status = Command::new(r"C:\Program Files\7-Zip\7z.exe")
            .arg("a")
            .arg(&output_file)
            .arg("./")
            .current_dir(&staging_dir)
            .status()?;

        anyhow::ensure!(status.success());
        eprintln!("Done");

        Ok(())
    }

    fn package_tar(&self, archive_extension: &str) -> anyhow::Result<()> {
        let staging_dir = self.prepare_staging_dir()?;
        let output_dir = self.prepare_output_dir()?;
        let package_name = self.build_package_name()?;
        let output_file = output_dir.join(format!("{}.{}", package_name, archive_extension));

        let mut staging_dir_contents = Vec::new();

        for entry in std::fs::read_dir(&staging_dir)? {
            let entry = entry?;

            staging_dir_contents.push(entry.file_name());
        }

        eprintln!("Creating archive {:?} of {:?}", output_file, staging_dir);
        let status = Command::new("tar")
            .arg("-c")
            .arg("-f")
            .arg(&output_file)
            .arg("-v")
            .arg("-z")
            .args(staging_dir_contents)
            .current_dir(&staging_dir)
            .status()?;

        anyhow::ensure!(status.success());
        eprintln!("Done");

        Ok(())
    }

    fn build_package_name(&self) -> anyhow::Result<String> {
        let version = package_version()?;
        let friendly_target = target_triple_to_friendly_name(&self.target_triple);
        let package_name = format!("warcat-{}-{}", version, friendly_target);

        Ok(package_name)
    }

    fn prepare_staging_dir(&self) -> anyhow::Result<PathBuf> {
        let version = package_version()?;
        // let package_name = self.build_package_name()?;

        let target_dir = target_dir()?;
        let staging_dir = target_dir.join("xtask-package-bin-staging");
        // let content_dir = staging_dir.join(&package_name);
        let content_dir = staging_dir.clone();

        if staging_dir.exists() {
            eprintln!("Removing directory {:?}", staging_dir);
            std::fs::remove_dir_all(&staging_dir)?;
        }

        eprintln!("Creating directory {:?}", content_dir);
        std::fs::create_dir_all(&content_dir)?;

        let source_bin_path = target_dir
            .join(&self.target_triple)
            .join("release")
            .join(format!("warcat{}", EXE_SUFFIX));

        let dest_bin_path = content_dir.join(format!("warcat-{}-installer{}", version, EXE_SUFFIX));

        for (from, to) in [
            (source_bin_path.as_path(), dest_bin_path.as_path()),
            (Path::new("LICENSE.txt"), &content_dir.join("LICENSE.txt")),
            (Path::new("README.md"), &content_dir.join("README.txt")),
        ] {
            eprintln!("Copying {:?} -> {:?}", from, to);
            std::fs::copy(from, to)?;
        }

        Ok(staging_dir)
    }

    fn prepare_output_dir(&self) -> anyhow::Result<PathBuf> {
        let target_dir = target_dir()?;
        let output_dir = target_dir.join("xtask-package-bin-output");

        if output_dir.exists() {
            eprintln!("Removing directory {:?}", output_dir);
            std::fs::remove_dir_all(&output_dir)?;
        }

        eprintln!("Creating directory {:?}", output_dir);
        std::fs::create_dir_all(&output_dir)?;

        Ok(output_dir)
    }
}

fn target_triple_to_friendly_name(target_triple: &str) -> &str {
    match target_triple {
        "x86_64-pc-windows-msvc" => "windows-x86_64",
        "aarch64-pc-windows-msvc" => "windows-aarch64",
        "x86_64-apple-darwin" => "macos-x86_64",
        "aarch64-apple-darwin" => "macos-aarch64",
        "x86_64-unknown-linux-musl" => "linux-x86_64",
        "aarch64-unknown-linux-musl" => "linux-aarch64",
        _ => unimplemented!(),
    }
}

fn target_dir() -> anyhow::Result<PathBuf> {
    let metadata = cargo_metadata::MetadataCommand::new().exec()?;
    Ok(metadata.target_directory.into_std_path_buf())
}

fn package_version() -> anyhow::Result<String> {
    let metadata = cargo_metadata::MetadataCommand::new().exec()?;
    let package = metadata
        .packages
        .iter()
        .find(|package| package.name == "warcat")
        .ok_or_else(|| anyhow::anyhow!("couldn't get package version"))?;
    Ok(package.version.to_string())
}
