use std::collections::BTreeMap;
use crate::book::{Entry, Kind};
use zikaron::json::Value;

pub const ENTRY_TYPE: &str = "retraction";

pub const SUBJECT: &str = "subject";

pub const NOTE_MD: &str = "note_md";

pub fn body(subject: &str, note_md: &str) -> Value {
    let mut m = vec![(SUBJECT.to_string(), Value::Str(subject.to_string()))];
    let note = note_md.trim();
    if !note.is_empty() {
        m.push((NOTE_MD.to_string(), Value::Str(note.to_string())));
    }
    Value::Obj(m)
}

pub fn shape_ok(body: &Value) -> bool {
    let Value::Obj(m) = body else { return false };
    let subject = m.iter().any(|(k, v)| k == SUBJECT && matches!(v, Value::Str(s) if zikaron::hexfmt::is_hex32(s)));
    let note = m.iter().all(|(k, v)| k != NOTE_MD || matches!(v, Value::Str(_)));
    subject && note
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Invalid {
    Shape,
    NotInLedger,
    NotAWork,
    Repeated,
}

impl Invalid {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub id: String,
    pub pos: u64,
    pub kind: Kind,
    pub raw_type: String,
    pub subject: Option<String>,
    pub shape_ok: bool,
}

impl Line {
    pub fn at(pos: usize, e: &Entry) -> Line {
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
            pos: pos as u64,
            kind: e.kind,
            raw_type: e.entry_type.clone(),
            subject,
            shape_ok: convention && shape_ok(&e.body),
        }
    }
}

pub fn is_retraction(kind: Kind, _raw_type: &str) -> bool {
    kind == Kind::Retraction
}

pub fn lines_of(entries: &[Entry]) -> Vec<Line> {
    entries.iter().enumerate().map(|(i, e)| Line::at(i, e)).collect()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    pub deleted: BTreeMap<String, (u64, String)>,
    pub invalid: BTreeMap<String, (u64, Invalid)>,
    pub count: usize,
}

impl Reading {
    pub fn is_deleted(&self, history_id: &str) -> bool {
        self.deleted.keys().any(|k| k.eq_ignore_ascii_case(history_id))
    }
}

pub fn read(lines: &[Line]) -> Reading {
    let mut out = Reading::default();
    let mut ordered: Vec<&Line> = lines.iter().collect();
    ordered.sort_by_key(|l| l.pos);
    for r in ordered.iter().filter(|l| is_retraction(l.kind, &l.raw_type)) {
        out.count += 1;
        let verdict = match r.subject.as_deref() {
            None => Err(Invalid::Shape),
            Some(_) if !r.shape_ok => Err(Invalid::Shape),
            Some(s) => match lines.iter().find(|x| x.id.eq_ignore_ascii_case(s)) {
                None => Err(Invalid::NotInLedger),
                Some(x) if x.kind != Kind::History => Err(Invalid::NotAWork),
                Some(x) if x.pos >= r.pos => Err(Invalid::NotInLedger),
                Some(x) if out.is_deleted(&x.id) => Err(Invalid::Repeated),
                Some(x) => Ok(x.id.clone()),
            },
        };
        match verdict {
            Ok(h) => {
                out.deleted.insert(h, (r.pos, r.id.clone()));
            }
            Err(why) => {
                out.invalid.insert(r.id.clone(), (r.pos, why));
            }
        }
    }
    out
}

pub fn read_items(items: &[Vec<u8>]) -> (Vec<Entry>, Reading) {
    let entries: Vec<Entry> = match crate::book::read(items, &crate::book::empty_fragment()) {
        Ok(l) => l.entries,
        Err(_) => items.iter().filter_map(|b| crate::book::check(b).ok()).collect(),
    };
    let reading = read(&lines_of(&entries));
    (entries, reading)
}

pub fn may_retract(lines: &[Line], subject: &str) -> Result<String, Invalid> {
    let s = subject.trim();
    if !zikaron::hexfmt::is_hex32(s) {
        return Err(Invalid::Shape);
    }
    let Some(x) = lines.iter().find(|l| l.id.eq_ignore_ascii_case(s)) else {
        return Err(Invalid::NotInLedger);
    };
    if x.kind != Kind::History {
        return Err(Invalid::NotAWork);
    }
    if read(lines).is_deleted(&x.id) {
        return Err(Invalid::Repeated);
    }
    Ok(x.id.clone())
}

pub fn work_deleted(lines: &[Line], histories: &[String]) -> bool {
    let reading = read(lines);
    !histories.is_empty() && histories.iter().all(|h| reading.is_deleted(h))
}

pub mod revocation {
    use crate::book::Kind;
    use super::Line;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Invalid {
        NotInLedger,
        NotAGrant,
    }

    impl Invalid {
        pub fn token(self) -> &'static str {
            match self {
                Invalid::NotInLedger => "E_REVOKE_NOT_IN_LEDGER",
                Invalid::NotAGrant => "E_REVOKE_NOT_A_GRANT",
            }
        }

        pub const ALL: [Invalid; 2] = [Invalid::NotInLedger, Invalid::NotAGrant];
    }

    pub fn may_revoke(lines: &[Line], grant: &str) -> Result<String, Invalid> {
        let g = grant.trim();
        let Some(x) = lines.iter().find(|l| l.id.eq_ignore_ascii_case(g)) else {
            return Err(Invalid::NotInLedger);
        };
        if x.kind != Kind::Grant {
            return Err(Invalid::NotAGrant);
        }
        Ok(x.id.clone())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn line(n: u8, kind: Kind) -> Line {
            Line { id: format!("0x{}", format!("{n:02x}").repeat(32)), pos: u64::from(n), kind, raw_type: kind.as_str().to_string(), subject: None, shape_ok: true }
        }

        #[test]
        fn only_a_grant_of_this_ledger_is_revoked() {
            let lines = vec![line(1, Kind::Genesis), line(2, Kind::Grant)];
            assert_eq!(may_revoke(&lines, &lines[1].id.to_ascii_uppercase().replace("0X", "0x")), Ok(lines[1].id.clone()));
            assert_eq!(may_revoke(&lines, &format!("0x{}", "09".repeat(32))), Err(Invalid::NotInLedger));
            assert_eq!(may_revoke(&lines, &lines[0].id), Err(Invalid::NotAGrant));
        }
    }
}

pub mod annotation {
    use super::Line;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Invalid {
        Shape,
        NotInLedger,
        NotATarget,
    }

    impl Invalid {
        pub fn token(self) -> &'static str {
            match self {
                Invalid::Shape => "E_NAME_SHAPE",
                Invalid::NotInLedger => "E_NAME_NOT_IN_LEDGER",
                Invalid::NotATarget => "E_NAME_NOT_A_TARGET",
            }
        }

        pub const ALL: [Invalid; 3] = [Invalid::Shape, Invalid::NotInLedger, Invalid::NotATarget];
    }

    pub fn may_name(lines: &[Line], subject: &str) -> Result<String, Invalid> {
        let s = subject.trim();
        if !zikaron::hexfmt::is_hex32(s) {
            return Err(Invalid::Shape);
        }
        let Some(x) = lines.iter().find(|l| l.id.eq_ignore_ascii_case(s)) else {
            return Err(Invalid::NotInLedger);
        };
        if !x.kind.may_be_named() {
            return Err(Invalid::NotATarget);
        }
        Ok(x.id.clone())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::book::Kind;

        fn line(n: u8, kind: Kind) -> Line {
            Line { id: format!("0x{}", format!("{n:02x}").repeat(32)), pos: u64::from(n), kind, raw_type: kind.as_str().to_string(), subject: None, shape_ok: true }
        }

        #[test]
        fn only_an_entry_of_this_ledger_is_named() {
            let lines = vec![line(1, Kind::Genesis), line(2, Kind::History)];
            assert_eq!(may_name(&lines, &lines[1].id), Ok(lines[1].id.clone()));
            assert_eq!(may_name(&lines, "0x12"), Err(Invalid::Shape));
            assert_eq!(may_name(&lines, &format!("0x{}", "09".repeat(32))), Err(Invalid::NotInLedger));
            let lines = vec![line(3, Kind::Adoption)];
            assert_eq!(may_name(&lines, &lines[0].id), Err(Invalid::NotATarget));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(n: u8) -> String {
        format!("0x{}", format!("{n:02x}").repeat(32))
    }

    fn line(pos: u64, kind: Kind, raw: &str, id: &str, subject: Option<&str>) -> Line {
        let convention = is_retraction(kind, raw);
        Line {
            id: id.to_string(),
            pos,
            kind,
            raw_type: raw.to_string(),
            subject: subject.map(str::to_string),
            shape_ok: convention && subject.map(zikaron::hexfmt::is_hex32).unwrap_or(false),
        }
    }

    #[test]
    fn the_production_law_refuses_every_shape_the_reading_would_call_invalid() {
        let mut lines = vec![
            line(0, Kind::Genesis, Kind::Genesis.as_str(), &h(1), None),
            line(1, Kind::History, Kind::History.as_str(), &h(2), None),
            line(2, Kind::History, Kind::History.as_str(), &h(3), None),
            line(3, Kind::Retraction, ENTRY_TYPE, &h(4), Some(&h(2))),
        ];
        assert_eq!(may_retract(&lines, "0x12"), Err(Invalid::Shape));
        assert_eq!(may_retract(&lines, ""), Err(Invalid::Shape));
        assert_eq!(may_retract(&lines, &h(9)), Err(Invalid::NotInLedger));
        assert_eq!(may_retract(&lines, &h(1)), Err(Invalid::NotAWork));
        assert_eq!(may_retract(&lines, &h(2)), Err(Invalid::Repeated));
        let upper = h(3).to_uppercase().replacen("0X", "0x", 1);
        assert_eq!(may_retract(&lines, &upper), Ok(h(3)));
        lines.push(line(4, Kind::Retraction, ENTRY_TYPE, &h(5), Some(&h(3))));
        let r = read(&lines);
        assert!(r.is_deleted(&h(3)) && r.invalid.is_empty(), "过了产出律的删除读作有效");
    }

    #[test]
    fn the_body_shape_is_closed_and_every_refusal_has_its_own_token() {
        assert!(shape_ok(&body(&h(2), "")));
        assert!(shape_ok(&body(&h(2), "写错了")));
        assert!(!shape_ok(&Value::Obj(vec![(SUBJECT.into(), Value::Str("0x12".into()))])));
        assert!(shape_ok(&Value::Obj(vec![(SUBJECT.into(), Value::Str(h(2))), ("extra".into(), Value::Int(1))])), "多余成员是数据(§6.10)");
        assert!(!shape_ok(&Value::Obj(vec![(SUBJECT.into(), Value::Str(h(2))), (NOTE_MD.into(), Value::Int(1))])));
        let mut tokens: Vec<&str> = Invalid::ALL.iter().map(|x| x.token()).collect();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), Invalid::ALL.len());
    }
}
