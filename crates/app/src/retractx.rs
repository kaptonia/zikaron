//! The "delete" reading convention, built on the open entry types of law §6.9.
//!
//! Law §6.9 makes `entryType` an open enumeration: types beyond the seven are fully verified per §4, the body
//! need only be an object, §4.3 step 12 never fails because of it, and the audit lists it under §8.7's
//! `UNKNOWN_TYPE` without changing the label. This desk adds one entry type through that opening, as a reading
//! only: readers that know it (this desk) apply the convention table; other readers list it as an unknown type
//! per the law. The core, the kit crate and the law text are unchanged.
//!
//! The convention table (type literal, body keys, body shape, reading table, production rule) lives in
//! [`zikaron_glue::retraction`], because deletes have two producers (this desk and the CLI's `zikaron retract`)
//! that must write and read the same shape. This file only maps this desk's ledger rows ([`Row`]) onto it.
//!
//! Reading:
//!
//! * Well formed and pointing at a `history` in this ledger that was not already deleted: that entry reads as
//!   "deleted".
//! * A repeated delete, a delete of a non-record, one pointing at an entry outside this ledger (including
//!   other people's ledgers), or a malformed one: the delete entry reads as invalid, with the reason. The
//!   ledger is never refused, and no error is added to §4.3.
//!
//! A deleted record is marked "deleted" in the record list and record bundle list, cannot be the subject of a
//! new grant, and leaves the anchor queue if not yet anchored. The delete entry itself is queued and anchored
//! as usual; grants already issued are not affected.

use crate::ledgerx::Row;
use zikaron::tokens::EntryType;
use zikaron_glue::retraction as convention;

pub use convention::{body, shape_ok, Invalid, Reading, ENTRY_TYPE, SUBJECT};

/// How a row of this desk's ledger table looks to the convention.
pub fn line(r: &Row) -> convention::Line {
    convention::Line {
        id: r.id.clone(),
        seq: r.seq,
        kind: r.kind,
        raw_type: r.facts.raw_type.clone(),
        subject: r.facts.subject.clone(),
        shape_ok: r.facts.shape_ok,
    }
}

fn lines(rows: &[Row]) -> Vec<convention::Line> {
    rows.iter().map(line).collect()
}

/// Whether this row is a delete-convention entry.
pub fn is_retraction(row: &Row) -> bool {
    convention::is_retraction(row.kind, &row.facts.raw_type)
}

/// Read a ledger with the convention table.
pub fn read(rows: &[Row]) -> Reading {
    convention::read(&lines(rows))
}

/// The valid delete pairs: (delete entry id, deleted entry id).
pub fn pairs(rows: &[Row]) -> Vec<(String, String)> {
    read(rows).deleted.into_iter().map(|(subject, (_, retraction))| (retraction, subject)).collect()
}

/// Production rule (from the convention table): whether this ledger can delete `subject` now. On success,
/// returns that record's id.
pub fn may_retract(rows: &[Row], subject: &str) -> Result<String, Invalid> {
    convention::may_retract(&lines(rows), subject)
}

/// Whether this record (content hash) is deleted in this ledger: it has records, and every one is deleted.
/// Having no record at all (someone else's record, a relicense's upstream) does not count as deleted.
pub fn work_deleted(rows: &[Row], work: &str) -> bool {
    let reading = read(rows);
    let mine: Vec<&Row> = rows
        .iter()
        .filter(|r| r.kind == EntryType::History && r.work.as_deref().map(|w| w.eq_ignore_ascii_case(work.trim())).unwrap_or(false))
        .collect();
    !mine.is_empty() && mine.iter().all(|r| reading.is_deleted(&r.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledgerx::{Facts, Lamp};

    fn row(seq: u64, kind: EntryType, raw: &str, id: &str, subject: Option<&str>, shape_ok: bool) -> Row {
        Row {
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

    /// A valid delete deletes its target; repeated, non-record, outside-ledger and malformed deletes read as
    /// invalid, and none of them refuses the ledger.
    #[test]
    fn the_reading_table_holds() {
        let rows = vec![
            row(0, EntryType::Genesis, EntryType::Genesis.as_str(), &h(1), None, false),
            row(1, EntryType::History, EntryType::History.as_str(), &h(2), None, false),
            row(2, EntryType::History, EntryType::History.as_str(), &h(3), None, false),
            row(3, EntryType::Other, ENTRY_TYPE, &h(4), Some(&h(2)), true),
            row(4, EntryType::Other, ENTRY_TYPE, &h(5), Some(&h(2)), true),
            row(5, EntryType::Other, ENTRY_TYPE, &h(6), Some(&h(1)), true),
            row(6, EntryType::Other, ENTRY_TYPE, &h(7), Some(&h(9)), true),
            row(7, EntryType::Other, ENTRY_TYPE, &h(8), None, false),
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
        assert!(!shape_ok(&zikaron::json::Value::Obj(vec![(SUBJECT.into(), zikaron::json::Value::Str(h(2))), ("extra".into(), zikaron::json::Value::Int(1))])));
    }
}
