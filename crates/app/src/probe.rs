use crate::fault::{classify, Fault};
use crate::trace::{self, Sink};
use zikaron_ui::fonts;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub role: &'static str,
    pub file: &'static str,
    pub index: u32,
    pub bytes: u64,
    pub path: String,
    pub place: &'static str,
    pub licence: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub faces: Vec<Face>,
    pub missing: Vec<&'static str>,
    pub sink: String,
    pub trace_lines: Option<usize>,
    pub marks: usize,
}

impl Report {
    pub fn found(&self) -> usize {
        self.faces.len()
    }
}

pub fn run() -> Result<Report, Fault> {
    crate::trace::mark(crate::feature::Feature::H1);
    let found = fonts::find();
    let mut faces = Vec::new();
    for f in &found.faces {
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
