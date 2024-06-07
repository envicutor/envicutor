use core::fmt;

use tokio::process::Command;

/*
- Should the temp directory be created in that struct?
    - There shouldn't be a temp directory in the run stage
    - Just have another struct called TempDir or something that creates a temporary directory with a random name
Isolate {
    box_id
}

static init() {
    isolate --init --cg -b{box_id}
}

run(command, limits, mounts) {

}

drop() {
    isolate --cleanup --cg -b{box_id}
}
*/
pub struct Isolate {
    box_id: u32,
}

pub struct IsolateError {
    message: String,
}

impl fmt::Display for IsolateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Isolate {
    pub async fn init(box_id: u32) -> Result<Isolate, IsolateError> {
        let res = Command::new("isolate")
            .args(["--init", "--cg", &format!("-b{}", box_id)])
            .output()
            .await
            .map_err(|e| IsolateError {
                message: format!("Failed to run `isolate --init`\nError: {e}"),
            })?;
        if !res.status.success() {
            return Err(IsolateError {
                message: format!(
                    "`isolate --init` failed with\nstderr: {}\nstdout: {}",
                    String::from_utf8_lossy(&res.stderr),
                    String::from_utf8_lossy(&res.stdout)
                ),
            });
        }
        Ok(Isolate { box_id })
    }
}

impl Drop for Isolate {
    fn drop(&mut self) {
        let box_id = self.box_id;
        tokio::spawn(async move {
            let res = Command::new("isolate")
                .args(["--init", "--cg", &format!("-b{}", box_id)])
                .output()
                .await;
            match res {
                Ok(res) => {
                    if !res.status.success() {
                        eprintln!(
                            "`isolate --init` failed with\nstderr: {}\nstdout: {}",
                            String::from_utf8_lossy(&res.stderr),
                            String::from_utf8_lossy(&res.stdout)
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Failed to run `isolate --cleanup`\nError: {e}");
                }
            }
        });
    }
}
