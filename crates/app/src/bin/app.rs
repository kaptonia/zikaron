//! The shipped build: the window. It takes no arguments.
//!
//! Anyone passing arguments gets a named refusal rather than having them silently ignored.
fn main() {
    if std::env::args().len() > 1 {
        eprintln!("E_ARGS the window takes no arguments");
        std::process::exit(2)
    }
    std::process::exit(app::window::run())
}
