use std::io::Error;

use tokio::process::Command;

struct StageConstraints {
    time: u32,
    memory: u32,
    no_processes: u32,
    output_size: u32,
    error_size: u32,
    file_size: u32,
    networking: bool,
    no_files: u32,
}

async fn run_this_stage(
    stage: &str,
    main_program: &str,
    args: &[&str],
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
    cmd.arg("--")
        .arg("/bin/bash")
        .arg("-c");
    return Ok(true);
}

fn main() {
    println!("Hello, world!");
}
