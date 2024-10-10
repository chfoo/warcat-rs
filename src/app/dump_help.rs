pub fn dump_help() -> anyhow::Result<()> {
    let config = clap_markdown::MarkdownOptions::new()
        .show_footer(false)
        .show_table_of_contents(false)
        .title("{title}".to_string());

    println!(
        "{}",
        clap_markdown::help_markdown_custom::<super::arg::Args>(&config)
    );

    Ok(())
}
