//! The identity register's bytes, one reading for both ends: its rows with seats as addresses and words, homes
//! as written, and the network cell carried as it stands. The desktop turns this into its typed register (keys,
//! roles, the network choice it can resolve); the command line reads it to find which slot signs for a home.

use crate::envelope::SEATS;
use zikaron::json::{self, Value};

/// Registry shape name, defined once.
pub const SHAPE: &str = "zikaron-desk/identities/1";

/// The slot member every row writes: one slot per key, by address. Any other value is refused by name.
pub const SLOT_OWN: &str = "own";

/// An empty seat: its address and home fields hold this (read back as "unoccupied").
pub const UNSEATED: &str = "";
/// The name field of an unnamed identity (the screen falls back to its kind).
pub const NO_LABEL: &str = "";
/// The "created" field when no time is recorded (the screen says "not recorded").
pub const NO_CREATED: &str = "";
/// The backup location field of rows with no backup file yet.
pub const NO_BACKUP_AT: &str = "";

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

/// A seat word as the register spells it (one of `envelope::SEATS`).
pub fn seat_word(s: &str) -> Option<&'static str> {
    SEATS.into_iter().find(|w| *w == s)
}

/// An address as written: `0x` plus forty hex digits, case ignored.
pub fn address(s: &str) -> Option<[u8; 20]> {
    let low = s.trim().to_ascii_lowercase();
    if !zikaron::hexfmt::is_hex20(&low) {
        return None;
    }
    zikaron::hexfmt::decode(&low)?.try_into().ok()
}

/// One register row as written. At least one seat holds an address, and two seats never hold the same one.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// The address of an occupied seat: the author seat's when both are occupied.
    pub id: String,
    pub author: Option<[u8; 20]>,
    pub grantee: Option<[u8; 20]>,
    /// The seats' homes; [`UNSEATED`] when empty.
    pub author_home: String,
    pub grantee_home: String,
    pub backed_words: bool,
    pub backed_file: bool,
    /// [`NO_BACKUP_AT`] if never.
    pub backup_at: String,
    /// [`NO_LABEL`] when unnamed.
    pub label: String,
    /// [`NO_CREATED`] when not recorded.
    pub created: String,
    /// The network cell as it stands (`None` without one). Reading it is the desktop's business.
    pub network: Option<Value>,
}

impl Line {
    /// The kind follows the seats: both is recovery words, one is an existing key.
    pub fn kind(&self) -> Kind {
        if self.author.is_some() && self.grantee.is_some() {
            Kind::Words
        } else {
            Kind::Existing
        }
    }

    /// This seat's address (`seat` one of `envelope::SEATS`).
    pub fn address(&self, seat: &str) -> Option<[u8; 20]> {
        match seat {
            s if s == SEATS[0] => self.author,
            s if s == SEATS[1] => self.grantee,
            _ => None,
        }
    }
}

/// The register as written: the current identity and seat, the rows, and the homes deleted identities left
/// (identity id, seat, home).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Book {
    pub current: Option<(String, &'static str)>,
    pub lines: Vec<Line>,
    pub left: Vec<(String, &'static str, String)>,
}

impl Book {
    /// Canonical bytes (member order and spelling from the core's canonicalizer). Empty seats write
    /// [`UNSEATED`] in address and home; `kind` and `slot` are computed and checked field by field when read
    /// back.
    pub fn to_bytes(&self) -> Vec<u8> {
        let s = |x: &str| Value::Str(x.to_string());
        let obj = |m: Vec<(&str, Value)>| Value::Obj(m.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
        let addr = |a: Option<[u8; 20]>| a.map(|a| zikaron::hexfmt::encode(&a)).unwrap_or_else(|| UNSEATED.to_string());
        let rows: Vec<Value> = self
            .lines
            .iter()
            .map(|r| {
                let mut m = vec![
                    ("author", s(&addr(r.author))),
                    ("backup", obj(vec![("at", s(&r.backup_at)), ("file", Value::Bool(r.backed_file)), ("words", Value::Bool(r.backed_words))])),
                    ("created", s(&r.created)),
                    ("grantee", s(&addr(r.grantee))),
                    ("homes", obj(vec![("author", s(&r.author_home)), ("grantee", s(&r.grantee_home))])),
                    ("id", s(&r.id)),
                    ("kind", s(r.kind().as_str())),
                    ("label", s(&r.label)),
                    ("slot", s(SLOT_OWN)),
                ];
                if let Some(n) = &r.network {
                    m.push(("network", n.clone()));
                }
                obj(m)
            })
            .collect();
        let mut top = Vec::new();
        if let Some((id, seat)) = &self.current {
            top.push(("current", obj(vec![("identity", s(id)), ("seat", s(seat))])));
        }
        top.push(("identities", Value::Arr(rows)));
        if !self.left.is_empty() {
            let left: Vec<Value> = self.left.iter().map(|(id, seat, home)| obj(vec![("home", s(home)), ("identity", s(id)), ("seat", s(seat))])).collect();
            top.push(("left", Value::Arr(left)));
        }
        top.push(("shape", s(SHAPE)));
        json::canon_bytes(&obj(top))
    }

    /// Read back, naming the field that is wrong. Every member this version writes is required (an empty
    /// `label`, `created` or `backup.at` is written as such: [`NO_LABEL`], [`NO_CREATED`], [`NO_BACKUP_AT`]).
    /// A row with one key on both seats, with no seat, whose kind or id does not follow its seats, or with a
    /// slot other than [`SLOT_OWN`], is no shape this version writes and is refused by name.
    pub fn parse(bytes: &[u8]) -> Result<Book, String> {
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
        let mut lines: Vec<Line> = Vec::new();
        for it in &items {
            // A seat address field: [`UNSEATED`] means the seat is empty; otherwise an address, or refused by
            // name.
            let seat_addr = |k: &str| -> Result<Option<[u8; 20]>, String> {
                let raw = text(it, k)?;
                if raw == UNSEATED {
                    return Ok(None);
                }
                address(&raw).map(Some).ok_or_else(|| k.to_string())
            };
            let backup = field(it, "backup").ok_or("backup")?;
            let homes = field(it, "homes").ok_or("homes")?;
            if text(it, "slot")? != SLOT_OWN {
                return Err("slot".into());
            }
            let kind = Kind::parse(&text(it, "kind")?).ok_or("kind")?;
            let author = seat_addr("author")?;
            let grantee = seat_addr("grantee")?;
            match (author, grantee) {
                // One key on both seats: no shape this version writes.
                (Some(a), Some(g)) if a == g => return Err("grantee".into()),
                // Both seats empty: no reading.
                (None, None) => return Err("author".into()),
                _ => {}
            }
            // The kind field is computed from the seats; a stored value that differs is refused by name.
            let computed = if author.is_some() && grantee.is_some() { Kind::Words } else { Kind::Existing };
            if computed != kind {
                return Err("kind".into());
            }
            let line = Line {
                id: text(it, "id")?,
                author,
                grantee,
                author_home: text(&homes, "author")?,
                grantee_home: text(&homes, "grantee")?,
                backed_words: flag(&backup, "words")?,
                backed_file: flag(&backup, "file")?,
                backup_at: text(&backup, "at")?,
                label: text(it, "label")?,
                created: text(it, "created")?,
                network: field(it, "network"),
            };
            // The name is the address of an occupied seat: the author seat's when both are occupied,
            // otherwise the one seat's.
            let named = author.or(grantee).map(|a| zikaron::hexfmt::encode(&a)).unwrap_or_default();
            if line.id != named {
                return Err("id".into());
            }
            if lines.iter().any(|r| r.id == line.id) {
                return Err(format!("id {}", line.id));
            }
            lines.push(line);
        }
        let current = match field(&v, "current") {
            None => None,
            Some(c) => {
                let id = text(&c, "identity")?;
                let seat = seat_word(&text(&c, "seat")?).ok_or("seat")?;
                if !lines.iter().any(|r| r.id == id) {
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
                    let seat = seat_word(&text(it, "seat")?).ok_or("left seat")?;
                    left.push((text(it, "identity")?, seat, text(it, "home")?));
                }
            }
            Some(_) => return Err("left".into()),
        }
        Ok(Book { current, lines, left })
    }
}
