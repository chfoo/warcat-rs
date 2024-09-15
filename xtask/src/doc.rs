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
    let config = clap_markdown::MarkdownOptions::new()
        .show_footer(false)
        .show_table_of_contents(false)
        .title("CLI Reference".to_string());
    let text = "% ATTENTION: This file was automatically generated using cargo xtask.\n\
        % Do not edit this file!\n\n"
        .to_owned()
        + &clap_markdown::help_markdown_custom::<warcat::app::arg::Args>(&config);

    std::fs::write("doc/cli_reference.md", text.as_bytes())?;

    Ok(())
}