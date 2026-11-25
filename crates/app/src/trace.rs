use crate::fault::{classify, Fault};
use crate::feature::Feature;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sink {
    Memory,
    File(PathBuf),
}

pub const SINK_ENV: &str = "ZIKARON_TRACE";

pub const RING: usize = 512;

#[derive(Debug, Default)]
pub struct Trace {
    ring: Vec<&'static str>,
    dropped: usize,
    sink: Option<Sink>,
    trouble: Option<Fault>,
    full: Option<Fault>,
    full_said: bool,
}

fn cell() -> &'static Mutex<Trace> {
    static C: OnceLock<Mutex<Trace>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Trace::default()))
}

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

pub fn open() -> Sink {
    let mut t = cell().lock().unwrap_or_else(|e| e.into_inner());
    settle(&mut t)
}

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

pub fn take_full() -> Option<Fault> {
    let mut t = cell().lock().unwrap_or_else(|e| e.into_inner());
    if t.full_said {
        return None;
    }
    let f = t.full.clone()?;
    t.full_said = true;
    Some(f)
}

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

pub fn over_cap(size: u64, line: u64) -> bool {
    size + line > TRACE_CAP_BYTES
}

pub fn dropped() -> usize {
    cell().lock().unwrap_or_else(|e| e.into_inner()).dropped
}

pub fn ring() -> Vec<&'static str> {
    cell().lock().unwrap_or_else(|e| e.into_inner()).ring.clone()
}

pub fn sink() -> Sink {
    cell()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .sink
        .clone()
        .unwrap_or(Sink::Memory)
}

pub fn trouble() -> Option<Fault> {
    cell().lock().unwrap_or_else(|e| e.into_inner()).trouble.clone()
}
