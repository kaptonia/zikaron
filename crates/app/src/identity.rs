//! Identities. An identity is either the two seat keys derived from twelve recovery words along the family
//! paths, or one adopted existing key.
//!
//! Where things live:
//!
//! * keys and seeds only in the key vault, one slot per key, slot names built by `places` (the account
//!   carries the address);
//! * a registry in the machine directory (`places::registry_file`): kind, the two seat addresses and homes,
//!   and whether a backup was made. It holds no key material: no byte of a private key, seed or word;
//! * one home per seat; new identities' homes are placed by `home::identity_home`.
//!
//! Machines without a registry (older versions) have only the random anchor key at the account base. It is
//! read as an existing identity without writing anything: the slot is not moved, deleted or renamed, both
//! seats use it, the home stays, the address is unchanged. The first identity action (new, import, backup,
//! switch, delete) writes that row to the registry along with its own change.
//!
//! Keys first, then the registry: a new identity's keys and seed go into the vault and are read back and
//! checked against the address before the registry row is written, so no registered identity lacks its keys.
//! Deleting goes the other way (keys first, then the row), and neither seat's home loses a byte (records
//! only grow).

use crate::fault::{Fault, Known};
use crate::family::ENTROPY_BYTES;
use crate::key::{Address, Secret};
use crate::roles::Role;
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};

/// Registry shape name, defined once.
pub const SHAPE: &str = "zikaron-desk/identities/1";

/// The two kinds of identity. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Two seat keys derived from recovery words.
    Words,
    /// An adopted existing key (no recovery words; backed up only as a file).
    Existing,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Words => "words",
            Kind::Existing => "existing",
        }
    }

    fn parse(s: &str) -> Option<Kind> {
        [Kind::Words, Kind::Existing].into_iter().find(|k| k.as_str() == s)
    }
}

/// Which kind of slot a key lives in. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    /// The account-base key (the older random anchor key; used as is when adopted as an existing identity).
    Base,
    /// One slot per key, by address.
    Own,
}

impl Slot {
    pub fn as_str(self) -> &'static str {
        match self {
            Slot::Base => "base",
            Slot::Own => "own",
        }
    }

    fn parse(s: &str) -> Option<Slot> {
        [Slot::Base, Slot::Own].into_iter().find(|k| k.as_str() == s)
    }
}

fn seat_of(s: &str) -> Option<Role> {
    Role::ALL.into_iter().find(|r| r.as_str() == s)
}

/// Which seats an identity's keys occupy. Closed.
///
/// One key, one seat. With the same key on both seats, grantee-seat signatures would use the author's key,
/// and a reader of the ledger could not tell an author's receipt from a grantee's entry. This type cannot
/// express two seats sharing one address except for the read-only [`Keys::Legacy`] shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Keys {
    /// Two seat keys derived from recovery words. The paths differ, so the two addresses always differ
    /// (`parse` checks).
    Both { author: Address, grantee: Address },
    /// An adopted existing key: only the seat chosen at import; the other stays empty.
    One { seat: Role, addr: Address },
    /// How an older machine's account-base random anchor key reads: both seats share it. Nothing creates this
    /// shape anew (import makes [`Keys::One`], building makes [`Keys::Both`]); it only lets an older machine
    /// be read, and its slot name stays the account base.
    Legacy { addr: Address },
}

impl Keys {
    /// This seat's address; `None` for an empty seat.
    pub fn address(&self, seat: Role) -> Option<Address> {
        match (self, seat) {
            (Keys::Both { author, .. }, Role::Author) => Some(*author),
            (Keys::Both { grantee, .. }, Role::Grantee) => Some(*grantee),
            (Keys::One { seat: s, addr }, _) if *s == seat => Some(*addr),
            (Keys::One { .. }, _) => None,
            (Keys::Legacy { addr }, _) => Some(*addr),
        }
    }

    /// Occupied seats (in [`Role::ALL`] order).
    pub fn seats(&self) -> Vec<Role> {
        Role::ALL.into_iter().filter(|r| self.address(*r).is_some()).collect()
    }

    /// The kind follows the key shape; it is not stored separately, since two records of one fact eventually
    /// disagree.
    pub fn kind(&self) -> Kind {
        match self {
            Keys::Both { .. } => Kind::Words,
            Keys::One { .. } | Keys::Legacy { .. } => Kind::Existing,
        }
    }

    /// Slot shape likewise: only rows read from older machines use the account base.
    pub fn slot(&self) -> Slot {
        match self {
            Keys::Legacy { .. } => Slot::Base,
            _ => Slot::Own,
        }
    }
}

/// An empty seat: its address and home fields hold this (read back as "unoccupied").
pub const UNSEATED: &str = "";
/// The name field of an unnamed identity (the screen falls back to its kind).
pub const NO_LABEL: &str = "";
/// The "created" field of rows written before it existed (the screen says "not recorded").
pub const NO_CREATED: &str = "";
/// The backup location field of rows with no backup file yet.
pub const NO_BACKUP_AT: &str = "";

/// One registry row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The identity's name: the address of an occupied seat (`0x` plus 40 lowercase hex); the author seat's
    /// when both are occupied.
    pub id: String,
    /// Which seats this identity's keys occupy (kind and slot shape follow from it).
    pub keys: Keys,
    /// The author seat's home; [`UNSEATED`] when empty.
    pub author_home: String,
    /// The grantee seat's home; [`UNSEATED`] when empty.
    pub grantee_home: String,
    /// The recovery words were confirmed, or it was imported from words.
    pub backed_words: bool,
    /// A keystore file was exported, or it was imported from one.
    pub backed_file: bool,
    /// Where the last exported key file was written; [`NO_BACKUP_AT`] if never. The flag records that it was
    /// done; this records where, so the disk can be checked.
    pub backup_at: String,
    /// The name the user gave. No decision reads it.
    pub label: String,
    /// When this row was created (the `keystore::utc` form). No decision reads it.
    pub created: String,
    /// The network this identity chose when it was made ([`Chosen`]: a row of `deploy::KNOWN`, or custom with
    /// what one seat filled in). `None` for rows made before identities chose one, until a writer opens a home
    /// and records this machine's row ([`backfill_network`]). Both seats' homes take it
    /// (`action::open_home_at`).
    pub network: Option<Chosen>,
    /// The network cell as read, when this version could not read it whole (a later version's form). Written
    /// back unchanged while the choice stands, so nothing is lost; dropped once the choice or the filled-in
    /// values change here.
    pub unread_network: Option<String>,
}

/// The network an identity chose: a row of the known deployments table by name (a name from a later version's
/// table is kept as written), or custom with what one seat filled in (`None` until complete). The type cannot
/// hold a row with hand-filled values, and choosing another network drops the filled-in values with the old
/// choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Chosen {
    Row(String),
    Custom(Option<crate::deploy::Custom>),
}

impl Chosen {
    /// The choice a name makes: `deploy::CUSTOM` is custom, nothing filled in yet; any other name a row.
    pub fn named(name: &str) -> Chosen {
        if name == crate::deploy::CUSTOM {
            Chosen::Custom(None)
        } else {
            Chosen::Row(name.to_string())
        }
    }

    /// The choice's name, as the table and the network cell spell it.
    pub fn name(&self) -> &str {
        match self {
            Chosen::Row(n) => n,
            Chosen::Custom(_) => crate::deploy::CUSTOM,
        }
    }

    /// What was filled in by hand, when the choice is custom and complete.
    pub fn custom(&self) -> Option<&crate::deploy::Custom> {
        match self {
            Chosen::Custom(c) => c.as_ref(),
            Chosen::Row(_) => None,
        }
    }
}

impl Row {
    /// This identity's kind (from [`Keys`], not stored separately).
    pub fn kind(&self) -> Kind {
        self.keys.kind()
    }

    /// Which kind of slot this identity's key lives in (from [`Keys`]).
    pub fn slot(&self) -> Slot {
        self.keys.slot()
    }

    /// The signing address of this seat; `None` for an empty seat.
    pub fn address(&self, seat: Role) -> Option<Address> {
        self.keys.address(seat)
    }

    /// The seats this row occupies (in [`Role::ALL`] order).
    pub fn seats(&self) -> Vec<Role> {
        self.keys.seats()
    }

    /// This seat's home; `None` for an empty seat.
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

    /// Which slot this seat's key lives in; `None` for an empty seat.
    pub fn account(&self, seat: Role) -> Option<String> {
        let a = self.address(seat)?;
        Some(match self.slot() {
            Slot::Base => crate::places::key_account().to_string(),
            Slot::Own => crate::places::key_slot(&a),
        })
    }

    /// Every slot this identity occupies (dropped one by one on delete).
    pub fn accounts(&self) -> Vec<String> {
        let mut v: Vec<String> = Role::ALL.iter().filter_map(|r| self.account(*r)).collect();
        v.dedup();
        if self.kind() == Kind::Words {
            v.push(crate::places::seed_slot(&self.id));
        }
        v
    }

    /// Whether any backup was made.
    pub fn backed(&self) -> bool {
        self.backed_words || self.backed_file
    }

    /// The first seat this row occupies. Entering an identity, or moving to the next one after a delete,
    /// lands here: landing on an empty seat would put the user on a seat with no key while the screen says
    /// "current identity".
    pub fn first_seat(&self) -> Role {
        self.keys.seats().first().copied().unwrap_or(Role::Author)
    }

    /// The network this identity's homes take: its chosen row, or what was filled in by hand; `None` when it
    /// chose none, or chose "custom" and nothing is filled in yet.
    pub fn network_now(&self) -> Option<crate::deploy::Network<'_>> {
        crate::deploy::resolve(self.network.as_ref().map(Chosen::name), self.network.as_ref().and_then(Chosen::custom))
    }

    /// Which of this row's seats has its home at `root`.
    pub fn seat_at(&self, root: &Path) -> Option<Role> {
        Role::ALL.into_iter().find(|s| self.home(*s).map(|h| crate::home::same_place(&h, root)).unwrap_or(false))
    }
}

/// The identities on this machine.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Registry {
    /// The current identity and seat.
    pub current: Option<(String, Role)>,
    pub rows: Vec<Row>,
    /// Homes a deleted identity left on disk (identity id, seat, home). They are still this machine's local
    /// data: a new master key reseals and renames them with the rest, and importing the identity again finds
    /// them where its homes are named.
    pub left: Vec<(String, Role, String)>,
}

impl Registry {
    /// The current row and seat.
    pub fn now(&self) -> Option<(&Row, Role)> {
        let (id, seat) = self.current.as_ref()?;
        self.find(id).map(|r| (r, *seat))
    }

    pub fn find(&self, id: &str) -> Option<&Row> {
        self.rows.iter().find(|r| r.id == id)
    }

    /// Canonical bytes (member order and spelling from the core's canonicalizer).
    ///
    /// Empty seats write [`UNSEATED`] in address and home; `kind` and `slot` are computed from [`Keys`] and
    /// checked field by field when read back.
    pub fn to_bytes(&self) -> Vec<u8> {
        let s = |x: &str| Value::Str(x.to_string());
        let obj = |m: Vec<(&str, Value)>| Value::Obj(m.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
        let seat_addr = |r: &Row, seat: Role| r.address(seat).map(|a| a.hex()).unwrap_or_else(|| UNSEATED.to_string());
        let rows: Vec<Value> = self
            .rows
            .iter()
            .map(|r| {
                let mut m = vec![
                    ("author", s(&seat_addr(r, Role::Author))),
                    (
                        "backup",
                        obj(vec![
                            ("at", s(&r.backup_at)),
                            ("file", Value::Bool(r.backed_file)),
                            ("words", Value::Bool(r.backed_words)),
                        ]),
                    ),
                    ("created", s(&r.created)),
                    ("grantee", s(&seat_addr(r, Role::Grantee))),
                    ("homes", obj(vec![("author", s(&r.author_home)), ("grantee", s(&r.grantee_home))])),
                    ("id", s(&r.id)),
                    ("kind", s(r.kind().as_str())),
                    ("label", s(&r.label)),
                    ("slot", s(r.slot().as_str())),
                ];
                // A cell this version could not read whole goes back as it came while its choice stands.
                let raw = r.unread_network.as_deref().and_then(|t| zikaron::json::parse(t.as_bytes()).ok());
                match (&r.network, raw) {
                    (Some(n), Some(raw)) if raw.member("name").and_then(|x| x.as_str()) == Some(n.name()) && n.custom().is_none() => {
                        m.push(("network", raw));
                    }
                    (Some(n), _) => m.push(("network", network_value(n))),
                    (None, _) => {}
                }
                obj(m)
            })
            .collect();
        let mut top = Vec::new();
        if let Some((id, seat)) = &self.current {
            top.push(("current", obj(vec![("identity", s(id)), ("seat", s(seat.as_str()))])));
        }
        top.push(("identities", Value::Arr(rows)));
        if !self.left.is_empty() {
            let left: Vec<Value> = self.left.iter().map(|(id, seat, home)| obj(vec![("home", s(home)), ("identity", s(id)), ("seat", s(seat.as_str()))])).collect();
            top.push(("left", Value::Arr(left)));
        }
        top.push(("shape", s(SHAPE)));
        json::canon_bytes(&obj(top))
    }

    /// Read back, naming the field that is wrong.
    ///
    /// Rows written by older versions still read:
    ///
    /// * missing `label`, `created` and `backup.at` take named defaults ([`NO_LABEL`], [`NO_CREATED`],
    ///   [`NO_BACKUP_AT`]);
    /// * recovery-word rows have different addresses and read as [`Keys::Both`];
    /// * the account-base row (`slot` is `base`) reads as [`Keys::Legacy`], both seats sharing it;
    /// * existing-key rows that wrote the same key to both seats (`slot` is `own` with equal addresses) read
    ///   as [`Keys::One`] on the author seat, leaving the grantee seat empty. No migration: the row reads
    ///   without crashing or being lost, and the user can delete and import again.
    pub fn parse(bytes: &[u8]) -> Result<Registry, String> {
        let v = json::parse(bytes).map_err(|t| format!("{t:?}"))?;
        let field = |v: &Value, k: &str| -> Option<Value> {
            match v {
                Value::Obj(m) => m.iter().find(|(n, _)| n == k).map(|(_, x)| x.clone()),
                _ => None,
            }
        };
        let text = |v: &Value, k: &str| -> Result<String, String> {
            match field(v, k) {
                Some(Value::Str(x)) => Ok(x),
                _ => Err(k.to_string()),
            }
        };
        // Missing means a named default (older rows lack these fields); present but malformed is refused by
        // name.
        let text_or = |v: &Value, k: &str, fallback: &str| -> Result<String, String> {
            match field(v, k) {
                None => Ok(fallback.to_string()),
                Some(Value::Str(x)) => Ok(x),
                Some(_) => Err(k.to_string()),
            }
        };
        let flag = |v: &Value, k: &str| -> Result<bool, String> {
            match field(v, k) {
                Some(Value::Bool(b)) => Ok(b),
                _ => Err(k.to_string()),
            }
        };
        if text(&v, "shape")? != SHAPE {
            return Err("shape".into());
        }
        let Some(Value::Arr(items)) = field(&v, "identities") else {
            return Err("identities".into());
        };
        let mut rows = Vec::new();
        for it in &items {
            // A seat address field: [`UNSEATED`] means the seat is empty; otherwise an address, or refused by
            // name.
            let seat_addr = |k: &str| -> Result<Option<Address>, String> {
                let raw = text(it, k)?;
                if raw == UNSEATED {
                    return Ok(None);
                }
                Address::parse(&raw).map(Some).ok_or_else(|| k.to_string())
            };
            let backup = field(it, "backup").ok_or("backup")?;
            let homes = field(it, "homes").ok_or("homes")?;
            let slot = Slot::parse(&text(it, "slot")?).ok_or("slot")?;
            let kind = Kind::parse(&text(it, "kind")?).ok_or("kind")?;
            let author = seat_addr("author")?;
            let grantee = seat_addr("grantee")?;
            let keys = match (author, grantee, slot) {
                (Some(a), Some(g), Slot::Base) if a == g => Keys::Legacy { addr: a },
                // The old existing-key shape: both seats the same address with its own slot. The grantee
                // seat becomes empty.
                (Some(a), Some(g), Slot::Own) if a == g => Keys::One { seat: Role::Author, addr: a },
                (Some(a), Some(g), Slot::Own) => Keys::Both { author: a, grantee: g },
                (Some(a), None, Slot::Own) => Keys::One { seat: Role::Author, addr: a },
                (None, Some(g), Slot::Own) => Keys::One { seat: Role::Grantee, addr: g },
                // Both seats empty, or an account-base row with two addresses: neither has a reading.
                _ => return Err("author".into()),
            };
            // The kind field is computed from the key shape; a stored value that differs is refused by name.
            if keys.kind() != kind {
                return Err("kind".into());
            }
            if keys.slot() != slot {
                return Err("slot".into());
            }
            let row = Row {
                id: text(it, "id")?,
                keys,
                author_home: text(&homes, "author")?,
                grantee_home: text(&homes, "grantee")?,
                backed_words: flag(&backup, "words")?,
                backed_file: flag(&backup, "file")?,
                backup_at: text_or(&backup, "at", NO_BACKUP_AT)?,
                label: text_or(it, "label", NO_LABEL)?,
                created: text_or(it, "created", NO_CREATED)?,
                network: None,
                unread_network: None,
            };
            // The network cell is read leniently and carried. A cell whose name this build lacks (a row of a
            // later table) keeps that name, so the row still has a choice: no network resolves from it, its
            // homes keep what they have, nothing overwrites it, and writing the table writes the name back. A
            // cell without a name is no choice. One cell never makes the whole table unreadable.
            let (network, unread_network) = match field(it, "network") {
                Some(v) => match network_of(&v) {
                    Some(c) => (Some(c), None),
                    None => (
                        v.member("name").and_then(|n| n.as_str()).filter(|n| !n.is_empty()).map(Chosen::named),
                        Some(String::from_utf8_lossy(&zikaron::json::canon_bytes(&v)).to_string()),
                    ),
                },
                None => (None, None),
            };
            let row = Row { network, unread_network, ..row };
            // The name is the address of an occupied seat: the author seat's when both are occupied,
            // otherwise the one seat's.
            let named = match row.keys {
                Keys::Both { author, .. } => author.hex(),
                Keys::One { addr, .. } | Keys::Legacy { addr } => addr.hex(),
            };
            if row.id != named {
                return Err("id".into());
            }
            if rows.iter().any(|r: &Row| r.id == row.id) {
                return Err(format!("id {}", row.id));
            }
            rows.push(row);
        }
        let current = match field(&v, "current") {
            None => None,
            Some(c) => {
                let id = text(&c, "identity")?;
                let seat = seat_of(&text(&c, "seat")?).ok_or("seat")?;
                if !rows.iter().any(|r| r.id == id) {
                    return Err("current".into());
                }
                Some((id, seat))
            }
        };
        let mut left = Vec::new();
        match field(&v, "left") {
            None => {}
            Some(Value::Arr(a)) => {
                for it in &a {
                    let seat = seat_of(&text(it, "seat")?).ok_or("left seat")?;
                    left.push((text(it, "identity")?, seat, text(it, "home")?));
                }
            }
            Some(_) => return Err("left".into()),
        }
        Ok(Registry { current, rows, left })
    }
}

fn base() -> String {
    crate::places::key_account().to_string()
}

/// Which row and seat is current now, including the account-base key.
///
/// Every path that needs the current identity asks this one function, which follows "the current row", not
/// "the first row of the table".
///
/// It reads the table it is given ([`view`]: without a registry, the account-base key reads as the current
/// row). Registered rows only are a separate question (`register::now_row_listed`).
pub fn now_row(view: &Registry) -> Option<(Row, Role)> {
    // Mark the trace here too, so direct calls that bypass `apply` (tests, the CLI) are traced.
    crate::trace::mark(crate::feature::Feature::H2);
    view.now().map(|(r, s)| (r.clone(), s))
}

/// Which slot signs now. Without a registry (or without the current identity in it), the account base.
///
/// An empty current seat is `None` (an existing key holds one seat; the other has no key). It never falls
/// back to the account base, which would silently sign for this seat with another identity's key.
///
/// Which row is current is answered by [`now_row`]. Without a registry the view holds the account-base row,
/// whose keys are `Keys::Legacy` in `Slot::Base`, so both seats answer with the account base.
pub fn account_now(view: &Registry) -> Option<String> {
    match now_row(view) {
        Some((row, seat)) => row.account(seat),
        None => Some(base()),
    }
}

/// Which seat to land on at start. Same rule as entering or switching identities: the first seat the identity
/// occupies (`Row::first_seat`).
///
/// If the registry points at an empty seat (the app was last closed there), it lands on the first occupied
/// seat and records that in the given table; an occupied seat stays. The empty seat is reached only when the
/// user goes there; starting on it would make the first-run wizard offer to create an identity the user
/// already has. `listed` is the register as read (`None` without one); writing the table back is the
/// caller's job.
pub fn land_at_boot(listed: &mut Registry) -> Result<Option<(Row, Role)>, Fault> {
    let Some((row, seat)) = listed.now().map(|(r, s)| (r.clone(), s)) else { return Ok(None) };
    if row.address(seat).is_some() {
        return Ok(Some((row, seat)));
    }
    let landed = row.first_seat();
    let row = switch(listed, &row.id, landed)?;
    Ok(Some((row, landed)))
}

/// Which home to open now: the current identity's seat home when the registry has one; without a registry,
/// the three-level resolution.
///
/// An empty current seat is `None`: that seat has no home, and there is no fallback to another, which would
/// put the user into another seat's home (or the machine pointer's) for every page to read. This is the same
/// fact `Shell::close_home` acts on, answered here once. `listed` is the register as read.
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

/// The identity row the account-base key reads as (not written).
fn base_row() -> Result<Option<Row>, Fault> {
    // A closed vault answers "unreadable now", not an error. This runs on the startup path that reads
    // identities and needs key bytes, which need the master key; erroring would greet a machine with no
    // passcode with an unactionable "the vault is locked" toast on its first frame. As with
    // `Shell::refresh_anchor`, reads the product starts itself answer "unavailable" while the vault is closed.
    if !crate::keybox::state()?.keys_ready() {
        return Ok(None);
    }
    let Some(s) = crate::key::load_at(&base())? else {
        return Ok(None);
    };
    let a = s
        .address()
        .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string()))?;
    let home = crate::home::where_is()?.display().to_string();
    Ok(Some(Row {
        id: a.hex(),
        keys: Keys::Legacy { addr: a },
        author_home: home.clone(),
        grantee_home: home,
        backed_words: false,
        backed_file: false,
        backup_at: NO_BACKUP_AT.to_string(),
        label: NO_LABEL.to_string(),
        created: NO_CREATED.to_string(),
        network: None,
        unread_network: None,
    }))
}

/// The identities on this machine now: the registry if present (`listed`, as read); otherwise the
/// account-base key read as an existing identity (if there).
pub fn view(listed: Option<Registry>, seat: Role) -> Result<Registry, Fault> {
    if let Some(r) = listed {
        return Ok(r);
    }
    Ok(match base_row()? {
        Some(row) => Registry { current: Some((row.id.clone(), seat)), rows: vec![row], left: Vec::new() },
        None => Registry::default(),
    })
}

/// Words not yet built into an identity (memory only, until confirmed). Wiped when dropped.
pub struct Fresh {
    entropy: [u8; ENTROPY_BYTES],
    /// The three random cells for copy confirmation (zero-based, ascending).
    pub picks: [usize; 3],
}

impl Fresh {
    /// A copy handed to the background task (each wiped when dropped). Not `Clone`: copying a secret must be
    /// written out at the call site.
    pub fn twin(&self) -> Fresh {
        Fresh { entropy: self.entropy, picks: self.picks }
    }
}

impl Drop for Fresh {
    fn drop(&mut self) {
        for b in self.entropy.iter_mut() {
            unsafe { std::ptr::write_volatile(b, 0) };
        }
    }
}

impl Fresh {
    /// The twelve words in order, each held in the secret type (one fixed block, zeroed when cleared or
    /// dropped), so the copy shown to the user is protected like the words themselves. The phrase they are
    /// split from is zeroed once split.
    pub fn words(&self) -> Vec<crate::secret::Secret> {
        let mut phrase = crate::cryptx::phrase_of(&self.entropy);
        let words = phrase.split(' ').map(crate::secret::Secret::of).collect();
        // SAFETY: zeroes are valid UTF-8; the string is dropped right after.
        unsafe { zikaron_ui::secret::wipe(phrase.as_bytes_mut()) };
        words
    }

    /// The address this seat will derive.
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
    // Byte rejection sampling: values beyond the last full multiple of the word count are discarded so early
    // words are not favored.
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

/// Generate new words: 16 bytes of system entropy read as twelve English words.
pub fn fresh() -> Result<Fresh, Fault> {
    let raw = random(ENTROPY_BYTES)?;
    let mut entropy = [0u8; ENTROPY_BYTES];
    entropy.copy_from_slice(&raw);
    Ok(Fresh { entropy, picks: picks()? })
}

/// Read pasted words. Word count, word list and checksum are each refused by name.
pub fn from_words(text: &str) -> Result<Fresh, Fault> {
    match crate::cryptx::entropy_of(text) {
        Ok(entropy) => Ok(Fresh { entropy, picks: picks()? }),
        Err(crate::cryptx::PhraseTrouble::WordCount(n)) => Err(Fault::known(Known::PhraseWords, n.to_string())),
        Err(crate::cryptx::PhraseTrouble::NotAPhrase) => Err(Fault::known(Known::PhraseInvalid, String::new())),
    }
}

/// Confirm the copy: the three chosen cells are compared; a mismatch names the cell.
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

/// The registry row that collides with these addresses (`None` when none).
fn row_with(reg: &Registry, addrs: &[Address]) -> Option<Row> {
    reg.rows
        .iter()
        .find(|r| addrs.iter().any(|a| Role::ALL.into_iter().any(|seat| r.address(seat) == Some(*a))))
        .cloned()
}

/// What making or importing an identity with these keys and this name meets in the register: the one rule
/// both adding paths ([`add_words`], [`add_existing`]) and the import's pre-check (before a key file is
/// written) ask.
/// - Its keys on a row of the same kind whose slots are missing: that row, to be refilled (`Ok(Some)`).
/// - Its keys on a row otherwise (slots complete, or another kind): refused by name, nothing written —
///   [`Known::IdentityHereAs`] when a name was given that is not the row's (the row's name in the evidence;
///   the row is not renamed), else [`Known::IdentityExists`].
/// - Its keys on no row, and a name given that another row already has: refused by name
///   ([`Known::IdentityNameTaken`]), nothing written.
/// - Otherwise a new identity (`Ok(None)`).
///
/// A name is compared as written, surrounding whitespace dropped; an empty name counts as none given.
pub fn meets(reg: &Registry, keys: &Keys, label: &str) -> Result<Option<Row>, Fault> {
    let label = label.trim();
    let addrs: Vec<Address> = match keys {
        Keys::Both { author, grantee } => vec![*author, *grantee],
        Keys::One { addr, .. } | Keys::Legacy { addr } => vec![*addr],
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

/// Whether every slot this row occupies is in the vault. "This identity is already on this machine" is
/// judged by slots, not by the registry row: after a machine change, a vault reset or a test cleanup, the row
/// can remain while the slots are gone, and the identity can no longer sign here.
pub fn slots_present(row: &Row) -> Result<bool, Fault> {
    for acct in row.accounts() {
        // `present` checks slot names only (no master key), so it answers while the vault is locked or
        // absent, as the "key not in the local vault" line and the first-run lamps need.
        if !crate::keybox::present(&acct)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Fill in each missing slot (present ones are untouched).
fn fill_key(acct: &str, s: &Secret) -> Result<(), Fault> {
    if !crate::keybox::present(acct)? {
        crate::key::install_at(acct, s)?;
    }
    Ok(())
}

/// After filling slots the row stays: slots are read back and checked, backup marks only accumulate, and the
/// current seat becomes the first one it occupies.
fn restored(reg: &mut Registry, mut row: Row, words: bool, file: bool) -> Result<Row, Fault> {
    verify_back(&row)?;
    row.backed_words |= words;
    row.backed_file |= file;
    if let Some(r) = reg.rows.iter_mut().find(|r| r.id == row.id) {
        *r = row.clone();
    }
    reg.current = Some((row.id.clone(), first_seat(&row)));
    Ok(row)
}

/// The first seat this row occupies. Landing a grantee-only identity on the author seat would put the user on
/// a seat with no key while the screen says "current identity".
fn first_seat(row: &Row) -> Role {
    row.first_seat()
}

/// Reads keys back once built and compares: the system saying it stored a key is only a claim; the vault is
/// the source of truth.
fn verify_back(row: &Row) -> Result<(), Fault> {
    for seat in row.keys.seats() {
        let (Some(acct), Some(want)) = (row.account(seat), row.address(seat)) else { continue };
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
    // Imported again: the homes it left are its homes again (the same names under the same master key).
    reg.left.retain(|(i, _, _)| !i.eq_ignore_ascii_case(&row.id));
    reg.rows.push(row.clone());
    reg.current = Some((row.id.clone(), first_seat(&row)));
    Ok(row)
}

/// Builds a recovery-word identity: one seed slot and one slot per seat key, read back and checked, then the
/// registry row in the given table (written by the caller, `register::change`); the current seat becomes its
/// author seat. Collisions with the register are judged by [`meets`] under the given name (`label`).
pub fn add_words(reg: &mut Registry, f: &Fresh, backed_words: bool, label: &str) -> Result<Row, Fault> {
    let malformed = || Fault::known(Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string());
    let author = crate::key::derived(&f.entropy, Role::Author).ok_or_else(malformed)?;
    let grantee = crate::key::derived(&f.entropy, Role::Grantee).ok_or_else(malformed)?;
    let a = author.address().ok_or_else(malformed)?;
    let g = grantee.address().ok_or_else(malformed)?;
    // The same words and kind with missing slots: fill the slots, keep the row. Anything else the register
    // already has is refused by name (`meets`).
    if let Some(had) = meets(reg, &Keys::Both { author: a, grantee: g }, label)? {
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
    let id = a.hex();
    let row = Row {
        author_home: crate::home::identity_home(&id, Role::Author)?.display().to_string(),
        grantee_home: crate::home::identity_home(&id, Role::Grantee)?.display().to_string(),
        id,
        keys: Keys::Both { author: a, grantee: g },
        backed_words,
        backed_file: false,
        backup_at: NO_BACKUP_AT.to_string(),
        label: NO_LABEL.to_string(),
        created: stamp(),
        network: None,
        unread_network: None,
    };
    crate::keybox::put(&crate::places::seed_slot(&row.id), &f.entropy)?;
    // The recovery seal: only the primary identity has one. The first identity on this machine becomes
    // primary (after a forgotten passcode or too many failures, these words reopen the master key); every
    // later one is secondary and gets none.
    crate::keybox::add_recovery(&row.id, crate::keybox::PrimaryKind::Words, &f.entropy)?;
    crate::key::install_at(&crate::places::key_slot(&a), &author)?;
    crate::key::install_at(&crate::places::key_slot(&g), &grantee)?;
    verify_back(&row)?;
    enroll(reg, row)
}

/// Adopts an existing key (imported private key or keystore file). One slot, only on `seat`; the other seat
/// stays empty.
///
/// Writing the same key to both seats would make grantee-seat signatures use the author's key, and a reader
/// of the ledger could not tell an author's receipt from a grantee's entry. For keys on both seats, import
/// another key or use a recovery-word identity.
pub fn add_existing(reg: &mut Registry, s: &Secret, seat: Role, backed_file: bool, label: &str) -> Result<Row, Fault> {
    let a = s
        .address()
        .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string()))?;
    // The same key, same seat, slot missing: fill the slot, keep the row. Anything else is refused by name
    // (`meets`), including the same key trying to take the other seat (one key, one seat).
    if let Some(had) = meets(reg, &Keys::One { seat, addr: a }, label)? {
        fill_key(&crate::places::key_slot(&a), s)?;
        if !crate::keybox::has_recovery(&had.id)? {
            crate::key::seal_recovery(&had.id, s)?;
        }
        return restored(reg, had, false, backed_file);
    }
    let id = a.hex();
    // An empty seat has no home: a home is where a seat writes, a seat without a key writes nothing, and an
    // empty directory would suggest the seat is in use.
    let home = crate::home::identity_home(&id, seat)?.display().to_string();
    let (author_home, grantee_home) = match seat {
        Role::Author => (home, UNSEATED.to_string()),
        Role::Grantee => (UNSEATED.to_string(), home),
    };
    let row = Row {
        author_home,
        grantee_home,
        id,
        keys: Keys::One { seat, addr: a },
        backed_words: false,
        backed_file,
        backup_at: NO_BACKUP_AT.to_string(),
        label: NO_LABEL.to_string(),
        created: stamp(),
        network: None,
        unread_network: None,
    };
    crate::key::install_at(&crate::places::key_slot(&a), s)?;
    // The recovery seal, only when this identity becomes primary (the first on this machine). An
    // existing-key identity has no words; it recovers with its exported keystore file and password.
    crate::key::seal_recovery(&row.id, s)?;
    verify_back(&row)?;
    enroll(reg, row)
}

/// When this row was created, spelled by `keystore::utc`. No decision reads it; the screen uses it only to
/// help recognize the identity.
fn stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    crate::keystore::utc(secs)
}

/// Renames an identity. Only the name field changes (no decision reads it).
pub fn rename(reg: &mut Registry, id: &str, label: &str) -> Result<Row, Fault> {
    let Some(r) = reg.rows.iter_mut().find(|r| r.id == id) else {
        return Err(Fault::known(Known::NoIdentity, id.to_string()));
    };
    r.label = label.trim().to_string();
    Ok(r.clone())
}

/// Records the network an identity chose (a row name, or `deploy::CUSTOM`) when it has none yet: a new row
/// gets the choice of the sheet that made it; an identity imported again keeps its own. A name outside the
/// choices is refused by name and nothing changes.
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

/// Records the known row `d` (this machine's choice, which their homes took when first opened) on rows made
/// before identities chose a network. `action::open_home_at` asks once per row; a row with a choice is never
/// changed. A row whose homes already hold another network (`agrees` says no) is left without a choice:
/// recording `d` would put its two seats on two chains. Returns how many rows were recorded.
pub fn backfill_network(reg: &mut Registry, d: &crate::deploy::Deployment, agrees: impl Fn(&Row) -> bool) -> usize {
    let mut n = 0;
    for r in reg.rows.iter_mut().filter(|r| r.network.is_none()) {
        if !agrees(r) {
            continue;
        }
        r.network = Some(Chosen::Row(d.name.to_string()));
        n += 1;
    }
    n
}

/// Sets the network an identity chose, replacing the old one (the wizard's network step, which decides the
/// network of the identity it made). Hand-filled values go with the old choice.
pub fn set_network(reg: &mut Registry, id: &str, name: &str) -> Result<Row, Fault> {
    if !crate::deploy::is_choice(name) {
        return Err(Fault::known(Known::IdentitiesShape, name.to_string()));
    }
    let Some(r) = reg.rows.iter_mut().find(|r| r.id == id) else {
        return Err(Fault::known(Known::NoIdentity, id.to_string()));
    };
    // Hand-filled values belong to the custom choice, so replacing the choice drops them.
    if r.network.as_ref().map(Chosen::name) != Some(name) {
        r.network = Some(Chosen::named(name));
        r.unread_network = None;
    }
    Ok(r.clone())
}

/// Records what a seat filled in by hand for an identity that chose "custom" (the other seat takes it). An
/// identity that chose a row, or none, records nothing.
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

/// The registry's network cell: `{"name":…}`, plus the hand-filled values when present:
/// `{"chainId":…,"endpoints":["chain=url",…],"fromBlock":…,"name":"custom","registry":"0x…"}`.
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

/// The network cell read strictly; `None` when any part of it does not read, including hand-filled values
/// next to a row's name (a row carries none).
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

/// The registered row whose seat has its home at `root`, and that seat.
pub fn owner_of(reg: &Registry, root: &Path) -> Option<(Row, Role)> {
    reg.rows.iter().find_map(|r| r.seat_at(root).map(|s| (r.clone(), s)))
}

/// After a home moves, the registry field that pointed at the old place follows to the new one. The registry
/// records identity homes; moving only the pointer would reopen the old place on the next start and never
/// write the new one (splitting the ledger in two). Returns how many fields changed (the caller writes the
/// table when any did).
pub fn rehome(reg: &mut Registry, old: &Path, new: &Path) -> usize {
    let to = new.display().to_string();
    let mut n = 0;
    for r in reg.rows.iter_mut() {
        // An empty seat's home field is [`UNSEATED`], not a path: a move never touches it.
        if r.author_home != UNSEATED && crate::home::same_place(Path::new(&r.author_home), old) {
            r.author_home = to.clone();
            n += 1;
        }
        if r.grantee_home != UNSEATED && crate::home::same_place(Path::new(&r.grantee_home), old) {
            r.grantee_home = to.clone();
            n += 1;
        }
    }
    n
}

/// Switches the current identity and seat to the given pair in the given table.
pub fn switch(reg: &mut Registry, id: &str, seat: Role) -> Result<Row, Fault> {
    let row = reg.find(id).cloned().ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    reg.current = Some((row.id.clone(), seat));
    Ok(row)
}

/// Deletes an identity. The product checks the precondition: some backup was made, or (for the author) a
/// handover is on record; otherwise refused. Keys first, then the row; neither seat's home loses a byte.
/// Returns the deleted row and the current pair afterwards (if any identity remains). The row leaves the
/// given table; writing it is the caller's job (`register::change`), after the keys are gone.
pub fn delete(reg: &mut Registry, id: &str, handed_over: bool) -> Result<(Row, Option<(Row, Role)>), Fault> {
    let row = reg.find(id).cloned().ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    // The primary identity recovers the passcode, so it is not deleted directly; another one is made primary
    // first (`rekey::set_primary`). Checked before anything is touched.
    if crate::keybox::primary()?.map(|(p, _)| p.eq_ignore_ascii_case(&row.id)).unwrap_or(false) {
        return Err(Fault::known(Known::PrimaryDelete, id.to_string()));
    }
    if !row.backed() && !handed_over {
        return Err(Fault::known(Known::DeleteUnbacked, id.to_string()));
    }
    // A secondary identity opens nothing: its slots go and the master key stays.
    for acct in row.accounts() {
        crate::keybox::drop_item(&acct)?;
    }
    // A vault written before the primary was recorded may still hold a seal for it: dropped with it.
    crate::keybox::drop_recovery(&row.id)?;
    reg.rows.retain(|r| r.id != row.id);
    // Its homes stay on disk (they hold the ledger); the register keeps where, so they remain this machine's.
    for seat in Role::ALL {
        if let Some(h) = row.home(seat) {
            reg.left.retain(|(i, s, _)| !(i.eq_ignore_ascii_case(&row.id) && *s == seat));
            reg.left.push((row.id.clone(), seat, h.display().to_string()));
        }
    }
    if reg.current.as_ref().map(|(c, _)| c == &row.id).unwrap_or(true) {
        reg.current = reg.rows.first().map(|r| (r.id.clone(), first_seat(r)));
    }
    let next = reg.now().map(|(r, s)| (r.clone(), s));
    Ok((row, next))
}

/// Records a backup: the words were confirmed, or a keystore file was exported.
///
/// `at` is where the key file actually landed: the flag says it was done, this says where, so whether the
/// backup is on disk can be checked later ([`backup_seen`]).
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

/// Whether this identity's backup file is on disk now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackupSeen {
    /// The location the registry recorded ([`NO_BACKUP_AT`] if never).
    pub at: String,
    /// The registry flag "a key file was exported".
    pub marked: bool,
    /// A file actually exists at that path now.
    pub exists: bool,
    /// The file reads back as a keystore V3 whose address is this identity's address on this seat.
    pub opens: bool,
}

/// Looks on disk for the backup file.
///
/// The flag remembers that a backup was done; the disk is the current fact. After a backup that did not
/// land (disk full or read-only) or a file moved away, the flag stays true while the file is gone, and
/// deleting the identity on the flag's word would lose the key forever. This only reads: it returns memory
/// and fact separately and lets the caller decide.
///
/// `opens` checks only the file's shape and address (the keystore V3 `address` field), without the password:
/// the address needs no decryption, and the password stays with the user.
pub fn backup_seen(row: &Row) -> BackupSeen {
    let mut out = BackupSeen { at: row.backup_at.clone(), marked: row.backed_file, ..BackupSeen::default() };
    if row.backup_at == NO_BACKUP_AT {
        return out;
    }
    let p = Path::new(&row.backup_at);
    let Ok(bytes) = std::fs::read(p) else { return out };
    out.exists = true;
    let Ok(shape) = crate::keystore::shape(&bytes) else { return out };
    let want: Vec<Address> = row.keys.seats().iter().filter_map(|s| row.address(*s)).collect();
    out.opens = shape.compliant().is_empty() && shape.address.map(|a| want.contains(&a)).unwrap_or(false);
    out
}

/// Settles the primary identity of a vault written by an older version before one was recorded (the first
/// unlock after upgrading): the earliest recovery-word identity, or else the earliest one
/// (`keybox::settle_primary`). Returns the chosen id when this pass settled it. `listed` is the register as
/// read (`None` without one).
pub fn settle_primary(listed: Option<&Registry>) -> Result<Option<String>, Fault> {
    // Without a register there is no row to choose from, but the vault is still asked: if it already records
    // a primary, it drops the seals that are not that primary's.
    let rows: Vec<(String, crate::keybox::PrimaryKind, String)> = listed
        .map(|reg| reg.rows.as_slice())
        .unwrap_or(&[])
        .iter()
        .map(|r| {
            let k = if r.kind() == Kind::Words { crate::keybox::PrimaryKind::Words } else { crate::keybox::PrimaryKind::KeyFile };
            (r.id.clone(), k, r.created.clone())
        })
        .collect();
    crate::keybox::settle_primary(&rows)
}

/// Recovers this machine's vault with recovery words: the words pass the word list and checksum first;
/// entropy never leaves this module.
pub fn recover_with_words(words: &str, new_pin: &str) -> Result<(), Fault> {
    let f = from_words(words)?;
    recover_with(&f, new_pin)
}

/// As above, with the words already parsed (the action layer checks the words in the frame and sends
/// derivation to the background).
pub fn recover_with(f: &Fresh, new_pin: &str) -> Result<(), Fault> {
    let malformed = || Fault::known(Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string());
    let a = f.address(Role::Author).ok_or_else(malformed)?;
    let g = f.address(Role::Grantee).ok_or_else(malformed)?;
    let id = a.hex();
    let slots = vec![crate::places::seed_slot(&id), crate::places::key_slot(&a), crate::places::key_slot(&g)];
    crate::keybox::recover(&f.entropy, new_pin, &id, &slots)
}

/// This identity's twelve words, read from the seed slot now. An existing-key identity has no words and says
/// so by name. Called only by the show step, which passes the local passcode gate first (`Action::pin_asked`).
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
    for b in raw.iter_mut() {
        unsafe { std::ptr::write_volatile(b, 0) };
    }
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
        }
    }

    /// Registry round trip, and its bytes contain nothing resembling a private key or words (only addresses,
    /// paths, kinds, flags).
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

    /// A malformed row is named by field; a current identity pointing outside the table is refused.
    #[test]
    fn registry_refuses_named_columns() {
        let reg = Registry { current: None, rows: vec![row(7, Kind::Words)], left: Vec::new() };
        let t = String::from_utf8(reg.to_bytes()).unwrap();
        assert_eq!(Registry::parse(t.replace("\"words\",", "\"wordz\",").as_bytes()).unwrap_err(), "kind");
        let bad = t.replacen("{\"identities\"", "{\"current\":{\"identity\":\"0x01\",\"seat\":\"author\"},\"identities\"", 1);
        assert_eq!(Registry::parse(bad.as_bytes()).unwrap_err(), "current");
    }

    /// A network cell whose name this build lacks is carried: the row keeps the name (no network resolves
    /// from it), backfill skips it, and the table writes it back; a cell without a name is no choice.
    #[test]
    fn a_network_cell_this_build_lacks_is_carried() {
        let mut r = row(7, Kind::Words);
        r.network = Some(Chosen::named(crate::deploy::DEFAULT));
        let reg = Registry { current: None, rows: vec![r], left: Vec::new() };
        let t = String::from_utf8(reg.to_bytes()).unwrap();
        let off = t.replace(&format!("\"name\":\"{}\"", crate::deploy::DEFAULT), "\"name\":\"elsewhere\"");
        assert_ne!(off, t);
        let mut back = Registry::parse(off.as_bytes()).unwrap();
        assert_eq!(back.rows[0].network, Some(Chosen::Row("elsewhere".into())));
        assert!(back.rows[0].network_now().is_none());
        assert_eq!(backfill_network(&mut back, crate::deploy::named(crate::deploy::DEFAULT).unwrap(), |_| true), 0);
        assert_eq!(String::from_utf8(back.to_bytes()).unwrap(), off);
        let nameless = t.replace(&format!("{{\"name\":\"{}\"}}", crate::deploy::DEFAULT), "{}");
        assert_ne!(nameless, t);
        assert_eq!(Registry::parse(nameless.as_bytes()).unwrap().rows[0].network, None);
    }

    /// A custom network cell this build cannot read whole (a later version's form) is written back byte for
    /// byte; only a change made here (a new choice, or filled-in values) replaces it.
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
        // Values filled in here replace the carried cell too (the row still chose "custom").
        let mut filled = Registry::parse(off.as_bytes()).unwrap();
        let c = crate::deploy::Custom { chain_id: 5, registry: crate::key::Address([0x22; 20]), from_block: 3, endpoints: vec!["5=https://m.example".to_string()] };
        assert_eq!(remember_custom(&mut filled, &id, c).ok(), Some(true));
        let written = String::from_utf8(filled.to_bytes()).unwrap();
        assert!(!written.contains("0x10") && !written.contains("n.example"), "the carried cell is gone: {written}");
        assert!(written.contains("m.example") && written.contains(&"22".repeat(20)), "what was filled in is written: {written}");
        set_network(&mut back, &id, crate::deploy::DEFAULT).unwrap();
        assert!(!String::from_utf8(back.to_bytes()).unwrap().contains("0x10"), "a new choice replaces it");
    }

    /// Backfill records only rows with no choice; rows with a row name or "custom" keep theirs, and a second
    /// pass records nothing.
    #[test]
    fn backfill_records_only_rows_without_a_choice() {
        let other = crate::deploy::KNOWN.iter().find(|d| d.name != crate::deploy::DEFAULT).unwrap();
        let mut named = row(11, Kind::Words);
        named.network = Some(Chosen::named(other.name));
        let mut custom = row(9, Kind::Existing);
        custom.network = Some(Chosen::named(crate::deploy::CUSTOM));
        let mut reg = Registry { current: None, rows: vec![row(7, Kind::Words), named.clone(), custom.clone()], left: Vec::new() };
        let d = crate::deploy::named(crate::deploy::DEFAULT).unwrap();
        assert_eq!(backfill_network(&mut reg, d, |_| true), 1);
        assert_eq!(reg.rows[0].network, Some(Chosen::Row(crate::deploy::DEFAULT.into())));
        assert_eq!(reg.rows[1], named);
        assert_eq!(reg.rows[2], custom);
        let once = reg.clone();
        assert_eq!(backfill_network(&mut reg, other, |_| true), 0);
        assert_eq!(reg, once);
        // A row whose homes hold another network is skipped.
        let mut held = Registry { current: None, rows: vec![row(7, Kind::Words)], left: Vec::new() };
        assert_eq!(backfill_network(&mut held, d, |_| false), 0);
        assert_eq!(held.rows[0].network, None);
    }

    /// Every network cell form older or later files write reads into one choice: a row name alone; custom with
    /// and without values; no cell; values with no name (no choice, carried); custom with unreadable values
    /// (custom, nothing filled in, carried); values next to a row name (the row, values ignored, carried).
    /// Choosing a row afterwards drops custom's values.
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
