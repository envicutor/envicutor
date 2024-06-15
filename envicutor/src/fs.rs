use std::fs::Permissions;

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
