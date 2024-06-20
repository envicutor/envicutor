use std::env;
use std::fs;
use std::io;
use std::process::exit;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <number>", args[0]);
        exit(1);
    }

    let input: u64 = args[1].parse().expect("Invalid box id");
    let dir = format!("/tmp/{input}-submission");

    if fs::metadata(&dir).is_ok() {
        match fs::remove_dir_all(&dir) {
            Ok(()) => println!("Directory {} removed successfully.", dir),
            Err(err) => {
                eprintln!("Error removing directory {}: {}", dir, err);
                exit(1);
            }
        }
    } else {
        eprintln!("Error: Directory {dir} does not exist.");
        exit(1);
    }

    Ok(())
}
