use crate::fault::{Fault, Known};
use crate::family::ENTROPY_BYTES;
use crate::key::{Address, Secret};
use crate::roles::Role;
use std::path::{Path, PathBuf};
use zikaron::json::Value;

pub use zikaron_seal::identities::{Kind, Office, NO_BACKUP_AT, NO_CREATED, NO_LABEL, SHAPE, SHAPE_V1, UNSEATED};
use zikaron_seal::identities::{Book, Line};

fn seat_of(s: &str) -> Option<Role> {
    Role::ALL.into_iter().find(|r| r.as_str() == s)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Keys {
    Both { author: Address, grantee: Address },
    One { seat: Role, addr: Address },
}

impl Keys {
    pub fn address(&self, seat: Role) -> Option<Address> {
        match (self, seat) {
            (Keys::Both { author, .. }, Role::Author) => Some(*author),
            (Keys::Both { grantee, .. }, Role::Grantee) => Some(*grantee),
            (Keys::One { seat: s, addr }, _) if *s == seat => Some(*addr),
            (Keys::One { .. }, _) => None,
        }
    }

    pub fn seats(&self) -> Vec<Role> {
        Role::ALL.into_iter().filter(|r| self.address(*r).is_some()).collect()
    }

    pub fn kind(&self) -> Kind {
        match self {
            Keys::Both { .. } => Kind::Words,
            Keys::One { .. } => Kind::Existing,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub keys: Keys,
    pub author_office: Option<Office>,
    pub grantee_office: Option<Office>,
    pub author_home: String,
    pub grantee_home: String,
    pub backed_words: bool,
    pub backed_file: bool,
    pub backup_at: String,
    pub label: String,
    pub created: String,
    pub network: Option<Chosen>,
    pub unread_network: Option<String>,
    pub reroot_of: Option<Address>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Chosen {
    Row(String),
    Custom(Option<crate::deploy::Custom>),
}

impl Chosen {
    pub fn named(name: &str) -> Chosen {
        if name == crate::deploy::CUSTOM {
            Chosen::Custom(None)
        } else {
            Chosen::Row(name.to_string())
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Chosen::Row(n) => n,
            Chosen::Custom(_) => crate::deploy::CUSTOM,
        }
    }

    pub fn custom(&self) -> Option<&crate::deploy::Custom> {
        match self {
            Chosen::Custom(c) => c.as_ref(),
            Chosen::Row(_) => None,
        }
    }
}

impl Row {
    pub fn kind(&self) -> Kind {
        self.keys.kind()
    }

    pub fn address(&self, seat: Role) -> Option<Address> {
        self.keys.address(seat)
    }

    pub fn office(&self, seat: Role) -> Option<Office> {
        let root = self.address(seat)?;
        let written = match seat {
            Role::Author => self.author_office.clone(),
            Role::Grantee => self.grantee_office.clone(),
        };
        Some(written.unwrap_or(Office::root(root.0)))
    }

    pub fn current(&self, seat: Role) -> Option<Address> {
        self.office(seat).map(|o| Address(o.current))
    }

    pub fn seats(&self) -> Vec<Role> {
        self.keys.seats()
    }

    pub fn home(&self, seat: Role) -> Option<PathBuf> {
        if self.address(seat).is_none() {
            return None;
        }
        let h = match seat {
            Role::Author => &self.author_home,
            Role::Grantee => &self.grantee_home,
        };
        if h == UNSEATED {
            return None;
        }
        Some(PathBuf::from(h))
    }

    pub fn account(&self, seat: Role) -> Option<String> {
        Some(crate::places::key_slot(&self.current(seat)?))
    }

    pub fn needed_accounts(&self) -> Vec<String> {
        let mut v: Vec<String> = Role::ALL.iter().filter_map(|r| self.account(*r)).collect();
        v.dedup();
        if self.kind() == Kind::Words {
            v.push(crate::places::seed_slot(&self.id));
        }
        v
    }

    pub fn accounts(&self) -> Vec<String> {
        let mut v: Vec<String> = Role::ALL.iter().filter_map(|r| self.account(*r)).collect();
        for r in Role::ALL {
            if let Some(o) = self.office(r) {
                for a in o.former.iter().chain(o.next.iter()) {
                    v.push(crate::places::key_slot(&Address(*a)));
                }
            }
        }
        v.dedup();
        if self.kind() == Kind::Words {
            v.push(crate::places::seed_slot(&self.id));
        }
        v
    }

    pub fn backed(&self) -> bool {
        self.backed_words || self.backed_file
    }

    pub fn first_seat(&self) -> Role {
        self.keys.seats().first().copied().unwrap_or(Role::Author)
    }

    pub fn seat_here_or_first(&self, here: Role) -> Role {
        if self.keys.seats().contains(&here) {
            here
        } else {
            self.first_seat()
        }
    }

    pub fn network_now(&self) -> Option<crate::deploy::Network<'_>> {
        crate::deploy::resolve(self.network.as_ref().map(Chosen::name), self.network.as_ref().and_then(Chosen::custom))
    }

    pub fn seat_at(&self, root: &Path) -> Option<Role> {
        Role::ALL.into_iter().find(|s| self.home(*s).map(|h| crate::home::same_place(&h, root)).unwrap_or(false))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registry {
    pub current: Option<(String, Role)>,
    pub rows: Vec<Row>,
    pub left: Vec<(String, Role, String)>,
}

impl Registry {
    pub fn now(&self) -> Option<(&Row, Role)> {
        let (id, seat) = self.current.as_ref()?;
        self.find(id).map(|r| (r, *seat))
    }

    pub fn find(&self, id: &str) -> Option<&Row> {
        self.rows.iter().find(|r| r.id == id)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let lines = self
            .rows
            .iter()
            .map(|r| {
                let raw = r.unread_network.as_deref().and_then(|t| zikaron::json::parse(t.as_bytes()).ok());
                let network = match (&r.network, raw) {
                    (Some(n), Some(raw)) if raw.member("name").and_then(|x| x.as_str()) == Some(n.name()) && n.custom().is_none() => Some(raw),
                    (Some(n), _) => Some(network_value(n)),
                    (None, _) => None,
                };
                Line {
                    id: r.id.clone(),
                    author: r.address(Role::Author).map(|a| a.0),
                    grantee: r.address(Role::Grantee).map(|a| a.0),
                    author_office: r.office(Role::Author),
                    grantee_office: r.office(Role::Grantee),
                    author_home: r.author_home.clone(),
                    grantee_home: r.grantee_home.clone(),
                    backed_words: r.backed_words,
                    backed_file: r.backed_file,
                    backup_at: r.backup_at.clone(),
                    label: r.label.clone(),
                    created: r.created.clone(),
                    network,
                    reroot_of: r.reroot_of.map(|a| a.0),
                }
            })
            .collect();
        Book {
            current: self.current.as_ref().map(|(id, seat)| (id.clone(), seat.as_str())),
            lines,
            left: self.left.iter().map(|(id, seat, home)| (id.clone(), seat.as_str(), home.clone())).collect(),
            upgraded: false,
        }
        .to_bytes()
    }

    pub fn parse(bytes: &[u8]) -> Result<Registry, String> {
        Registry::parse_shaped(bytes).map(|(r, _)| r)
    }

    pub fn parse_shaped(bytes: &[u8]) -> Result<(Registry, bool), String> {
        let book = Book::parse(bytes)?;
        let upgraded = book.upgraded;
        let role = |w: &str| seat_of(w).ok_or_else(|| w.to_string());
        let mut rows = Vec::with_capacity(book.lines.len());
        for l in book.lines {
            let keys = match (l.author, l.grantee) {
                (Some(a), Some(g)) => Keys::Both { author: Address(a), grantee: Address(g) },
                (Some(a), None) => Keys::One { seat: Role::Author, addr: Address(a) },
                (None, Some(g)) => Keys::One { seat: Role::Grantee, addr: Address(g) },
                (None, None) => return Err("author".into()),
            };
            let (network, unread_network) = match &l.network {
                Some(v) => match network_of(v) {
                    Some(c) => (Some(c), None),
                    None => (
                        v.member("name").and_then(|n| n.as_str()).filter(|n| !n.is_empty()).map(Chosen::named),
                        Some(String::from_utf8_lossy(&zikaron::json::canon_bytes(v)).to_string()),
                    ),
                },
                None => (None, None),
            };
            rows.push(Row {
                id: l.id,
                keys,
                author_office: l.author_office.filter(|o| o.generation != 0 || o.next.is_some() || !o.former.is_empty()),
                grantee_office: l.grantee_office.filter(|o| o.generation != 0 || o.next.is_some() || !o.former.is_empty()),
                author_home: l.author_home,
                grantee_home: l.grantee_home,
                backed_words: l.backed_words,
                backed_file: l.backed_file,
                backup_at: l.backup_at,
                label: l.label,
                created: l.created,
                network,
                unread_network,
                reroot_of: l.reroot_of.map(Address),
            });
        }
        let current = match book.current {
            Some((id, seat)) => Some((id, role(seat)?)),
            None => None,
        };
        let left = book.left.into_iter().map(|(id, seat, home)| Ok((id, role(seat)?, home))).collect::<Result<_, String>>()?;
        Ok((Registry { current, rows, left }, upgraded))
    }
}

fn office_mut(row: &mut Row, seat: Role) -> Result<&mut Option<Office>, Fault> {
    if row.address(seat).is_none() {
        return Err(Fault::known(Known::SeatUnseated, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::IdSeatEmpty).to_string())));
    }
    Ok(match seat {
        Role::Author => &mut row.author_office,
        Role::Grantee => &mut row.grantee_office,
    })
}

pub fn set_next(reg: &mut Registry, id: &str, seat: Role, next: Option<Address>) -> Result<Row, Fault> {
    let row = reg.rows.iter_mut().find(|r| r.id.eq_ignore_ascii_case(id)).ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    let now = row.office(seat).ok_or_else(|| Fault::known(Known::SeatUnseated, id.to_string()))?;
    let slot = office_mut(row, seat)?;
    let mut o = now;
    o.next = next.map(|a| a.0);
    *slot = if o.generation == 0 && o.next.is_none() && o.former.is_empty() { None } else { Some(o) };
    Ok(row.clone())
}

pub fn take_office(reg: &mut Registry, id: &str, seat: Role) -> Result<Row, Fault> {
    let row = reg.rows.iter_mut().find(|r| r.id.eq_ignore_ascii_case(id)).ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    let root = row.address(seat).ok_or_else(|| Fault::known(Known::SeatUnseated, id.to_string()))?;
    let now = row.office(seat).ok_or_else(|| Fault::known(Known::SeatUnseated, id.to_string()))?;
    let next = now.next.ok_or_else(|| Fault::known(Known::NoIdentity, format!("{id} next")))?;
    let mut former = now.former.clone();
    if now.current != root.0 && !former.contains(&now.current) {
        former.push(now.current);
    }
    *office_mut(row, seat)? = Some(Office { generation: now.generation + 1, current: next, next: None, former });
    key_file_gone_by(row);
    Ok(row.clone())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeatLedger {
    Holds,
    Unchecked,
    None,
}

pub fn seat_ledger(row: &Row, seat: Role) -> SeatLedger {
    let Some(home) = row.home(seat).and_then(|h| crate::home::Home::open(&h).ok()) else { return SeatLedger::None };
    if crate::ledgerx::head(&home).ok().flatten().is_some() {
        return SeatLedger::Holds;
    }
    match crate::restorex::read(&home) {
        Ok(None) => SeatLedger::None,
        Ok(Some(_)) | Err(_) => SeatLedger::Unchecked,
    }
}

pub fn reroot(reg: &mut Registry, id: &str, new: Address) -> Result<Row, Fault> {
    let row = reg.rows.iter_mut().find(|r| r.id.eq_ignore_ascii_case(id)).ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    let Keys::Both { author, grantee } = row.keys else { return Err(Fault::known(Known::IdentitiesShape, id.to_string())) };
    if seat_ledger(row, Role::Grantee) != SeatLedger::None {
        return Err(Fault::known(Known::IdentitiesShape, id.to_string()));
    }
    row.keys = Keys::Both { author, grantee: new };
    row.grantee_office = Some(Office { generation: 0, current: new.0, next: None, former: vec![grantee.0] });
    Ok(row.clone())
}

fn key_file_gone_by(row: &mut Row) {
    row.backed_file = false;
    row.backup_at = NO_BACKUP_AT.to_string();
}

pub fn hold_former(reg: &mut Registry, id: &str, seat: Role, keys: &[Address]) -> Result<Row, Fault> {
    let row = reg.rows.iter_mut().find(|r| r.id.eq_ignore_ascii_case(id)).ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    let root = row.address(seat).ok_or_else(|| Fault::known(Known::SeatUnseated, id.to_string()))?;
    let Some(mut o) = row.office(seat) else { return Ok(row.clone()) };
    for k in keys {
        if *k != root && k.0 != o.current && !o.former.contains(&k.0) {
            o.former.push(k.0);
        }
    }
    if o.generation != 0 || o.next.is_some() || !o.former.is_empty() {
        *office_mut(row, seat)? = Some(o);
    }
    Ok(row.clone())
}

pub fn put_office(reg: &mut Registry, id: &str, seat: Role, generation: u64, current: Address) -> Result<Row, Fault> {
    let row = reg.rows.iter_mut().find(|r| r.id.eq_ignore_ascii_case(id)).ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    let root = row.address(seat).ok_or_else(|| Fault::known(Known::SeatUnseated, id.to_string()))?;
    let now = row.office(seat).ok_or_else(|| Fault::known(Known::SeatUnseated, id.to_string()))?;
    let mut former = now.former.clone();
    if now.current != root.0 && now.current != current.0 && !former.contains(&now.current) {
        former.push(now.current);
    }
    former.retain(|a| *a != current.0);
    let generation = if current == root { 0 } else { generation.max(1) };
    *office_mut(row, seat)? = if generation == 0 && former.is_empty() { None } else { Some(Office { generation, current: current.0, next: None, former }) };
    if now.current != current.0 {
        key_file_gone_by(row);
    }
    Ok(row.clone())
}

pub fn now_row(view: &Registry) -> Option<(Row, Role)> {
    crate::trace::mark(crate::feature::Feature::H2);
    view.now().map(|(r, s)| (r.clone(), s))
}

pub fn account_now(view: &Registry) -> Option<String> {
    now_row(view).and_then(|(row, seat)| row.account(seat))
}

pub fn land_at_boot(listed: &mut Registry) -> Result<Option<(Row, Role)>, Fault> {
    let Some((row, seat)) = listed.now().map(|(r, s)| (r.clone(), s)) else { return Ok(None) };
    if row.address(seat).is_some() {
        return Ok(Some((row, seat)));
    }
    let landed = row.first_seat();
    let row = switch(listed, &row.id, landed)?;
    Ok(Some((row, landed)))
}

pub fn home_now(listed: Option<&Registry>) -> Result<Option<PathBuf>, Fault> {
    if let Some((row, seat)) = listed.and_then(|r| r.now()) {
        if let Some(h) = row.home(seat) {
            return Ok(Some(h));
        }
        if row.address(seat).is_none() {
            return Ok(None);
        }
    }
    crate::home::where_is().map(Some)
}

pub fn view(listed: Option<Registry>) -> Registry {
    listed.unwrap_or_default()
}

pub struct Fresh {
    entropy: [u8; ENTROPY_BYTES],
    pub picks: [usize; 3],
}

impl Fresh {
    pub(crate) fn entropy(&self) -> &[u8; ENTROPY_BYTES] {
        &self.entropy
    }

    pub fn twin(&self) -> Fresh {
        Fresh { entropy: self.entropy, picks: self.picks }
    }
}

impl Drop for Fresh {
    fn drop(&mut self) {
        zikaron_os::wipe(&mut self.entropy);
    }
}

impl Fresh {
    pub fn words(&self) -> Vec<crate::secret::Secret> {
        let mut phrase = crate::cryptx::phrase_of(&self.entropy);
        let words = phrase.split(' ').map(crate::secret::Secret::of).collect();
        zikaron_os::wipe_string(&mut phrase);
        words
    }

    pub fn address(&self, seat: Role) -> Option<Address> {
        crate::key::derived(&self.entropy, seat).and_then(|s| s.address())
    }
}

fn random(n: usize) -> Result<Vec<u8>, Fault> {
    crate::key::random(n)
}

fn picks() -> Result<[usize; 3], Fault> {
    let words = crate::family::WORDS;
    let mut got: Vec<usize> = Vec::new();
    let limit = 256 - (256 % words);
    while got.len() < 3 {
        for b in random(16)? {
            let b = b as usize;
            if b < limit && !got.contains(&(b % words)) {
                got.push(b % words);
                if got.len() == 3 {
                    break;
                }
            }
        }
    }
    got.sort_unstable();
    Ok([got[0], got[1], got[2]])
}

pub fn fresh() -> Result<Fresh, Fault> {
    let raw = random(ENTROPY_BYTES)?;
    let mut entropy = [0u8; ENTROPY_BYTES];
    entropy.copy_from_slice(&raw);
    Ok(Fresh { entropy, picks: picks()? })
}

pub fn from_words(text: &str) -> Result<Fresh, Fault> {
    match crate::cryptx::entropy_of(text) {
        Ok(entropy) => Ok(Fresh { entropy, picks: picks()? }),
        Err(crate::cryptx::PhraseTrouble::WordCount(n)) => Err(Fault::known(Known::PhraseWords, n.to_string())),
        Err(crate::cryptx::PhraseTrouble::NotAPhrase) => Err(Fault::known(Known::PhraseInvalid, String::new())),
    }
}

pub fn confirm(f: &Fresh, answers: &[(usize, crate::secret::Secret)]) -> Result<(), Fault> {
    let words = f.words();
    for p in f.picks {
        let got = answers.iter().find(|(i, _)| *i == p).map(|(_, w)| w.expose().trim().to_ascii_lowercase());
        if got.as_deref() != words.get(p).map(|w| w.expose()) {
            return Err(Fault::known(Known::PhraseConfirm, format!("#{}", p + 1)));
        }
    }
    Ok(())
}

fn row_with(reg: &Registry, addrs: &[Address]) -> Option<Row> {
    reg.rows
        .iter()
        .find(|r| addrs.iter().any(|a| Role::ALL.into_iter().any(|seat| r.address(seat) == Some(*a))))
        .cloned()
}

pub fn meets(reg: &Registry, keys: &Keys, label: &str) -> Result<Option<Row>, Fault> {
    let label = label.trim();
    let addrs: Vec<Address> = match keys {
        Keys::Both { author, grantee } => vec![*author, *grantee],
        Keys::One { addr, .. } => vec![*addr],
    };
    let first = addrs.first().map(|a| a.hex()).unwrap_or_default();
    if let Some(had) = row_with(reg, &addrs) {
        if had.keys == *keys && !slots_present(&had)? {
            return Ok(Some(had));
        }
        if !label.is_empty() && label != had.label {
            return Err(Fault::known(Known::IdentityHereAs, format!("{first} · {}", had.label)));
        }
        return Err(Fault::known(Known::IdentityExists, first));
    }
    if !label.is_empty() && reg.rows.iter().any(|r| r.label == label) {
        return Err(Fault::known(Known::IdentityNameTaken, label.to_string()));
    }
    Ok(None)
}

pub fn slots_present(row: &Row) -> Result<bool, Fault> {
    for acct in row.needed_accounts() {
        if !crate::keybox::present(&acct)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn fill_key(acct: &str, s: &Secret) -> Result<(), Fault> {
    if !crate::keybox::present(acct)? {
        crate::key::install_at(acct, s)?;
    }
    Ok(())
}

fn restored(reg: &mut Registry, mut row: Row, words: bool, file: bool) -> Result<Row, Fault> {
    verify_back(&row)?;
    row.backed_words |= words;
    row.backed_file |= file;
    if let Some(r) = reg.rows.iter_mut().find(|r| r.id == row.id) {
        *r = row.clone();
    }
    reg.current = Some((row.id.clone(), row.seat_here_or_first(seat_in_use(reg))));
    Ok(row)
}

fn seat_in_use(reg: &Registry) -> Role {
    reg.current.as_ref().map(|(_, s)| *s).unwrap_or(Role::Author)
}

fn verify_back(row: &Row) -> Result<(), Fault> {
    for seat in row.keys.seats() {
        let (Some(acct), Some(want)) = (row.account(seat), row.current(seat)) else { continue };
        match crate::key::load_at(&acct)? {
            Some(s) if s.address() == Some(want) => {}
            _ => return Err(Fault::known(Known::KeyNotStored, acct)),
        }
    }
    if row.kind() == Kind::Words {
        let acct = crate::places::seed_slot(&row.id);
        match crate::keybox::get(&acct)? {
            Some(b) if b.len() == ENTROPY_BYTES => {}
            _ => return Err(Fault::known(Known::KeyNotStored, acct)),
        }
    }
    Ok(())
}

fn enroll(reg: &mut Registry, row: Row) -> Result<Row, Fault> {
    reg.left.retain(|(i, _, _)| !i.eq_ignore_ascii_case(&row.id));
    let here = seat_in_use(reg);
    reg.rows.push(row.clone());
    reg.current = Some((row.id.clone(), row.seat_here_or_first(here)));
    Ok(row)
}

pub fn add_words(reg: &mut Registry, f: &Fresh, backed_words: bool, label: &str) -> Result<Row, Fault> {
    add_words_rooted(reg, f, backed_words, label, [None, None])
}

pub fn derived_pair(f: &Fresh) -> Option<(Address, Address)> {
    let a = crate::key::derived(&f.entropy, Role::Author)?.address()?;
    let g = crate::key::derived(&f.entropy, Role::Grantee)?.address()?;
    Some((a, g))
}

pub fn add_words_rooted(reg: &mut Registry, f: &Fresh, backed_words: bool, label: &str, roots: [Option<Address>; 2]) -> Result<Row, Fault> {
    let malformed = || Fault::known(Known::KeyMalformed, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail013).to_string()));
    let author = crate::key::derived(&f.entropy, Role::Author).ok_or_else(malformed)?;
    let grantee = crate::key::derived(&f.entropy, Role::Grantee).ok_or_else(malformed)?;
    let a = author.address().ok_or_else(malformed)?;
    let g = grantee.address().ok_or_else(malformed)?;
    let (ra, rg) = (roots[0].unwrap_or(a), roots[1].unwrap_or(g));
    let office = |root: Address, key: Address| (root != key).then(|| Office { generation: 1, current: key.0, next: None, former: Vec::new() });
    if let Some(had) = meets(reg, &Keys::Both { author: ra, grantee: rg }, label)? {
        if !crate::keybox::present(&crate::places::seed_slot(&had.id))? {
            crate::keybox::put(&crate::places::seed_slot(&had.id), &f.entropy)?;
        }
        if !crate::keybox::has_recovery(&had.id)? {
            crate::keybox::add_recovery(&had.id, crate::keybox::PrimaryKind::Words, &f.entropy)?;
        }
        fill_key(&crate::places::key_slot(&a), &author)?;
        fill_key(&crate::places::key_slot(&g), &grantee)?;
        return restored(reg, had, backed_words, false);
    }
    let id = ra.hex();
    let row = Row {
        author_home: crate::home::identity_home(&id, Role::Author)?.display().to_string(),
        grantee_home: crate::home::identity_home(&id, Role::Grantee)?.display().to_string(),
        id,
        keys: Keys::Both { author: ra, grantee: rg },
        author_office: office(ra, a),
        grantee_office: office(rg, g),
        backed_words,
        backed_file: false,
        backup_at: NO_BACKUP_AT.to_string(),
        label: NO_LABEL.to_string(),
        created: stamp(),
        network: None,
        unread_network: None,
        reroot_of: None,
    };
    crate::keybox::put(&crate::places::seed_slot(&row.id), &f.entropy)?;
    crate::keybox::add_recovery(&row.id, crate::keybox::PrimaryKind::Words, &f.entropy)?;
    crate::key::install_at(&crate::places::key_slot(&a), &author)?;
    crate::key::install_at(&crate::places::key_slot(&g), &grantee)?;
    verify_back(&row)?;
    enroll(reg, row)
}

pub fn add_existing(reg: &mut Registry, s: &Secret, seat: Role, backed_file: bool, label: &str) -> Result<Row, Fault> {
    add_existing_rooted(reg, s, seat, backed_file, label, None)
}

pub fn add_existing_rooted(reg: &mut Registry, s: &Secret, seat: Role, backed_file: bool, label: &str, root: Option<Address>) -> Result<Row, Fault> {
    let key = s
        .address()
        .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail013).to_string())))?;
    let a = root.unwrap_or(key);
    if let Some(had) = meets(reg, &Keys::One { seat, addr: a }, label)? {
        fill_key(&crate::places::key_slot(&key), s)?;
        if !crate::keybox::has_recovery(&had.id)? {
            crate::key::seal_recovery(&had.id, s)?;
        }
        return restored(reg, had, false, backed_file);
    }
    let id = a.hex();
    let home = crate::home::identity_home(&id, seat)?.display().to_string();
    let (author_home, grantee_home) = match seat {
        Role::Author => (home, UNSEATED.to_string()),
        Role::Grantee => (UNSEATED.to_string(), home),
    };
    let office = (a != key).then(|| Office { generation: 1, current: key.0, next: None, former: Vec::new() });
    let (author_office, grantee_office) = match seat {
        Role::Author => (office, None),
        Role::Grantee => (None, office),
    };
    let row = Row {
        author_home,
        grantee_home,
        id,
        keys: Keys::One { seat, addr: a },
        author_office,
        grantee_office,
        backed_words: false,
        backed_file,
        backup_at: NO_BACKUP_AT.to_string(),
        label: NO_LABEL.to_string(),
        created: stamp(),
        network: None,
        unread_network: None,
        reroot_of: None,
    };
    crate::key::install_at(&crate::places::key_slot(&key), s)?;
    crate::key::seal_recovery(&row.id, s)?;
    verify_back(&row)?;
    enroll(reg, row)
}

fn stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    crate::keystore::utc(secs)
}

pub fn rename(reg: &mut Registry, id: &str, label: &str) -> Result<Row, Fault> {
    let Some(r) = reg.rows.iter_mut().find(|r| r.id == id) else {
        return Err(Fault::known(Known::NoIdentity, id.to_string()));
    };
    r.label = label.trim().to_string();
    Ok(r.clone())
}

pub fn choose_network(reg: &mut Registry, id: &str, name: &str) -> Result<Row, Fault> {
    if !crate::deploy::is_choice(name) {
        return Err(Fault::known(Known::IdentitiesShape, name.to_string()));
    }
    let Some(r) = reg.rows.iter_mut().find(|r| r.id == id) else {
        return Err(Fault::known(Known::NoIdentity, id.to_string()));
    };
    if r.network.is_none() {
        r.network = Some(Chosen::named(name));
    }
    Ok(r.clone())
}

pub fn set_network(reg: &mut Registry, id: &str, name: &str) -> Result<Row, Fault> {
    if !crate::deploy::is_choice(name) {
        return Err(Fault::known(Known::IdentitiesShape, name.to_string()));
    }
    let Some(r) = reg.rows.iter_mut().find(|r| r.id == id) else {
        return Err(Fault::known(Known::NoIdentity, id.to_string()));
    };
    if r.network.as_ref().map(Chosen::name) != Some(name) {
        r.network = Some(Chosen::named(name));
        r.unread_network = None;
    }
    Ok(r.clone())
}

pub fn remember_custom(reg: &mut Registry, id: &str, c: crate::deploy::Custom) -> Result<bool, Fault> {
    let Some(r) = reg.rows.iter_mut().find(|r| r.id == id) else {
        return Err(Fault::known(Known::NoIdentity, id.to_string()));
    };
    match &mut r.network {
        Some(Chosen::Custom(filled)) if filled.as_ref() != Some(&c) => {
            *filled = Some(c);
            r.unread_network = None;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn network_value(chosen: &Chosen) -> Value {
    let mut m = vec![("name".to_string(), Value::Str(chosen.name().to_string()))];
    if let Some(c) = chosen.custom() {
        m.push(("chainId".to_string(), Value::Int(c.chain_id)));
        m.push(("endpoints".to_string(), Value::Arr(c.endpoints.iter().map(|e| Value::Str(e.clone())).collect())));
        m.push(("fromBlock".to_string(), Value::Int(c.from_block)));
        m.push(("registry".to_string(), Value::Str(c.registry.hex())));
    }
    Value::Obj(m)
}

fn network_of(v: &Value) -> Option<Chosen> {
    let name = match v.member("name") {
        Some(Value::Str(n)) if crate::deploy::is_choice(n) => n.clone(),
        _ => return None,
    };
    if v.member("chainId").is_none() {
        return Some(Chosen::named(&name));
    }
    if name != crate::deploy::CUSTOM {
        return None;
    }
    let chain_id = match v.member("chainId") {
        Some(Value::Int(n)) => *n,
        _ => return None,
    };
    let from_block = match v.member("fromBlock") {
        Some(Value::Int(n)) => *n,
        _ => return None,
    };
    let registry = match v.member("registry") {
        Some(Value::Str(x)) => Address::parse(x)?,
        _ => return None,
    };
    let endpoints = match v.member("endpoints") {
        Some(Value::Arr(a)) => a
            .iter()
            .map(|x| match x {
                Value::Str(e) if crate::chainx::Endpoint::parse(e).is_some() => Some(e.clone()),
                _ => None,
            })
            .collect::<Option<Vec<String>>>()?,
        _ => return None,
    };
    Some(Chosen::Custom(Some(crate::deploy::Custom { chain_id, registry, from_block, endpoints })))
}

pub fn owner_of(reg: &Registry, root: &Path) -> Option<(Row, Role)> {
    reg.rows.iter().find_map(|r| r.seat_at(root).map(|s| (r.clone(), s)))
}

pub fn rehome(reg: &mut Registry, old: &Path, new: &Path) -> usize {
    let to = new.display().to_string();
    let mut n = 0;
    for r in reg.rows.iter_mut() {
        if r.author_home != UNSEATED && crate::home::same_place(&crate::home::here(Path::new(&r.author_home)), old) {
            r.author_home = to.clone();
            n += 1;
        }
        if r.grantee_home != UNSEATED && crate::home::same_place(&crate::home::here(Path::new(&r.grantee_home)), old) {
            r.grantee_home = to.clone();
            n += 1;
        }
    }
    n
}

pub fn seat_home_set(reg: &mut Registry, id: &str, seat: Role, to: &str) -> Option<String> {
    let r = reg.rows.iter_mut().find(|r| r.id == id)?;
    let h = match seat {
        Role::Author => &mut r.author_home,
        Role::Grantee => &mut r.grantee_home,
    };
    if h == UNSEATED {
        return None;
    }
    Some(std::mem::replace(h, to.to_string()))
}

pub fn follow_machine(reg: &mut Registry) {
    let Ok(m) = crate::home::machine_dir() else { return };
    let here = |h: &str| crate::home::here_under(&m, Path::new(h)).display().to_string();
    for r in reg.rows.iter_mut() {
        for h in [&mut r.author_home, &mut r.grantee_home] {
            if *h != UNSEATED && !h.is_empty() {
                *h = here(h);
            }
        }
    }
    for (_, _, h) in reg.left.iter_mut() {
        *h = here(h);
    }
}

pub fn switch(reg: &mut Registry, id: &str, seat: Role) -> Result<Row, Fault> {
    let row = reg.find(id).cloned().ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    reg.current = Some((row.id.clone(), seat));
    Ok(row)
}

pub fn delete_allowed(row: &Row, handed_over: bool) -> Result<(), Fault> {
    if !row.backed() && !handed_over {
        return Err(Fault::known(Known::DeleteUnbacked, row.id.clone()));
    }
    Ok(())
}

pub fn delete(reg: &mut Registry, id: &str, handed_over: bool) -> Result<(Row, Option<(Row, Role)>), Fault> {
    let row = reg.find(id).cloned().ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    if crate::keybox::primary()?.map(|(p, _)| p.eq_ignore_ascii_case(&row.id)).unwrap_or(false) {
        return Err(Fault::known(Known::PrimaryDelete, id.to_string()));
    }
    delete_allowed(&row, handed_over)?;
    for acct in row.accounts() {
        crate::keybox::drop_item(&acct)?;
    }
    crate::keybox::drop_recovery(&row.id)?;
    reg.rows.retain(|r| r.id != row.id);
    for seat in Role::ALL {
        if let Some(h) = row.home(seat) {
            reg.left.retain(|(i, s, _)| !(i.eq_ignore_ascii_case(&row.id) && *s == seat));
            reg.left.push((row.id.clone(), seat, h.display().to_string()));
        }
    }
    if reg.current.as_ref().map(|(c, _)| c == &row.id).unwrap_or(true) {
        reg.current = reg.rows.first().map(|r| (r.id.clone(), r.first_seat()));
    }
    let next = reg.now().map(|(r, s)| (r.clone(), s));
    Ok((row, next))
}

pub fn mark(reg: &mut Registry, id: &str, words: bool, file: bool, at: Option<&str>) -> Result<(), Fault> {
    let Some(r) = reg.rows.iter_mut().find(|r| r.id == id) else {
        return Err(Fault::known(Known::NoIdentity, id.to_string()));
    };
    r.backed_words |= words;
    r.backed_file |= file;
    if let Some(p) = at {
        r.backup_at = p.to_string();
    }
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackupSeen {
    pub at: String,
    pub marked: bool,
    pub exists: bool,
    pub opens: bool,
}

pub fn backup_seen(row: &Row) -> BackupSeen {
    let mut out = BackupSeen { at: row.backup_at.clone(), marked: row.backed_file, ..BackupSeen::default() };
    if row.backup_at == NO_BACKUP_AT {
        return out;
    }
    let p = Path::new(&row.backup_at);
    let Ok(bytes) = std::fs::read(p) else { return out };
    out.exists = true;
    let Ok(shape) = crate::keystore::shape(&bytes) else { return out };
    let want: Vec<Address> = row.keys.seats().iter().filter_map(|s| row.current(*s)).collect();
    out.opens = shape.compliant().is_empty() && shape.address.map(|a| want.contains(&a)).unwrap_or(false);
    out
}

pub fn recover_with_words(words: &str, new_pin: &str) -> Result<(), Fault> {
    let f = from_words(words)?;
    recover_with(&f, new_pin)
}

pub fn recover_with(f: &Fresh, new_pin: &str) -> Result<(), Fault> {
    crate::keybox::recover(&f.entropy, new_pin)
}

pub fn words_of(view: &Registry, id: &str) -> Result<Vec<crate::secret::Secret>, Fault> {
    let row = view.find(id).ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    if row.kind() != Kind::Words {
        return Err(Fault::known(Known::NoWords, id.to_string()));
    }
    let acct = crate::places::seed_slot(id);
    let Some(mut raw) = crate::keybox::get(&acct)? else {
        return Err(Fault::known(Known::KeychainMissing, acct));
    };
    if raw.len() != ENTROPY_BYTES {
        return Err(Fault::known(Known::KeyMalformed, acct));
    }
    let mut e = [0u8; ENTROPY_BYTES];
    e.copy_from_slice(&raw);
    zikaron_os::wipe(&mut raw);
    let f = Fresh { entropy: e, picks: [0, 1, 2] };
    Ok(f.words())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(a: u8, kind: Kind) -> Row {
        let addr = Address([a; 20]);
        let keys = match kind {
            Kind::Words => Keys::Both { author: addr, grantee: Address([a.wrapping_add(1); 20]) },
            Kind::Existing => Keys::One { seat: Role::Author, addr },
        };
        Row {
            id: addr.hex(),
            keys,
            author_office: None,
            grantee_office: None,
            author_home: format!("/x/{a}/author"),
            grantee_home: match kind {
                Kind::Words => format!("/x/{a}/grantee"),
                Kind::Existing => UNSEATED.to_string(),
            },
            backup_at: NO_BACKUP_AT.to_string(),
            label: NO_LABEL.to_string(),
            created: NO_CREATED.to_string(),
            backed_words: kind == Kind::Words,
            backed_file: false,
            network: None,
            unread_network: None,
            reroot_of: None,
        }
    }

    #[test]
    fn delete_is_allowed_only_with_a_backup_or_a_handover() {
        let bare = row(5, Kind::Existing);
        let refused = delete_allowed(&bare, false).unwrap_err();
        assert_eq!((refused.which(), refused.tail()), (Some(Known::DeleteUnbacked), bare.id.as_str()));
        assert!(delete_allowed(&bare, true).is_ok(), "a handover on record");
        let mut filed = row(6, Kind::Existing);
        filed.backed_file = true;
        assert!(delete_allowed(&filed, false).is_ok(), "a key file backup");
        assert!(delete_allowed(&row(8, Kind::Words), false).is_ok(), "words written down");
    }

    #[test]
    fn registry_round_trips_without_key_material() {
        let mut chose = row(9, Kind::Existing);
        chose.network = Some(Chosen::Custom(Some(crate::deploy::Custom { chain_id: 31337, registry: Address([3; 20]), from_block: 7, endpoints: vec!["31337=https://n.example".into()] })));
        let mut row_named = row(11, Kind::Words);
        row_named.network = Some(Chosen::named(crate::deploy::DEFAULT));
        let reg = Registry { current: Some((row(7, Kind::Words).id, Role::Grantee)), rows: vec![row(7, Kind::Words), chose, row_named], left: Vec::new() };
        let b = reg.to_bytes();
        assert_eq!(Registry::parse(&b).unwrap(), reg);
        let text = String::from_utf8(b).unwrap();
        for k in ["seed", "secret", "private", "mnemonic", "entropy", "phrase", "ciphertext"] {
            assert!(!text.contains(k), "登记表里出现了 {k}");
        }
    }

    #[test]
    fn registry_refuses_named_columns() {
        let reg = Registry { current: None, rows: vec![row(7, Kind::Words)], left: Vec::new() };
        let t = String::from_utf8(reg.to_bytes()).unwrap();
        assert_eq!(Registry::parse(t.replace("\"words\",", "\"wordz\",").as_bytes()).unwrap_err(), "kind");
        let bad = t.replacen("{\"identities\"", "{\"current\":{\"identity\":\"0x01\",\"seat\":\"author\"},\"identities\"", 1);
        assert_eq!(Registry::parse(bad.as_bytes()).unwrap_err(), "current");
    }

    #[test]
    fn a_network_cell_this_build_lacks_is_carried() {
        let mut r = row(7, Kind::Words);
        r.network = Some(Chosen::named(crate::deploy::DEFAULT));
        let reg = Registry { current: None, rows: vec![r], left: Vec::new() };
        let t = String::from_utf8(reg.to_bytes()).unwrap();
        let off = t.replace(&format!("\"name\":\"{}\"", crate::deploy::DEFAULT), "\"name\":\"elsewhere\"");
        assert_ne!(off, t);
        let back = Registry::parse(off.as_bytes()).unwrap();
        assert_eq!(back.rows[0].network, Some(Chosen::Row("elsewhere".into())));
        assert!(back.rows[0].network_now().is_none());
        assert_eq!(String::from_utf8(back.to_bytes()).unwrap(), off);
        let nameless = t.replace(&format!("{{\"name\":\"{}\"}}", crate::deploy::DEFAULT), "{}");
        assert_ne!(nameless, t);
        assert_eq!(Registry::parse(nameless.as_bytes()).unwrap().rows[0].network, None);
    }

    #[test]
    fn a_custom_cell_this_build_cannot_read_is_written_back_whole() {
        let mut r = row(7, Kind::Words);
        r.network = Some(Chosen::named(crate::deploy::CUSTOM));
        let reg = Registry { current: None, rows: vec![r], left: Vec::new() };
        let t = String::from_utf8(reg.to_bytes()).unwrap();
        let later = format!("{{\"chainId\":31337,\"endpoints\":[\"31337=https://n.example\"],\"fromBlock\":\"0x10\",\"name\":\"{}\",\"registry\":\"0x{}\"}}", crate::deploy::CUSTOM, "11".repeat(20));
        let off = t.replace(&format!("{{\"name\":\"{}\"}}", crate::deploy::CUSTOM), &later);
        assert_ne!(off, t);
        let mut back = Registry::parse(off.as_bytes()).unwrap();
        assert_eq!(back.rows[0].network, Some(Chosen::Custom(None)));
        assert_eq!(String::from_utf8(back.to_bytes()).unwrap(), off, "carried whole");
        let id = back.rows[0].id.clone();
        let mut filled = Registry::parse(off.as_bytes()).unwrap();
        let c = crate::deploy::Custom { chain_id: 5, registry: crate::key::Address([0x22; 20]), from_block: 3, endpoints: vec!["5=https://m.example".to_string()] };
        assert_eq!(remember_custom(&mut filled, &id, c).ok(), Some(true));
        let written = String::from_utf8(filled.to_bytes()).unwrap();
        assert!(!written.contains("0x10") && !written.contains("n.example"), "the carried cell is gone: {written}");
        assert!(written.contains("m.example") && written.contains(&"22".repeat(20)), "what was filled in is written: {written}");
        set_network(&mut back, &id, crate::deploy::DEFAULT).unwrap();
        assert!(!String::from_utf8(back.to_bytes()).unwrap().contains("0x10"), "a new choice replaces it");
    }

    #[test]
    fn the_network_cell_reads_into_one_choice() {
        let base = String::from_utf8(Registry { current: None, rows: vec![row(7, Kind::Words)], left: Vec::new() }.to_bytes()).unwrap();
        let values = format!("\"chainId\":31337,\"endpoints\":[\"31337=https://n.example\"],\"fromBlock\":7,\"registry\":\"0x{}\"", "33".repeat(20));
        let with = |cell: Option<String>| {
            let t = match &cell {
                Some(c) => base.replacen("\"slot\":", &format!("\"network\":{c},\"slot\":"), 1),
                None => base.clone(),
            };
            let r = Registry::parse(t.as_bytes()).unwrap().rows[0].clone();
            (r.network, r.unread_network.is_some())
        };
        let filled = crate::deploy::Custom { chain_id: 31337, registry: Address([0x33; 20]), from_block: 7, endpoints: vec!["31337=https://n.example".into()] };
        let d = crate::deploy::DEFAULT;
        assert_eq!(with(Some(format!("{{\"name\":\"{d}\"}}"))), (Some(Chosen::Row(d.into())), false), "a row alone");
        assert_eq!(with(Some(format!("{{\"name\":\"{}\"}}", crate::deploy::CUSTOM))), (Some(Chosen::Custom(None)), false), "custom alone");
        assert_eq!(with(Some(format!("{{{values},\"name\":\"{}\"}}", crate::deploy::CUSTOM))), (Some(Chosen::Custom(Some(filled.clone()))), false), "custom with its values");
        assert_eq!(with(None), (None, false), "no cell");
        assert_eq!(with(Some(format!("{{{values}}}"))), (None, true), "values with no name: no choice, carried");
        assert_eq!(with(Some(format!("{{\"chainId\":\"x\",\"name\":\"{}\"}}", crate::deploy::CUSTOM))), (Some(Chosen::Custom(None)), true), "custom whose values do not read");
        assert_eq!(with(Some(format!("{{{values},\"name\":\"{d}\"}}"))), (Some(Chosen::Row(d.into())), true), "values beside a row: the row");
        let mut reg = Registry { current: None, rows: vec![row(7, Kind::Words)], left: Vec::new() };
        reg.rows[0].network = Some(Chosen::Custom(Some(filled)));
        let id = reg.rows[0].id.clone();
        assert_eq!(set_network(&mut reg, &id, d).unwrap().network, Some(Chosen::Row(d.into())), "a row chosen: custom's values go with it");
        assert_eq!(remember_custom(&mut reg, &id, crate::deploy::Custom { chain_id: 1, registry: Address([1; 20]), from_block: 0, endpoints: Vec::new() }).ok(), Some(false), "a row takes no hand-filled values");
        let custom = crate::deploy::CUSTOM;
        assert_eq!(set_network(&mut reg, &id, custom).unwrap().network, Some(Chosen::Custom(None)), "custom chosen after a row: nothing filled in yet");
        let again = crate::deploy::Custom { chain_id: 5, registry: Address([5; 20]), from_block: 1, endpoints: vec!["5=https://m.example".into()] };
        assert_eq!(remember_custom(&mut reg, &id, again.clone()).ok(), Some(true), "custom takes what is filled in");
        assert_eq!(set_network(&mut reg, &id, custom).unwrap().network, Some(Chosen::Custom(Some(again))), "custom chosen again: what was filled in stays");
        assert_eq!(set_network(&mut reg, &id, d).unwrap().network, Some(Chosen::Row(d.into())));
        assert_eq!(set_network(&mut reg, &id, custom).unwrap().network, Some(Chosen::Custom(None)), "custom after a row again: the earlier values went with their choice");
    }
}
