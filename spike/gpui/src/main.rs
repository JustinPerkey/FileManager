#![windows_subsystem = "windows"]

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value_of = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from)
    };
    let code = if let Some(dir) = value_of("--shoot") {
        fm_spike::shoot::run(&dir, args.iter().any(|a| a == "--dark"))
    } else if let Some(a) = value_of("--diff") {
        let at = |n: usize| {
            args.iter()
                .position(|x| x == "--diff")
                .and_then(|i| args.get(i + n))
                .map(PathBuf::from)
                .unwrap_or_default()
        };
        fm_spike::check::diff(&a, &at(2), &at(3))
    } else if let Some(dir) = value_of("--verify") {
        let reference = args
            .iter()
            .position(|a| a == "--verify")
            .and_then(|i| args.get(i + 2))
            .map_or_else(
                || PathBuf::from("../../docs/manual/images/pipeline-chosen.png"),
                PathBuf::from,
            );
        fm_spike::check::run(&dir, &reference)
    } else if args.iter().any(|a| a == "--pipeline") {
        fm_spike::demo::run(fm_spike::demo::Screen::Pipeline)
    } else if args.iter().any(|a| a == "--widgets") {
        fm_spike::demo::run(fm_spike::demo::Screen::Widgets)
    } else {
        fm_spike::app::run(args.iter().any(|a| a == "--self-check"))
    };
    std::process::exit(code);
}
