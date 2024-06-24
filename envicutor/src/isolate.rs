use std::{process::Stdio, time::Duration};

use anyhow::{anyhow, Error};
use tokio::{fs, io::AsyncWriteExt, process::Command, task::yield_now, time};

use crate::{
    limits::MandatoryLimits,
    types::{Kilobytes, Seconds},
};

pub struct Isolate {
    box_id: u64,
    metadata_file_path: String,
    run_pid: Option<u32>,
    pub box_dir: String,
}

#[derive(serde::Serialize)]
pub struct StageResult {
    pub memory: Option<Kilobytes>,
    pub exit_code: Option<u32>,
    pub exit_signal: Option<u32>,
    pub exit_message: Option<String>,
    pub exit_status: Option<String>,
    pub stdout: String,
    pub stderr: String,
    pub cpu_time: Option<Seconds>,
    pub wall_time: Option<Seconds>,
}

const ISOLATE_PATH: &str = "/usr/local/bin/isolate";

fn split_metadata_line(line: &str) -> (Result<&str, ()>, Result<&str, ()>) {
    let mut entry: Vec<&str> = line.split(':').collect();
    let value = match entry.pop() {
        Some(e) => Ok(e),
        None => Err(()),
    };
    let key = match entry.pop() {
        Some(e) => Ok(e),
        None => Err(()),
    };

    (key, value)
}

async fn add_env_vars_from_file(cmd: &mut Command, file_path: &str) -> Result<(), Error> {
    let env = fs::read_to_string(file_path)
        .await
        .map_err(|e| anyhow!("Failed to read environment variables from: {file_path}: {e}"))?;
    let lines = env.lines();

    let mut line_count = 0;
    let mut key = String::new();
    let mut value = String::new();
    for line in lines {
        if line.contains('=') {
            if !key.is_empty() {
                cmd.env(&key, &value);
            }
            let mut entry: Vec<&str> = line.split('=').collect();
            value = match entry.pop() {
                Some(e) => e.to_string(),
                None => {
                    return Err(anyhow!("Found a bad line in the env file: {file_path}"));
                }
            };
            key = match entry.pop() {
                Some(e) => e.to_string(),
                None => {
                    return Err(anyhow!("Found a bad line in the env file: {file_path}"));
                }
            };
        } else {
            value.push('\n');
            value.push_str(line);
        }
        line_count += 1;
        if line_count % 500 == 0 {
            yield_now().await;
        }
    }
    cmd.env(&key, &value);
    Ok(())
}

impl Isolate {
    pub async fn init(box_id: u64) -> Result<Isolate, Error> {
        let res = Command::new(ISOLATE_PATH)
            .args(["--init", "--cg", &format!("-b{}", box_id)])
            .output()
            .await
            .map_err(|e| anyhow!("Failed to get `isolate --init` output\nError: {e}"))?;
        if !res.status.success() {
            return Err(anyhow!(
                "`isolate --init` failed with\nstderr: {}\nstdout: {}",
                String::from_utf8_lossy(&res.stderr),
                String::from_utf8_lossy(&res.stdout),
            ));
        }
        Ok(Isolate {
            box_id,
            metadata_file_path: format!("/tmp/{box_id}-metadata.txt"),
            run_pid: None,
            box_dir: format!("{}/box", String::from_utf8_lossy(&res.stdout).trim()),
        })
    }

    pub async fn run(
        &mut self,
        mounts: &[&str],
        limits: &MandatoryLimits,
        stdin: Option<&str>,
        workdir: &str,
        env_file: &str,
        cmd_args: &[&str],
    ) -> Result<StageResult, Error> {
        let mut cmd = Command::new(ISOLATE_PATH);
        cmd.arg("--run")
            .arg(&format!("--meta={}", self.metadata_file_path))
            .arg("--cg")
            .arg("-s")
            .args(["-c", workdir])
            .arg("-e")
            .args(["-E", "HOME=/tmp"]);

        for dir in mounts {
            cmd.arg(format!("--dir={}", dir));
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

        add_env_vars_from_file(cmd.env_clear(), env_file).await?;

        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn isolate --run child process: {e}"))?;

        if let Some(pid) = child.id() {
            self.run_pid = Some(pid);
        }
        if let Some(stdin) = stdin {
            if let Some(mut stdin_handle) = child.stdin.take() {
                stdin_handle
                    .write_all(stdin.as_bytes())
                    .await
                    .map_err(|e| anyhow!("Failed to write to child process stdin: {e}"))?;
            }
        }
        let cmd_res = child
            .wait_with_output()
            .await
            .map_err(|e| anyhow!("Failed to get `isolate --run` output\nError: {e}"))?;
        self.run_pid = None;

        let mut memory: Option<Kilobytes> = None;
        let mut exit_code: Option<u32> = None;
        let mut exit_signal: Option<u32> = None;
        let mut exit_message: Option<String> = None;
        let mut exit_status: Option<String> = None;
        let mut cpu_time: Option<Seconds> = None;
        let mut wall_time: Option<Seconds> = None;
        let stdout = String::from_utf8_lossy(&cmd_res.stdout).to_string();
        let stderr = String::from_utf8_lossy(&cmd_res.stderr).to_string();

        let metadata_str = fs::read_to_string(&self.metadata_file_path)
            .await
            .map_err(|e| {
                anyhow!(
                    "Error reading metadata file: {}\nError: {}\nIsolate run stdout: {}\nIsolate run stderr: {}",
                    self.metadata_file_path,
                    e,
                    stdout,
                    stderr
                )
            })?;
        let metadata_lines = metadata_str.lines();
        for line in metadata_lines {
            let (key_res, value_res) = split_metadata_line(line);
            let key =
                key_res.map_err(|_| anyhow!("Failed to parse metadata file, received: {line}"))?;
            let value = value_res
                .map_err(|_| anyhow!("Failed to parse metadata file, received: {line}"))?;
            match key {
                "cg-mem" => {
                    memory = Some(value.parse().map_err(|_| {
                        anyhow!("Failed to parse memory usage, received value: {value}")
                    })?)
                }
                "exitcode" => {
                    exit_code = Some(value.parse().map_err(|_| {
                        anyhow!("Failed to parse exit code, received value: {value}")
                    })?)
                }
                "exitsig" => {
                    exit_signal = Some(value.parse().map_err(|_| {
                        anyhow!("Failed to parse exit signal, received value: {value}")
                    })?)
                }
                "message" => exit_message = Some(value.to_string()),
                "status" => exit_status = Some(value.to_string()),
                "time" => {
                    cpu_time = Some(value.parse().map_err(|_| {
                        anyhow!("Failed to parse cpu time, received value: {value}")
                    })?)
                }
                "time-wall" => {
                    wall_time = Some(value.parse().map_err(|_| {
                        anyhow!("Failed to parse wall time, received value: {value}")
                    })?)
                }
                _ => {}
            }
        }

        if exit_status == Some("XX".to_string()) {
            return Err(anyhow!(
                "Failed to run isolate --run\nstdout: {}\nstderr: {}",
                stdout,
                stderr
            ));
        }
        let result = StageResult {
            cpu_time,
            exit_code,
            exit_message,
            exit_signal,
            exit_status,
            memory,
            stderr,
            stdout,
            wall_time,
        };

        Ok(result)
    }
}

impl Drop for Isolate {
    fn drop(&mut self) {
        let box_id = self.box_id;
        let metadata_file_path = self.metadata_file_path.clone();
        let run_pid_opt = self.run_pid;
        tokio::spawn(async move {
            if let Some(run_pid) = run_pid_opt {
                let kill_res = Command::new("/bin/kill")
                    .arg("-9")
                    .arg(run_pid.to_string())
                    .output()
                    .await;
                if let Err(e) = kill_res {
                    eprintln!(
                        "Could not kill `isolate --run` process. Maybe it has already exited: {e}"
                    );
                }
                time::sleep(Duration::from_millis(50)).await;
            }
            let res = Command::new(ISOLATE_PATH)
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
            let res = fs::remove_file(&metadata_file_path).await;
            if let Err(e) = res {
                eprintln!("Failed to remove: {metadata_file_path}\nError: {e}");
            }
        });
    }
}
