use crate::keybox::Plain;
use crate::places::{credential_file_account, credential_file_for};
use crate::trouble::{random, Code, Trouble};
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};

pub mod member {
    pub const EXPIRES: &str = "expires";
    pub const ISSUED: &str = "issued";
    pub const PASSES: &str = "passes";
    pub const ALL: [&str; 3] = [EXPIRES, ISSUED, PASSES];
}

pub mod row {
    pub const ADDED: &str = "added";
    pub const MASTER: &str = "master";
    pub const NOTE: &str = "note";
    pub const ALL: [&str; 3] = [ADDED, MASTER, NOTE];
}

pub const DOMAIN: &[u8] = b"zikaron/cli-credential/2";

pub const PASS_ENV: &str = "ZIKARON_CLI_PASS";

pub const PASS_PREFIX: &str = "zkp_";

pub const PASS_BYTES: usize = 32;

pub const NOTE_MAX: usize = 40;

pub const PASSES_MAX: usize = 16;

const TAG_HEX: usize = 16;

const MASTER_BYTES: usize = crate::envelope::NONCE + 32 + 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Term {
    pub issued: u64,
    pub expires: Option<u64>,
}

impl Term {
    pub fn expired(&self, now: u64) -> bool {
        self.expires.is_some_and(|e| now >= e)
    }
}

pub struct Pass(Plain);

impl Pass {
    fn fresh() -> Result<Pass, Trouble> {
        Ok(Pass(Plain::new(random(PASS_BYTES)?)))
    }

    pub fn text(&self) -> String {
        let hex = zikaron::hexfmt::encode(self.0.bytes());
        format!("{PASS_PREFIX}{}", &hex[2..])
    }

    pub fn parse(text: &str) -> Option<Pass> {
        let body = text.strip_prefix(PASS_PREFIX)?;
        if body.len() != PASS_BYTES * 2 {
            return None;
        }
        let b = zikaron::hexfmt::decode(&format!("0x{body}"))?;
        (b.len() == PASS_BYTES).then(|| Pass(Plain::new(b)))
    }

    fn key(&self) -> Option<&[u8; 32]> {
        self.0.bytes().try_into().ok()
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Row {
    pub tag: String,
    pub note: String,
    pub added: u64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Status {
    pub term: Term,
    pub rows: Vec<Row>,
}

pub fn path(dir: &Path, account: &str) -> PathBuf {
    dir.join(credential_file_for(account))
}

pub fn issue(dir: &Path, account: &str, term: Term) -> Result<(), Trouble> {
    if term.expires.is_some_and(|e| e <= term.issued) || term.issued > json::MAX_INT || term.expires.is_some_and(|e| e > json::MAX_INT) {
        return Err(Trouble::new(Code::CredentialShape, format!("{} {:?}", term.issued, term.expires)));
    }
    write(dir, account, &Record { term, rows: Vec::new() })
}

pub fn remove(dir: &Path, account: &str) -> Result<bool, Trouble> {
    let p = path(dir, account);
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Trouble::io(&e, &p.display().to_string())),
    }
}

pub fn find(dir: &Path) -> Result<String, Trouble> {
    let listing = match std::fs::read_dir(dir) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Trouble::new(Code::CredentialMissing, dir.display().to_string())),
        Err(e) => return Err(Trouble::io(&e, &dir.display().to_string())),
    };
    let mut found: Vec<String> = Vec::new();
    for item in listing {
        let item = item.map_err(|e| Trouble::io(&e, &dir.display().to_string()))?;
        let name = item.file_name().to_string_lossy().into_owned();
        if let Some(a) = credential_file_account(&name) {
            found.push(a.to_string());
        }
    }
    found.sort();
    match found.len() {
        0 => Err(Trouble::new(Code::CredentialMissing, dir.display().to_string())),
        1 => Ok(found.remove(0)),
        _ => Err(Trouble::new(Code::CredentialShape, found.join(" "))),
    }
}

struct Kept {
    added: u64,
    master: Vec<u8>,
    note: String,
}

impl Kept {
    fn tag(&self) -> String {
        tag_of(&self.master[..crate::envelope::NONCE])
    }
}

fn tag_of(nonce: &[u8]) -> String {
    zikaron::hexfmt::encode(nonce)[2..2 + TAG_HEX].to_string()
}

struct Record {
    term: Term,
    rows: Vec<Kept>,
}

struct Lead {
    term: Term,
}

fn bytes_of(r: &Record) -> Vec<u8> {
    let rows = r
        .rows
        .iter()
        .map(|k| {
            Value::Obj(vec![
                (row::ADDED.to_string(), Value::Int(k.added)),
                (row::MASTER.to_string(), Value::Str(zikaron::hexfmt::encode(&k.master))),
                (row::NOTE.to_string(), Value::Str(k.note.clone())),
            ])
        })
        .collect();
    json::canon_bytes(&Value::Obj(vec![
        (member::EXPIRES.to_string(), r.term.expires.map(Value::Int).unwrap_or(Value::Null)),
        (member::ISSUED.to_string(), Value::Int(r.term.issued)),
        (member::PASSES.to_string(), Value::Arr(rows)),
    ]))
}

fn write(dir: &Path, account: &str, r: &Record) -> Result<(), Trouble> {
    crate::land::put_at(dir, &credential_file_for(account), &bytes_of(r))
}

fn bytes_at(dir: &Path, account: &str) -> Result<Vec<u8>, Trouble> {
    let p = path(dir, account);
    match std::fs::read(&p) {
        Ok(b) => Ok(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Trouble::new(Code::CredentialMissing, p.display().to_string())),
        Err(e) => Err(Trouble::io(&e, &p.display().to_string())),
    }
}

fn term_of(v: &Value) -> Option<Term> {
    let issued = match v.member(member::ISSUED)? {
        Value::Int(n) => *n,
        _ => return None,
    };
    let expires = match v.member(member::EXPIRES)? {
        Value::Int(n) if *n > issued => Some(*n),
        Value::Null => None,
        _ => return None,
    };
    Some(Term { issued, expires })
}

fn lead(bytes: &[u8]) -> Option<Lead> {
    json::parse(bytes).ok().as_ref().and_then(term_of).map(|term| Lead { term })
}

pub fn note_trouble(note: &str) -> Option<&'static str> {
    let n = note.chars().count();
    if n == 0 {
        return Some("EMPTY");
    }
    if n > NOTE_MAX {
        return Some("LONG");
    }
    if note.chars().any(|c| c.is_control() || c == '\u{2028}' || c == '\u{2029}') {
        return Some("LINE");
    }
    None
}

fn parsed(bytes: &[u8]) -> Result<Record, String> {
    let v = json::parse(bytes).map_err(|t| format!("{t:?}"))?;
    let Value::Obj(m) = &v else { return Err("{}".into()) };
    let names: Vec<&str> = m.iter().map(|(k, _)| k.as_str()).collect();
    if names != member::ALL {
        return Err(names.join(" "));
    }
    let term = term_of(&v).ok_or_else(|| format!("{} {}", member::ISSUED, member::EXPIRES))?;
    let Some(Value::Arr(items)) = v.member(member::PASSES) else { return Err(member::PASSES.into()) };
    if items.len() > PASSES_MAX {
        return Err(format!("{} {}", member::PASSES, items.len()));
    }
    let mut rows: Vec<Kept> = Vec::with_capacity(items.len());
    for (i, it) in items.iter().enumerate() {
        let bad = |what: &str| format!("{}[{i}] {what}", member::PASSES);
        let Value::Obj(rm) = it else { return Err(bad("{}")) };
        if rm.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>() != row::ALL {
            return Err(bad("members"));
        }
        let added = match it.member(row::ADDED) {
            Some(Value::Int(n)) => *n,
            _ => return Err(bad(row::ADDED)),
        };
        let master = it.member(row::MASTER).and_then(Value::as_str).and_then(zikaron::hexfmt::decode).filter(|b| b.len() == MASTER_BYTES).ok_or_else(|| bad(row::MASTER))?;
        let note = it.member(row::NOTE).and_then(Value::as_str).filter(|n| note_trouble(n).is_none()).ok_or_else(|| bad(row::NOTE))?.to_string();
        let kept = Kept { added, master, note };
        if rows.iter().any(|r| r.tag() == kept.tag()) {
            return Err(bad("tag"));
        }
        rows.push(kept);
    }
    Ok(Record { term, rows })
}

fn read(dir: &Path, account: &str) -> Result<Record, Trouble> {
    let bytes = bytes_at(dir, account)?;
    parsed(&bytes).map_err(|what| Trouble::new(Code::CredentialShape, format!("{}: {what}", path(dir, account).display())))
}

pub fn status(dir: &Path, account: &str) -> Result<Status, Trouble> {
    let r = read(dir, account)?;
    Ok(Status { term: r.term, rows: r.rows.iter().map(|k| Row { tag: k.tag(), note: k.note.clone(), added: k.added }).collect() })
}

pub fn add(dir: &Path, account: &str, mk: &[u8; 32], note: &str, now: u64) -> Result<(Pass, Row), Trouble> {
    let mut r = read(dir, account)?;
    if r.term.expired(now) {
        return Err(Trouble::new(Code::CredentialExpired, format!("{} {}", path(dir, account).display(), r.term.expires.unwrap_or_default())));
    }
    if let Some(why) = note_trouble(note) {
        return Err(Trouble::new(Code::CredentialNote, why));
    }
    if r.rows.len() >= PASSES_MAX {
        return Err(Trouble::new(Code::CredentialFull, PASSES_MAX.to_string()));
    }
    if now > json::MAX_INT {
        return Err(Trouble::new(Code::CredentialShape, row::ADDED));
    }
    let pass = Pass::fresh()?;
    let shape = || Trouble::new(Code::CredentialShape, row::MASTER);
    let k = pass.key().ok_or_else(shape)?;
    let nonce = loop {
        let n = random(crate::envelope::NONCE)?;
        if !r.rows.iter().any(|x| x.tag() == tag_of(&n)) {
            break n;
        }
    };
    let n: &[u8; 24] = nonce.as_slice().try_into().map_err(|_| shape())?;
    let sealed = crate::crypto::xchacha_seal(k, n, DOMAIN, mk).ok_or_else(shape)?;
    let mut master = nonce.clone();
    master.extend_from_slice(&sealed);
    let kept = Kept { added: now, master, note: note.to_string() };
    let shown = Row { tag: kept.tag(), note: kept.note.clone(), added: now };
    r.rows.push(kept);
    write(dir, account, &r)?;
    Ok((pass, shown))
}

pub fn drop_row(dir: &Path, account: &str, tag: &str) -> Result<(), Trouble> {
    let mut r = read(dir, account)?;
    let before = r.rows.len();
    r.rows.retain(|k| k.tag() != tag);
    if r.rows.len() == before {
        return Err(Trouble::new(Code::CredentialRowGone, tag.to_string()));
    }
    write(dir, account, &r)
}

pub fn open(dir: &Path, account: &str, now: u64, pass: Option<&str>) -> Result<Plain, Trouble> {
    let bytes = bytes_at(dir, account)?;
    let at = path(dir, account).display().to_string();
    if let Some(r) = lead(&bytes) {
        if r.term.expired(now) {
            remove(dir, account)?;
            let expires = r.term.expires.unwrap_or_default();
            return Err(Trouble::new(Code::CredentialExpired, format!("{at} {expires}")));
        }
    }
    let r = match parsed(&bytes) {
        Ok(r) => r,
        Err(what) => {
            remove(dir, account)?;
            return Err(Trouble::new(Code::CredentialShape, format!("{at}: {what}")));
        }
    };
    let Some(text) = pass.filter(|t| !t.is_empty()) else {
        return Err(Trouble::new(Code::CredentialPassMissing, PASS_ENV));
    };
    let rows = r.rows.len();
    let Some(k) = Pass::parse(text) else {
        return Err(Trouble::new(Code::CredentialPassUnknown, format!("FORM rows={rows}")));
    };
    let Some(key) = k.key() else {
        return Err(Trouble::new(Code::CredentialPassUnknown, format!("FORM rows={rows}")));
    };
    for kept in &r.rows {
        let (nonce, sealed) = kept.master.split_at(crate::envelope::NONCE);
        let Ok(n) = <&[u8; 24]>::try_from(nonce) else { continue };
        if let Some(mk) = crate::crypto::xchacha_open(key, n, DOMAIN, sealed).filter(|mk| mk.len() == 32) {
            return Ok(Plain::new(mk));
        }
    }
    Err(Trouble::new(Code::CredentialPassUnknown, format!("NONE rows={rows}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("zk-seal-credential-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("dir");
        d
    }

    fn code<T>(r: Result<T, Trouble>) -> Option<Code> {
        r.err().map(|t| t.code)
    }

    #[test]
    fn a_record_is_three_members_owner_only_and_holds_no_key() {
        let d = dir("closed");
        let mk = [7u8; 32];
        issue(&d, "t", Term { issued: 100, expires: Some(200) }).expect("issued");
        let (pass, row) = add(&d, "t", &mk, "tool", 150).expect("added");
        let p = path(&d, "t");
        assert!(zikaron_os::is_owner_only(&p).expect("checked"));
        let raw = std::fs::read(&p).expect("read");
        let v = json::parse(&raw).expect("json");
        let Value::Obj(m) = &v else { panic!("object") };
        assert_eq!(m.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), member::ALL);
        let rows = v.member(member::PASSES).and_then(Value::as_arr).expect("rows");
        let Value::Obj(rm) = &rows[0] else { panic!("row") };
        assert_eq!(rm.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), row::ALL);
        let text = String::from_utf8(raw.clone()).expect("utf8").to_ascii_lowercase();
        let hex = pass.text()[PASS_PREFIX.len()..].to_string();
        assert!(!text.contains(&hex) && !text.contains(PASS_PREFIX), "no key's text in the record");
        assert!(!raw.windows(PASS_BYTES).any(|w| w == pass.0.bytes()), "no key's bytes in the record");
        assert_eq!(open(&d, "t", 150, Some(&pass.text())).expect("opens").bytes(), &mk);
        assert_eq!(status(&d, "t").expect("status"), Status { term: Term { issued: 100, expires: Some(200) }, rows: vec![row] });
        assert_eq!(find(&d).expect("found"), "t");
    }

    #[test]
    fn the_record_is_judged_in_one_order() {
        let d = dir("order");
        assert_eq!(code(open(&d, "t", 1, None)), Some(Code::CredentialMissing));
        issue(&d, "t", Term { issued: 100, expires: Some(200) }).expect("issued");
        let (pass, _) = add(&d, "t", &[1u8; 32], "a", 110).expect("added");
        assert_eq!(code(open(&d, "t", 200, Some(&pass.text()))), Some(Code::CredentialExpired));
        assert!(!path(&d, "t").exists());
        issue(&d, "t", Term { issued: 100, expires: None }).expect("issued");
        assert_eq!(code(open(&d, "t", 150, None)), Some(Code::CredentialPassMissing));
        assert_eq!(open(&d, "t", 150, Some("")).err().map(|t| (t.code, t.tail)), Some((Code::CredentialPassMissing, PASS_ENV.to_string())));
        assert_eq!(open(&d, "t", 150, Some(&pass.text())).err().map(|t| (t.code, t.tail)), Some((Code::CredentialPassUnknown, "NONE rows=0".to_string())));
        let (pass, _) = add(&d, "t", &[1u8; 32], "a", 150).expect("added");
        assert_eq!(open(&d, "t", 150, Some(&pass.text())).expect("opens").bytes(), &[1u8; 32]);
        let upper = format!("{PASS_PREFIX}{}", pass.text()[PASS_PREFIX.len()..].to_ascii_uppercase());
        assert!(open(&d, "t", 150, Some(&upper)).is_ok());
    }

    #[test]
    fn a_key_never_added_deleted_or_not_in_its_form_opens_nothing_by_one_name() {
        let d = dir("unknown");
        issue(&d, "t", Term { issued: 1, expires: None }).expect("issued");
        let (kept, _) = add(&d, "t", &[2u8; 32], "kept", 2).expect("added");
        let (gone, gone_row) = add(&d, "t", &[2u8; 32], "gone", 3).expect("added");
        drop_row(&d, "t", &gone_row.tag).expect("dropped");
        let never = Pass::fresh().expect("fresh").text();
        let a = "a".repeat(64);
        for (text, why) in [
            (never, "NONE"),
            (gone.text(), "NONE"),
            (format!("zkq_{a}"), "FORM"),
            (format!("zkp_{}", "a".repeat(63)), "FORM"),
            (format!("zkp_{}", "a".repeat(65)), "FORM"),
            (format!("zkp_{}g", "a".repeat(63)), "FORM"),
            (format!(" {}", kept.text()), "FORM"),
            (format!("{}\n", kept.text()), "FORM"),
        ] {
            let t = open(&d, "t", 5, Some(&text)).err().expect("refused");
            assert_eq!((t.code, t.tail), (Code::CredentialPassUnknown, format!("{why} rows=1")), "{text:?}");
        }
        assert_eq!(open(&d, "t", 5, Some(&kept.text())).expect("the other key still opens").bytes(), &[2u8; 32]);
        assert!(path(&d, "t").exists(), "a wrong key leaves the record as it is");
    }

    #[test]
    fn deleting_one_key_leaves_the_other() {
        let d = dir("two");
        issue(&d, "t", Term { issued: 1, expires: None }).expect("issued");
        let (a, ra) = add(&d, "t", &[3u8; 32], "a", 2).expect("a");
        let (b, _) = add(&d, "t", &[3u8; 32], "b", 3).expect("b");
        drop_row(&d, "t", &ra.tag).expect("dropped");
        assert_eq!(code(open(&d, "t", 4, Some(&a.text()))), Some(Code::CredentialPassUnknown));
        assert!(open(&d, "t", 4, Some(&b.text())).is_ok());
        assert_eq!(code(drop_row(&d, "t", &ra.tag)), Some(Code::CredentialRowGone));
        assert_eq!(code(drop_row(&d, "t", "0000000000000000")), Some(Code::CredentialRowGone));
    }

    #[test]
    fn one_key_opens_wherever_it_is_brought() {
        let d = dir("shared");
        issue(&d, "t", Term { issued: 1, expires: None }).expect("issued");
        let (a, _) = add(&d, "t", &[4u8; 32], "a", 2).expect("a");
        let text = a.text();
        assert!(open(&d, "t", 3, Some(&text)).is_ok());
        assert!(open(&d, "t", 3, Some(&text)).is_ok());
    }

    #[test]
    fn a_note_is_one_short_line_and_the_keys_are_bounded() {
        let d = dir("bounds");
        assert_eq!(code(add(&d, "t", &[5u8; 32], "a", 1)), Some(Code::CredentialMissing));
        issue(&d, "t", Term { issued: 1, expires: Some(10) }).expect("issued");
        assert_eq!(code(add(&d, "t", &[5u8; 32], "a", 10)), Some(Code::CredentialExpired));
        issue(&d, "t", Term { issued: 1, expires: None }).expect("issued");
        let long = "字".repeat(NOTE_MAX + 1);
        for (note, why) in [("", "EMPTY"), (long.as_str(), "LONG"), ("a\nb", "LINE"), ("a\rb", "LINE"), ("a\tb", "LINE"), ("a\u{2028}b", "LINE")] {
            assert_eq!(add(&d, "t", &[5u8; 32], note, 2).err().map(|t| (t.code, t.tail)), Some((Code::CredentialNote, why.to_string())), "{note:?}");
        }
        assert!(add(&d, "t", &[5u8; 32], &"字".repeat(NOTE_MAX), 2).is_ok());
        for i in 1..PASSES_MAX {
            add(&d, "t", &[5u8; 32], &format!("p{i}"), 2).expect("added");
        }
        assert_eq!(status(&d, "t").expect("status").rows.len(), PASSES_MAX);
        assert_eq!(code(add(&d, "t", &[5u8; 32], "one more", 2)), Some(Code::CredentialFull));
    }

    #[test]
    fn a_record_not_in_its_shape_is_refused_and_removed() {
        let d = dir("shape");
        let p = path(&d, "t");
        let earlier = format!("{{\"expires\":null,\"issued\":1,\"key\":\"0x{}\",\"master\":\"0x{}\"}}", "11".repeat(32), "22".repeat(MASTER_BYTES));
        std::fs::write(&p, &earlier).expect("laid");
        assert_eq!(code(open(&d, "t", 5, None)), Some(Code::CredentialShape));
        assert!(!p.exists(), "removed");
        issue(&d, "t", Term { issued: 1, expires: None }).expect("issued");
        let (pass, _) = add(&d, "t", &[6u8; 32], "a", 2).expect("added");
        let text = String::from_utf8(std::fs::read(&p).expect("read")).expect("utf8");
        std::fs::write(&p, text.replacen('{', "{\"extra\":1,", 1)).expect("written");
        assert_eq!(code(open(&d, "t", 5, Some(&pass.text()))), Some(Code::CredentialShape));
        assert!(!p.exists(), "removed");
        assert_eq!(code(status(&d, "t")), Some(Code::CredentialMissing));
        issue(&d, "t", Term { issued: 1, expires: None }).expect("issued");
        let (pass, _) = add(&d, "t", &[6u8; 32], "a", 2).expect("added");
        let mut b = std::fs::read(&p).expect("read");
        let at = b.windows(8).position(|w| w == b"\"master\"").expect("master") + 80;
        b[at] = if b[at] == b'0' { b'1' } else { b'0' };
        std::fs::write(&p, b).expect("written");
        assert_eq!(code(open(&d, "t", 5, Some(&pass.text()))), Some(Code::CredentialPassUnknown));
    }

    #[test]
    fn a_reader_meanwhile_reads_a_whole_record() {
        let d = dir("whole");
        issue(&d, "t", Term { issued: 1, expires: None }).expect("issued");
        let (pass, _) = add(&d, "t", &[8u8; 32], "a", 2).expect("added");
        let text = pass.text();
        let reader = {
            let d = d.clone();
            std::thread::spawn(move || (0..200).all(|_| open(&d, "t", 3, Some(&text)).is_ok()))
        };
        for i in 0..14 {
            add(&d, "t", &[8u8; 32], &format!("n{i}"), 3).expect("added");
        }
        assert!(reader.join().expect("joined"), "every read opened");
    }

    #[test]
    fn a_record_that_never_expires_opens_at_any_time() {
        let d = dir("never");
        issue(&d, "t", Term { issued: 100, expires: None }).expect("issued");
        let (pass, _) = add(&d, "t", &[2u8; 32], "a", 100).expect("added");
        assert_eq!(open(&d, "t", u64::MAX, Some(&pass.text())).expect("opens").bytes(), &[2u8; 32]);
    }
}
