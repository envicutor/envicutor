use std::{env, io::Error, process::Stdio};

use tokio::{io::{AsyncWriteExt, AsyncReadExt, BufReader}, process::Command};

struct StageConstraints {
    time: u32,
    memory: u32,
    no_processes: u32,
    output_size: u64,
    error_size: u64,
    file_size: u32,
    networking: bool,
    no_files: u32,
}

struct StageResult {
    stage: String,
    r#type: String,
    value: String,
}

async fn run_this_stage(
    stage: &str,
    main_program: &str,
    args: &[&str],
    stdin: Option<&str>,
    constraints: StageConstraints,
) -> Result<bool, Error> {
    let mut success = true;
    let nix_bin_path_output = Command::new("readlink")
        .arg("-f")
        .arg("/root/.nix-profile/bin/")
        .output()
        .await?;
    let nix_bin_path = String::from_utf8_lossy(&nix_bin_path_output.stdout).into_owned();
    let mut cmd = Command::new("nsjail");
    cmd.arg("-t")
        .arg(constraints.time.to_string())
        .arg("--use_cgroupv2")
        .arg("--cgroup_mem_max")
        .arg((constraints.memory * 1000 * 1000).to_string()) // to bytes
        .arg("--cgroup_pids_max")
        .arg(constraints.no_processes.to_string())
        .arg("--cgroup_mem_swap_max")
        .arg("0")
        .arg("--rlimit_nofile")
        .arg(constraints.no_files.to_string())
        .arg("--rlimit_fsize")
        .arg(constraints.file_size.to_string()); // to mb
    if constraints.networking {
        cmd.arg("-N").arg("-R").arg("/etc/resolv.conf");
    }
    let mut cp = cmd
        .arg("--")
        .arg("/bin/bash")
        .arg("-c")
        .arg(format!(
            "export PATH=/bin:$PATH && mkdir /tmp/home && {}/nix-shell shell.nix --run {}",
            nix_bin_path, main_program
        ))
        .arg("envicutor")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped())
        .spawn()?;
    if let Some(s) = stdin {
        let mut handle = cp.stdin.take().unwrap();
        handle.write_all(s.as_bytes()).await?;
    }

    let stdout_reader = BufReader::new(cp.stdout.take().unwrap());
    let stderr_reader = BufReader::new(cp.stderr.take().unwrap());

    cp.wait().await?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    stdout_reader
        .take(constraints.output_size)
        .read_to_end(&mut stdout)
        .await?;
    stderr_reader
        .take(constraints.error_size)
        .read_to_end(&mut stderr)
        .await?;
    let stdout = String::from_utf8_lossy(&stdout).into_owned();
    let stderr = String::from_utf8_lossy(&stderr).into_owned();

    return Ok(true);
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let r: Vec<u32> = serde_json::from_str(&args[1]).unwrap();
}
