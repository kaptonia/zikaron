use crate::face::{shape_of, Consent, ConsentUnread, Named, Shape, Table, Unwritten, Writer};
use zikaron::hexfmt;
use zikaron::json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Barred {
    Unwritten(Unwritten),
    Unread(String),
    Rotating { office: String, key: String },
    NoHome { home: Option<u64>, ledger: Option<u64>, unsent: bool },
    HomeUnread { entry: String, chain: u64, read_to: Option<u64> },
    HomeChain { outside: bool, chain: u64 },
    Row(String),
    Consent(String),
    Address(String),
    Subject(String),
    NoEntry(String),
    Sealed { to: String },
}

impl Barred {
    pub fn code(&self) -> &'static str {
        match self {
            Barred::Unwritten(Unwritten::NoRoot) => "NO_GENESIS",
            Barred::Unwritten(_) => "FACE_UNWRITABLE",
            Barred::Unread(_) => "FACE_UNREAD",
            Barred::Rotating { .. } => "FACE_ROTATING",
            Barred::NoHome { .. } => "FACE_NO_HOME",
            Barred::HomeUnread { .. } => "FACE_UNREAD",
            Barred::HomeChain { .. } => "FACE_HOME_CHAIN",
            Barred::Row(_) => "FACE_ROW",
            Barred::Consent(_) => "CONSENT_REFUSED",
            Barred::Address(_) => "ADDRESS_SHAPE",
            Barred::Subject(_) => "CONTENT_SHAPE",
            Barred::NoEntry(_) => "SUBJECT_MISSING",
            Barred::Sealed { .. } => "FACE_SEALED",
        }
    }

    pub const CODES: [&'static str; 12] = [
        "NO_GENESIS",
        "FACE_UNWRITABLE",
        "FACE_UNREAD",
        "FACE_ROTATING",
        "FACE_NO_HOME",
        "FACE_HOME_CHAIN",
        "FACE_ROW",
        "CONSENT_REFUSED",
        "ADDRESS_SHAPE",
        "CONTENT_SHAPE",
        "SUBJECT_MISSING",
        "FACE_SEALED",
    ];

    pub fn detail(&self) -> String {
        let c = |c: &char| format!("U+{:04X}", *c as u32);
        match self {
            Barred::Unwritten(u) => match u {
                Unwritten::NoRoot => String::new(),
                Unwritten::Label { member } => format!("{member} label"),
                Unwritten::HandleChar(x) => format!("handle {}", c(x)),
                Unwritten::HandleEnds => "handle ends".to_string(),
                Unwritten::HandleLength(n) => format!("handle length {n}"),
                Unwritten::NoteLength(n) => format!("note length {n} > {}", crate::face::NOTE_BYTES),
                Unwritten::NameLength(n) => format!("name length {n}"),
                Unwritten::NameBarred(x) => format!("name {}", c(x)),
                Unwritten::Shape => "shape".to_string(),
                Unwritten::SameRoot { member } => format!("{member} root"),
            },
            Barred::Unread(w) | Barred::Row(w) | Barred::Consent(w) | Barred::Address(w) | Barred::Subject(w) | Barred::NoEntry(w) => w.clone(),
            Barred::Rotating { office, key } => format!("office={office} key={key}"),
            Barred::NoHome { home, ledger, .. } => {
                let chain = |c: &Option<u64>| c.map(|c| c.to_string()).unwrap_or_else(|| "none".to_string());
                format!("home {} ledger {}", chain(home), chain(ledger))
            }
            Barred::HomeChain { outside: true, chain } => format!("outside {chain}"),
            Barred::HomeChain { outside: false, chain } => format!("stands {chain}"),
            Barred::HomeUnread { entry, chain, read_to } => format!("home {entry} chain {chain} readTo {}", read_to.map(|b| b.to_string()).unwrap_or_else(|| "none".to_string())),
            Barred::Sealed { to } => format!("to {to}"),
        }
    }

    pub fn unanswered(&self) -> bool {
        matches!(self, Barred::HomeUnread { .. })
    }
}

impl From<Unwritten> for Barred {
    fn from(u: Unwritten) -> Barred {
        Barred::Unwritten(u)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerFace {
    pub id: String,
    pub body: Vec<u8>,
    pub seq: u64,
    pub home: bool,
}

pub fn ledger_faces(items: &[Vec<u8>]) -> Vec<LedgerFace> {
    let mut out: Vec<LedgerFace> = items
        .iter()
        .filter_map(|b| crate::book::check(b).ok())
        .filter(|e| e.kind == crate::book::Kind::Face)
        .map(|e| LedgerFace { id: e.id_hex().to_ascii_lowercase(), body: zikaron::json::canon_bytes(&e.body), seq: e.seq, home: shape_of(&e.body) == Some(Shape::Home) })
        .collect();
    out.sort_by_key(|f| f.seq);
    out
}

pub fn last_home(items: &[Vec<u8>]) -> Option<u64> {
    last_home_entry(items).map(|(_, c)| c)
}

pub fn last_home_entry(items: &[Vec<u8>]) -> Option<(String, u64)> {
    let mut homes: Vec<(u64, u64, String)> = items
        .iter()
        .filter_map(|b| crate::book::check(b).ok())
        .filter(|e| e.kind == crate::book::Kind::Face && shape_of(&e.body) == Some(Shape::Home))
        .filter_map(|e| Some((e.seq, e.body.member("chain")?.as_int()?, e.id_hex().to_ascii_lowercase())))
        .collect();
    homes.sort();
    homes.pop().map(|(_, c, id)| (id, c))
}

pub fn read_in(t: &Table, entry: &str) -> bool {
    seated_ids(t).contains(&entry.to_ascii_lowercase()) || t.misread.iter().any(|(id, _)| id.eq_ignore_ascii_case(entry))
}

pub fn written_reroot(items: &[Vec<u8>]) -> Option<(String, String)> {
    written_move(items, Shape::Reroot, "to")
}

fn written_move(items: &[Vec<u8>], shape: Shape, member: &str) -> Option<(String, String)> {
    items
        .iter()
        .filter_map(|b| crate::book::check(b).ok())
        .filter(|e| e.kind == crate::book::Kind::Face && shape_of(&e.body) == Some(shape))
        .max_by_key(|e| e.seq)
        .and_then(|e| Some((e.id_hex().to_ascii_lowercase(), e.body.member(member)?.as_str()?.to_ascii_lowercase())))
}

pub fn seated_ids(t: &Table) -> std::collections::BTreeSet<String> {
    let mut out: std::collections::BTreeSet<String> = t.homes.iter().map(|o| o.entry.to_ascii_lowercase()).collect();
    out.extend(t.reroot.iter().chain(t.origin.iter()).map(|l| l.entry.to_ascii_lowercase()));
    for sp in t.faces.iter().flat_map(|r| r.spans.iter()) {
        out.insert(sp.entry.to_ascii_lowercase());
        if let Some(r) = &sp.release {
            out.insert(r.to_ascii_lowercase());
        }
    }
    out
}

pub fn unseated(faces: &[LedgerFace], table: Option<&Table>) -> Option<usize> {
    if faces.is_empty() {
        return Some(0);
    }
    let t = table?;
    let ids = seated_ids(t);
    let bodies: std::collections::BTreeSet<&Vec<u8>> = faces.iter().filter(|f| ids.contains(&f.id)).map(|f| &f.body).collect();
    let last_seated_home = faces.iter().filter(|f| f.home && ids.contains(&f.id)).map(|f| f.seq).max();
    Some(faces.iter().filter(|f| !bodies.contains(&f.body)).filter(|f| !(f.home && last_seated_home.is_some_and(|s| f.seq < s))).count())
}

#[derive(Clone, Debug)]
pub struct Ready {
    pub items: Vec<Vec<u8>>,
    pub writer: Writer,
    pub table: Table,
    pub sent: Vec<String>,
}

pub fn ready(items: Vec<Vec<u8>>, table: Result<Table, String>, key: &str) -> Result<Ready, Barred> {
    let writer = Writer::of(&items)?;
    let table = table.map_err(Barred::Unread)?;
    if !table.root.eq_ignore_ascii_case(writer.root()) {
        return Err(Barred::Unread(format!("root {}", table.root)));
    }
    sealed(&table)?;
    if let Some((_, to)) = written_reroot(&items) {
        return Err(Barred::Sealed { to });
    }
    if !table.office.eq_ignore_ascii_case(key) {
        return Err(Barred::Rotating { office: table.office.clone(), key: key.to_ascii_lowercase() });
    }
    Ok(Ready { items, writer, table, sent: Vec::new() })
}

pub fn sealed(table: &Table) -> Result<(), Barred> {
    match &table.reroot {
        Some(l) => Err(Barred::Sealed { to: l.other.to_ascii_lowercase() }),
        None => Ok(()),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Consented {
    Have(Consent),
    SignHere { address: [u8; 20], role: String },
}

fn h20(v: &str) -> Option<[u8; 20]> {
    hexfmt::decode(v.trim())?.try_into().ok()
}

impl Ready {
    fn root(&self) -> Result<[u8; 20], Barred> {
        h20(self.writer.root()).ok_or(Barred::Unwritten(Unwritten::NoRoot))
    }

    pub fn with_sent(mut self, sent: Vec<String>) -> Ready {
        self.sent = sent.iter().map(|s| format!("0x{}", s.trim().trim_start_matches("0x").to_ascii_lowercase())).collect();
        self
    }

    pub fn home(&self) -> Result<u64, Barred> {
        let last = last_home_entry(&self.items);
        let ledger = last.as_ref().map(|(_, c)| *c);
        match (self.table.home, last) {
            (Some(h), _) if ledger == Some(h) => Ok(h),
            (home, Some((entry, chain))) if !read_in(&self.table, &entry) => match self.sent.contains(&entry) {
                true => Err(Barred::HomeUnread { read_to: self.table.basis.iter().find(|b| b.chain == chain).map(|b| b.to_block), entry, chain }),
                false => Err(Barred::NoHome { home, ledger, unsent: true }),
            },
            (home, _) => Err(Barred::NoHome { home, ledger, unsent: false }),
        }
    }

    fn bound(&self, face: &Named) -> bool {
        self.table.faces.iter().any(|r| r.named == *face && r.bound)
    }

    pub fn opening(&self, chain: u64, basis: &[u64]) -> Result<Value, Barred> {
        if !basis.contains(&chain) {
            return Err(Barred::HomeChain { outside: true, chain });
        }
        if self.table.home == Some(chain) && last_home(&self.items) == Some(chain) {
            return Err(Barred::HomeChain { outside: false, chain });
        }
        Ok(self.writer.home(chain)?)
    }

    pub fn persona(&self, name: &str) -> Result<Value, Barred> {
        self.home()?;
        Ok(self.writer.persona(name)?)
    }

    pub fn account(&self, platform: &str, handle: &str, note: &str) -> Result<Value, Barred> {
        self.home()?;
        let body = self.writer.account(platform, handle, Some(note))?;
        if self.bound(&Named::Account { platform: platform.to_string(), handle: handle.to_string() }) {
            return Err(Barred::Row(format!("bound account {platform} {handle}")));
        }
        Ok(body)
    }

    pub fn consent(&self, address: &str, role: &str, given: &str) -> Result<Consented, Barred> {
        let refused = |w: &str| Barred::Consent(w.to_string());
        let root = self.root()?;
        let parse = |text: &str| match Consent::parse(text) {
            Ok(c) => Ok(c),
            Err(ConsentUnread::Prefix) => Err(refused("PREFIX")),
            Err(ConsentUnread::Form) => Err(refused("FORM")),
        };
        let (address, role, given) = if address.trim().starts_with(crate::face::CONSENT_PREFIX) {
            let c = parse(address)?;
            if !role.trim().is_empty() && role.trim() != c.role {
                return Err(refused("FORM role"));
            }
            (c.address, c.role.clone(), Some(c))
        } else {
            let a = h20(address).filter(|_| address.trim().len() == 42).ok_or_else(|| Barred::Address(address.trim().to_string()))?;
            let role = role.trim().to_string();
            let given = match given.trim() {
                "" => None,
                c if c.starts_with(crate::face::CONSENT_PREFIX) => Some(parse(c)?),
                sig => {
                    let sig: [u8; 65] = (sig.len() == 132).then_some(()).and_then(|_| hexfmt::decode(sig)).and_then(|b| b.try_into().ok()).ok_or_else(|| refused("FORM sig"))?;
                    Some(Consent { root, address: a, role: role.clone(), sig })
                }
            };
            (a, role, given)
        };
        match given.or_else(|| held_consent(&self.items, &root, &address, &role)) {
            Some(c) => {
                let c = self.holding(c)?;
                if c.address != address || c.role != role {
                    return Err(refused("SIGNER"));
                }
                Ok(Consented::Have(c))
            }
            None => Ok(Consented::SignHere { address, role }),
        }
    }

    pub fn holding(&self, c: Consent) -> Result<Consent, Barred> {
        if c.root != self.root()? {
            return Err(Barred::Consent("ROOT".to_string()));
        }
        if !c.holds() {
            return Err(Barred::Consent("SIGNER".to_string()));
        }
        Ok(c)
    }

    pub fn key(&self, c: &Consent, note: &str) -> Result<Value, Barred> {
        self.home()?;
        let face = Named::Key { address: hexfmt::encode(&c.address), role: c.role.clone() };
        if self.bound(&face) {
            return Err(Barred::Row(format!("bound key {} {}", hexfmt::encode(&c.address), c.role)));
        }
        Ok(self.writer.key(&c.address, &c.role, &c.sig, Some(note))?)
    }

    pub fn reroot(&self, to: &str, note: &str) -> Result<Value, Barred> {
        self.home()?;
        let a = h20(to).filter(|_| to.trim().len() == 42).ok_or_else(|| Barred::Address(to.trim().to_string()))?;
        Ok(self.writer.reroot(&a, Some(note))?)
    }

    pub fn origin(&self, from: &str, note: &str) -> Result<Value, Barred> {
        self.home()?;
        let a = h20(from).filter(|_| from.trim().len() == 42).ok_or_else(|| Barred::Address(from.trim().to_string()))?;
        if let Some(o) = &self.table.origin {
            return Err(Barred::Row(format!("origin {}", o.entry)));
        }
        if let Some((id, _)) = written_move(&self.items, Shape::Origin, "from") {
            return Err(Barred::Row(format!("origin {id}")));
        }
        Ok(self.writer.origin(&a, Some(note))?)
    }

    pub fn release(&self, subject: &str) -> Result<Value, Barred> {
        self.home()?;
        let s: [u8; 32] = hexfmt::decode(subject.trim()).and_then(|b| b.try_into().ok()).ok_or_else(|| Barred::Subject(subject.trim().to_string()))?;
        let id = hexfmt::encode(&s);
        let open = self.table.faces.iter().flat_map(|row| row.spans.iter()).any(|sp| sp.entry.eq_ignore_ascii_case(&id) && sp.release.is_none());
        if !open {
            return Err(Barred::Row(format!("not bound {id}")));
        }
        Ok(self.writer.release(&s, None)?)
    }

    pub fn reseat(&self, entry: &str, queued: impl Fn(&str) -> bool) -> Result<Reseat, Barred> {
        let id = entry.trim().to_ascii_lowercase();
        let e = self.items.iter().filter_map(|b| crate::book::check(b).ok()).find(|e| e.id_hex().eq_ignore_ascii_case(&id) && e.kind == crate::book::Kind::Face).ok_or_else(|| Barred::NoEntry(id.clone()))?;
        let ids = seated_ids(&self.table);
        let faces = ledger_faces(&self.items);
        let body = zikaron::json::canon_bytes(&e.body);
        if ids.contains(&id) || faces.iter().any(|f| ids.contains(&f.id) && f.body == body) {
            return Err(Barred::Row(format!("seated {id}")));
        }
        if e.author.eq_ignore_ascii_case(&self.table.office) && queued(&e.id_hex()) {
            return Err(Barred::Row(format!("queued {id}")));
        }
        if shape_of(&e.body) == Some(Shape::Home) {
            return Ok(Reseat::Home { chain: e.body.member("chain").and_then(Value::as_int).unwrap_or_default(), id: e.id_hex() });
        }
        self.home()?;
        let id = e.id_hex();
        Ok(Reseat::Body { body: e.body, id })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Reseat {
    Home { chain: u64, id: String },
    Body { body: Value, id: String },
}

pub fn held_consent(items: &[Vec<u8>], root: &[u8; 20], address: &[u8; 20], role: &str) -> Option<Consent> {
    items
        .iter()
        .filter_map(|b| crate::book::check(b).ok())
        .filter(|e| e.kind == crate::book::Kind::Face && shape_of(&e.body) == Some(Shape::Key))
        .filter_map(|e| {
            let t = |k: &str| e.body.member(k).and_then(Value::as_str).map(str::to_string);
            let h = |k: &str| t(k).and_then(|x| hexfmt::decode(&x));
            Some(Consent { root: h("root")?.try_into().ok()?, address: h("address")?.try_into().ok()?, role: t("role")?, sig: h("sig")?.try_into().ok()? })
        })
        .find(|c| c.root == *root && c.address == *address && c.role == role && c.holds())
}

pub fn lineage_of(items: &[Vec<u8>]) -> Vec<[u8; 20]> {
    crate::book::read(items, &crate::book::empty_fragment()).map(|r| r.lineage.iter().filter_map(|k| h20(k)).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::face::{At, Basis, Opening, Row, Span};

    const KEY: [u8; 32] = [0x42; 32];

    fn me() -> [u8; 20] {
        zikaron::cryptox::address_of_privkey(&KEY).expect("a key")
    }

    fn seal(seq: u64, prev: Option<&str>, entry_type: &str, body: Value) -> Vec<u8> {
        let sealed = crate::book::seal::<()>(crate::book::Law::Two, &hexfmt::encode(&me()), entry_type, seq, prev, body, |pre, domain| {
            let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
            let (r, s, v) = zikaron::cryptox::sign_digest(&KEY, &digest).ok_or(())?;
            let mut raw = r.to_vec();
            raw.extend_from_slice(&s);
            raw.push(v);
            Ok(hexfmt::encode(&raw))
        });
        match sealed {
            crate::book::Sealed::Done(e) => e.bytes,
            _ => panic!("sealed"),
        }
    }

    fn ledger(bodies: &[Value]) -> Vec<Vec<u8>> {
        let mut out = vec![seal(0, None, "genesis", Value::Obj(vec![("statement_md".into(), Value::Str("s".into()))]))];
        for (i, b) in bodies.iter().enumerate() {
            let prev = hexfmt::encode(&zikaron::cryptox::sha256(out.last().expect("one")));
            out.push(seal(i as u64 + 1, Some(&prev), crate::face::ENTRY_TYPE, b.clone()));
        }
        out
    }

    fn id(b: &[u8]) -> String {
        hexfmt::encode(&zikaron::cryptox::sha256(b))
    }

    fn at() -> At {
        At { chain: 1, block: 1, time: 1, tx: format!("0x{}", "00".repeat(32)) }
    }

    fn table(items: &[Vec<u8>], home: Option<u64>, opened: &[&[u8]]) -> Table {
        let root = crate::book::root_of(items).expect("a root");
        Table {
            root: root.clone(),
            marker: format!("zikaron:{root}"),
            basis: Vec::new(),
            home,
            office: hexfmt::encode(&me()),
            persona: None,
            reroot: None,
            origin: None,
            lineage: Vec::new(),
            homes: opened.iter().map(|b| Opening { entry: id(b), chain: home.unwrap_or(0), block: 1, time: 1, tx: at().tx, sender: hexfmt::encode(&me()) }).collect(),
            faces: Vec::new(),
            misread: Vec::new(),
            unread: 0,
            unproven: 0,
            value: Value::Null,
        }
    }

    fn home_body(items: &[Vec<u8>], chain: u64) -> Value {
        Writer::of(items).expect("a writer").home(chain).expect("a body")
    }

    #[test]
    fn the_home_that_stands_is_opened_again_when_the_ledger_names_another() {
        let g = ledger(&[]);
        let first = home_body(&g, 11_155_111);
        let one = ledger(&[first.clone()]);
        let t = table(&one, Some(11_155_111), &[&one[1]]);
        let r = ready(one.clone(), Ok(t), &hexfmt::encode(&me())).expect("ready");
        assert_eq!(r.opening(11_155_111, &[11_155_111, 10]), Err(Barred::HomeChain { outside: false, chain: 11_155_111 }), "the ledger names the home that stands");
        assert_eq!(r.opening(31_337, &[11_155_111, 10]), Err(Barred::HomeChain { outside: true, chain: 31_337 }));
        assert!(r.home().is_ok());
        let stray = home_body(&one, 10);
        let two = ledger(&[first, stray]);
        let t = table(&two, Some(11_155_111), &[&two[1]]);
        let r = ready(two, Ok(t), &hexfmt::encode(&me())).expect("ready");
        assert_eq!(r.home(), Err(Barred::NoHome { home: Some(11_155_111), ledger: Some(10), unsent: true }), "bindings wait for the ledger to name the home");
        assert!(r.opening(11_155_111, &[11_155_111, 10]).is_ok(), "the standing home is opened again");
        assert!(r.opening(10, &[11_155_111, 10]).is_ok(), "the chain the stray entry named, still gated by its book");
    }

    #[test]
    fn a_home_entry_not_seated_is_told_by_where_it_stands() {
        let g = ledger(&[]);
        let one = ledger(&[home_body(&g, 11_155_111)]);
        let entry = id(&one[1]);
        let mut t = table(&one, None, &[]);
        t.basis = vec![Basis { chain: 11_155_111, from_block: 0, to_block: 41, registries: Vec::new(), book: [0; 20], book_from_block: 0 }];
        let key = hexfmt::encode(&me());
        let unsent = ready(one.clone(), Ok(t.clone()), &key).expect("ready");
        assert_eq!(unsent.home(), Err(Barred::NoHome { home: None, ledger: Some(11_155_111), unsent: true }), "never sent: no home");
        let sent = ready(one.clone(), Ok(t.clone()), &key).expect("ready").with_sent(vec![entry.trim_start_matches("0x").to_ascii_uppercase()]);
        let b = sent.home().expect_err("not read yet");
        assert_eq!(b, Barred::HomeUnread { entry: entry.clone(), chain: 11_155_111, read_to: Some(41) });
        assert_eq!((b.code(), b.unanswered(), b.detail()), ("FACE_UNREAD", true, format!("home {entry} chain 11155111 readTo 41")));
        let mut misread = t;
        misread.misread = vec![(entry.clone(), "UNSEATED".to_string())];
        let read = ready(one, Ok(misread), &key).expect("ready").with_sent(vec![entry]);
        assert_eq!(read.home(), Err(Barred::NoHome { home: None, ledger: Some(11_155_111), unsent: false }), "read and not seated: no home");
        assert!(!Barred::NoHome { home: None, ledger: None, unsent: true }.unanswered());
    }

    #[test]
    fn a_home_entry_a_later_seated_one_stands_in_for_is_owed_nothing() {
        let g = ledger(&[]);
        let a = home_body(&g, 11_155_111);
        let b = home_body(&g, 10);
        let again = home_body(&g, 11_155_111);
        let name = Writer::of(&g).expect("w").persona("名字").expect("p");
        let items = ledger(&[a.clone(), b.clone(), again]);
        let faces = ledger_faces(&items);
        let t = table(&items, Some(11_155_111), &[&items[3]]);
        assert_eq!(unseated(&faces, Some(&t)), Some(0), "the stray one is stood in for by the later seated one");
        let items = ledger(&[a, b, name]);
        let faces = ledger_faces(&items);
        let t = table(&items, Some(11_155_111), &[&items[1]]);
        assert_eq!(unseated(&faces, Some(&t)), Some(2), "the stray home last of the homes, and the name never seated, are owed");
        assert_eq!(unseated(&faces, None), None);
        assert_eq!(unseated(&[], None), Some(0));
    }

    #[test]
    fn every_write_asks_the_table_the_key_and_the_home_first() {
        let g = ledger(&[]);
        let a = home_body(&g, 1);
        let items = ledger(&[a]);
        let key = hexfmt::encode(&me());
        assert_eq!(ready(items.clone(), Err("unread".into()), &key).err().map(|b| b.code()), Some("FACE_UNREAD"));
        let mut other = table(&items, Some(1), &[&items[1]]);
        other.root = hexfmt::encode(&[0x77; 20]);
        assert_eq!(ready(items.clone(), Ok(other), &key).err().map(|b| b.code()), Some("FACE_UNREAD"));
        assert_eq!(ready(items.clone(), Ok(table(&items, Some(1), &[&items[1]])), &hexfmt::encode(&[0x55; 20])).err().map(|b| b.code()), Some("FACE_ROTATING"));
        let mut t = table(&items, Some(1), &[&items[1]]);
        t.faces.push(Row { named: Named::Account { platform: "x".into(), handle: "alice".into() }, bound: true, spans: vec![Span { entry: "0xab".into(), since: at(), release: None, until: None }] });
        let r = ready(items.clone(), Ok(t), &key).expect("ready");
        assert_eq!(r.account("x", "alice", "").err().map(|b| b.code()), Some("FACE_ROW"));
        assert_eq!(r.release(&format!("0x{}", "cd".repeat(32))).err().map(|b| b.detail()), Some(format!("not bound 0x{}", "cd".repeat(32))));
        assert_eq!(r.release("nope").err().map(|b| b.code()), Some("CONTENT_SHAPE"));
        let barred = r.persona("林\u{200D}夏").err().expect("barred");
        assert_eq!((barred.code(), barred.detail()), ("FACE_UNWRITABLE", "name U+200D".to_string()));
        assert!(r.persona("名字").is_ok());
        let r = ready(items.clone(), Ok(table(&items, None, &[])), &key).expect("ready");
        assert_eq!(r.persona("名字").err().map(|b| b.code()), Some("FACE_NO_HOME"));
        for b in [barred, Barred::Rotating { office: "o".into(), key: "k".into() }, Barred::NoHome { home: None, ledger: None, unsent: false }] {
            assert!(Barred::CODES.contains(&b.code()));
        }
    }

    #[test]
    fn no_home_says_what_the_ledger_and_the_table_name() {
        assert_eq!(Barred::NoHome { home: None, ledger: Some(10), unsent: true }.detail(), "home none ledger 10");
        assert_eq!(Barred::NoHome { home: Some(11_155_111), ledger: Some(10), unsent: false }.detail(), "home 11155111 ledger 10");
        assert_eq!(Barred::NoHome { home: None, ledger: None, unsent: false }.detail(), "home none ledger none");
    }

    #[test]
    fn a_consent_is_given_held_or_left_to_sign_here() {
        let g = ledger(&[]);
        let items = ledger(&[home_body(&g, 1)]);
        let key = hexfmt::encode(&me());
        let r = ready(items.clone(), Ok(table(&items, Some(1), &[&items[1]])), &key).expect("ready");
        let root: [u8; 20] = h20(r.writer.root()).expect("root");
        let far = [0x24u8; 32];
        let fa = zikaron::cryptox::address_of_privkey(&far).expect("a");
        let sig = crate::face::consent::sign(&far, &hexfmt::encode(&root), &hexfmt::encode(&fa), "judge").expect("sig");
        let c = Consent { root, address: fa, role: "judge".into(), sig };
        assert_eq!(r.consent(&c.text(), "", ""), Ok(Consented::Have(c.clone())));
        assert_eq!(r.consent(&hexfmt::encode(&fa), "judge", &hexfmt::encode(&sig)), Ok(Consented::Have(c.clone())));
        assert_eq!(r.consent(&hexfmt::encode(&fa), "judge", &c.text()), Ok(Consented::Have(c.clone())));
        assert_eq!(r.consent(&hexfmt::encode(&fa), "judge", ""), Ok(Consented::SignHere { address: fa, role: "judge".into() }));
        assert_eq!(r.consent(&hexfmt::encode(&fa), "buyer", &c.text()), Err(Barred::Consent("SIGNER".into())));
        let other = crate::face::consent::sign(&far, &hexfmt::encode(&[0x77; 20]), &hexfmt::encode(&fa), "judge").expect("sig");
        assert_eq!(r.consent(&Consent { root: [0x77; 20], address: fa, role: "judge".into(), sig: other }.text(), "", ""), Err(Barred::Consent("ROOT".into())));
        assert_eq!(r.consent("zikaron-consent:0x11", "", ""), Err(Barred::Consent("FORM".into())));
        assert_eq!(r.consent("0x12", "judge", ""), Err(Barred::Address("0x12".into())));
        let body = r.key(&c, "").expect("a key body");
        let items = ledger(&[home_body(&g, 1), body]);
        let r = ready(items.clone(), Ok(table(&items, Some(1), &[&items[1]])), &key).expect("ready");
        assert_eq!(r.consent(&hexfmt::encode(&fa), "judge", ""), Ok(Consented::Have(c)));
    }

    #[test]
    fn a_reroot_and_an_origin_take_the_home_and_a_sealed_seat_writes_nothing() {
        let g = ledger(&[]);
        let items = ledger(&[home_body(&g, 1)]);
        let key = hexfmt::encode(&me());
        let root = crate::book::root_of(&items).expect("root");
        let other = hexfmt::encode(&[0x66; 20]);
        let r = ready(items.clone(), Ok(table(&items, Some(1), &[&items[1]])), &key).expect("ready");
        let body = r.reroot(&other, "").expect("a reroot");
        assert_eq!(shape_of(&body), Some(Shape::Reroot));
        assert_eq!(shape_of(&r.origin(&other, "n").expect("an origin")), Some(Shape::Origin));
        let same = r.reroot(&root, "").err().expect("refused");
        assert_eq!((same.code(), same.detail()), ("FACE_UNWRITABLE", "to root".to_string()));
        assert_eq!(r.origin(&root, "").err().map(|b| b.detail()), Some("from root".to_string()));
        assert_eq!(r.reroot("0x12", "").err().map(|b| b.code()), Some("ADDRESS_SHAPE"));
        let second = hexfmt::encode(&[0x67; 20]);
        let rerooted = ledger(&[home_body(&g, 1), body.clone()]);
        let gate = ready(rerooted.clone(), Ok(table(&rerooted, Some(1), &[&rerooted[1]])), &key).err().map(|b| (b.code(), b.detail()));
        assert_eq!(gate, Some(("FACE_SEALED", format!("to {other}"))), "a reroot written closes the gate");
        let originated = ledger(&[home_body(&g, 1), r.origin(&other, "").expect("an origin")]);
        let ro = ready(originated.clone(), Ok(table(&originated, Some(1), &[&originated[1]])), &key).expect("ready");
        assert_eq!(ro.origin(&second, "").err().map(|b| (b.code(), b.detail().starts_with("origin 0x"))), Some(("FACE_ROW", true)), "a second origin");
        let r0 = ready(items.clone(), Ok(table(&items, None, &[])), &key).expect("ready");
        assert_eq!(r0.reroot(&other, "").err().map(|b| b.code()), Some("FACE_NO_HOME"), "a seat without a home makes neither");
        assert_eq!(r0.origin(&other, "").err().map(|b| b.code()), Some("FACE_NO_HOME"));
        let link = |e: &str| crate::face::Link { entry: e.to_string(), other: other.clone(), since: at() };
        let mut t = table(&items, Some(1), &[&items[1]]);
        t.origin = Some(link("0xaa"));
        let r = ready(items.clone(), Ok(t.clone()), &key).expect("ready");
        assert_eq!(r.origin(&other, "").err().map(|b| b.detail()), Some("origin 0xaa".to_string()));
        t.reroot = Some(link("0xbb"));
        let sealed_by = |t: Table, k: &str| ready(items.clone(), Ok(t), k).err().map(|b| (b.code(), b.detail()));
        assert_eq!(sealed_by(t.clone(), &key), Some(("FACE_SEALED", format!("to {other}"))));
        assert_eq!(sealed_by(t.clone(), &hexfmt::encode(&[0x55; 20])), Some(("FACE_SEALED", format!("to {other}"))), "the seal is asked before the key");
        assert_eq!(Barred::CODES.last(), Some(&"FACE_SEALED"));
        let ids = seated_ids(&t);
        assert!(ids.contains("0xaa") && ids.contains("0xbb"));
    }
}
