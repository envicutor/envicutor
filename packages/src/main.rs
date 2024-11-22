use std::{
    collections::{HashMap, HashSet},
    fs::{self, Permissions},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::Command,
};

use serde::Deserialize;

// Installs the packages and prepares their data

#[derive(Deserialize)]
struct Runtime {
    id: u16,
    name: String,
    version: String,
    nix_shell: String,
    compile_script: Option<String>,
    run_script: String,
    source_file_name: String,
}

const PACKAGES_DIR: &str = "/app/packages";
const RUNTIMES_DIR: &str = "/envicutor/runtimes";
const NIX_BIN_PATH: &str = "/root/.nix-profile/bin";

fn main() {
    let mut ids: HashSet<u16> = HashSet::new();
    let mut names_to_versions: HashMap<String, Vec<String>> = HashMap::new();
    let entries = fs::read_dir(PACKAGES_DIR).unwrap_or_else(|e| {
        panic!("Couldn't read the packages directory: {e}");
    });
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| {
            panic!("Failed to read entry: {e}");
        });
        let file_path = entry.path();
        if file_path.is_file() {
            if let Some(ext) = file_path.extension() {
                if ext == "toml" {
                    eprintln!("Processing: {:?}", file_path);

                    // Parse
                    let content = fs::read_to_string(file_path).unwrap_or_else(|e| {
                        panic!("Failed to read file content: {e}");
                    });
                    let runtime: Runtime = toml::from_str(&content).unwrap_or_else(|e| {
                        panic!("Failed to parse runtime: {e}");
                    });

                    // Check if same id exists
                    if ids.contains(&runtime.id) {
                        panic!("Found duplicate id: {}", runtime.id);
                    }
                    // Check if same name + version exists
                    if let Some(entry) = names_to_versions.get_mut(&runtime.name) {
                        if entry.contains(&runtime.version) {
                            panic!(
                                "Found duplicate runtime: {}-{}",
                                runtime.name, runtime.version
                            );
                        }
                        entry.push(runtime.version);
                    } else {
                        names_to_versions
                            .insert(runtime.name.clone(), vec![runtime.version.clone()]);
                    }
                    ids.insert(runtime.id);

                    let workdir = format!("{}/{}", RUNTIMES_DIR, runtime.id);

                    // Create runtime directory
                    create_dir_replacing_existing(&workdir);
                    let mut trx = Transaction::init(|| {
                        // Delete the directory on failure
                        fs::remove_dir_all(workdir.clone()).unwrap_or_else(|e| {
                            panic!("Failed to remove directory: {workdir}\nError: {e}");
                        });
                    });

                    // Write shell.nix
                    let nix_shell_path = format!("{workdir}/shell.nix");
                    fs::write(&nix_shell_path, &runtime.nix_shell).unwrap_or_else(|e| {
                        panic!("Failed to write shell.nix: {e}");
                    });

                    // Install package, create env file
                    let mut cmd = Command::new("env");
                    cmd.arg("-i")
                        .arg("PATH=/bin")
                        .arg(format!("{NIX_BIN_PATH}/nix-shell"))
                        .arg(nix_shell_path)
                        .args(["--run", &format!("{NIX_BIN_PATH}/bash -c env")]);
                    let cmd_res = cmd.output().unwrap_or_else(|e| {
                        panic!("Failed to run nix-shell: {e}");
                    });
                    let stdout = String::from_utf8_lossy(&cmd_res.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&cmd_res.stderr).to_string();
                    let success = cmd_res.status.success();
                    if !success {
                        panic!("Running nix-shell was not successful\nstdout: {stdout}\nstderr: {stderr}");
                    }
                    fs::write(format!("{workdir}/env"), stdout).unwrap_or_else(|e| {
                        panic!("Failed to write the env file: {e}");
                    });

                    // Write compile_script and run_script
                    if let Some(compile_script) = &runtime.compile_script {
                        if !compile_script.is_empty() {
                            write_file_and_set_permissions(
                                &format!("{workdir}/compile"),
                                compile_script,
                                Permissions::from_mode(0o755),
                            );
                        }
                    }
                    write_file_and_set_permissions(
                        &format!("{workdir}/run"),
                        &runtime.run_script,
                        Permissions::from_mode(0o755),
                    );

                    // TODO: write metadata

                    trx.commit();
                }
            }
        }
    }
}

pub fn write_file_and_set_permissions(path: &str, content: &String, perms: Permissions) {
    fs::write(path, content).unwrap_or_else(|e| panic!("Failed to write to {path}\nError: {e}"));
    fs::set_permissions(path, perms)
        .unwrap_or_else(|e| panic!("Failed to write permissions on {path}\nError: {e}"));
}

struct Transaction<T>
where
    T: Fn(),
{
    committed: bool,
    rollback_fn: T,
}

impl<T> Transaction<T>
where
    T: Fn(),
{
    fn init(rollback_fn: T) -> Transaction<T> {
        Self {
            committed: false,
            rollback_fn,
        }
    }
    fn commit(&mut self) {
        self.committed = true;
    }
}

impl<T> Drop for Transaction<T>
where
    T: Fn(),
{
    fn drop(&mut self) {
        (self.rollback_fn)();
    }
}

pub fn create_dir_replacing_existing(path: &String) {
    let path = Path::new(path);
    if path.exists() && path.is_dir() {
        eprintln!("Found an existing directory at: {:?}, replacing it", path);
        fs::remove_dir_all(path)
            .unwrap_or_else(|e| panic!("Failed to remove directory at: {:?}\nError: {e}", path));
    }
    fs::create_dir(path).unwrap_or_else(|e| {
        panic!("Failed to create: {:?}\nError: {e}", path);
    });
}
