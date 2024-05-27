// Some concepts here are inspired by SandKasten: https://github.com/Defelo/sandkasten

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

type Second = u32;
type Mb = u64;

#[derive(Clone)]
struct StageConstraints {
    time: Second,
    memory: Mb,
    no_processes: u32,
    output_size: u64,
    error_size: u64,
    file_size: Mb,
    networking: bool,
    no_files: u32,
}

#[derive(Serialize, Deserialize)]
struct StageOutput<'a> {
    stage: &'a str,
    stdout: &'a str,
    stderr: &'a str,
    time: u32,
    code: i32,
    signal: &'static str,
}

fn translate_signal(signal: i32) -> &'static str {
    match signal {
        1 => "SIGHUP",
        2 => "SIGINT",
        3 => "SIGQUIT",
        4 => "SIGILL",
        5 => "SIGTRAP",
        6 => "SIGABRT",
        7 => "SIGBUS",
        8 => "SIGFPE",
        9 => "SIGKILL",
        10 => "SIGUSR1",
        11 => "SIGSEGV",
        12 => "SIGUSR2",
        13 => "SIGPIPE",
        14 => "SIGALRM",
        15 => "SIGTERM",
        16 => "SIGSTKFLT",
        17 => "SIGCHLD",
        18 => "SIGCONT",
        19 => "SIGSTOP",
        20 => "SIGTSTP",
        21 => "SIGTTIN",
        22 => "SIGTTOU",
        23 => "SIGURG",
        24 => "SIGXCPU",
        25 => "SIGXFSZ",
        26 => "SIGVTALRM",
        27 => "SIGPROF",
        28 => "SIGWINCH",
        29 => "SIGIO",
        30 => "SIGPWR",
        31 => "SIGSYS",
        34 => "SIGRTMIN",
        35 => "SIGRTMIN+1",
        36 => "SIGRTMIN+2",
        37 => "SIGRTMIN+3",
        38 => "SIGRTMIN+4",
        39 => "SIGRTMIN+5",
        40 => "SIGRTMIN+6",
        41 => "SIGRTMIN+7",
        42 => "SIGRTMIN+8",
        43 => "SIGRTMIN+9",
        44 => "SIGRTMIN+10",
        45 => "SIGRTMIN+11",
        46 => "SIGRTMIN+12",
        47 => "SIGRTMIN+13",
        48 => "SIGRTMIN+14",
        49 => "SIGRTMIN+15",
        50 => "SIGRTMAX-14",
        51 => "SIGRTMAX-13",
        52 => "SIGRTMAX-12",
        53 => "SIGRTMAX-11",
        54 => "SIGRTMAX-10",
        55 => "SIGRTMAX-9",
        56 => "SIGRTMAX-8",
        57 => "SIGRTMAX-7",
        58 => "SIGRTMAX-6",
        59 => "SIGRTMAX-5",
        60 => "SIGRTMAX-4",
        61 => "SIGRTMAX-3",
        62 => "SIGRTMAX-2",
        63 => "SIGRTMAX-1",
        64 => "SIGRTMAX",
        _ => "Unknown",
    }
}

const DEPENDENCIES_STAGE: &str = "dependencies";
const COMPILE_STAGE: &str = "compile";
const RUN_STAGE: &str = "run";

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
        cmd.arg("-N")
            .arg("-R")
            .arg("/etc/resolv.conf")
            .arg("-R")
            .arg("/etc/ssl");
    }

    cmd.arg("--").arg(format!("{}/nix-shell", nix_bin_path));

    if stage != DEPENDENCIES_STAGE {
        cmd.arg("--no-substitute");
    }

    let mut cp = cmd
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
        stage,
        stdout: stdout.as_str(),
        stderr: stderr.as_str(),
        time: 0,
        code: exit_status.code().unwrap(),
        signal: translate_signal(exit_status.signal().unwrap_or(-1)),
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
        DEPENDENCIES_STAGE,
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
            COMPILE_STAGE,
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
        RUN_STAGE,
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
