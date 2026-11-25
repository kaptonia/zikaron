use crate::ledgerx::Row;
use zikaron_glue::book::Kind as EntryType;
use zikaron_glue::retraction as convention;

pub use convention::{body, shape_ok, Invalid, Reading, ENTRY_TYPE, SUBJECT};

pub fn line(r: &Row) -> convention::Line {
    convention::Line {
        id: r.id.clone(),
        pos: r.pos(),
        kind: r.kind,
        raw_type: r.facts.raw_type.clone(),
        subject: r.facts.subject.clone(),
        shape_ok: r.facts.shape_ok,
    }
}

pub fn lines(rows: &[Row]) -> Vec<convention::Line> {
    rows.iter().map(line).collect()
}

pub fn is_retraction(row: &Row) -> bool {
    convention::is_retraction(row.kind, &row.facts.raw_type)
}

pub fn old_retractions(rows: &[Row]) -> usize {
    rows.iter().filter(|r| r.law == zikaron_glue::book::Law::One && is_retraction(r)).count()
}

pub fn read(rows: &[Row]) -> Reading {
    convention::read(&lines(rows))
}

pub fn deleted_by(rows: &[Row], reading: &Reading, id: &str) -> Option<u64> {
    let (_, (pos, by)) = reading.deleted.iter().find(|(k, _)| k.eq_ignore_ascii_case(id.trim()))?;
    Some(rows.iter().find(|r| r.id.eq_ignore_ascii_case(by)).map(|r| r.seq).unwrap_or(*pos & ((1u64 << 62) - 1)))
}

pub fn pairs(rows: &[Row]) -> Vec<(String, String)> {
    read(rows).deleted.into_iter().map(|(subject, (_, retraction))| (retraction, subject)).collect()
}

pub fn may_retract(rows: &[Row], subject: &str) -> Result<String, Invalid> {
    convention::may_retract(&lines(rows), subject)
}

pub fn work_deleted(rows: &[Row], work: &str) -> bool {
    let histories: Vec<String> = rows.iter().filter(|r| r.kind == EntryType::History && r.work.as_deref().is_some_and(|w| w.eq_ignore_ascii_case(work.trim()))).map(|r| r.id.clone()).collect();
    convention::work_deleted(&lines(rows), &histories)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledgerx::{Facts, Lamp};

    fn row(seq: u64, kind: EntryType, raw: &str, id: &str, subject: Option<&str>, shape_ok: bool) -> Row {
        Row {
            law: zikaron_glue::book::Law::Two,
            seq,
            kind,
            id: id.to_string(),
            prev: None,
            author: String::new(),
            summary: String::new(),
            lamp: Lamp::Landed,
            tx: None,
            bytes: 0,
            work: None,
            facts: Facts { subject: subject.map(str::to_string), shape_ok, raw_type: raw.to_string(), ..Facts::default() },
            anchored_at: None,
        }
    }

    fn h(n: u8) -> String {
        format!("0x{}", format!("{n:02x}").repeat(32))
    }

    #[test]
    fn a_deletion_is_named_by_the_deleting_entrys_seq() {
        let rows = vec![
            row(0, EntryType::Genesis, EntryType::Genesis.as_str(), &h(1), None, false),
            row(1, EntryType::History, EntryType::History.as_str(), &h(2), None, false),
            row(7, EntryType::Retraction, ENTRY_TYPE, &h(4), Some(&h(2)), true),
        ];
        let reading = read(&rows);
        assert_eq!(reading.deleted.get(&h(2)).map(|x| x.0), Some((1u64 << 62) + 7), "the reading orders by the one list");
        assert_eq!(deleted_by(&rows, &reading, &h(2)), Some(7), "the page names #7");
        assert_eq!(deleted_by(&rows, &reading, &h(4)), None, "an entry not deleted names none");
    }

    #[test]
    fn the_reading_table_holds() {
        let rows = vec![
            row(0, EntryType::Genesis, EntryType::Genesis.as_str(), &h(1), None, false),
            row(1, EntryType::History, EntryType::History.as_str(), &h(2), None, false),
            row(2, EntryType::History, EntryType::History.as_str(), &h(3), None, false),
            row(3, EntryType::Retraction, ENTRY_TYPE, &h(4), Some(&h(2)), true),
            row(4, EntryType::Retraction, ENTRY_TYPE, &h(5), Some(&h(2)), true),
            row(5, EntryType::Retraction, ENTRY_TYPE, &h(6), Some(&h(1)), true),
            row(6, EntryType::Retraction, ENTRY_TYPE, &h(7), Some(&h(9)), true),
            row(7, EntryType::Retraction, ENTRY_TYPE, &h(8), None, false),
            row(8, EntryType::Other, "someone-elses-type", &h(10), Some(&h(3)), true),
        ];
        let r = read(&rows);
        assert_eq!(r.count, 5);
        assert!(r.is_deleted(&h(2)));
        assert!(!r.is_deleted(&h(3)), "别的类型不是约定条目");
        assert_eq!(r.invalid.get(&h(5)).map(|x| x.1), Some(Invalid::Repeated));
        assert_eq!(r.invalid.get(&h(6)).map(|x| x.1), Some(Invalid::NotAWork));
        assert_eq!(r.invalid.get(&h(7)).map(|x| x.1), Some(Invalid::NotInLedger));
        assert_eq!(r.invalid.get(&h(8)).map(|x| x.1), Some(Invalid::Shape));
        assert_eq!(may_retract(&rows, &h(2)), Err(Invalid::Repeated), "产出律与读法同一张表");
        assert_eq!(may_retract(&rows, &h(3)), Ok(h(3)));
        let mut rows = rows;
        rows[1].work = Some(h(20));
        rows[2].work = Some(h(21));
        assert!(work_deleted(&rows, &h(20)), "唯一一条存证删了即记录已删除");
        assert!(!work_deleted(&rows, &h(21)), "没删的记录不算");
        assert!(!work_deleted(&rows, &h(22)), "本账里没有的记录不算删除");
    }

    #[test]
    fn the_body_shape_is_closed() {
        assert!(shape_ok(&body(&h(2), "")));
        assert!(shape_ok(&body(&h(2), "写错了")));
        assert!(!shape_ok(&zikaron::json::Value::Obj(vec![(SUBJECT.into(), zikaron::json::Value::Str("0x12".into()))])));
        assert!(shape_ok(&zikaron::json::Value::Obj(vec![(SUBJECT.into(), zikaron::json::Value::Str(h(2))), ("extra".into(), zikaron::json::Value::Int(1))])), "a member the law does not name passes (law §6.10)");
    }
}
