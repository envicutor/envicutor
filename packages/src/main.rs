use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
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
                    let content = fs::read_to_string(file_path).unwrap_or_else(|e| {
                        panic!("Failed to read file content: {e}");
                    });
                    let runtime: Runtime = toml::from_str(&content).unwrap_or_else(|e| {
                        panic!("Failed to parse runtime: {e}");
                    });
                    if ids.contains(&runtime.id) {
                        panic!("Found duplicate id: {}", runtime.id);
                    }
                    if let Some(entry) = names_to_versions.get_mut(&runtime.name) {
                        if entry.contains(&runtime.version) {
                            panic!(
                                "Found duplicate runtime: {}-{}",
                                runtime.name, runtime.version
                            );
                        }
                        entry.push(runtime.version);
                    } else {
                        let mut vec = Vec::new();
                        vec.push(runtime.version);
                        names_to_versions.insert(runtime.name.clone(), vec);
                    }
                    ids.insert(runtime.id);

                    let workdir = format!("{}/{}", RUNTIMES_DIR, runtime.id);
                    create_dir_replacing_existing(&workdir);
                    let mut trx = Transaction::init(|| {
                        fs::remove_dir_all(workdir.clone()).unwrap_or_else(|e| {
                            panic!("Failed to remove directory: {workdir}\nError: {e}");
                        });
                    });
                    fs::write(format!("{}/shell.nix", workdir), runtime.nix_shell).unwrap_or_else(
                        |e| {
                            panic!("Failed to write shell.nix: {e}");
                        },
                    );

                    trx.commit();
                }
            }
        }
    }
}

struct Transaction<T>
where
    T: Fn() -> (),
{
    committed: bool,
    rollback_fn: T,
}

impl<T> Transaction<T>
where
    T: Fn() -> (),
{
    fn init(rollback_fn: T) -> Transaction<T> {
        return Self {
            committed: false,
            rollback_fn,
        };
    }
    fn commit(&mut self) {
        self.committed = true;
    }
}

impl<T> Drop for Transaction<T>
where
    T: Fn() -> (),
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
    fs::create_dir(&path).unwrap_or_else(|e| {
        panic!("Failed to create: {:?}\nError: {e}", path);
    });
}
