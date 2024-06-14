use anyhow::{anyhow, Error};
use tokio::fs;

pub struct TempDir {
    pub path: String,
}

impl TempDir {
    pub async fn new(path: String) -> Result<TempDir, Error> {
        if fs::try_exists(&path)
            .await
            .map_err(|e| anyhow!("Failed to check if {path} exists\nError: {e}"))?
        {
            fs::remove_dir_all(&path)
                .await
                .map_err(|e| anyhow!("Failed to remove: {path}\nError: {e}"))?;
        }
        fs::create_dir(&path)
            .await
            .map_err(|e| anyhow!("Failed to create: {path}\nError: {e}"))?;

        Ok(TempDir { path })
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let path = self.path.clone();
        tokio::spawn(async move {
            let res = tokio::fs::remove_dir_all(&path).await;
            if let Err(e) = res {
                eprintln!("Failed to remove {path}\nError: {e}");
            }
        });
    }
}
