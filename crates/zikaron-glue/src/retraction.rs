//! The retraction convention, built on the open entry types of law §6.9.
//!
//! The app and the CLI both write retractions, so they must write and read the same shape. The whole convention
//! lives here: type literal, body keys, body shape, and the reading and writing rules.
//!
//! - `entryType` = [`ENTRY_TYPE`];
//! - body [`SUBJECT`] (hex32, the `entry_id` of a `history` entry in this ledger, required);
//! - body [`NOTE_MD`] (prose, optional).
//!
//! Reading goes by ascending seq. A well-shaped retraction pointing at an earlier, not yet deleted `history` in
//! this ledger marks it deleted. A repeated deletion, a non-history subject, a subject not in this ledger or a
//! malformed body reads as invalid with its reason ([`Invalid`]). Reading never rejects a ledger and adds no
//! error to law §4.3.
//!
//! Before writing, ask [`may_retract`]: the subject must be hex32, a `history` on this ledger's lineage, and not
//! yet deleted; otherwise it is refused by name ([`Invalid::token`]), so no writer produces an invalid
//! retraction.
//!
//! This adds nothing to the spec: the core verifies the entry under law §4, the audit lists it under
//! `UNKNOWN_TYPE`, and the ledger's label is unaffected.

use std::collections::BTreeMap;
use zikaron::entry::Entry;
use zikaron::json::Value;
use zikaron::tokens::EntryType;

/// The convention's type literal.
pub const ENTRY_TYPE: &str = "retraction";

/// Body key naming the retracted entry.
pub const SUBJECT: &str = "subject";

/// Body key of the optional note (the same name as in the law §6.8 annotation body).
pub const NOTE_MD: &str = "note_md";

/// Build a retraction body. An empty `note_md` leaves the field out (it is optional).
pub fn body(subject: &str, note_md: &str) -> Value {
    let mut m = vec![(SUBJECT.to_string(), Value::Str(subject.to_string()))];
    let note = note_md.trim();
    if !note.is_empty() {
        m.push((NOTE_MD.to_string(), Value::Str(note.to_string())));
    }
    Value::Obj(m)
}

/// Whether a body fits the convention: exactly `subject` (hex32) and an optional `note_md` (string), nothing
/// else.
pub fn shape_ok(body: &Value) -> bool {
    let Value::Obj(m) = body else { return false };
    let mut subject = false;
    for (k, v) in m {
        match (k.as_str(), v) {
            (SUBJECT, Value::Str(s)) if zikaron::hexfmt::is_hex32(s) => subject = true,
            (NOTE_MD, Value::Str(_)) => {}
            _ => return false,
        }
    }
    subject
}

/// Why a retraction is invalid, which is also why writing one is refused. Closed set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Invalid {
    /// The body does not fit (subject missing, not hex32, or extra members).
    Shape,
    /// The subject is not in this ledger (another ledger's entry included).
    NotInLedger,
    /// The subject is not a work record (`history`).
    NotAWork,
    /// The subject was already deleted.
    Repeated,
}

impl Invalid {
    /// The refusal token; the CLI passes it on unchanged.
    pub fn token(self) -> &'static str {
        match self {
            Invalid::Shape => "E_RETRACT_SHAPE",
            Invalid::NotInLedger => "E_RETRACT_NOT_IN_LEDGER",
            Invalid::NotAWork => "E_RETRACT_NOT_A_WORK",
            Invalid::Repeated => "E_RETRACT_REPEATED",
        }
    }

    pub const ALL: [Invalid; 4] = [Invalid::Shape, Invalid::NotInLedger, Invalid::NotAWork, Invalid::Repeated];
}

/// The fields of a ledger entry the convention uses; the reading and writing rules use nothing else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// Entry id (`0x` plus 64 hex).
    pub id: String,
    pub seq: u64,
    /// The type as classified by the core's law §6 table; unlisted types are `Other`.
    pub kind: EntryType,
    /// The type literal on the wire, unchanged.
    pub raw_type: String,
    /// The subject of a retraction; `None` for other entries.
    pub subject: Option<String>,
    /// Whether a retraction's body fits the convention; false for other entries.
    pub shape_ok: bool,
}

impl Line {
    /// Read the fields the convention needs from an entry the core returned.
    pub fn of(e: &Entry) -> Line {
        let convention = is_retraction(e.kind, &e.entry_type);
        let subject = match &e.body {
            Value::Obj(m) if convention => m.iter().find(|(k, _)| k == SUBJECT).and_then(|(_, v)| match v {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            }),
            _ => None,
        };
        Line {
            id: e.id_hex(),
            seq: e.seq,
            kind: e.kind,
            raw_type: e.entry_type.clone(),
            subject,
            shape_ok: convention && shape_ok(&e.body),
        }
    }
}

/// Whether an entry is a retraction (an unlisted type whose literal is exactly the convention's).
pub fn is_retraction(kind: EntryType, raw_type: &str) -> bool {
    kind == EntryType::Other && raw_type == ENTRY_TYPE
}

/// A ledger as read by the convention.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    /// Deleted work records: `history` id to (retraction seq, retraction id).
    pub deleted: BTreeMap<String, (u64, String)>,
    /// Retractions read as invalid: retraction id to (seq, reason).
    pub invalid: BTreeMap<String, (u64, Invalid)>,
    /// How many retraction-type entries the ledger holds (valid and invalid).
    pub count: usize,
}

impl Reading {
    /// Whether a work record was deleted.
    pub fn is_deleted(&self, history_id: &str) -> bool {
        self.deleted.keys().any(|k| k.eq_ignore_ascii_case(history_id))
    }
}

/// Read a ledger. Row order does not matter; reading goes by ascending seq (the first retraction counts,
/// later repeats are invalid).
pub fn read(lines: &[Line]) -> Reading {
    let mut out = Reading::default();
    let mut ordered: Vec<&Line> = lines.iter().collect();
    ordered.sort_by_key(|l| l.seq);
    for r in ordered.iter().filter(|l| is_retraction(l.kind, &l.raw_type)) {
        out.count += 1;
        let verdict = match r.subject.as_deref() {
            None => Err(Invalid::Shape),
            Some(_) if !r.shape_ok => Err(Invalid::Shape),
            Some(s) => match lines.iter().find(|x| x.id.eq_ignore_ascii_case(s)) {
                None => Err(Invalid::NotInLedger),
                Some(x) if x.kind != EntryType::History => Err(Invalid::NotAWork),
                // A retraction can only delete an earlier entry (the ledger only grows, so a later entry
                // cannot have been deleted by it).
                Some(x) if x.seq >= r.seq => Err(Invalid::NotInLedger),
                Some(x) if out.is_deleted(&x.id) => Err(Invalid::Repeated),
                Some(x) => Ok(x.id.clone()),
            },
        };
        match verdict {
            Ok(h) => {
                out.deleted.insert(h, (r.seq, r.id.clone()));
            }
            Err(why) => {
                out.invalid.insert(r.id.clone(), (r.seq, why));
            }
        }
    }
    out
}

/// The writing rule: whether this ledger can take a retraction of `subject` now. Returns the work record's id
/// (as spelled in the ledger) or the reason. The new entry follows the ledger head, so it points at an earlier
/// entry and a retraction that passes this rule always reads as valid.
pub fn may_retract(lines: &[Line], subject: &str) -> Result<String, Invalid> {
    let s = subject.trim();
    if !zikaron::hexfmt::is_hex32(s) {
        return Err(Invalid::Shape);
    }
    let Some(x) = lines.iter().find(|l| l.id.eq_ignore_ascii_case(s)) else {
        return Err(Invalid::NotInLedger);
    };
    if x.kind != EntryType::History {
        return Err(Invalid::NotAWork);
    }
    if read(lines).is_deleted(&x.id) {
        return Err(Invalid::Repeated);
    }
    Ok(x.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(n: u8) -> String {
        format!("0x{}", format!("{n:02x}").repeat(32))
    }

    fn line(seq: u64, kind: EntryType, raw: &str, id: &str, subject: Option<&str>) -> Line {
        let convention = is_retraction(kind, raw);
        Line {
            id: id.to_string(),
            seq,
            kind,
            raw_type: raw.to_string(),
            subject: subject.map(str::to_string),
            shape_ok: convention && subject.map(zikaron::hexfmt::is_hex32).unwrap_or(false),
        }
    }

    /// Each of the four refusals fires; a valid subject returns the ledger's spelling, and the resulting
    /// retraction reads as valid.
    #[test]
    fn the_production_law_refuses_every_shape_the_reading_would_call_invalid() {
        let mut lines = vec![
            line(0, EntryType::Genesis, EntryType::Genesis.as_str(), &h(1), None),
            line(1, EntryType::History, EntryType::History.as_str(), &h(2), None),
            line(2, EntryType::History, EntryType::History.as_str(), &h(3), None),
            line(3, EntryType::Other, ENTRY_TYPE, &h(4), Some(&h(2))),
        ];
        assert_eq!(may_retract(&lines, "0x12"), Err(Invalid::Shape));
        assert_eq!(may_retract(&lines, ""), Err(Invalid::Shape));
        assert_eq!(may_retract(&lines, &h(9)), Err(Invalid::NotInLedger));
        assert_eq!(may_retract(&lines, &h(1)), Err(Invalid::NotAWork));
        assert_eq!(may_retract(&lines, &h(2)), Err(Invalid::Repeated));
        let upper = h(3).to_uppercase().replacen("0X", "0x", 1);
        assert_eq!(may_retract(&lines, &upper), Ok(h(3)));
        lines.push(line(4, EntryType::Other, ENTRY_TYPE, &h(5), Some(&h(3))));
        let r = read(&lines);
        assert!(r.is_deleted(&h(3)) && r.invalid.is_empty(), "过了产出律的删除读作有效");
    }

    #[test]
    fn the_body_shape_is_closed_and_every_refusal_has_its_own_token() {
        assert!(shape_ok(&body(&h(2), "")));
        assert!(shape_ok(&body(&h(2), "写错了")));
        assert!(!shape_ok(&Value::Obj(vec![(SUBJECT.into(), Value::Str("0x12".into()))])));
        assert!(!shape_ok(&Value::Obj(vec![(SUBJECT.into(), Value::Str(h(2))), ("extra".into(), Value::Int(1))])));
        let mut tokens: Vec<&str> = Invalid::ALL.iter().map(|x| x.token()).collect();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), Invalid::ALL.len());
    }
}
