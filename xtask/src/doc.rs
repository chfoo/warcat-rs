use std::process::Command;

pub fn build_doc() -> anyhow::Result<()> {
    let status = if cfg!(windows) {
        Command::new("cmd.exe")
            .arg("/c")
            .arg("make.bat")
            .arg("html")
            .current_dir("doc/")
            .status()?
    } else {
        Command::new("make")
            .arg("html")
            .current_dir("doc/")
            .status()?
    };

    if !status.success() {
        anyhow::bail!("command failure {:?}", status.code())
    } else {
        Ok(())
    }
}

pub fn gen_cli_doc() -> anyhow::Result<()> {
    let cargo = std::env::var("CARGO")?;
    let output = Command::new(cargo)
        .arg("run")
        .arg("--features=bin")
        .arg("--")
        .arg("dump-help")
        .stderr(std::process::Stdio::inherit())
        .output()?;

    let text = String::from_utf8(output.stdout)?;
    let text = text.replace("{title}", "CLI Reference");

    let text = "% ATTENTION: This file was automatically generated using cargo xtask.\n\
        % Do not manually edit this file!\n\n"
        .to_owned()
        + &text;

    std::fs::write("doc/cli_reference.md", text.as_bytes())?;

    Ok(())
}
