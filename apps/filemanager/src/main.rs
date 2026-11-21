#![windows_subsystem = "windows"]

fn main() {
    let self_check = std::env::args().skip(1).any(|a| a == "--self-check");
    std::process::exit(fm_app::run(self_check));
}
