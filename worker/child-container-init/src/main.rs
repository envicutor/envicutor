use std::{
    io::Error,
    os::unix::process::ExitStatusExt,
    process::{exit, Stdio},
};

use serde::{Deserialize, Serialize};

use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

#[derive(Clone)]
struct StageConstraints {
    time: u32,
    memory: u64,
    no_processes: u32,
    output_size: u64,
    error_size: u64,
    file_size: u32,
    networking: bool,
    no_files: u32,
}

#[derive(Serialize, Deserialize)]
struct StageOutput {
    stage: String,
    stdout: String,
    stderr: String,
    time: u32,
    code: i32,
    signal: String,
}

fn translate_signal(signal: i32) -> String {
    match signal {
        1 => "SIGHUP".to_string(),
        2 => "SIGINT".to_string(),
        3 => "SIGQUIT".to_string(),
        4 => "SIGILL".to_string(),
        5 => "SIGTRAP".to_string(),
        6 => "SIGABRT".to_string(),
        7 => "SIGBUS".to_string(),
        8 => "SIGFPE".to_string(),
        9 => "SIGKILL".to_string(),
        10 => "SIGUSR1".to_string(),
        11 => "SIGSEGV".to_string(),
        12 => "SIGUSR2".to_string(),
        13 => "SIGPIPE".to_string(),
        14 => "SIGALRM".to_string(),
        15 => "SIGTERM".to_string(),
        16 => "SIGSTKFLT".to_string(),
        17 => "SIGCHLD".to_string(),
        18 => "SIGCONT".to_string(),
        19 => "SIGSTOP".to_string(),
        20 => "SIGTSTP".to_string(),
        21 => "SIGTTIN".to_string(),
        22 => "SIGTTOU".to_string(),
        23 => "SIGURG".to_string(),
        24 => "SIGXCPU".to_string(),
        25 => "SIGXFSZ".to_string(),
        26 => "SIGVTALRM".to_string(),
        27 => "SIGPROF".to_string(),
        28 => "SIGWINCH".to_string(),
        29 => "SIGIO".to_string(),
        30 => "SIGPWR".to_string(),
        31 => "SIGSYS".to_string(),
        34 => "SIGRTMIN".to_string(),
        35 => "SIGRTMIN+1".to_string(),
        36 => "SIGRTMIN+2".to_string(),
        37 => "SIGRTMIN+3".to_string(),
        38 => "SIGRTMIN+4".to_string(),
        39 => "SIGRTMIN+5".to_string(),
        40 => "SIGRTMIN+6".to_string(),
        41 => "SIGRTMIN+7".to_string(),
        42 => "SIGRTMIN+8".to_string(),
        43 => "SIGRTMIN+9".to_string(),
        44 => "SIGRTMIN+10".to_string(),
        45 => "SIGRTMIN+11".to_string(),
        46 => "SIGRTMIN+12".to_string(),
        47 => "SIGRTMIN+13".to_string(),
        48 => "SIGRTMIN+14".to_string(),
        49 => "SIGRTMIN+15".to_string(),
        50 => "SIGRTMAX-14".to_string(),
        51 => "SIGRTMAX-13".to_string(),
        52 => "SIGRTMAX-12".to_string(),
        53 => "SIGRTMAX-11".to_string(),
        54 => "SIGRTMAX-10".to_string(),
        55 => "SIGRTMAX-9".to_string(),
        56 => "SIGRTMAX-8".to_string(),
        57 => "SIGRTMAX-7".to_string(),
        58 => "SIGRTMAX-6".to_string(),
        59 => "SIGRTMAX-5".to_string(),
        60 => "SIGRTMAX-4".to_string(),
        61 => "SIGRTMAX-3".to_string(),
        62 => "SIGRTMAX-2".to_string(),
        63 => "SIGRTMAX-1".to_string(),
        64 => "SIGRTMAX".to_string(),
        _ => "Unknown".to_string(),
    }
}

async fn run_this_stage(
    stage: &str,
    main_program: &str,
    args: &[&str],
    stdin: Option<&str>,
    constraints: StageConstraints,
) -> Result<bool, Error> {
    let nix_bin_path_output = Command::new("readlink")
        .arg("-f")
        .arg("/home/envicutor/.nix-profile/bin/")
        .output()
        .await?;
    let mut nix_bin_path = String::from_utf8_lossy(&nix_bin_path_output.stdout).into_owned();
    nix_bin_path = nix_bin_path.trim().to_string();

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
        .arg("/submission")
        .arg("--cwd")
        .arg("/submission")
        .arg("-B")
        .arg("/tmp")
        .arg("-B")
        .arg("/nix")
        .arg("-B")
        .arg("/home/envicutor")
        .arg("-R")
        .arg("/bin")
        .arg("-R")
        .arg("/lib")
        .arg("-R")
        .arg("/usr/lib")
        .arg("-R")
        .arg("/dev/null")
        .arg("-R")
        .arg("/dev/urandom")
        .arg("--user")
        .arg("envicutor:envicutor")
        .arg("--group")
        .arg("envicutor:envicutor")
        .arg("-E")
        .arg("HOME=/home/envicutor")
        .arg("-E")
        .arg("PATH=/bin");

    if constraints.networking {
        cmd.arg("-N").arg("-R").arg("/etc/resolv.conf");
    }

    let mut cp = cmd
        .arg("--")
        .arg(format!("{}/nix-shell", nix_bin_path))
        .arg("shell.nix")
        .arg("--run")
        .arg(main_program)
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
        stage: stage.to_string(),
        stdout,
        stderr,
        time: 0,
        code: exit_status.code().unwrap(),
        signal: translate_signal(exit_status.signal().unwrap_or(-1)).to_string(),
    };

    println!("{}", serde_json::to_string(&stage_output)?);

    Ok(exit_status.code().unwrap() == 0)
}

#[tokio::main]
async fn main() {
    let stage_constraints = StageConstraints {
        time: 10000,
        memory: 100000,
        no_processes: 100,
        output_size: 100000,
        error_size: 100000,
        file_size: 100000,
        networking: true,
        no_files: 100,
    };

    if !run_this_stage(
        "dependencies",
        "sleep 0",
        &[],
        None,
        stage_constraints.clone(),
    )
    .await
    .unwrap()
    {
        exit(1);
    }

    if fs::try_exists("cutor-compile.sh").await.unwrap()
        && !run_this_stage(
            "compile",
            "./cutor-compile.sh",
            &[],
            None,
            stage_constraints.clone(),
        )
        .await
        .unwrap()
    {
        exit(1);
    }

    if !run_this_stage(
        "run",
        "./cutor-run.sh",
        &[],
        Some("hello world"),
        stage_constraints.clone(),
    )
    .await
    .unwrap()
    {
        exit(1);
    }
}
