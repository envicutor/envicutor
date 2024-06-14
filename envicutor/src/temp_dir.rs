use anyhow::{anyhow, Error};

pub struct TempDir {
    pub path: String,
}

impl TempDir {
    pub async fn new(path: String) -> Result<TempDir, Error> {
        crate::fs::create_dir_replacing_existing(&path)
            .await
            .map_err(|e| anyhow!("Failed to create directory {path}\nError: {e}"))?;
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
