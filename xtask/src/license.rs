use std::{fs::File, io::Write};

use cargo_license::GetDependenciesOpt;
use cargo_license_cargo_metadata::MetadataCommand;

pub fn generate_license_file() -> anyhow::Result<()> {
    let mut command = MetadataCommand::new();
    command.features(cargo_license_cargo_metadata::CargoOpt::SomeFeatures(vec![
        "bin".to_string()
    ]));

    let opt = GetDependenciesOpt {
        avoid_build_deps: true,
        avoid_dev_deps: true,
        ..Default::default()
    };

    let dependencies = cargo_license::get_dependencies_from_cargo_lock(command, opt)?;

    let mut file = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open("xtask/src/dist_license.txt")?;

    writeln!(
        file,
        "Automatically generated using xtask. Do not manually edit!"
    )?;
    writeln!(file, "</>")?;

    writeln!(file, "License")?;
    writeln!(file, "=======")?;
    writeln!(file)?;

    for dependency in dependencies {
        writeln!(file, "{} {}", &dependency.name, &dependency.version)?;
        writeln!(file, "-----")?;
        writeln!(file)?;

        writeln!(file, "Authors:")?;
        for author in dependency
            .authors
            .as_deref()
            .unwrap_or("<unknown>")
            .split("|")
        {
            writeln!(file, "    {}", author)?
        }

        writeln!(file, "License:")?;
        writeln!(
            file,
            "    {}",
            dependency.license.as_deref().unwrap_or("<unknown>")
        )?;

        writeln!(file, "Repository:")?;
        writeln!(
            file,
            "    {}",
            dependency.repository.as_deref().unwrap_or("<unknown>")
        )?;

        writeln!(file)?;
        writeln!(file)?;
    }

    Ok(())
}
