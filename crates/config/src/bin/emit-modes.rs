//! Writes the game modes to stdout as JSON, or checks a committed copy is current.
//!
//! The client bundles the committed file instead of parsing TOML in the browser. CI runs
//! this with `--check` so a mode added or edited on disk cannot be missing from the game.

use std::path::Path;
use std::process::ExitCode;

const DEFAULT_MODES_DIR: &str = "modes";
const DEFAULT_PATH: &str = "modes.generated.json";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let current = match config::modes_json(Path::new(DEFAULT_MODES_DIR)) {
        Ok(text) => text,
        Err(errors) => {
            for e in errors {
                eprintln!("{e}");
            }
            return ExitCode::FAILURE;
        }
    };

    match args.first().map(String::as_str) {
        Some("--check") => {
            let path = args.get(1).map(String::as_str).unwrap_or(DEFAULT_PATH);
            let committed = match std::fs::read_to_string(path) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("cannot read {path}: {e}");
                    eprintln!("regenerate it with: cargo run -p config --bin emit-modes > {path}");
                    return ExitCode::FAILURE;
                }
            };
            if committed == current {
                println!("{path} is up to date");
                ExitCode::SUCCESS
            } else {
                eprintln!("{path} is out of date with {DEFAULT_MODES_DIR}/.");
                eprintln!("regenerate it with: cargo run -p config --bin emit-modes > {path}");
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
