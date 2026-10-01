//! Trace channel: when a public entry point of this component is crossed, append its component code to the
//! trace file as a trace mark.
//!
//! Same channel as the core's: with `ZIKARON_TRACE` pointing at a file, each crossing appends `<id>\n`;
//! without it nothing happens. Trace marks are diagnostic only and never enter a decision.
//!
//! This crate carries its own copy of the emitter because it has no dependency on the core, and a dependency
//! added only for tracing would show up as a real edge in the dependency graph.

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

/// The component code this crate writes as its trace mark.
pub const A1: &str = "A1";

/// Channel name, in one place.
pub const TRACE_ENV: &str = "ZIKARON_TRACE";

static SINK: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Cross a public entry point: append the trace mark. A failed write changes nothing else.
pub fn mark(id: &str) {
    let sink = SINK.get_or_init(|| std::env::var_os(TRACE_ENV).map(PathBuf::from));
    if let Some(path) = sink {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let mut line = String::with_capacity(id.len() + 1);
            line.push_str(id);
            line.push('\n');
            // Stop writing at the cap (set once in `.cargo/config.toml`).
            let size = f.metadata().map(|m| m.len()).unwrap_or(0);
            if over_cap(size, line.len() as u64) {
                return;
            }
            let _ = f.write_all(line.as_bytes());
        }
    }
}

/// Upper bound of the trace file in bytes, read at compile time from `[env] ZIKARON_TRACE_CAP_BYTES` in
/// `.cargo/config.toml`; a malformed value fails the build.
pub const TRACE_CAP_BYTES: u64 = cap_of(env!("ZIKARON_TRACE_CAP_BYTES"));

const fn cap_of(s: &str) -> u64 {
    let b = s.as_bytes();
    assert!(!b.is_empty(), "ZIKARON_TRACE_CAP_BYTES 空着");
    let mut n: u64 = 0;
    let mut i = 0;
    while i < b.len() {
        assert!(b[i] >= b'0' && b[i] <= b'9', "ZIKARON_TRACE_CAP_BYTES 只许十进制数字");
        n = n * 10 + (b[i] - b'0') as u64;
        i += 1;
    }
    n
}

/// Whether writing `line` more bytes to a file of `size` bytes would pass the cap.
pub fn over_cap(size: u64, line: u64) -> bool {
    size + line > TRACE_CAP_BYTES
}
