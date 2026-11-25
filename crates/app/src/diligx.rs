use crate::fault::{Fault, Known};
use zikaron::json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Succession {
    pub seq: u64,
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub effective: u64,
}

fn text<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).and_then(|(_, x)| match x {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        }),
        _ => None,
    }
}

fn int(v: &Value, k: &str) -> Option<u64> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).and_then(|(_, x)| match x {
            Value::Int(n) => Some(*n),
            _ => None,
        }),
        _ => None,
    }
}

pub fn successions(ledger: &zikaron_glue::book::Ledger) -> Vec<Succession> {
    ledger
        .successions()
        .map(|e| Succession {
            seq: e.seq,
            id: zikaron::hexfmt::encode(&e.id),
            from: e.author.clone(),
            to: text(&e.body, "to").unwrap_or("").to_string(),
            kind: text(&e.body, "kind").unwrap_or("").to_string(),
            effective: int(&e.body, "effective").unwrap_or(0),
        })
        .collect()
}

pub fn successions_in(bytes: &[Vec<u8>]) -> Vec<Succession> {
    zikaron_glue::book::read(bytes, &zikaron_glue::book::empty_fragment()).map(|r| successions(&r)).unwrap_or_default()
}

pub fn double_sale(
    rows: &[crate::grantx::Row],
    work: &str,
    window: Option<(u64, u64)>,
) -> Vec<crate::grantx::Row> {
    overlapping(rows, work, window).filter(|r| !r.revoked && r.undecided.is_none()).cloned().collect()
}

pub fn double_sale_undecided(rows: &[crate::grantx::Row], work: &str, window: Option<(u64, u64)>) -> Vec<crate::grantx::Row> {
    overlapping(rows, work, window).filter(|r| !r.revoked && r.undecided.is_some()).cloned().collect()
}

fn overlapping<'a>(rows: &'a [crate::grantx::Row], work: &str, window: Option<(u64, u64)>) -> impl Iterator<Item = &'a crate::grantx::Row> {
    let want = work.trim().to_ascii_lowercase();
    rows.iter().filter(move |r| window.is_some() && !want.is_empty() && r.work.to_ascii_lowercase() == want && crate::grantx::overlaps(r.window, window))
}

#[derive(Clone, Debug)]
pub struct Read {
    pub who: String,
    pub anchors: usize,
    pub asked: usize,
    pub entries: usize,
    pub label: String,
    pub timeline: Vec<crate::ledgerx::Row>,
    pub grants: Vec<crate::grantx::Row>,
    pub work: String,
    pub window: Option<(u64, u64)>,
    pub clash: Vec<crate::grantx::Row>,
    pub undecided: Vec<crate::grantx::Row>,
    pub successions: Vec<Succession>,
    pub lineage: Vec<String>,
    pub now: Option<u64>,
    pub latest: Option<u64>,
    pub from: Option<(crate::supplyx::Level, String)>,
    pub files: Option<usize>,
    pub misses: Vec<(crate::supplyx::Level, crate::fault::Fault)>,
    pub missed: Vec<crate::widex::Missed>,
    pub pile: Vec<Vec<u8>>,
    pub audit_key: crate::sendx::behind::Key,
}

impl Read {
    pub fn only_anchors(&self) -> bool {
        self.entries == 0
    }

    pub fn double_sold(&self) -> bool {
        !self.clash.is_empty()
    }

    pub fn double_sale_undecided(&self) -> bool {
        self.clash.is_empty() && !self.undecided.is_empty()
    }
}

pub fn assemble(
    book: crate::readerx::Book,
    fragment: &Value,
    bytes: &[Vec<u8>],
    work: &str,
    window: Option<(u64, u64)>,
) -> Result<Read, Fault> {
    crate::trace::mark(crate::feature::Feature::D1);
    let work = work.trim().to_string();
    if !work.is_empty() && !zikaron::hexfmt::is_hex32(&work) {
        return Err(Fault::known(Known::ContentShape, work));
    }
    let read = zikaron_glue::book::read(bytes, fragment).ok();
    let clash = double_sale(&book.grants, &work, window);
    let undecided = double_sale_undecided(&book.grants, &work, window);
    Ok(Read {
        pile: Vec::new(),
        audit_key: crate::sendx::behind::Key([0; 32]),
        missed: Vec::new(),
        latest: book.latest,
        who: book.who,
        anchors: book.anchors,
        asked: book.asked,
        entries: book.entries,
        label: book.label,
        timeline: book.timeline,
        grants: book.grants,
        work,
        window,
        clash,
        undecided,
        successions: read.as_ref().map(successions).unwrap_or_default(),
        lineage: read.map(|r| r.lineage.into_iter().collect()).unwrap_or_default(),
        now: None,
        from: None,
        files: None,
        misses: Vec::new(),
    })
}

pub const EVIDENCE_WEIGHT: &str = "evidenceWeight";

pub const EVIDENCE_WEIGHT_NONE: &str = "none";

pub fn snapshot_value(r: &Read) -> Value {
    let s = |x: &str| Value::Str(x.to_string());
    let grants = r
        .grants
        .iter()
        .map(|g| {
            let mut o: Vec<(String, Value)> = vec![
                ("grantee".into(), s(&g.grantee)),
                ("id".into(), s(&g.id)),
                ("revoked".into(), if g.undecided.is_some() { Value::Null } else { Value::Bool(g.revoked) }),
                ("seq".into(), Value::Int(g.seq)),
                ("terms".into(), s(&g.terms)),
                (
                    "window".into(),
                    match g.window {
                        Some((a, b)) => Value::Obj(vec![("from".into(), Value::Int(a)), ("to".into(), Value::Int(b))]),
                        None => Value::Null,
                    },
                ),
                ("work".into(), s(&g.work)),
            ];
            if let Some(u) = &g.undecided {
                let mut why: Vec<(String, Value)> = vec![("cause".into(), s(u.code()))];
                if let Some(c) = u.chain() {
                    why.push(("chain".into(), Value::Int(c)));
                }
                o.push(("undecided".into(), Value::Obj(why)));
                o.sort_by(|a, b| a.0.cmp(&b.0));
            }
            Value::Obj(o)
        })
        .collect();
    let succ = r
        .successions
        .iter()
        .map(|x| {
            Value::Obj(vec![
                ("effective".into(), Value::Int(x.effective)),
                ("from".into(), s(&x.from)),
                ("id".into(), s(&x.id)),
                ("kind".into(), s(&x.kind)),
                ("seq".into(), Value::Int(x.seq)),
                ("to".into(), s(&x.to)),
            ])
        })
        .collect();
    let mut top: Vec<(String, Value)> = vec![
        ("anchors".into(), Value::Int(r.anchors as u64)),
        ("asked".into(), Value::Int(r.asked as u64)),
        ("clash".into(), Value::Arr(r.clash.iter().map(|c| s(&c.id)).collect())),
        ("entriesSeen".into(), Value::Int(r.entries as u64)),
        (EVIDENCE_WEIGHT.into(), s(EVIDENCE_WEIGHT_NONE)),
        ("grants".into(), Value::Arr(grants)),
        ("label".into(), s(&r.label)),
        ("lineage".into(), Value::Arr(r.lineage.iter().map(|x| s(x)).collect())),
        ("recomputeFrom".into(), s("chain")),
        ("successions".into(), Value::Arr(succ)),
        ("who".into(), s(&r.who)),
        (
            "window".into(),
            match r.window {
                Some((a, b)) => Value::Obj(vec![("from".into(), Value::Int(a)), ("to".into(), Value::Int(b))]),
                None => Value::Null,
            },
        ),
        ("work".into(), s(&r.work)),
    ];
    if !r.undecided.is_empty() {
        top.push(("clashUndecided".into(), Value::Arr(r.undecided.iter().map(|c| s(&c.id)).collect())));
        top.sort_by(|a, b| a.0.cmp(&b.0));
    }
    Value::Obj(top)
}

pub fn snapshot(r: &Read, to: &std::path::Path) -> Result<usize, Fault> {
    crate::trace::mark(crate::feature::Feature::D1);
    if to.as_os_str().is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail108).to_string())));
    }
    let bytes = zikaron::json::canon_bytes(&snapshot_value(r));
    zikaron_glue::landing::land_bytes(to, &bytes).map_err(|t| {
        Fault::of_landing(t)
    })?;
    Ok(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grantx::{Row, Undecided};

    fn row(id: u8, revoked: bool, undecided: Option<Undecided>) -> Row {
        Row {
            id: format!("0x{}", format!("{id:02x}").repeat(32)),
            seq: id as u64,
            grantee: format!("0x{}", "33".repeat(20)),
            work: format!("0x{}", "44".repeat(32)),
            terms: format!("0x{}", "55".repeat(32)),
            window: None,
            exclusive: false,
            exclusive_from: crate::termsx::Exclusive::No,
            doc: None,
            doc_name: None,
            revoked,
            unnoticed: false,
            member: true,
            superseded: false,
            undecided,
        }
    }

    fn read(grants: Vec<Row>) -> Read {
        let book = crate::readerx::Book { who: format!("0x{}", "66".repeat(20)), anchors: 0, asked: 1, entries: grants.len(), label: String::new(), timeline: Vec::new(), grants, latest: None };
        assemble(book, &zikaron_glue::book::empty_fragment(), &[], &format!("0x{}", "44".repeat(32)), Some((1, 2))).expect("assembled")
    }

    #[test]
    fn an_undecided_withdrawal_is_neither_clash_nor_clear_and_the_snapshot_says_so() {
        let unread = Undecided::Unread(11_155_111, "Notices not read 11155111: x".into());
        let r = read(vec![row(1, true, None), row(2, false, Some(unread)), row(3, false, None)]);
        assert_eq!(r.clash.iter().map(|g| g.seq).collect::<Vec<_>>(), vec![3], "the decided live grant clashes");
        assert_eq!(r.undecided.iter().map(|g| g.seq).collect::<Vec<_>>(), vec![2], "the undecided one is named apart");
        assert!(r.double_sold() && !r.double_sale_undecided());
        let only = read(vec![row(1, true, None), row(2, false, Some(Undecided::Uncounted(11_155_111)))]);
        assert!(!only.double_sold() && only.double_sale_undecided(), "neither red nor green");
        let snap = snapshot_value(&only);
        let g2 = snap.member("grants").and_then(Value::as_arr).and_then(|a| a.iter().find(|x| x.member("seq").and_then(Value::as_int) == Some(2))).cloned().expect("row");
        assert_eq!(g2.member("revoked"), Some(&Value::Null));
        assert_eq!(g2.member("undecided").and_then(|u| u.member("cause")).and_then(Value::as_str), Some("uncounted"));
        assert_eq!(g2.member("undecided").and_then(|u| u.member("chain")).and_then(Value::as_int), Some(11_155_111));
        assert_eq!(snap.member("clashUndecided").and_then(Value::as_arr).map(Vec::len), Some(1));
        let plain = snapshot_value(&read(vec![row(1, true, None), row(3, false, None)]));
        assert!(plain.member("clashUndecided").is_none(), "no undecided grant: the snapshot's form as before");
        assert!(plain.member("grants").and_then(Value::as_arr).is_some_and(|a| a.iter().all(|x| x.member("undecided").is_none() && matches!(x.member("revoked"), Some(Value::Bool(_))))));
        assert_eq!(row(2, false, Some(Undecided::NotApplying(None))).badge(Some(1)), crate::grantx::Badge::Unknown, "no reading, never live");
    }
}
