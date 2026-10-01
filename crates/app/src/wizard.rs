//! First-window checklist page. Two steps, not skippable, resumable, each step keeps only a
//! verification mark.
//!
//! ─── Not skippable, by construction ───
//!
//! The dangerous form is several independent checkboxes plus a reminder "go in order". The design: the
//! checklist can only answer which step is next ([`Wizard::next`]), and [`Wizard::tick`] accepts only that
//! step; passing another is refused by name (`STEP_SKIPPED`). So a call like "skip the terms template and
//! anchor the grant" turns red at once.
//!
//! ─── Each step keeps only a verification mark ───
//!
//! Each step records one verification mark plus the person's own note of evidence (a hash or a sentence).
//! This desk cannot read the facts behind the evidence, and recording them here would be a second ledger, so
//! only the mark is kept.
//!
//! ─── Resumable ───
//!
//! The checklist lives in the home's `settings` room as a canonical value (the core's `json`), so copying a
//! home carries it along (any copy is equivalent). Close the app and reopen it, and it stands where it was.

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use zikaron::json::{self, Value};

/// The checklist file's name. One name, one home.
pub const FILE: &str = "first-window.json";

/// The two steps particular to zikaron. Closed, and the order is themselves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// 1 · Fill the terms template (record hash, window).
    Terms,
    /// 2 · Anchor the grant (drafter prefilled).
    Anchor,
}

/// Names in older files of steps removed from the checklist. Closed. Such a line is skipped, and the
/// remaining lines are checked in today's order: an older file ticked up to it resumes at the step after it,
/// and later ticks continue as they were. Removing a step only adds a name here; the reading is not written
/// anywhere else.
pub const RETIRED: [&str; 2] = ["bond", "undertaking"];

impl Step {
    pub const ALL: [Step; 2] = [Step::Terms, Step::Anchor];

    pub fn as_str(self) -> &'static str {
        match self {
            Step::Terms => "terms",
            Step::Anchor => "anchor",
        }
    }

    pub fn parse(x: &str) -> Option<Step> {
        Step::ALL.into_iter().find(|s| s.as_str() == x)
    }

    /// This step's position (from zero).
    pub fn at(self) -> usize {
        Step::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

/// A verification mark: which step, and the evidence the person noted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub step: Step,
    /// The person's own note of evidence (a hash or a sentence). This desk does not judge it.
    pub said: String,
}

/// A checklist.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wizard {
    /// The steps ticked so far, in order.
    pub marks: Vec<Mark>,
}

impl Wizard {
    /// How many steps are done.
    pub fn done(&self) -> usize {
        self.marks.len()
    }

    /// Which step is next. `None` when both are done.
    pub fn next(&self) -> Option<Step> {
        Step::ALL.get(self.marks.len()).copied()
    }

    /// Whether this step is ticked.
    pub fn has(&self, s: Step) -> bool {
        self.marks.iter().any(|m| m.step == s)
    }

    /// Tick a step. Accepts only `next()`; any other step is refused by name at once.
    pub fn tick(&mut self, s: Step, said: &str) -> Result<Step, Fault> {
        let Some(want) = self.next() else {
            return Err(Fault::known(
                Known::StepSkipped,
                crate::lang::t(crate::lang::Key::Tail223).to_string(),
            ));
        };
        if s != want {
            return Err(Fault::known(
                Known::StepSkipped,
                crate::lang::filln(crate::lang::Key::Tail224, &[&(want.as_str()).to_string(), &(s.as_str()).to_string()]),
            ));
        }
        self.marks.push(Mark { step: s, said: said.trim().to_string() });
        Ok(s)
    }

    /// Start over (pressed by the person).
    pub fn clear(&mut self) {
        self.marks.clear();
    }

    /// Read. No file means an empty checklist: "not started" is not an error.
    pub fn read(home: &Home) -> Result<Wizard, Fault> {
        let p = home.dir(Slot::Settings).join(FILE);
        // Sealed (`local::Doc::FirstWindow`): no file is an empty checklist; locked or not opening is refused
        // by name, never read as empty.
        let Some(bytes) = crate::local::read(&p, crate::local::Doc::FirstWindow)? else {
            return Ok(Wizard::default());
        };
        let v = json::parse(&bytes)
            .map_err(|t| Fault::known(Known::SettingsShape, format!("{}: {t:?}", p.display())))?;
        let Value::Obj(m) = &v else {
            return Err(Fault::known(Known::SettingsShape, p.display().to_string()));
        };
        let Some((_, Value::Arr(rows))) = m.iter().find(|(k, _)| k == member::MARKS) else {
            return Err(Fault::known(
                Known::SettingsShape,
                crate::lang::filln(crate::lang::Key::Tail225, &[&(p.display()).to_string()]),
            ));
        };
        let mut marks = Vec::new();
        for r in rows {
            let Value::Obj(rm) = r else {
                return Err(Fault::known(Known::SettingsShape, p.display().to_string()));
            };
            let step = match rm.iter().find(|(k, _)| k == member::STEP) {
                // Lines in older files for removed steps (`RETIRED`): skipped, and the rest checked in
                // today's order.
                Some((_, Value::Str(s))) if RETIRED.contains(&s.as_str()) => continue,
                Some((_, Value::Str(s))) => Step::parse(s),
                _ => None,
            };
            let Some(step) = step else {
                return Err(Fault::known(
                    Known::SettingsShape,
                    crate::lang::filln(crate::lang::Key::Tail226, &[&(p.display()).to_string()]),
                ));
            };
            let said = match rm.iter().find(|(k, _)| k == member::SAID) {
                Some((_, Value::Str(s))) => s.clone(),
                _ => String::new(),
            };
            marks.push(Mark { step, said });
        }
        // The order read back must still be the two steps' order. A hand-edited file may not turn the
        // checklist into a jumbled table: then `next()` would no longer say what the checklist says.
        for (i, m) in marks.iter().enumerate() {
            if Step::ALL.get(i) != Some(&m.step) {
                return Err(Fault::known(
                    Known::SettingsShape,
                    crate::lang::filln(crate::lang::Key::Tail227, &[&(p.display()).to_string()]),
                ));
            }
        }
        Ok(Wizard { marks })
    }

    /// Write. Overwriting the old checklist is intended.
    pub fn write(&self, home: &Home) -> Result<(), Fault> {
        let rows: Vec<Value> = self
            .marks
            .iter()
            .map(|m| {
                Value::Obj(vec![
                    (member::SAID.to_string(), Value::Str(m.said.clone())),
                    (member::STEP.to_string(), Value::Str(m.step.as_str().to_string())),
                ])
            })
            .collect();
        let bytes = json::canon_bytes(&Value::Obj(vec![(member::MARKS.to_string(), Value::Arr(rows))]));
        // Writing to disk has one method (`local::put`: sealed, written aside, then renamed).
        crate::local::put(&home.dir(Slot::Settings), FILE, crate::local::Doc::FirstWindow, &bytes)
    }
}

/// The wizard file's member names. One name, one home: the reader, the writer and anything reading the raw
/// members spell them only here.
pub mod member {
    pub const MARKS: &str = "marks";
    pub const SAID: &str = "said";
    pub const STEP: &str = "step";
}
