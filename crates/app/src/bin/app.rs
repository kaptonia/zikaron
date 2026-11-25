#![windows_subsystem = "windows"]

#![forbid(unsafe_code)]

fn main() {
    if std::env::args().len() > 1 {
        std::process::exit(app::window::refuse_arguments())
    }
    std::process::exit(app::window::run())
}
