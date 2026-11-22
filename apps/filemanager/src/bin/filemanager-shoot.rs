//! `filemanager-shoot --out <dir> [--only <name>,...] [--prefix <p>] [--dark]`
//!
//! Renders the GPUI app's scenarios to PNG (see `fm_app::shoot`). Needs the
//! `shoot` feature and, to capture, Windows at 100% display scale. Never part
//! of the release exe.
//!
//! The binary runs each scenario in its own child process (itself, with the
//! hidden `--child <name>` flag), so scenarios share no state and GPUI's
//! one-run-per-process limit does not matter. A child that runs longer than
//! 60 s is killed (`FAIL <name>: timed out after 60 s`) and the rest still run. It prints one line per scenario
//! (`ok <name>` or `FAIL <name>: <reason>`) and exits non-zero if any failed.

use std::process::{Command, ExitCode};

use fm_app::shoot::cli::{parse, run_children, Args, CHILD_TIMEOUT};
use fm_app::shoot::{run_scenario, scenarios};

const USAGE: &str =
    "usage: filemanager-shoot --out <dir> [--only <name>,...] [--prefix <p>] [--dark]";

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
    let (failed, _) = run_children(
        &selected,
        args.child_timeout.unwrap_or(CHILD_TIMEOUT),
        |name| {
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
            cmd
        },
    );
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
