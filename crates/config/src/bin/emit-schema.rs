//! Writes the settings schema to stdout, or checks a committed copy is current.
//!
//! The committed schema is what a client builds its settings screen from. CI runs this
//! with `--check` so the file cannot silently fall behind the descriptor tables.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let current = config::schema_json();

    match args.first().map(String::as_str) {
        Some("--check") => {
            let path = args.get(1).map(String::as_str).unwrap_or(DEFAULT_PATH);
            let committed = match std::fs::read_to_string(path) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("cannot read {path}: {e}");
                    eprintln!("regenerate it with: cargo run -p config --bin emit-schema > {path}");
                    return ExitCode::FAILURE;
                }
            };
            if committed == current {
                println!("{path} is up to date");
                ExitCode::SUCCESS
            } else {
                eprintln!("{path} is out of date with the descriptor tables.");
                eprintln!("regenerate it with: cargo run -p config --bin emit-schema > {path}");
                ExitCode::FAILURE
            }
        }
        Some(other) => {
            eprintln!("unknown argument `{other}`; expected --check [path]");
            ExitCode::FAILURE
        }
        None => {
            print!("{current}");
            ExitCode::SUCCESS
        }
    }
}

const DEFAULT_PATH: &str = "config-schema.json";
