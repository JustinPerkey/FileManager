//! `filemanager-shoot --out <dir> [--only <name>,...] [--prefix <p>] [--dark]`
//!
//! Renders the GPUI app's scenarios to PNG (see `fm_app::shoot`). Needs the
//! `shoot` feature and, to capture, Windows at 100% display scale. Never part
//! of the release exe.
//!
//! The binary runs each scenario in its own child process (itself, with the
//! hidden `--child <name>` flag), so scenarios share no state and GPUI's
//! one-run-per-process limit does not matter. It prints one line per scenario
//! (`ok <name>` or `FAIL <name>: <reason>`) and exits non-zero if any failed.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

use fm_app::shoot::{run_scenario, scenarios};

struct Args {
    out: PathBuf,
    only: Option<Vec<String>>,
    prefix: String,
    dark: bool,
    child: Option<String>,
}

const USAGE: &str =
    "usage: filemanager-shoot --out <dir> [--only <name>,...] [--prefix <p>] [--dark]";

fn parse(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args {
        out: PathBuf::new(),
        only: None,
        prefix: String::new(),
        dark: false,
        child: None,
    };
    let mut out = None;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--only" => {
                parsed.only = Some(value("--only")?.split(',').map(str::to_owned).collect());
            }
            "--prefix" => parsed.prefix = value("--prefix")?,
            "--dark" => parsed.dark = true,
            "--child" => parsed.child = Some(value("--child")?),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    parsed.out = out.ok_or("--out is required")?;
    Ok(parsed)
}

fn main() -> ExitCode {
    let args = match parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Some(name) = &args.child {
        return child(name, &args);
    }
    let all = scenarios();
    let names: Vec<&str> = all.iter().map(|s| s.name).collect();
    let selected: Vec<&str> = match &args.only {
        None => names.clone(),
        Some(only) => {
            if let Some(unknown) = only.iter().find(|n| !names.contains(&n.as_str())) {
                eprintln!("unknown scenario {unknown}; known: {}", names.join(", "));
                return ExitCode::from(2);
            }
            names
                .iter()
                .copied()
                .filter(|n| only.iter().any(|o| o == n))
                .collect()
        }
    };
    if args.dark {
        eprintln!("note: --dark has no effect yet (the theme override is not available)");
    }
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("cannot find myself: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut failed = false;
    for name in selected {
        let mut cmd = Command::new(&exe);
        cmd.arg("--child")
            .arg(name)
            .arg("--out")
            .arg(&args.out)
            .arg("--prefix")
            .arg(&args.prefix);
        if args.dark {
            cmd.arg("--dark");
        }
        match cmd.status() {
            Ok(status) if status.success() => {}
            Ok(status) => {
                // The child prints its own FAIL line; a crash prints none.
                if status.code() != Some(1) {
                    println!("FAIL {name}: the scenario process ended with {status}");
                }
                failed = true;
            }
            Err(e) => {
                println!("FAIL {name}: could not start the scenario process: {e}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Runs one scenario in this process and prints its line.
fn child(name: &str, args: &Args) -> ExitCode {
    let Some(scenario) = scenarios().into_iter().find(|s| s.name == name) else {
        println!("FAIL {name}: no such scenario");
        return ExitCode::FAILURE;
    };
    match run_scenario(&scenario, &args.out, &args.prefix) {
        Ok(_) => {
            println!("ok {name}");
            ExitCode::SUCCESS
        }
        Err(reason) => {
            println!("FAIL {name}: {reason}");
            ExitCode::FAILURE
        }
    }
}
