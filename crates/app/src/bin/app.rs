//! The shipped build: the window. It takes no arguments.
//!
//! Anyone passing arguments gets a named refusal rather than having them silently ignored.
//!
//! A window program: started from the desktop it opens no console beside the window (the subsystem is read by
//! the Windows linker only; other systems' builds ignore it).
#![windows_subsystem = "windows"]

fn main() {
    // Said through the platform interface: standard error as always, and where a window program has no
    // terminal, a dialog the person sees.
    if std::env::args().len() > 1 {
        std::process::exit(app::window::refuse_arguments())
    }
    std::process::exit(app::window::run())
}
