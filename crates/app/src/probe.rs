//! Self-check: really reads the disk.
//!
//! The first thing on the shell that shows a real source. It reads where each of the four fonts comes from
//! (the two embedded ones give their byte counts, the two taken from the system give their location and byte
//! count on this machine), and the trace channel's sink and how many lines its file has. Not one hard-coded
//! number, not one container that is always empty; when something cannot be read, the error goes through
//! `fault`'s three paths, never pretending it was read.

use crate::fault::{classify, Fault};
use crate::trace::{self, Sink};
use zikaron_ui::fonts;

/// One font's reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub role: &'static str,
    pub file: &'static str,
    pub index: u32,
    pub bytes: u64,
    /// The path on this machine for a system font; embedded fonts have no path, written empty.
    pub path: String,
    /// Embedded or taken from the system.
    pub place: &'static str,
    /// The licence shipped with an embedded font; system fonts have none.
    pub licence: &'static str,
}

/// One self-check's reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub faces: Vec<Face>,
    pub missing: Vec<&'static str>,
    pub sink: String,
    /// How many lines the sink file has (the in-memory ring form has no file, so None).
    pub trace_lines: Option<usize>,
    pub marks: usize,
}

impl Report {
    pub fn found(&self) -> usize {
        self.faces.len()
    }
}

/// Run once. This is the work on the background thread, so it touches no egui.
pub fn run() -> Result<Report, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are marked too.
    crate::trace::mark(crate::feature::Feature::H1);
    let found = fonts::find();
    let mut faces = Vec::new();
    for f in &found.faces {
        // System fonts are read from disk now (unreadable is refused by name, never pretending); embedded
        // fonts' byte counts come from the table itself.
        let bytes = match &f.path {
            Some(p) => std::fs::metadata(p).map_err(|e| classify(&e, &p.display().to_string()))?.len(),
            None => f.bytes,
        };
        faces.push(Face {
            role: f.role.as_str(),
            file: f.file,
            index: f.index,
            bytes,
            path: f.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
            place: f.place.as_str(),
            licence: f.licence().unwrap_or_default(),
        });
    }
    let sink = trace::sink();
    let trace_lines = match &sink {
        Sink::Memory => None,
        Sink::File(p) => {
            let text = std::fs::read_to_string(p).map_err(|e| classify(&e, &p.display().to_string()))?;
            Some(text.lines().count())
        }
    };
    Ok(Report {
        faces,
        missing: found.missing.iter().map(|r| r.as_str()).collect(),
        sink: match &sink {
            Sink::Memory => "memory".to_string(),
            Sink::File(p) => p.display().to_string(),
        },
        trace_lines,
        marks: trace::dropped(),
    })
}
