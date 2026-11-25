pub const ENTRY_TYPE: &str = zikaron_face::ENTRY_TYPE;

pub const DOMAIN: &str = zikaron_face::DOMAIN;

pub const MARKER_PREFIX: &str = zikaron_face::MARKER_PREFIX;

use zikaron::hexfmt;
use zikaron::json::Value;
pub use zikaron_face::input::NoTable;
pub use zikaron_face::{consent, names, reading};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Basis {
    pub chain: u64,
    pub from_block: u64,
    pub to_block: u64,
    pub registries: Vec<[u8; 20]>,
    pub book: [u8; 20],
    pub book_from_block: u64,
}

fn h20(a: &[u8; 20]) -> String {
    hexfmt::encode(a)
}

fn addr(v: Option<&Value>) -> Option<[u8; 20]> {
    hexfmt::decode(v?.as_str()?)?.try_into().ok()
}

fn int(v: &Value, k: &str) -> Option<u64> {
    v.member(k).and_then(Value::as_int)
}

fn text(v: &Value, k: &str) -> Option<String> {
    v.member(k).and_then(Value::as_str).map(str::to_string)
}

pub fn carried(calldata: &[u8]) -> Option<&[u8]> {
    zikaron_face::carried::carried(calldata).map(|_| &calldata[36..])
}

pub fn keys_in_reach(root: &[u8; 20], accessions: &[Value]) -> Vec<[u8; 20]> {
    let mut out = vec![*root];
    for x in accessions {
        if addr(x.member("root")) == Some(*root) {
            out.extend(addr(x.member("to")));
        }
    }
    out.sort();
    out.dedup();
    out
}

pub fn moment(head_times: &[u64]) -> Option<u64> {
    head_times.iter().min()?.checked_sub(1)
}

pub fn input(root: &[u8; 20], basis: &[Basis], anchors: &[Value], accessions: &[Value], pile: &[Vec<u8>]) -> Value {
    let mut chains: Vec<&Basis> = basis.iter().collect();
    chains.sort_by_key(|b| b.chain);
    chains.dedup_by_key(|b| b.chain);
    let of = |v: &Value| int(v, "chainId").and_then(|c| chains.iter().find(|b| b.chain == c).copied());
    let anchors: Vec<Value> = anchors
        .iter()
        .filter(|a| of(a).is_some_and(|b| int(a, "blockNumber").is_some_and(|n| (b.from_block..=b.to_block).contains(&n))))
        .cloned()
        .collect();
    let accessions: Vec<Value> = accessions
        .iter()
        .filter(|x| of(x).is_some_and(|b| int(x, "blockNumber").is_some_and(|n| (b.book_from_block..=b.to_block).contains(&n))))
        .cloned()
        .collect();
    let mut pile: Vec<String> = pile.iter().map(|b| hexfmt::encode(b)).collect();
    pile.sort();
    pile.dedup();
    let chain = |b: &Basis| {
        let mut regs = b.registries.clone();
        regs.sort();
        regs.dedup();
        Value::Obj(vec![
            ("book".into(), Value::Str(h20(&b.book))),
            ("bookFromBlock".into(), Value::Int(b.book_from_block)),
            ("chainId".into(), Value::Int(b.chain)),
            ("fromBlock".into(), Value::Int(b.from_block)),
            ("registries".into(), Value::Arr(regs.iter().map(|r| Value::Str(h20(r))).collect())),
            ("toBlock".into(), Value::Int(b.to_block)),
        ])
    };
    Value::Obj(vec![
        ("accessions".into(), Value::Arr(accessions)),
        ("anchors".into(), Value::Arr(anchors)),
        ("chains".into(), Value::Arr(chains.into_iter().map(chain).collect())),
        ("pile".into(), Value::Arr(pile.into_iter().map(Value::Str).collect())),
        ("root".into(), Value::Str(h20(root))),
    ])
}

pub fn read(input: &Value) -> Result<Table, NoTable> {
    let inp = zikaron_face::input::from_value(input)?;
    Table::of(zikaron_face::reading::table(&inp)).ok_or(NoTable)
}

#[derive(Clone, PartialEq, Eq)]
pub struct Shown(String);

const FSI: char = '\u{2068}';
const PDI: char = '\u{2069}';

impl Shown {
    pub fn lay_alone(&self) -> String {
        format!("{FSI}{}{PDI}", self.0)
    }

    pub fn copy_name(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Shown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Shown({} scalar values)", self.0.chars().count())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Persona {
    pub entry: String,
    pub name: Shown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct At {
    pub chain: u64,
    pub block: u64,
    pub time: u64,
    pub tx: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub key: String,
    pub moved: Option<Moved>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moved {
    pub chain: u64,
    pub block: u64,
    pub time: u64,
    pub tx_index: u64,
    pub log_index: u64,
    pub tx: String,
    pub holder: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opening {
    pub entry: String,
    pub chain: u64,
    pub block: u64,
    pub time: u64,
    pub tx: String,
    pub sender: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub entry: String,
    pub other: String,
    pub since: At,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Named {
    Account { platform: String, handle: String },
    Key { address: String, role: String },
    Persona(Shown),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub entry: String,
    pub since: At,
    pub release: Option<String>,
    pub until: Option<At>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub named: Named,
    pub bound: bool,
    pub spans: Vec<Span>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub root: String,
    pub marker: String,
    pub basis: Vec<Basis>,
    pub home: Option<u64>,
    pub office: String,
    pub persona: Option<Persona>,
    pub reroot: Option<Link>,
    pub origin: Option<Link>,
    pub lineage: Vec<Line>,
    pub homes: Vec<Opening>,
    pub faces: Vec<Row>,
    pub misread: Vec<(String, String)>,
    pub unread: u64,
    pub unproven: u64,
    pub value: Value,
}

fn arr<'a>(v: &'a Value, k: &str) -> Option<&'a Vec<Value>> {
    v.member(k)?.as_arr()
}

fn at_of(v: &Value) -> Option<At> {
    Some(At { chain: int(v, "chainId")?, block: int(v, "blockNumber")?, time: int(v, "blockTimestamp")?, tx: text(v, "tx")? })
}

fn basis_of(c: &Value) -> Option<Basis> {
    Some(Basis {
        chain: int(c, "chainId")?,
        from_block: int(c, "fromBlock")?,
        to_block: int(c, "toBlock")?,
        registries: arr(c, "registries")?.iter().map(|r| addr(Some(r))).collect::<Option<_>>()?,
        book: addr(c.member("book"))?,
        book_from_block: int(c, "bookFromBlock")?,
    })
}

impl Table {
    pub fn of(v: Value) -> Option<Table> {
        let persona = match v.member("persona")? {
            Value::Null => None,
            p => Some(Persona { entry: text(p, "entry")?, name: Shown(text(p, "name_md")?) }),
        };
        let link = |m: &str, k: &str| -> Option<Option<Link>> {
            match v.member(m)? {
                Value::Null => Some(None),
                l => Some(Some(Link { entry: text(l, "entry")?, other: text(l, k)?, since: at_of(l.member("since")?)? })),
            }
        };
        let reroot = link("reroot", "to")?;
        let origin = link("origin", "from")?;
        let home = match v.member("home")? {
            Value::Null => None,
            h => Some(h.as_int()?),
        };
        let lineage = arr(&v, "lineage")?
            .iter()
            .map(|l| {
                let moved = match l.member("accession")? {
                    Value::Null => None,
                    a => Some(Moved {
                        chain: int(a, "chainId")?,
                        block: int(a, "blockNumber")?,
                        time: int(a, "blockTimestamp")?,
                        tx_index: int(a, "txIndex")?,
                        log_index: int(a, "logIndex")?,
                        tx: text(a, "tx")?,
                        holder: text(a, "holder")?,
                    }),
                };
                Some(Line { key: text(l, "key")?, moved })
            })
            .collect::<Option<_>>()?;
        let homes = arr(&v, "homes")?
            .iter()
            .map(|o| Some(Opening { entry: text(o, "entry")?, chain: int(o, "chainId")?, block: int(o, "blockNumber")?, time: int(o, "blockTimestamp")?, tx: text(o, "tx")?, sender: text(o, "sender")? }))
            .collect::<Option<_>>()?;
        let faces = arr(&v, "faces")?
            .iter()
            .map(|r| {
                let named = match r.member("kind")?.as_str()? {
                    "account" => Named::Account { platform: text(r, "platform")?, handle: text(r, "handle")? },
                    "key" => Named::Key { address: text(r, "address")?, role: text(r, "role")? },
                    "persona" => Named::Persona(Shown(text(r, "name_md")?)),
                    _ => return None,
                };
                let bound = match r.member("state")?.as_str()? {
                    "bound" => true,
                    "released" => false,
                    _ => return None,
                };
                let spans = arr(r, "spans")?
                    .iter()
                    .map(|s| {
                        let until = match s.member("until")? {
                            Value::Null => None,
                            u => Some(at_of(u)?),
                        };
                        let release = match s.member("release")? {
                            Value::Null => None,
                            x => Some(x.as_str()?.to_string()),
                        };
                        Some(Span { entry: text(s, "entry")?, since: at_of(s.member("since")?)?, release, until })
                    })
                    .collect::<Option<_>>()?;
                Some(Row { named, bound, spans })
            })
            .collect::<Option<_>>()?;
        let misread = arr(&v, "misread")?.iter().map(|m| Some((text(m, "entry")?, text(m, "reason")?))).collect::<Option<_>>()?;
        Some(Table {
            root: text(&v, "root")?,
            marker: text(&v, "marker")?,
            basis: arr(&v, "basis")?.iter().map(basis_of).collect::<Option<_>>()?,
            home,
            office: text(&v, "office")?,
            persona,
            reroot,
            origin,
            lineage,
            homes,
            faces,
            misread,
            unread: int(&v, "unread")?,
            unproven: int(&v, "unproven")?,
            value: v,
        })
    }
}

pub use zikaron_face::shapes::{shape_of, Shape};

pub const NOTE_BYTES: usize = 280;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unwritten {
    NoRoot,
    Label { member: &'static str },
    HandleChar(char),
    HandleEnds,
    HandleLength(usize),
    NoteLength(usize),
    NameLength(usize),
    NameBarred(char),
    Shape,
    SameRoot { member: &'static str },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Writer {
    root: String,
}

fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

impl Writer {
    pub fn of(items: &[Vec<u8>]) -> Result<Writer, Unwritten> {
        crate::book::root_of(items).map(|root| Writer { root: root.to_ascii_lowercase() }).map_err(|_| Unwritten::NoRoot)
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    fn checked(body: Value, shape: Shape) -> Result<Value, Unwritten> {
        if shape_of(&body) == Some(shape) {
            Ok(body)
        } else {
            Err(Unwritten::Shape)
        }
    }

    fn label(v: &str, member: &'static str) -> Result<(), Unwritten> {
        if zikaron_face::names::is_label(v) {
            Ok(())
        } else {
            Err(Unwritten::Label { member })
        }
    }

    fn note(note: Option<&str>) -> Result<Option<Value>, Unwritten> {
        match note.filter(|n| !n.is_empty()) {
            None => Ok(None),
            Some(n) if n.len() > NOTE_BYTES => Err(Unwritten::NoteLength(n.len())),
            Some(n) => Ok(Some(s(n))),
        }
    }

    fn with_note(mut members: Vec<(String, Value)>, note: Option<Value>) -> Value {
        if let Some(n) = note {
            members.push(("note_md".into(), n));
        }
        members.sort_by(|a, b| a.0.cmp(&b.0));
        Value::Obj(members)
    }

    pub fn home(&self, chain: u64) -> Result<Value, Unwritten> {
        let body = Value::Obj(vec![("act".into(), s("home")), ("chain".into(), Value::Int(chain)), ("root".into(), s(&self.root))]);
        Self::checked(body, Shape::Home)
    }

    pub fn account(&self, platform: &str, handle: &str, note: Option<&str>) -> Result<Value, Unwritten> {
        Self::label(platform, "platform")?;
        if let Some(c) = handle.chars().find(|c| !(' '..='~').contains(c)) {
            return Err(Unwritten::HandleChar(c));
        }
        let n = handle.chars().count();
        if !(1..=128).contains(&n) {
            return Err(Unwritten::HandleLength(n));
        }
        if handle.starts_with(' ') || handle.ends_with(' ') {
            return Err(Unwritten::HandleEnds);
        }
        let note = Self::note(note)?;
        let body = Self::with_note(vec![("act".into(), s("bind")), ("handle".into(), s(handle)), ("kind".into(), s("account")), ("platform".into(), s(platform)), ("root".into(), s(&self.root))], note);
        Self::checked(body, Shape::Account)
    }

    pub fn key(&self, address: &[u8; 20], role: &str, sig: &[u8; 65], note: Option<&str>) -> Result<Value, Unwritten> {
        Self::label(role, "role")?;
        let note = Self::note(note)?;
        let body = Self::with_note(
            vec![
                ("act".into(), s("bind")),
                ("address".into(), s(&hexfmt::encode(address))),
                ("kind".into(), s("key")),
                ("role".into(), s(role)),
                ("root".into(), s(&self.root)),
                ("sig".into(), s(&hexfmt::encode(sig))),
            ],
            note,
        );
        Self::checked(body, Shape::Key)
    }

    pub fn persona(&self, name: &str) -> Result<Value, Unwritten> {
        let name = nfc(name);
        let n = name.chars().count();
        if !(1..=32).contains(&n) {
            return Err(Unwritten::NameLength(n));
        }
        if let Some(c) = zikaron_face::names::first_barred(&name) {
            return Err(Unwritten::NameBarred(c));
        }
        let body = Value::Obj(vec![("act".into(), s("bind")), ("kind".into(), s("persona")), ("name_md".into(), s(&name)), ("root".into(), s(&self.root))]);
        Self::checked(body, Shape::Persona)
    }

    pub fn reroot(&self, to: &[u8; 20], note: Option<&str>) -> Result<Value, Unwritten> {
        self.other_root(to, "to")?;
        let note = Self::note(note)?;
        let body = Self::with_note(vec![("act".into(), s("reroot")), ("root".into(), s(&self.root)), ("to".into(), s(&hexfmt::encode(to)))], note);
        Self::checked(body, Shape::Reroot)
    }

    pub fn origin(&self, from: &[u8; 20], note: Option<&str>) -> Result<Value, Unwritten> {
        self.other_root(from, "from")?;
        let note = Self::note(note)?;
        let body = Self::with_note(vec![("act".into(), s("origin")), ("from".into(), s(&hexfmt::encode(from))), ("root".into(), s(&self.root))], note);
        Self::checked(body, Shape::Origin)
    }

    fn other_root(&self, other: &[u8; 20], member: &'static str) -> Result<(), Unwritten> {
        if hexfmt::encode(other) == self.root {
            Err(Unwritten::SameRoot { member })
        } else {
            Ok(())
        }
    }

    pub fn release(&self, subject: &[u8; 32], note: Option<&str>) -> Result<Value, Unwritten> {
        let note = Self::note(note)?;
        let body = Self::with_note(vec![("act".into(), s("release")), ("root".into(), s(&self.root)), ("subject".into(), s(&hexfmt::encode(subject)))], note);
        Self::checked(body, Shape::Release)
    }
}

pub fn joined(first: &Table, second: &Table) -> bool {
    zikaron_face::reading::joined(&first.value, &second.value)
}

pub fn nfc(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    text.nfc().collect()
}

pub fn seat_chains(items: &[Vec<u8>]) -> std::collections::BTreeMap<String, u64> {
    let entries: Vec<crate::book::Entry> = items.iter().filter_map(|b| crate::book::check(b).ok()).collect();
    seats_of(&entries)
}

pub fn seats_of(entries: &[crate::book::Entry]) -> std::collections::BTreeMap<String, u64> {
    let mut faces: Vec<&crate::book::Entry> = entries.iter().filter(|e| e.law == crate::book::Law::Two && e.entry_type == ENTRY_TYPE).collect();
    faces.sort_by_key(|e| e.seq);
    let mut out = std::collections::BTreeMap::new();
    let mut home: Option<u64> = None;
    for e in faces {
        match zikaron_face::shapes::shape_of(&e.body) {
            Some(Shape::Home) => {
                let c = e.body.member("chain").and_then(Value::as_int);
                if let Some(c) = c {
                    out.insert(e.id_hex().to_ascii_lowercase(), c);
                    home = Some(c);
                }
            }
            Some(_) => {
                if let Some(h) = home {
                    out.insert(e.id_hex().to_ascii_lowercase(), h);
                }
            }
            None => {}
        }
    }
    out
}

pub const CONSENT_PREFIX: &str = "zikaron-consent:";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Consent {
    pub root: [u8; 20],
    pub address: [u8; 20],
    pub role: String,
    pub sig: [u8; 65],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsentUnread {
    Prefix,
    Form,
}

impl Consent {
    pub fn text(&self) -> String {
        format!("{CONSENT_PREFIX}{}:{}:{}:{}", hexfmt::encode(&self.root), hexfmt::encode(&self.address), self.role, hexfmt::encode(&self.sig))
    }

    pub fn parse(text: &str) -> Result<Consent, ConsentUnread> {
        let rest = text.trim().strip_prefix(CONSENT_PREFIX).ok_or(ConsentUnread::Prefix)?;
        let parts: Vec<&str> = rest.split(':').collect();
        let [root, address, role, sig] = parts.as_slice() else { return Err(ConsentUnread::Form) };
        let h20 = |v: &str| -> Option<[u8; 20]> { (v.len() == 42).then_some(())?; hexfmt::decode(v)?.try_into().ok() };
        let root = h20(root).ok_or(ConsentUnread::Form)?;
        let address = h20(address).ok_or(ConsentUnread::Form)?;
        if !zikaron_face::names::is_label(role) {
            return Err(ConsentUnread::Form);
        }
        let sig: [u8; 65] = (sig.len() == 132).then_some(()).and_then(|_| hexfmt::decode(sig)).and_then(|b| b.try_into().ok()).ok_or(ConsentUnread::Form)?;
        Ok(Consent { root, address, role: role.to_string(), sig })
    }

    pub fn holds(&self) -> bool {
        let body = Value::Obj(vec![
            ("address".into(), s(&hexfmt::encode(&self.address))),
            ("role".into(), s(&self.role)),
            ("root".into(), s(&hexfmt::encode(&self.root))),
            ("sig".into(), s(&hexfmt::encode(&self.sig))),
        ]);
        zikaron_face::consent::holds(&body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: [u8; 20] = [0x11; 20];

    fn acc(chain: u64, block: u64, to: [u8; 20], root: [u8; 20]) -> Value {
        Value::Obj(vec![
            ("blockNumber".into(), Value::Int(block)),
            ("blockTimestamp".into(), Value::Int(1000 + block)),
            ("chainId".into(), Value::Int(chain)),
            ("holder".into(), Value::Str(h20(&R))),
            ("logIndex".into(), Value::Int(0)),
            ("root".into(), Value::Str(h20(&root))),
            ("to".into(), Value::Str(h20(&to))),
            ("tx".into(), Value::Str(format!("0x{}", format!("{block:02x}").repeat(32)))),
            ("txIndex".into(), Value::Int(0)),
        ])
    }

    fn basis(chain: u64, to: u64) -> Basis {
        Basis { chain, from_block: 5, to_block: to, registries: vec![[0x33; 20], [0x22; 20]], book: [0x44; 20], book_from_block: 3 }
    }

    #[test]
    fn the_keys_in_reach_are_the_root_and_every_key_handed_on_under_it() {
        let (k1, k2, other) = ([0x55; 20], [0x66; 20], [0x77; 20]);
        let got = keys_in_reach(&R, &[acc(1, 10, k1, R), acc(1, 11, k2, R), acc(1, 12, other, [0x99; 20])]);
        assert_eq!(got, vec![R, k1, k2]);
        assert_eq!(keys_in_reach(&R, &[]), vec![R], "a root that never handed on: itself alone");
    }

    #[test]
    fn the_moment_is_a_second_before_the_earliest_head() {
        assert_eq!(moment(&[1_000, 990, 1_200]), Some(989));
        assert_eq!(moment(&[]), None);
        assert_eq!(moment(&[0, 5]), None);
    }

    #[test]
    fn the_input_holds_the_basis_records_and_pile_the_core_reads() {
        let b = [basis(10, 50), basis(1, 40), basis(10, 99)];
        let accs = [acc(1, 4, [0x55; 20], R), acc(1, 2, [0x56; 20], R), acc(1, 41, [0x57; 20], R), acc(7, 20, [0x58; 20], R)];
        let v = input(&R, &b, &[], &accs, &[b"x".to_vec(), b"x".to_vec()]);
        let t = read(&v).expect("a valid input");
        assert_eq!(t.basis.iter().map(|c| (c.chain, c.to_block)).collect::<Vec<_>>(), vec![(1, 40), (10, 50)]);
        assert_eq!(t.basis[0].registries, vec![[0x22; 20], [0x33; 20]]);
        assert_eq!(v.member("accessions").and_then(Value::as_arr).map(Vec::len), Some(1), "only block 4 lies in 3..=40 on a basis chain");
        assert_eq!(v.member("pile").and_then(Value::as_arr).map(Vec::len), Some(1));
        assert_eq!((t.home, t.persona.is_none(), t.faces.len(), t.unread, t.unproven), (None, true, 0, 0, 0));
        assert_eq!(t.marker, format!("zikaron:{}", h20(&R)));
        assert_eq!(t.lineage.len(), 2, "the root, and the key block 4 handed it to");
        let no = Value::Obj(vec![("root".into(), Value::Str("nope".into()))]);
        assert_eq!(read(&no), Err(NoTable), "an input of another shape is no table, never an empty one");
    }

    #[test]
    fn only_an_anchor_call_with_the_entry_after_its_word_carries_it() {
        let e = br#"{"author":"0x"}"#.to_vec();
        let mut c = zikaron_face::ANCHOR_SELECTOR.to_vec();
        c.extend_from_slice(&zikaron::cryptox::sha256(&e));
        assert_eq!(carried(&c), None, "the word alone");
        let mut tail = c.clone();
        tail.extend_from_slice(b"other bytes");
        assert_eq!(carried(&tail), None, "a tail that does not hash to the word");
        let mut many = vec![0x13, 0x0f, 0x48, 0x2f];
        many.extend_from_slice(&c[4..]);
        many.extend_from_slice(&e);
        assert_eq!(carried(&many), None, "another selector");
    }

    #[test]
    fn records_with_two_clocks_on_one_chain_are_no_table() {
        let mut a = acc(1, 10, [0x55; 20], R);
        let mut b = acc(1, 10, [0x56; 20], R);
        if let (Value::Obj(ma), Value::Obj(mb)) = (&mut a, &mut b) {
            ma.iter_mut().filter(|(k, _)| k == "logIndex").for_each(|(_, v)| *v = Value::Int(0));
            mb.iter_mut().filter(|(k, _)| k == "logIndex").for_each(|(_, v)| *v = Value::Int(1));
            mb.iter_mut().filter(|(k, _)| k == "blockTimestamp").for_each(|(_, v)| *v = Value::Int(5));
        }
        let v = input(&R, &[basis(1, 40)], &[], &[a.clone(), b], &[]);
        assert_eq!(read(&v), Err(NoTable));
        assert!(read(&input(&R, &[basis(1, 40)], &[], &[a], &[])).is_ok(), "one clock: a table");
    }

    #[test]
    fn a_display_name_is_laid_alone_and_copied_as_itself() {
        let s = Shown("שלום·名".to_string());
        assert_eq!(s.lay_alone(), "\u{2068}שלום·名\u{2069}");
        assert_eq!(s.copy_name(), "שלום·名");
        assert_eq!(format!("{s:?}"), "Shown(6 scalar values)");
    }

    fn writer() -> Writer {
        Writer { root: h20(&R) }
    }

    #[test]
    fn a_display_name_is_written_in_nfc_within_the_table() {
        let w = writer();
        assert!(w.persona(&"名".repeat(32)).is_ok(), "32 scalar values");
        assert_eq!(w.persona(&"名".repeat(33)), Err(Unwritten::NameLength(33)));
        assert_eq!(w.persona(""), Err(Unwritten::NameLength(0)));
        assert_eq!(w.persona("Lin Xia"), Err(Unwritten::NameBarred(' ')));
        assert_eq!(w.persona("a\u{200D}b\u{FE0F}"), Err(Unwritten::NameBarred('\u{200D}')), "the first barred one");
        assert_eq!(w.persona("\u{2764}\u{FE0F}"), Err(Unwritten::NameBarred('\u{FE0F}')));
        let composed = w.persona("caf\u{E9}").expect("composed");
        let decomposed = w.persona("cafe\u{301}").expect("decomposed");
        assert_eq!(composed, decomposed, "one name, one body");
        assert_eq!(composed.member("name_md").and_then(Value::as_str), Some("caf\u{E9}"));
        assert!(w.persona(&format!("{}e\u{301}", "名".repeat(31))).is_ok(), "32 once composed");
        assert!(composed.member("note_md").is_none(), "a persona carries no note");
    }

    #[test]
    fn an_account_is_its_platforms_ascii_name() {
        let w = writer();
        assert_eq!(w.account("x", "林夏", None), Err(Unwritten::HandleChar('林')));
        assert_eq!(w.account("x", "a\tb", None), Err(Unwritten::HandleChar('\t')));
        assert_eq!(w.account("x", " alice", None), Err(Unwritten::HandleEnds));
        assert_eq!(w.account("x", "alice ", None), Err(Unwritten::HandleEnds));
        assert!(w.account("x", &"a".repeat(128), None).is_ok());
        assert_eq!(w.account("x", &"a".repeat(129), None), Err(Unwritten::HandleLength(129)));
        assert_eq!(w.account("X", "alice", None), Err(Unwritten::Label { member: "platform" }));
        assert_eq!(w.account("", "alice", None), Err(Unwritten::Label { member: "platform" }));
        let with = w.account("github", "alice", Some(&"n".repeat(280))).expect("280 bytes");
        assert_eq!(with.member("note_md").and_then(Value::as_str).map(str::len), Some(280));
        assert_eq!(w.account("github", "alice", Some(&"n".repeat(281))), Err(Unwritten::NoteLength(281)));
        assert_eq!(w.account("github", "alice", Some(&"林".repeat(94))), Err(Unwritten::NoteLength(282)), "bytes, not characters");
        assert!(w.account("github", "alice", Some("")).expect("empty").member("note_md").is_none());
        assert_eq!(zikaron_face::shapes::shape_of(&with), Some(Shape::Account));
    }

    #[test]
    fn the_five_shapes_are_the_cores() {
        let w = writer();
        assert_eq!(shape_of(&w.home(11_155_111).expect("home")), Some(Shape::Home));
        assert_eq!(shape_of(&w.key(&[0x44; 20], "payout", &[0x1b; 65], Some("n")).expect("key")), Some(Shape::Key));
        assert_eq!(w.key(&[0x44; 20], "Pay out", &[0x1b; 65], None), Err(Unwritten::Label { member: "role" }));
        assert_eq!(shape_of(&w.release(&[0x55; 32], None).expect("release")), Some(Shape::Release));
        for b in [w.home(1).unwrap(), w.persona("n").unwrap(), w.release(&[1; 32], None).unwrap()] {
            assert_eq!(b.member("root").and_then(Value::as_str), Some(h20(&R).as_str()));
        }
    }

    #[test]
    fn a_consent_text_reads_back_and_holds_under_its_root_alone() {
        let key = [0x07u8; 32];
        let address: [u8; 20] = zikaron::cryptox::address_of_privkey(&key).expect("an address");
        let sig = zikaron_face::consent::sign(&key, &h20(&R), &h20(&address), "payout").expect("signed");
        let c = Consent { root: R, address, role: "payout".into(), sig };
        let text = c.text();
        assert!(text.starts_with("zikaron-consent:0x"));
        assert_eq!(Consent::parse(&format!("  {text}\n")), Ok(c.clone()));
        let up = |b: &[u8]| hexfmt::encode(b).to_ascii_uppercase().replacen("0X", "0x", 1);
        assert_eq!(Consent::parse(&format!("zikaron-consent:{}:{}:payout:{}", up(&R), up(&address), up(&sig))), Ok(c.clone()), "hex of either case");
        assert!(c.holds());
        assert!(!Consent { root: [0x77; 20], ..c.clone() }.holds(), "another root");
        assert!(!Consent { role: "judge".into(), ..c.clone() }.holds(), "another role");
        assert!(!Consent { address: [0x66; 20], ..c.clone() }.holds(), "signed by another address");
        assert_eq!(Consent::parse(&text[8..]), Err(ConsentUnread::Prefix));
        assert_eq!(Consent::parse("zikaron-consent:0x11:0x22:payout:0x33"), Err(ConsentUnread::Form));
        assert_eq!(Consent::parse(&format!("{text}:extra")), Err(ConsentUnread::Form));
        assert_eq!(Consent::parse(&text.replace(":payout:", ":Pay:")), Err(ConsentUnread::Form));
    }
}

