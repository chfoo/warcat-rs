use std::{io::Cursor, path::Path};

use data_encoding::HEXLOWER;
use digest::Digest;
use minisign::SecretKey;

pub fn compute_digests(minisign_secret_key: Option<&Path>) -> anyhow::Result<()> {
    let minisign_secret_key = if let Some(path) = minisign_secret_key {
        Some(get_minisign_secret_key(path)?)
    } else {
        None
    };

    let package_dir = crate::package::target_dir()?.join("github-artifacts");

    let mut entries: Vec<_> = package_dir.read_dir()?.collect();
    entries.sort_unstable_by_key(|item| item.as_ref().unwrap().file_name());

    let mut doc = toml_edit::DocumentMut::new();
    let mut file_table = toml_edit::Table::new();

    for entry in entries {
        let entry = entry.unwrap();
        let filename = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("non-utf-8 path"))?;

        let data = std::fs::read(entry.path())?;

        let mut sha256_hasher = sha2::Sha256::new();
        let mut sha512_hasher = sha2::Sha512::new();
        let mut blake2b_hasher = blake2::Blake2b512::new();
        let mut blake3_hasher = blake3::Hasher::new();

        sha256_hasher.update(&data);
        sha512_hasher.update(&data);
        blake2b_hasher.update(&data);
        blake3_hasher.update(&data);

        let sha256_digest = sha256_hasher.finalize();
        let sha512_digest = sha512_hasher.finalize();
        let blake2b_digest = blake2b_hasher.finalize();
        let blake3_digest = blake3_hasher.finalize();

        let mut values = toml_edit::Table::new();
        values.insert("sha256", HEXLOWER.encode(&sha256_digest).into());
        values.insert("sha512", HEXLOWER.encode(&sha512_digest).into());
        values.insert("blake2b", HEXLOWER.encode(&blake2b_digest).into());
        values.insert("blake3", HEXLOWER.encode(blake3_digest.as_slice()).into());

        if let Some(key) = &minisign_secret_key {
            let signature = minisign::sign(None, key, Cursor::new(&data), None, None)?;
            values.insert("minisign", signature.to_string().into());
        }

        file_table.insert(&filename, toml_edit::Item::Table(values));
    }

    doc.insert("files", toml_edit::Item::Table(file_table));
    let text = doc.to_string();
    println!("{}", text);

    Ok(())
}

fn get_minisign_secret_key(path: &Path) -> anyhow::Result<SecretKey> {
    let password = rpassword::prompt_password("Secret key password: ")?;
    eprintln!("Loading key...");
    let key = minisign::SecretKey::from_file(path, Some(password))?;
    eprintln!("OK");
    Ok(key)
}
