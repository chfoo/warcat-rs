use std::{fs::File, path::Path};

use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue},
};
use zip::ZipArchive;

const REPO_USER: &str = "chfoo";
const REPO_NAME: &str = "warcat-rs";

pub fn download_artifacts(access_token: &Path, workflow_id: &str) -> anyhow::Result<()> {
    let token = std::fs::read_to_string(access_token)?;
    let token = token.trim_ascii();

    let mut headers = HeaderMap::new();
    let mut token_value = HeaderValue::from_str(&format!("Bearer {}", token))?;
    token_value.set_sensitive(true);
    headers.insert("Accept", "application/vnd.github+json".try_into()?);
    headers.insert("Authorization", token_value);
    headers.insert("X-GitHub-Api-Version", "2022-11-28".try_into()?);
    headers.insert("User-Agent", "warcat-rs-xtask".try_into()?);

    let client = Client::builder()
        .https_only(true)
        .gzip(true)
        .default_headers(headers)
        .build()?;

    eprintln!("Getting artifacts..");
    let response = client
        .get(format!(
            "https://api.github.com/repos/{}/{}/actions/runs/{}/artifacts",
            REPO_USER, REPO_NAME, workflow_id
        ))
        .send()?;

    eprintln!(" .. {}", response.status());

    if !response.status().is_success() {
        eprintln!("  {:?}", &response);
        eprintln!("  {:?}", response.text());

        anyhow::bail!("response error")
    }

    let doc: serde_json::Value = response.json()?;

    let artifacts = doc
        .as_object()
        .unwrap()
        .get("artifacts")
        .unwrap()
        .as_array()
        .unwrap();

    let artifact_ids: Vec<u64> = artifacts
        .iter()
        .map(|value| {
            value
                .as_object()
                .unwrap()
                .get("id")
                .unwrap()
                .as_u64()
                .unwrap()
        })
        .collect();

    let download_dir = tempfile::tempdir()?;
    let output_dir = super::package::target_dir()?.join("github-artifacts");

    eprintln!("Output directory {:?}", output_dir);
    std::fs::create_dir_all(&output_dir)?;

    for artifact_id in artifact_ids {
        eprintln!("Downloading artifact {}", artifact_id);
        let mut response = client
            .get(format!(
                "https://api.github.com/repos/{}/{}/actions/artifacts/{}/zip",
                REPO_USER, REPO_NAME, artifact_id
            ))
            .send()?;

        eprintln!(" .. {}", response.status());
        response.error_for_status_ref()?;

        let artifact_path = download_dir.path().join(format!("{}.zip", artifact_id));
        let mut file = File::options()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&artifact_path)?;
        std::io::copy(&mut response, &mut file)?;

        eprintln!("Extracting {:?}", &artifact_path);
        let file = File::open(&artifact_path)?;
        let mut zip = ZipArchive::new(file)?;
        zip.extract(&output_dir)?;
    }

    download_dir.close()?;
    eprintln!("Done");
    Ok(())
}
