//! Runs, verifies and renders recorded games.

mod render;
mod script;

use engine::{ENGINE_VER, Engine, Handling, MatchConfig};
use replay::Replay;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
usage: replay <command> [options]

  run <file.replay> [--render] [--every N] [--on-lock]
        Re-run a replay. --render draws the board; --every draws every N ticks,
        --on-lock draws once per piece.

  verify <file.replay>
        Re-run a replay and compare against the result it claims.

  compile <file.script> -o <file.replay> [--mode <id>] [--modes-dir <dir>]
        Turn a hand-written input script into a replay.

  checksum <file.replay> [--at N]
        Print the state checksum, optionally partway through.

  modes [--modes-dir <dir>]
        List and validate the available modes.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };

    let result = match cmd.as_str() {
        "run" => cmd_run(&args[1..]),
        "verify" => cmd_verify(&args[1..]),
        "compile" => cmd_compile(&args[1..]),
        "checksum" => cmd_checksum(&args[1..]),
        "modes" => cmd_modes(&args[1..]),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        other => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

// ---- commands --------------------------------------------------------------

fn cmd_run(args: &[String]) -> Result<(), String> {
    let path = positional(args, "a replay file")?;
    let r = load_replay(&path)?;

    let draw_every: Option<u32> = flag_value(args, "--every").map(parse_u32).transpose()?;
    let on_lock = args.iter().any(|a| a == "--on-lock");
    let render_at_end = args.iter().any(|a| a == "--render");

    let mut engine = Engine::new(r.seed, &r.config, &r.handling);
    for (i, b) in r.buttons().iter().enumerate() {
        let out = engine.tick(*b);
        let tick = i as u32 + 1;
        let show = draw_every.is_some_and(|n| n > 0 && tick % n == 0)
            || (on_lock && out.contains(engine::Events::PIECE_LOCKED));
        if show {
            print!("{}", render::render(&engine));
        }
        if engine.is_over() {
            break;
        }
    }
    if render_at_end || (draw_every.is_none() && !on_lock) {
        print!("{}", render::render(&engine));
    }

    let s = engine.stats();
    println!(
        "ticks {}  pieces {}  lines {}  attack {}  {}",
        s.tick,
        s.pieces,
        s.lines,
        s.attack_sent,
        if engine.is_over() {
            "topped out"
        } else {
            "still alive"
        }
    );
    println!("checksum {:#018x}", engine.checksum());
    Ok(())
}

fn cmd_verify(args: &[String]) -> Result<(), String> {
    let path = positional(args, "a replay file")?;
    let r = load_replay(&path)?;

    if !r.is_verifiable() {
        // Deliberately not an error. The recording is still perfectly watchable; it just
        // cannot have its result re-checked under rules it was not played under.
        println!(
            "{}: recorded under rules v{}, this build is v{}. Playable, but the result \
             cannot be re-checked.",
            path.display(),
            r.engine_ver,
            ENGINE_VER
        );
        return Ok(());
    }

    let (_, actual) = r.simulate();
    if actual == r.claimed {
        println!("{}: verified", path.display());
        println!(
            "  ticks {}  pieces {}  lines {}  attack {}",
            actual.final_tick, actual.pieces, actual.lines, actual.attack
        );
        return Ok(());
    }

    let mut report = format!("{}: does NOT match its claimed result\n", path.display());
    diff(
        &mut report,
        "final_tick",
        r.claimed.final_tick,
        actual.final_tick,
    );
    diff(&mut report, "lines", r.claimed.lines, actual.lines);
    diff(&mut report, "pieces", r.claimed.pieces, actual.pieces);
    diff(&mut report, "attack", r.claimed.attack, actual.attack);
    diff(&mut report, "checksum", r.claimed.checksum, actual.checksum);
    if r.claimed.topped_out != actual.topped_out {
        report.push_str(&format!(
            "  topped_out: claimed {}, actual {}\n",
            r.claimed.topped_out, actual.topped_out
        ));
    }
    Err(report)
}

fn diff<T: PartialEq + std::fmt::Display>(out: &mut String, name: &str, claimed: T, actual: T) {
    if claimed != actual {
        out.push_str(&format!("  {name}: claimed {claimed}, actual {actual}\n"));
    }
}

fn cmd_compile(args: &[String]) -> Result<(), String> {
    let path = positional(args, "a script file")?;
    let out_path = flag_value(args, "-o")
        .or_else(|| flag_value(args, "--out"))
        .ok_or("compile needs -o <file.replay>")?;

    let text = read(&path)?;
    let s = script::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;

    let mode_id = flag_value(args, "--mode").or_else(|| s.mode.clone());
    let config = match &mode_id {
        Some(id) => find_mode(args, id)?,
        None => MatchConfig::default(),
    };

    let r = Replay::record(s.seed, &config, &Handling::default(), &s.buttons);
    let json = serde_json::to_string_pretty(&r)
        .map_err(|e| format!("could not encode the replay: {e}"))?;
    std::fs::write(&out_path, json + "\n")
        .map_err(|e| format!("could not write {out_path}: {e}"))?;

    println!(
        "{} -> {} ({} ticks, {} runs, {} pieces, {} lines)",
        path.display(),
        out_path,
        r.tick_count(),
        r.inputs.len(),
        r.claimed.pieces,
        r.claimed.lines
    );
    Ok(())
}

fn cmd_checksum(args: &[String]) -> Result<(), String> {
    let path = positional(args, "a replay file")?;
    let r = load_replay(&path)?;
    let at: Option<u32> = flag_value(args, "--at").map(parse_u32).transpose()?;

    let mut engine = Engine::new(r.seed, &r.config, &r.handling);
    for (i, b) in r.buttons().iter().enumerate() {
        if at.is_some_and(|n| i as u32 >= n) {
            break;
        }
        engine.tick(*b);
    }
    println!("{:#018x}", engine.checksum());
    Ok(())
}

fn cmd_modes(args: &[String]) -> Result<(), String> {
    let dir = modes_dir(args);
    match config::load_modes(&dir) {
        Ok(modes) => {
            println!("{} modes in {}", modes.len(), dir.display());
            for m in modes {
                println!("  {:<12} {:<14} {:?}", m.id, m.name, m.goal);
            }
            Ok(())
        }
        Err(errors) => {
            let mut report = String::new();
            for e in errors {
                report.push_str(&format!("{e}\n"));
            }
            Err(report.trim_end().to_string())
        }
    }
}

// ---- helpers ---------------------------------------------------------------

fn positional(args: &[String], what: &str) -> Result<PathBuf, String> {
    args.iter()
        .find(|a| !a.starts_with('-'))
        .map(PathBuf::from)
        .ok_or_else(|| format!("expected {what}\n\n{USAGE}"))
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn parse_u32(s: String) -> Result<u32, String> {
    s.parse().map_err(|_| format!("`{s}` is not a number"))
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("could not read {}: {e}", path.display()))
}

fn load_replay(path: &Path) -> Result<Replay, String> {
    let text = read(path)?;
    serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a valid replay: {e}", path.display()))
}

fn modes_dir(args: &[String]) -> PathBuf {
    flag_value(args, "--modes-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("modes"))
}

fn find_mode(args: &[String], id: &str) -> Result<MatchConfig, String> {
    let dir = modes_dir(args);
    let modes = config::load_modes(&dir).map_err(|errors| {
        errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    modes
        .into_iter()
        .find(|m| m.id == id)
        .map(|m| m.config)
        .ok_or_else(|| format!("no mode `{id}` in {}", dir.display()))
}
