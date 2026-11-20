#![windows_subsystem = "windows"]

fn main() {
    let self_check = std::env::args().any(|a| a == "--self-check");
    std::process::exit(fm_spike::app::run(self_check));
}
