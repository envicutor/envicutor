use std::{env, io::Error, os::unix::process::ExitStatusExt, process::Stdio};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

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

const signals: [&str; 64] = [
    "SIGHUP",
    "SIGINT",
    "SIGQUIT",
    "SIGILL",
    "SIGTRAP",
    "SIGABRT",
    "SIGBUS",
    "SIGFPE",
    "SIGKILL",
    "SIGUSR1",
    "SIGSEGV",
    "SIGUSR2",
    "SIGPIPE",
    "SIGALRM",
    "SIGTERM",
    "SIGSTKFLT",
    "SIGCHLD",
    "SIGCONT",
    "SIGSTOP",
    "SIGTSTP",
    "SIGTTIN",
    "SIGTTOU",
    "SIGURG",
    "SIGXCPU",
    "SIGXFSZ",
    "SIGVTALRM",
    "SIGPROF",
    "SIGWINCH",
    "SIGIO",
    "SIGPWR",
    "",
    "",
    "SIGSYS",
    "SIGRTMIN",
    "SIGRTMIN+1",
    "SIGRTMIN+2",
    "SIGRTMIN+3",
    "SIGRTMIN+4",
    "SIGRTMIN+5",
    "SIGRTMIN+6",
    "SIGRTMIN+7",
    "SIGRTMIN+8",
    "SIGRTMIN+9",
    "SIGRTMIN+10",
    "SIGRTMIN+11",
    "SIGRTMIN+12",
    "SIGRTMIN+13",
    "SIGRTMIN+14",
    "SIGRTMIN+15",
    "SIGRTMAX-14",
    "SIGRTMAX-13",
    "SIGRTMAX-12",
    "SIGRTMAX-11",
    "SIGRTMAX-10",
    "SIGRTMAX-9",
    "SIGRTMAX-8",
    "SIGRTMAX-7",
    "SIGRTMAX-6",
    "SIGRTMAX-5",
    "SIGRTMAX-4",
    "SIGRTMAX-3",
    "SIGRTMAX-2",
    "SIGRTMAX-1",
    "SIGRTMAX",
];
struct StageOutput {
    stdout: String,

    stderr: String,

    time: u32,
    code: i32,
    signal: String,
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
        .arg(constraints.file_size.to_string()) // to mb
        .arg("-B")
        .arg("/app")
        .arg("--cwd")
        .arg("/app")
        .arg("-B")
        .arg("/tmp")
        .arg("-R")
        .arg("/nix")
        .arg("-R")
        .arg("/bin")
        .arg("-R")
        .arg("/lib:/lib");
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

    let exit_status = cp.wait().await?;

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

    let stage_output = StageOutput {
        stdout,
        stderr,
        time: 0,
        code: exit_status.code().unwrap(),
        signal: signals[exit_status.signal().unwrap() as usize].to_string(),
    };

    return Ok(true);
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let r: Vec<u32> = serde_json::from_str(&args[1]).unwrap();
}
