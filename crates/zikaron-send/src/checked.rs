use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};
use zikaron_anchor::scan::{Fact, FactKey, Known, Sightings};

pub const DIR: &str = "checked";
pub const FILE: &str = "facts.json";
pub const LOGS: &str = "logs.json";

pub fn logs_file(part: &str) -> String {
    format!("logs-{part}.json")
}

pub fn logs_part(file: &str) -> Option<&str> {
    file.strip_prefix("logs-")?.strip_suffix(".json")
}

pub fn audit_file(key: &crate::behind::Key) -> String {
    format!("audit-{}.json", key.hex())
}

pub fn is_logs(file: &str) -> bool {
    file == LOGS || logs_part(file).is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}
pub const FORM: &str = "zikaron.checked-facts/2";
pub const FORM_1: &str = "zikaron.checked-facts/1";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Held {
    pub agreed: Known,
    pub single: Known,
    pub proved: std::collections::BTreeSet<FactKey>,
    pub keys: std::collections::BTreeMap<[u8; 20], [u8; 65]>,
}

pub trait FailSaid {
    fn said(&self) -> String;
    fn locked(&self) -> bool {
        false
    }
}

impl FailSaid for zikaron_seal::Trouble {
    fn said(&self) -> String {
        format!("{:?} {}", self.code, self.tail)
    }
    fn locked(&self) -> bool {
        self.code == zikaron_seal::Code::Locked
    }
}

pub fn unkept_of<F: FailSaid>(f: &F) -> zikaron_anchor::settled::Unkept {
    if f.locked() { zikaron_anchor::settled::Unkept::Locked } else { zikaron_anchor::settled::Unkept::Said(f.said()) }
}

impl FailSaid for () {
    fn said(&self) -> String {
        String::new()
    }
}

pub trait Room: Send + Sync {
    type Fail: FailSaid;
    fn read(&self, file: &str) -> Option<Vec<u8>>;
    fn put(&self, file: &str, plain: &[u8]) -> Result<(), Self::Fail>;
    fn hold(&self, _file: &str) -> Option<zikaron_os::LockFile> {
        None
    }
    fn public(&self) -> bool {
        false
    }
    fn files(&self) -> Vec<String> {
        Vec::new()
    }
    fn remove(&self, file: &str) {
        let _ = file;
    }
}

pub fn files_in(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_file())).map(|e| e.file_name().to_string_lossy().into_owned()).collect()
}

pub fn forget_logs_in(dir: &Path) -> std::io::Result<bool> {
    let mut had = false;
    for file in files_in(dir).into_iter().filter(|f| is_logs(f)) {
        match std::fs::remove_file(dir.join(&file)) {
            Ok(()) => had = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(had)
}

pub fn hold_at(dir: &Path, file: &str) -> Option<zikaron_os::LockFile> {
    std::fs::create_dir_all(dir).ok()?;
    let l = zikaron_os::LockFile::open(zikaron_os::owner_only(zikaron_os::Options::new().read(true).write(true).create(true)), &dir.join(format!("{file}.lock"))).ok()?;
    zikaron_os::lock_wait(l.file()).then_some(l)
}

pub fn rel(file: &str) -> String {
    format!("{DIR}/{file}")
}

pub struct Sealed(pub PathBuf);

impl Room for Sealed {
    type Fail = zikaron_seal::Trouble;

    fn read(&self, file: &str) -> Option<Vec<u8>> {
        let raw = std::fs::read(self.0.join(DIR).join(file)).ok()?;
        let key = zikaron_seal::keybox::local_key().ok()?;
        let names = zikaron_seal::keybox::name_key().ok();
        let ex = zikaron_seal::envelope::Expect { owner: Some(zikaron_seal::envelope::Owner::Machine), doc: zikaron_seal::envelope::Doc::Checked, rel: Some(rel(file)) };
        zikaron_seal::envelope::open_with(&key, &ex, &raw, &rel(file), names.as_ref()).ok()
    }

    fn put(&self, file: &str, plain: &[u8]) -> Result<(), zikaron_seal::Trouble> {
        let key = zikaron_seal::keybox::local_key()?;
        let id = zikaron_seal::envelope::Ident { owner: zikaron_seal::envelope::Owner::Machine, doc: zikaron_seal::envelope::Doc::Checked, logical: rel(file) };
        let sealed = zikaron_seal::envelope::seal_checked(&key, &id, plain)?;
        zikaron_seal::land::put_at(&self.0.join(DIR), file, &sealed)
    }

    fn hold(&self, file: &str) -> Option<zikaron_os::LockFile> {
        hold_at(&self.0.join(DIR), file)
    }

    fn files(&self) -> Vec<String> {
        files_in(&self.0.join(DIR))
    }

    fn remove(&self, file: &str) {
        let _ = std::fs::remove_file(self.0.join(DIR).join(file));
    }
}

pub const PLAIN_DIR: &str = "cli-read";

pub struct Plain(pub PathBuf);

impl Plain {
    pub fn dir(&self) -> PathBuf {
        self.0.join(PLAIN_DIR)
    }
}

impl Room for Plain {
    type Fail = zikaron_seal::Trouble;

    fn read(&self, file: &str) -> Option<Vec<u8>> {
        std::fs::read(self.dir().join(file)).ok()
    }

    fn put(&self, file: &str, plain: &[u8]) -> Result<(), zikaron_seal::Trouble> {
        zikaron_seal::land::put_at(&self.dir(), file, plain)
    }

    fn hold(&self, file: &str) -> Option<zikaron_os::LockFile> {
        hold_at(&self.dir(), file)
    }

    fn public(&self) -> bool {
        true
    }

    fn files(&self) -> Vec<String> {
        files_in(&self.dir())
    }

    fn remove(&self, file: &str) {
        let _ = std::fs::remove_file(self.dir().join(file));
    }
}

impl Sealed {
    pub fn machine(&self) -> &Path {
        &self.0
    }
}

pub fn facts<R: Room + ?Sized>(room: &R) -> Known {
    zikaron_anchor::action::facts("agreed", || on_disk(room).agreed)
}

pub fn facts_proved<R: Room + ?Sized>(room: &R) -> Known {
    zikaron_anchor::action::facts("proved", || {
        let h = on_disk(room);
        Known(h.single.0.into_iter().filter(|(k, _)| h.proved.contains(k)).collect(), Default::default())
    })
}

pub fn facts_single<R: Room + ?Sized>(room: &R) -> Known {
    zikaron_anchor::action::facts("single", || {
        let h = on_disk(room);
        let mut all = h.single;
        all.0.extend(h.agreed.0);
        all
    })
}

fn on_disk<R: Room + ?Sized>(room: &R) -> Held {
    room.read(FILE).and_then(|b| parse(&b)).unwrap_or_default()
}

pub static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn add<R: Room + ?Sized>(room: &R, new: &[(FactKey, Fact)]) -> Result<usize, R::Fail> {
    put_in(room, new, true)
}

pub fn add_single<R: Room + ?Sized>(room: &R, new: &[(FactKey, Fact)]) -> Result<usize, R::Fail> {
    put_in(room, new, false)
}

pub fn key_of<R: Room + ?Sized>(room: &R, address: &[u8; 20]) -> Option<[u8; 65]> {
    on_disk(room).keys.get(address).copied().filter(|k| zikaron::cryptox::address_of_pubkey(k) == *address)
}

pub fn add_key<R: Room + ?Sized>(room: &R, address: &[u8; 20], key: &[u8; 65]) -> Result<bool, R::Fail> {
    if zikaron::cryptox::address_of_pubkey(key) != *address {
        return Ok(false);
    }
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let _lock = room.hold(FILE);
    let mut held = on_disk(room);
    if held.keys.get(address) == Some(key) {
        return Ok(false);
    }
    held.keys.insert(*address, *key);
    room.put(FILE, &render(&held))?;
    Ok(true)
}

fn put_in<R: Room + ?Sized>(room: &R, new: &[(FactKey, Fact)], agreed: bool) -> Result<usize, R::Fail> {
    if new.is_empty() {
        return Ok(0);
    }
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let _lock = room.hold(FILE);
    let mut held = on_disk(room);
    let was = held.clone();
    for (k, f) in new {
        if agreed {
            held.single.0.remove(k);
            held.proved.remove(k);
            held.agreed.0.insert(*k, *f);
        } else if !held.agreed.0.contains_key(k) {
            held.single.0.insert(*k, *f);
            if zikaron_anchor::ahead::fact_proved(k) {
                held.proved.insert(*k);
            } else {
                held.proved.remove(k);
            }
        }
    }
    let count = |h: &Held| h.agreed.0.len() + h.single.0.len();
    let added = count(&held) - count(&was);
    if held != was {
        room.put(FILE, &render(&held))?;
    }
    zikaron_anchor::action::facts_now();
    Ok(added)
}

pub fn render(h: &Held) -> Vec<u8> {
    let facts: Vec<Value> = h
        .agreed
        .0
        .iter()
        .map(|(k, f)| (k, f, "agreed"))
        .chain(h.single.0.iter().map(|(k, f)| (k, f, "single")))
        .map(|(key, f, source)| {
            let (chain, block, tx, index) = key;
            let mut members = vec![
                ("block".into(), Value::Str(zikaron::hexfmt::encode(block))),
                ("blockNumber".into(), Value::Int(f.block_number)),
                ("chainId".into(), Value::Int(*chain)),
                ("emitter".into(), Value::Str(zikaron::hexfmt::encode(&f.emitter))),
                ("hash".into(), Value::Str(zikaron::hexfmt::encode(&f.hash))),
                ("index".into(), Value::Int(*index)),
            ];
            if source == "single" && h.proved.contains(key) {
                members.push(("proved".into(), Value::Bool(true)));
            }
            members.extend([
                ("sender".into(), Value::Str(zikaron::hexfmt::encode(&f.sender))),
                ("source".into(), Value::Str(source.into())),
                ("timestamp".into(), Value::Int(f.block_timestamp)),
                ("tx".into(), Value::Str(zikaron::hexfmt::encode(tx))),
                ("verdict".into(), Value::Str(f.verdict.as_str().into())),
            ]);
            Value::Obj(members)
        })
        .collect();
    let mut members = vec![("facts".into(), Value::Arr(facts)), ("form".into(), Value::Str(FORM.into()))];
    if !h.keys.is_empty() {
        let keys = h.keys.iter().map(|(a, k)| Value::Obj(vec![("address".into(), Value::Str(zikaron::hexfmt::encode(a))), ("key".into(), Value::Str(zikaron::hexfmt::encode(k)))])).collect();
        members.push(("keys".into(), Value::Arr(keys)));
    }
    json::canon_bytes(&Value::Obj(members))
}

pub fn parse(b: &[u8]) -> Option<Held> {
    let v = json::parse(b).ok()?;
    let first = match v.member("form").and_then(|f| f.as_str()) {
        Some(FORM) => false,
        Some(FORM_1) => true,
        _ => return None,
    };
    let Some(Value::Arr(facts)) = v.member("facts") else { return None };
    let h32 = |x: Option<&Value>| -> Option<[u8; 32]> { x.and_then(|s| s.as_str()).and_then(zikaron::hexfmt::decode).and_then(|b| b.try_into().ok()) };
    let h20 = |x: Option<&Value>| -> Option<[u8; 20]> { x.and_then(|s| s.as_str()).and_then(zikaron::hexfmt::decode).and_then(|b| b.try_into().ok()) };
    let int = |x: Option<&Value>| match x {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    };
    let mut out = Held::default();
    for f in facts {
        let agreed = match (first, f.member("source").and_then(|s| s.as_str())) {
            (true, _) | (false, Some("agreed")) => true,
            (false, Some("single")) => false,
            _ => return None,
        };
        let verdict = f.member("verdict").and_then(|s| s.as_str()).and_then(zikaron::tokens::Verdict::parse)?;
        if verdict == zikaron::tokens::Verdict::Unproven {
            return None;
        }
        let key = (int(f.member("chainId"))?, h32(f.member("block"))?, h32(f.member("tx"))?, int(f.member("index"))?);
        match f.member("proved") {
            None => {}
            Some(Value::Bool(true)) if !agreed => {
                out.proved.insert(key);
            }
            Some(Value::Bool(_)) => {}
            Some(_) => return None,
        }
        let into = if agreed { &mut out.agreed } else { &mut out.single };
        into.0.insert(
            key,
            Fact {
                block_number: int(f.member("blockNumber"))?,
                sender: h20(f.member("sender"))?,
                hash: h32(f.member("hash"))?,
                emitter: h20(f.member("emitter"))?,
                block_timestamp: int(f.member("timestamp"))?,
                verdict,
            },
        );
    }
    match v.member("keys") {
        None => {}
        Some(Value::Arr(keys)) => {
            for k in keys {
                let key: [u8; 65] = k.member("key").and_then(|s| s.as_str()).and_then(zikaron::hexfmt::decode).and_then(|b| b.try_into().ok())?;
                out.keys.insert(h20(k.member("address"))?, key);
            }
        }
        Some(_) => return None,
    }
    let single = std::mem::take(&mut out.single);
    out.single.0 = single.0.into_iter().filter(|(k, _)| !out.agreed.0.contains_key(k)).collect();
    let proved = std::mem::take(&mut out.proved);
    out.proved = proved.into_iter().filter(|k| out.single.0.contains_key(k)).collect();
    Some(out)
}

pub fn agreed(runs: &[(Vec<u64>, Sightings)], thin: &[u64]) -> Vec<(FactKey, Fact)> {
    let Some((_, first)) = runs.first() else { return Vec::new() };
    first
        .iter()
        .filter(|(k, f)| {
            let chain = k.0;
            !thin.contains(&chain)
                && runs.iter().filter(|(chains, _)| chains.contains(&chain)).count() >= 2
                && runs.iter().filter(|(chains, _)| chains.contains(&chain)).all(|(_, seen)| seen.iter().any(|(k2, f2)| k2 == k && f2 == f))
        })
        .cloned()
        .collect()
}

pub fn single(runs: &[(Vec<u64>, Sightings)], thin: &[u64]) -> Vec<(FactKey, Fact)> {
    thin.iter()
        .filter_map(|c| {
            let mut read = runs.iter().filter(|(chains, _)| chains.contains(c));
            match (read.next(), read.next()) {
                (Some((_, seen)), None) => Some(seen.iter().filter(|(k, _)| k.0 == *c).cloned().collect::<Vec<_>>()),
                _ => None,
            }
        })
        .flatten()
        .collect()
}

pub struct Record<R: Room>(pub R);

impl<R: Room> crate::Facts for Record<R> {
    fn known(&self) -> Known {
        Known(facts(&self.0).0, facts_proved(&self.0).0)
    }

    fn known_single(&self) -> Known {
        facts_single(&self.0)
    }

    fn add(&self, runs: &[(Vec<u64>, Sightings)], thin: &[u64]) {
        let _ = add(&self.0, &agreed(runs, thin));
        let _ = add_single(&self.0, &single(runs, thin));
    }
}

pub const NOTICED: &str = "noticed.json";
pub const NOTICED_FORM: &str = "zikaron.noticed/1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Notice {
    pub grant: [u8; 32],
    pub chain: u64,
    pub tx: [u8; 32],
    pub block: u64,
}

pub fn noticed<R: Room + ?Sized>(room: &R) -> Vec<Notice> {
    room.read(NOTICED).and_then(|b| parse_noticed(&b)).unwrap_or_default()
}

pub fn add_noticed<R: Room + ?Sized>(room: &R, n: Notice) -> Result<bool, R::Fail> {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let _lock = room.hold(NOTICED);
    let mut all = noticed(room);
    if all.iter().any(|x| x.grant == n.grant && x.chain == n.chain) {
        return Ok(false);
    }
    all.push(n);
    all.sort_by_key(|x| (x.grant, x.chain));
    room.put(NOTICED, &render_noticed(&all))?;
    Ok(true)
}

fn render_noticed(all: &[Notice]) -> Vec<u8> {
    let rows = all
        .iter()
        .map(|n| {
            Value::Obj(vec![
                ("block".into(), Value::Int(n.block)),
                ("chainId".into(), Value::Int(n.chain)),
                ("grant".into(), Value::Str(zikaron::hexfmt::encode(&n.grant))),
                ("tx".into(), Value::Str(zikaron::hexfmt::encode(&n.tx))),
            ])
        })
        .collect();
    json::canon_bytes(&Value::Obj(vec![("form".into(), Value::Str(NOTICED_FORM.into())), ("noticed".into(), Value::Arr(rows))]))
}

fn parse_noticed(b: &[u8]) -> Option<Vec<Notice>> {
    let v = json::parse(b).ok()?;
    if v.member("form").and_then(|f| f.as_str()) != Some(NOTICED_FORM) {
        return None;
    }
    let Some(Value::Arr(rows)) = v.member("noticed") else { return None };
    let h32 = |x: Option<&Value>| -> Option<[u8; 32]> { x.and_then(|s| s.as_str()).and_then(zikaron::hexfmt::decode).and_then(|b| b.try_into().ok()) };
    let int = |x: Option<&Value>| match x {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    };
    rows.iter().map(|r| Some(Notice { grant: h32(r.member("grant"))?, chain: int(r.member("chainId"))?, tx: h32(r.member("tx"))?, block: int(r.member("block"))? })).collect()
}

pub struct Logs<R: Room>(pub R);

impl<R: Room> zikaron_anchor::settled::Keep for Logs<R> {
    fn load(&self, part: &str) -> Option<Vec<u8>> {
        self.0.read(&logs_file(part))
    }

    fn save(&self, part: &str, plain: &[u8]) -> Result<(), zikaron_anchor::settled::Unkept> {
        self.0.put(&logs_file(part), plain).map_err(|f| unkept_of(&f))?;
        self.0.remove(LOGS);
        Ok(())
    }

    fn parts(&self) -> Vec<String> {
        self.0.files().iter().filter_map(|f| logs_part(f).map(str::to_string)).collect()
    }

    fn hold(&self) -> Option<Box<dyn std::any::Any>> {
        self.0.hold(LOGS).map(|l| Box::new(l) as Box<dyn std::any::Any>)
    }

    fn public(&self) -> bool {
        self.0.public()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zikaron::tokens::Verdict;

    fn fact(n: u64) -> Fact {
        Fact { block_number: 9, sender: [5; 20], hash: [6; 32], emitter: [7; 20], block_timestamp: n, verdict: Verdict::Counted }
    }

    #[test]
    fn only_facts_every_run_read_alike_land() {
        let k = |c: u64, i: u64| (c, [1u8; 32], [2u8; 32], i);
        let runs = vec![
            (vec![1], vec![(k(1, 0), fact(10)), (k(1, 1), fact(11)), (k(1, 2), fact(12))]),
            (vec![1], vec![(k(1, 0), fact(10)), (k(1, 1), fact(99))]),
        ];
        assert_eq!(agreed(&runs, &[]), vec![(k(1, 0), fact(10))]);
        assert!(agreed(&runs, &[1]).is_empty(), "a chain one place answered gives no fact");
        assert!(agreed(&runs[..1], &[]).is_empty(), "one run is one source");
    }

    #[test]
    fn the_record_reads_back_whole_or_not_at_all() {
        let mut h = Held::default();
        h.agreed.0.insert((1, [3; 32], [4; 32], 7), Fact { block_number: 9, sender: [5; 20], hash: [6; 32], emitter: [7; 20], block_timestamp: 5, verdict: Verdict::Void });
        h.single.0.insert((1, [3; 32], [4; 32], 8), fact(6));
        let b = render(&h);
        assert_eq!(parse(&b), Some(h.clone()));
        let t = String::from_utf8(b).unwrap();
        assert_eq!(parse(t.replace(Verdict::Void.as_str(), Verdict::Unproven.as_str()).as_bytes()), None);
        assert_eq!(parse(t.replace(FORM, "zikaron.checked-facts/3").as_bytes()), None);
        let old = t.replace(FORM, FORM_1);
        let back = parse(old.as_bytes()).expect("the form before sources");
        assert_eq!((back.agreed.0.len(), back.single.0.len()), (2, 0), "every fact it held was an agreed one");
    }

    #[test]
    fn a_single_source_fact_marked_proved_reads_back_and_is_taken_for_its_receipt_alone() {
        let (k_proved, k_word, k_agreed) = ((1, [3; 32], [4; 32], 1), (1, [3; 32], [4; 32], 2), (1, [3; 32], [4; 32], 3));
        let mut h = Held::default();
        h.single.0.insert(k_proved, fact(1));
        h.single.0.insert(k_word, fact(2));
        h.agreed.0.insert(k_agreed, fact(3));
        h.proved.insert(k_proved);
        let b = render(&h);
        assert_eq!(parse(&b), Some(h.clone()));
        let t = String::from_utf8(b).unwrap();
        assert_eq!(t.matches(r#""proved":true"#).count(), 1, "one mark: {t}");
        assert_eq!(parse(t.replace(r#""proved":true"#, r#""proved":1"#).as_bytes()), None, "a mark that is no boolean");
        let before = parse(t.replace(r#""proved":true,"#, "").as_bytes()).expect("a record before the mark");
        assert!(before.proved.is_empty() && before.single.0.len() == 2, "read unmarked");
        let on_agreed = t.replacen(r#""source":"agreed""#, r#""proved":true,"source":"agreed""#, 1);
        assert_eq!(parse(on_agreed.as_bytes()).map(|x| x.proved), Some([k_proved].into_iter().collect()), "a mark on an agreed fact is none");
        #[derive(Default)]
        struct Mem(std::sync::Mutex<Option<Vec<u8>>>);
        impl Room for Mem {
            type Fail = ();
            fn read(&self, _file: &str) -> Option<Vec<u8>> {
                self.0.lock().unwrap().clone()
            }
            fn put(&self, _file: &str, plain: &[u8]) -> Result<(), ()> {
                *self.0.lock().unwrap() = Some(plain.to_vec());
                Ok(())
            }
        }
        let room = Mem(std::sync::Mutex::new(Some(render(&h))));
        let known = crate::Facts::known(&Record(room));
        assert_eq!(known.0.keys().copied().collect::<Vec<_>>(), vec![k_agreed], "the agreed facts");
        assert_eq!(known.1.keys().copied().collect::<Vec<_>>(), vec![k_proved], "beside them the one marked proved");
        let room = Mem(std::sync::Mutex::new(Some(render(&h))));
        add(&room, &[(k_proved, fact(1))]).expect("written");
        let after = on_disk(&room);
        assert!(after.proved.is_empty() && after.agreed.0.contains_key(&k_proved), "agreed, the mark gone");
    }

    #[test]
    fn a_part_the_room_cannot_write_is_said_with_why() {
        struct ReadOnly;
        impl Room for ReadOnly {
            type Fail = zikaron_seal::Trouble;
            fn read(&self, _file: &str) -> Option<Vec<u8>> {
                None
            }
            fn put(&self, file: &str, _plain: &[u8]) -> Result<(), zikaron_seal::Trouble> {
                Err(zikaron_seal::Trouble::io(&std::io::Error::from(std::io::ErrorKind::PermissionDenied), file))
            }
        }
        let said = zikaron_anchor::settled::Keep::save(&Logs(ReadOnly), "notes", b"x").expect_err("not written");
        assert!(matches!(&said, zikaron_anchor::settled::Unkept::Said(w) if !w.is_empty()), "why it was not written: {said:?}");
    }

    #[test]
    fn a_part_not_written_for_the_store_locked_is_told_apart() {
        struct Locked;
        impl Room for Locked {
            type Fail = zikaron_seal::Trouble;
            fn read(&self, _file: &str) -> Option<Vec<u8>> {
                None
            }
            fn put(&self, _file: &str, _plain: &[u8]) -> Result<(), zikaron_seal::Trouble> {
                Err(zikaron_seal::Trouble::new(zikaron_seal::Code::Locked, String::new()))
            }
        }
        let got = zikaron_anchor::settled::Keep::save(&Logs(Locked), "notes", b"x");
        assert_eq!(got, Err(zikaron_anchor::settled::Unkept::Locked));
    }

    #[test]
    fn a_found_key_reads_back_beside_the_facts_and_one_not_its_addresss_is_none() {
        #[derive(Default)]
        struct Mem(std::sync::Mutex<std::collections::BTreeMap<String, Vec<u8>>>);
        impl Room for Mem {
            type Fail = ();
            fn read(&self, file: &str) -> Option<Vec<u8>> {
                self.0.lock().unwrap().get(file).cloned()
            }
            fn put(&self, file: &str, plain: &[u8]) -> Result<(), ()> {
                self.0.lock().unwrap().insert(file.into(), plain.to_vec());
                Ok(())
            }
        }
        let secret = [0x2a; 32];
        let digest = [0x5c; 32];
        let (r, s, recid) = zikaron::cryptox::sign_digest(&secret, &digest).expect("a signature");
        let key = zikaron::cryptox::recover_pubkey(&digest, &r, &s, recid - 27).expect("its key");
        let address = zikaron::cryptox::address_of_privkey(&secret).expect("its address");
        let room = Mem::default();
        let k = (1, [3; 32], [4; 32], 7);
        add(&room, &[(k, fact(5))]).expect("a fact");
        let bare = room.read(FILE).expect("written");
        assert!(!String::from_utf8_lossy(&bare).contains("\"keys\""), "a record without keys is written as before");
        assert_eq!(key_of(&room, &address), None);
        assert_eq!(add_key(&room, &address, &key), Ok(true));
        assert_eq!(add_key(&room, &address, &key), Ok(false), "kept once");
        assert_eq!(key_of(&room, &address), Some(key));
        assert!(facts(&room).0.contains_key(&k), "the facts kept beside it");
        add(&room, &[((1, [3; 32], [4; 32], 8), fact(6))]).expect("another fact");
        assert_eq!(key_of(&room, &address), Some(key), "a later fact keeps the key");
        let mut other = key;
        other[64] ^= 1;
        assert_eq!(add_key(&room, &[0x11; 20], &key), Ok(false), "a key not that address's own is never kept");
        let mut forged = parse(&room.read(FILE).expect("written")).expect("reads");
        forged.keys.insert(address, other);
        room.put(FILE, &render(&forged)).expect("written");
        assert_eq!(key_of(&room, &address), None, "a key on record not the address's own is no key");
        let t = String::from_utf8(render(&forged)).unwrap();
        let head = t.split("\"keys\":").next().expect("the members before the keys");
        assert_eq!(parse(format!("{head}\"keys\":\"0x\"}}").as_bytes()), None, "keys in another shape: the whole record is unread");
    }

    #[test]
    fn a_single_source_fact_serves_a_single_source_reading_only() {
        #[derive(Default)]
        struct Mem(std::sync::Mutex<std::collections::BTreeMap<String, Vec<u8>>>);
        impl Room for Mem {
            type Fail = ();
            fn read(&self, file: &str) -> Option<Vec<u8>> {
                self.0.lock().unwrap().get(file).cloned()
            }
            fn put(&self, file: &str, plain: &[u8]) -> Result<(), ()> {
                self.0.lock().unwrap().insert(file.into(), plain.to_vec());
                Ok(())
            }
        }
        let room = Mem::default();
        let k = |i: u64| (1u64, [1u8; 32], [2u8; 32], i);
        assert_eq!(add_single(&room, &[(k(0), fact(10)), (k(1), fact(11))]), Ok(2));
        assert!(facts(&room).0.is_empty(), "an agreed reading takes no single-source fact");
        assert_eq!(facts_single(&room).0.len(), 2);
        assert_eq!(add(&room, &[(k(0), fact(20))]), Ok(0), "it takes the single-source fact's place");
        assert_eq!(add_single(&room, &[(k(0), fact(30))]), Ok(0));
        assert_eq!((facts(&room).0.get(&k(0)), facts_single(&room).0.get(&k(0))), (Some(&fact(20)), Some(&fact(20))), "the agreed fact stands");
        let runs = vec![(vec![1, 2], vec![(k(5), fact(1)), ((2, [1; 32], [2; 32], 0), fact(2))]), (vec![2], vec![((2, [1; 32], [2; 32], 0), fact(2))])];
        assert_eq!(single(&runs, &[1]), vec![(k(5), fact(1))], "chain 1 read by one run alone");
        assert!(single(&runs, &[2]).is_empty(), "chain 2 read by two runs is not one node's reading");
    }
}
