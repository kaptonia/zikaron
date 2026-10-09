//! The shipped window binary. It takes no arguments and refuses any it is given instead of ignoring them.
//!
//! `windows_subsystem` stops Windows from opening a console beside the window; other platforms ignore it.
#![windows_subsystem = "windows"]

fn main() {
    // The refusal goes to standard error and, where a window program has no terminal, to a dialog.
    if std::env::args().len() > 1 {
        std::process::exit(app::window::refuse_arguments())
    }
    std::process::exit(app::window::run())
}
