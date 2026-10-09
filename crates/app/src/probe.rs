//! Self-check from real disk reads.
//!
//! Reports where each of the four fonts comes from (embedded fonts with their byte counts, system fonts with
//! their path and size on this machine), the trace sink, and how many lines the trace file has. Nothing is
//! hard-coded; anything unreadable is reported as an error through `fault`, never shown as if it were read.

use crate::fault::{classify, Fault};
use crate::trace::{self, Sink};
use zikaron_ui::fonts;

/// One font's details.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub role: &'static str,
    pub file: &'static str,
    pub index: u32,
    pub bytes: u64,
    /// The path of a system font on this machine; empty for embedded fonts.
    pub path: String,
    /// Embedded or taken from the system.
    pub place: &'static str,
    /// The licence shipped with an embedded font; system fonts have none.
    pub licence: &'static str,
}

/// A self-check result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub faces: Vec<Face>,
    pub missing: Vec<&'static str>,
    pub sink: String,
    /// How many lines the trace file has (`None` for the in-memory sink).
    pub trace_lines: Option<usize>,
    pub marks: usize,
}

impl Report {
    pub fn found(&self) -> usize {
        self.faces.len()
    }
}

/// Run the self-check. Runs on a background thread, so it does not touch egui.
pub fn run() -> Result<Report, Fault> {
    // Traced here so direct calls that bypass `apply` (tests, the CLI) are traced too.
    crate::trace::mark(crate::feature::Feature::H1);
    let found = fonts::find();
    let mut faces = Vec::new();
    for f in &found.faces {
        // System font sizes are read from disk now (an unreadable font is an error); embedded font sizes come
        // from the font table.
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
