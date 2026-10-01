//! `zikaron`: the command-line entry point.
//!
//! This file reads the command line, dispatches to a verb, prints the answer and exits with its code. No
//! decision is made here: verbs live in `zikaron_cli::verbs`, exit codes in `zikaron_cli::codes`, printing in
//! `zikaron_cli::out`.

use zikaron_cli::args::Args;
use zikaron_cli::{out, verbs};

fn main() {
    let argv: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let a = Args::read(argv);
    // Neither `emit` nor `misuse` returns: the process has two mutually exclusive exits, so nothing follows
    // this line.
    out::emit(verbs::run(&a))
}
