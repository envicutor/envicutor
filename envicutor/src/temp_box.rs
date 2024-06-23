use anyhow::{anyhow, Error};
use tokio::{fs, process::Command};

pub struct TempBox {
    box_id: u64,
    pub path: String,
}

async fn remove_dir(box_id: u64) -> Result<(), Error> {
    let mut cmd = Command::new("/envicutor/delete_submission");
    cmd.arg(box_id.to_string());
    match cmd.output().await {
        Ok(res) => {
            if !res.status.success() {
                return Err(anyhow!(
                    "Failed to remove temp box of id: {}\nstdout: {}\nstderr: {}",
                    box_id,
                    String::from_utf8_lossy(&res.stdout),
                    String::from_utf8_lossy(&res.stderr)
                ));
            }
        }
        Err(e) => {
            return Err(anyhow!(
                "Failed to remove temp box of id: {box_id}\nError: {e}"
            ));
        }
    }
    Ok(())
}

impl TempBox {
    pub async fn new(box_id: u64) -> Result<TempBox, Error> {
        let path = format!("/tmp/{box_id}-submission");
        if fs::try_exists(&path)
            .await
            .map_err(|e| anyhow!("Failed to check if {path} exists\nError: {e}"))?
        {
            eprintln!("Found an existing directory at: {path}, replacing it");
            remove_dir(box_id).await?;
        }
        fs::create_dir(&path)
            .await
            .map_err(|e| anyhow!("Failed to create: {path}\nError: {e}"))?;
        Ok(TempBox { box_id, path })
    }
}

impl Drop for TempBox {
    fn drop(&mut self) {
        let box_id = self.box_id;
        tokio::spawn(async move {
            let res = remove_dir(box_id).await;
            if let Err(e) = res {
                eprintln!("{e}");
            }
        });
    }
}
