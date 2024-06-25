use std::{fs::Permissions, path::Path};

use anyhow::{anyhow, Error};
use tokio::fs;

pub async fn create_dir_replacing_existing(path: &String) -> Result<(), Error> {
    if fs::try_exists(&path)
        .await
        .map_err(|e| anyhow!("Failed to check if {path} exists\nError: {e}"))?
    {
        eprintln!("Found an existing directory at: {path}, replacing it");
        fs::remove_dir_all(&path)
            .await
            .map_err(|e| anyhow!("Failed to remove: {path}\nError: {e}"))?;
    }
    fs::create_dir(&path)
        .await
        .map_err(|e| anyhow!("Failed to create: {path}\nError: {e}"))?;
    Ok(())
}

pub async fn write_file_and_set_permissions(
    path: &String,
    content: &String,
    perms: Permissions,
) -> Result<(), Error> {
    fs::write(path, content)
        .await
        .map_err(|e| anyhow!("Failed to write to {path}\nError: {e}"))?;
    fs::set_permissions(path, perms)
        .await
        .map_err(|e| anyhow!("Failed to write permissions on {path}\nError: {e}"))?;
    Ok(())
}

pub async fn copy_dir_all(src: impl AsRef<Path>, dest: impl AsRef<Path>) -> Result<(), Error> {
    fs::create_dir_all(&dest)
        .await
        .map_err(|e| anyhow!("Failed to create destination directory {e}"))?;
    let mut entries = fs::read_dir(src)
        .await
        .map_err(|e| anyhow!("Failed to read {e}"))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| anyhow!("Failed to get the next directory entry: {e}"))?
    {
        let ty = entry
            .file_type()
            .await
            .map_err(|e| anyhow!("Failed to get the filetype of an entry: {e}"))?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dest.as_ref().join(entry.file_name()))
                .await
                .map_err(|e| anyhow!("Failed to recursively copy directory: {e}"))?;
        } else {
            fs::copy(entry.path(), dest.as_ref().join(entry.file_name()))
                .await
                .map_err(|e| anyhow!("Failed to copy file to directory: {e}"))?;
        }
    }
    Ok(())
}
