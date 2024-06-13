use core::fmt;

use tokio::process::Command;

use crate::limits::MandatoryLimits;

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
    - Return cmd res, metadata
}

drop() {
    isolate --cleanup --cg -b{box_id}
}

Mount {
}
*/

pub struct Mount {
    dir: String,
}

pub struct Isolate {
    box_id: u32,
    box_dir: String,
    metadata_file_path: String,
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

        let box_dir = String::from_utf8_lossy(&res.stdout).trim().to_string();
        let metadata_file_path = format!("{box_dir}/metadata.txt");
        Ok(Isolate {
            box_id,
            box_dir: String::from_utf8_lossy(&res.stdout).trim().to_string(),
            metadata_file_path,
        })
    }

    /*
    run
    - isolate --run
    */
    pub async fn run(&self, mounts: &[Mount], limits: &MandatoryLimits, cmd_args: &[String]) {
        let cmd = Command::new("isolate")
            .arg("--run")
            .arg(&format!("--meta={}", self.metadata_file_path))
            .arg("--cg");

        for mount in mounts {
            cmd.arg(format!("--dir={}", mount.dir));
        }

        cmd.arg(format!("--cg-mem={}", limits.memory))
            .arg(format!("--wall-time={}", limits.wall_time))
            .arg(format!("--time={}", limits.cpu_time))
            .arg(format!("--extra-time={}", limits.extra_time))
            .arg(format!("--open-files={}", limits.max_open_files))
            .arg(format!("--fsize={}", limits.max_file_size))
            .arg(format!("--processes={}", limits.max_number_of_processes))
            .arg(format!("-b{}", self.box_id))
            .arg("--")
            .args(cmd_args);

        let cmd_res = cmd
            .output()
            .await
            .map_err(|e| {
                eprintln!("Failed to run isolate command: {e}");
                INTERNAL_SERVER_ERROR_RESPONSE.into_response()
            })?;
    }
}

impl Drop for Isolate {
    fn drop(&mut self) {
        let box_id = self.box_id;
        tokio::spawn(async move {
            let res = Command::new("isolate")
                .args(["--cleanup", "--cg", &format!("-b{}", box_id)])
                .output()
                .await;
            match res {
                Ok(res) => {
                    if !res.status.success() {
                        eprintln!(
                            "`isolate --cleanup` failed with\nstderr: {}\nstdout: {}",
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
