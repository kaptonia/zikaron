//! The diagnostic trace channel: on in release builds; a mark's content is a component code.
//!
//! The channel ships with the build rather than being a temporary debugging aid, so this file has no `cfg`
//! that could compile it out. Marks are diagnostics only: nothing in the product decides anything from them.
//!
//! Two sinks: an in-memory ring (always) and a file (when `ZIKARON_TRACE` names one). A file that cannot be
//! written is not silent: the error goes to `trouble` and is shown in the UI.
//!
//! The environment variable name and the `<id>\n` line format match `zikaron::trace` and
//! `zikaron_store::trace` byte for byte. Each crate keeps its own copy rather than depending on another crate
//! just to emit a mark: tracing is diagnostics, not a feature, and should not add dependency edges. This is
//! the third copy, deliberately.
//!
//! This copy adds two things the shell needs: the in-memory ring (the UI shows how many marks were emitted
//! this run) and `trouble` (a sink that cannot be opened must be reported). It also takes the closed type
//! `Feature` instead of `&str`, so free text cannot be passed and marks cannot grow into a second log.

use crate::fault::{classify, Fault};
use crate::feature::Feature;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Where trace marks land.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sink {
    /// Only in the in-memory ring.
    Memory,
    /// The in-memory ring plus this file.
    File(PathBuf),
}

/// The environment variable naming the sink file (spelled only here).
pub const SINK_ENV: &str = "ZIKARON_TRACE";

/// At most this many marks are kept in the ring; marks are diagnostics and must not grow without bound.
pub const RING: usize = 512;

#[derive(Debug, Default)]
pub struct Trace {
    ring: Vec<&'static str>,
    dropped: usize,
    sink: Option<Sink>,
    trouble: Option<Fault>,
    /// Set when the sink file reaches its cap: writing stops there and this error is reported once.
    full: Option<Fault>,
    full_said: bool,
}

fn cell() -> &'static Mutex<Trace> {
    static C: OnceLock<Mutex<Trace>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Trace::default()))
}

/// Settle the sink. Only the first call counts; later calls return the same answer.
///
/// Both `open` and `mark` call it, so a mark emitted before the channel opens loses nothing: it opens the
/// channel itself. This matters because the widget library emits its mark before the shell starts; if the
/// sink waited for `open`, those marks would land only in the ring, and partial loss is the hardest kind to
/// notice.
fn settle(t: &mut Trace) -> Sink {
    if let Some(s) = &t.sink {
        return s.clone();
    }
    let sink = match std::env::var(SINK_ENV) {
        Ok(p) if !p.is_empty() => {
            let path = PathBuf::from(p);
            match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                Ok(_) => Sink::File(path),
                Err(e) => {
                    t.trouble = Some(classify(&e, &path.display().to_string()));
                    Sink::Memory
                }
            }
        }
        _ => Sink::Memory,
    };
    t.sink = Some(sink.clone());
    sink
}

/// Open the channel: read the environment once and settle the sink; an error opening the sink file is kept
/// for the UI. Calling twice is safe. The shell calls it once at startup so that error is seen early (`mark`
/// opens the channel itself if needed).
pub fn open() -> Sink {
    let mut t = cell().lock().unwrap_or_else(|e| e.into_inner());
    settle(&mut t)
}

/// Emit a trace mark. Its content is the component code and nothing else.
pub fn mark(f: Feature) {
    let mut t = cell().lock().unwrap_or_else(|e| e.into_inner());
    let id = f.id();
    t.dropped += 1;
    if t.ring.len() == RING {
        t.ring.remove(0);
    }
    t.ring.push(id);
    if let Sink::File(p) = settle(&mut t) {
        if let Ok(mut h) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
            // Writing stops at the cap (set only in `.cargo/config.toml`, shared by all three emitters); the
            // first time it is reached, an error is recorded, which the shell shows (`take_full`).
            let size = h.metadata().map(|m| m.len()).unwrap_or(0);
            if over_cap(size, id.len() as u64 + 1) {
                if t.full.is_none() {
                    t.full = Some(Fault::known(crate::fault::Known::TraceFull, format!("{} · {}", p.display(), TRACE_CAP_BYTES)));
                }
                return;
            }
            let _ = writeln!(h, "{id}");
        }
    }
}

/// The error for the sink file reaching its cap, returned only once (`None` afterwards). The shell takes it
/// when receiving background results and shows it in the trouble bar.
pub fn take_full() -> Option<Fault> {
    let mut t = cell().lock().unwrap_or_else(|e| e.into_inner());
    if t.full_said {
        return None;
    }
    let f = t.full.clone()?;
    t.full_said = true;
    Some(f)
}

/// The trace sink file holds at most this many bytes. The value lives only in the workspace's
/// `.cargo/config.toml` under `[env] ZIKARON_TRACE_CAP_BYTES`, read by all three emitters; a malformed value
/// fails to compile.
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

/// Whether writing `line` more bytes to a file now `size` bytes long would exceed the cap.
pub fn over_cap(size: u64, line: u64) -> bool {
    size + line > TRACE_CAP_BYTES
}

/// How many marks were emitted this run.
pub fn dropped() -> usize {
    cell().lock().unwrap_or_else(|e| e.into_inner()).dropped
}

/// The marks in the ring now, in emission order.
pub fn ring() -> Vec<&'static str> {
    cell().lock().unwrap_or_else(|e| e.into_inner()).ring.clone()
}

/// The sink.
pub fn sink() -> Sink {
    cell()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .sink
        .clone()
        .unwrap_or(Sink::Memory)
}

/// The error from opening the channel, if any. Not silent: the UI must show it.
pub fn trouble() -> Option<Fault> {
    cell().lock().unwrap_or_else(|e| e.into_inner()).trouble.clone()
}
