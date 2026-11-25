#![forbid(unsafe_code)]

use zikaron_cli::args::Args;
use zikaron_cli::{out, verbs};

fn main() {
    let argv: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let a = Args::read(argv);
    out::emit(verbs::run(&a))
}
