//! The `filemanager-shoot` command line and its child-process runner, in the
//! library so they are testable.

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// How long one scenario's child process may run before it is killed.
pub const CHILD_TIMEOUT: Duration = Duration::from_secs(60);

/// The parsed command line.
#[derive(Debug)]
pub struct Args {
    pub out: PathBuf,
    pub only: Option<Vec<String>>,
    pub prefix: String,
    pub dark: bool,
    /// Hidden: run this one scenario in this process.
    pub child: Option<String>,
    /// Hidden: replaces [`CHILD_TIMEOUT`] (milliseconds).
    pub child_timeout: Option<Duration>,
}

/// Parses the arguments after the program name.
///
/// # Errors
/// An unknown flag, a flag without its value, or a missing `--out`.
pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args {
        out: PathBuf::new(),
        only: None,
        prefix: String::new(),
        dark: false,
        child: None,
        child_timeout: None,
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
            "--child-timeout-ms" => {
                let ms = value("--child-timeout-ms")?;
                let ms = ms
                    .parse()
                    .map_err(|_| format!("--child-timeout-ms needs a number, not {ms}"))?;
                parsed.child_timeout = Some(Duration::from_millis(ms));
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    parsed.out = out.ok_or("--out is required")?;
    Ok(parsed)
}

/// Runs one child per name (the command comes from `command`), killing any
/// that outlives `timeout`. Prints a `FAIL` line for what the child cannot
/// report itself (a crash, a timeout, a failure to start). Returns those lines
/// and whether any child failed; every name is run either way.
pub fn run_children(
    names: &[&str],
    timeout: Duration,
    mut command: impl FnMut(&str) -> Command,
) -> (bool, Vec<String>) {
    let mut failed = false;
    let mut lines = Vec::new();
    let mut report = |line: String| {
        println!("{line}");
        lines.push(line);
    };
    for name in names {
        match command(name).spawn() {
            Err(e) => {
                report(format!(
                    "FAIL {name}: could not start the scenario process: {e}"
                ));
                failed = true;
            }
            Ok(mut child) => match wait(&mut child, timeout) {
                Ok(Some(status)) if status.success() => {}
                Ok(Some(status)) => {
                    // The child prints its own FAIL line; a crash prints none.
                    if status.code() != Some(1) {
                        report(format!(
                            "FAIL {name}: the scenario process ended with {status}"
                        ));
                    }
                    failed = true;
                }
                Ok(None) => {
                    report(format!(
                        "FAIL {name}: timed out after {}",
                        describe(timeout)
                    ));
                    failed = true;
                }
                Err(e) => {
                    report(format!(
                        "FAIL {name}: could not wait for the scenario process: {e}"
                    ));
                    failed = true;
                }
            },
        }
    }
    (failed, lines)
}

/// `60 s`, or `200 ms` for a timeout under a second.
fn describe(timeout: Duration) -> String {
    if timeout.subsec_nanos() == 0 {
        format!("{} s", timeout.as_secs())
    } else {
        format!("{} ms", timeout.as_millis())
    }
}

/// The exit status, or `None` after killing a child that ran past `timeout`.
fn wait(child: &mut Child, timeout: Duration) -> std::io::Result<Option<std::process::ExitStatus>> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if start.elapsed() >= timeout {
            child.kill()?;
            child.wait()?;
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
