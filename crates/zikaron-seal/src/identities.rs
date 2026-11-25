use crate::envelope::SEATS;
use zikaron::json::{self, Value};

pub const SHAPE: &str = "zikaron-desk/identities/2";
pub const SHAPE_V1: &str = "zikaron-desk/identities/1";

pub const SLOT_OWN: &str = "own";

pub const UNSEATED: &str = "";
pub const NO_LABEL: &str = "";
pub const NO_CREATED: &str = "";
pub const NO_BACKUP_AT: &str = "";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Words,
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

pub fn seat_word(s: &str) -> Option<&'static str> {
    SEATS.into_iter().find(|w| *w == s)
}

pub fn address(s: &str) -> Option<[u8; 20]> {
    let low = s.trim().to_ascii_lowercase();
    if !zikaron::hexfmt::is_hex20(&low) {
        return None;
    }
    zikaron::hexfmt::decode(&low)?.try_into().ok()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Office {
    pub generation: u64,
    pub current: [u8; 20],
    pub next: Option<[u8; 20]>,
    pub former: Vec<[u8; 20]>,
}

impl Office {
    pub fn root(root: [u8; 20]) -> Office {
        Office { generation: 0, current: root, next: None, former: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub id: String,
    pub author: Option<[u8; 20]>,
    pub grantee: Option<[u8; 20]>,
    pub author_office: Option<Office>,
    pub grantee_office: Option<Office>,
    pub author_home: String,
    pub grantee_home: String,
    pub backed_words: bool,
    pub backed_file: bool,
    pub backup_at: String,
    pub label: String,
    pub created: String,
    pub network: Option<Value>,
    pub reroot_of: Option<[u8; 20]>,
}

impl Line {
    pub fn kind(&self) -> Kind {
        if self.author.is_some() && self.grantee.is_some() {
            Kind::Words
        } else {
            Kind::Existing
        }
    }

    pub fn address(&self, seat: &str) -> Option<[u8; 20]> {
        match seat {
            s if s == SEATS[0] => self.author,
            s if s == SEATS[1] => self.grantee,
            _ => None,
        }
    }

    pub fn office(&self, seat: &str) -> Option<Office> {
        let root = self.address(seat)?;
        let written = match seat {
            s if s == SEATS[0] => self.author_office.clone(),
            _ => self.grantee_office.clone(),
        };
        Some(written.unwrap_or(Office::root(root)))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Book {
    pub current: Option<(String, &'static str)>,
    pub lines: Vec<Line>,
    pub left: Vec<(String, &'static str, String)>,
    pub upgraded: bool,
}

impl Book {
    pub fn to_bytes(&self) -> Vec<u8> {
        let s = |x: &str| Value::Str(x.to_string());
        let obj = |m: Vec<(&str, Value)>| Value::Obj(m.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
        let seat = |r: &Line, w: &str| match (r.address(w), r.office(w)) {
            (Some(root), Some(o)) => {
                let mut m = vec![("current", s(&zikaron::hexfmt::encode(&o.current)))];
                if !o.former.is_empty() {
                    m.push(("former", Value::Arr(o.former.iter().map(|a| s(&zikaron::hexfmt::encode(a))).collect())));
                }
                m.push(("generation", Value::Int(o.generation)));
                if let Some(n) = o.next {
                    m.push(("next", s(&zikaron::hexfmt::encode(&n))));
                }
                m.push(("root", s(&zikaron::hexfmt::encode(&root))));
                obj(m)
            }
            _ => s(UNSEATED),
        };
        let rows: Vec<Value> = self
            .lines
            .iter()
            .map(|r| {
                let mut m = vec![
                    ("author", seat(r, SEATS[0])),
                    ("backup", obj(vec![("at", s(&r.backup_at)), ("file", Value::Bool(r.backed_file)), ("words", Value::Bool(r.backed_words))])),
                    ("created", s(&r.created)),
                    ("grantee", seat(r, SEATS[1])),
                    ("homes", obj(vec![("author", s(&r.author_home)), ("grantee", s(&r.grantee_home))])),
                    ("id", s(&r.id)),
                    ("kind", s(r.kind().as_str())),
                    ("label", s(&r.label)),
                    ("slot", s(SLOT_OWN)),
                ];
                if let Some(n) = &r.network {
                    m.push(("network", n.clone()));
                }
                if let Some(o) = &r.reroot_of {
                    m.push(("rerootOf", s(&zikaron::hexfmt::encode(o))));
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
        let upgraded = match text(&v, "shape")?.as_str() {
            SHAPE => false,
            SHAPE_V1 => true,
            _ => return Err("shape".into()),
        };
        let Some(Value::Arr(items)) = field(&v, "identities") else {
            return Err("identities".into());
        };
        let mut lines: Vec<Line> = Vec::new();
        for it in &items {
            let seat_of = |k: &str| -> Result<Option<([u8; 20], Office)>, String> {
                match field(it, k) {
                    Some(Value::Str(raw)) if raw == UNSEATED => Ok(None),
                    Some(Value::Str(raw)) if upgraded => address(&raw).map(|a| Some((a, Office::root(a)))).ok_or_else(|| k.to_string()),
                    Some(o @ Value::Obj(_)) if !upgraded => {
                        let at = |m: &str| text(&o, m).ok().and_then(|x| address(&x)).ok_or_else(|| format!("{k}.{m}"));
                        let root = at("root")?;
                        let current = at("current")?;
                        let generation = match field(&o, "generation") {
                            Some(Value::Int(n)) => n,
                            _ => return Err(format!("{k}.generation")),
                        };
                        if (generation == 0) != (current == root) {
                            return Err(format!("{k}.current"));
                        }
                        let next = match field(&o, "next") {
                            None => None,
                            Some(_) => Some(at("next")?).filter(|n| *n != root && *n != current).map(Some).ok_or_else(|| format!("{k}.next"))?,
                        };
                        let former = match field(&o, "former") {
                            None => Vec::new(),
                            Some(Value::Arr(xs)) => xs
                                .iter()
                                .map(|x| match x {
                                    Value::Str(t) => address(t).filter(|a| *a != root && *a != current),
                                    _ => None,
                                })
                                .collect::<Option<Vec<_>>>()
                                .ok_or_else(|| format!("{k}.former"))?,
                            Some(_) => return Err(format!("{k}.former")),
                        };
                        Ok(Some((root, Office { generation, current, next, former })))
                    }
                    _ => Err(k.to_string()),
                }
            };
            let backup = field(it, "backup").ok_or("backup")?;
            let homes = field(it, "homes").ok_or("homes")?;
            if text(it, "slot")? != SLOT_OWN {
                return Err("slot".into());
            }
            let kind = Kind::parse(&text(it, "kind")?).ok_or("kind")?;
            let (author, author_office) = seat_of("author")?.map_or((None, None), |(r, o)| (Some(r), Some(o)));
            let (grantee, grantee_office) = seat_of("grantee")?.map_or((None, None), |(r, o)| (Some(r), Some(o)));
            match (author, grantee) {
                (Some(a), Some(g)) if a == g => return Err("grantee".into()),
                (None, None) => return Err("author".into()),
                _ => {}
            }
            let computed = if author.is_some() && grantee.is_some() { Kind::Words } else { Kind::Existing };
            if computed != kind {
                return Err("kind".into());
            }
            let line = Line {
                id: text(it, "id")?,
                author,
                grantee,
                author_office,
                grantee_office,
                author_home: text(&homes, "author")?,
                grantee_home: text(&homes, "grantee")?,
                backed_words: flag(&backup, "words")?,
                backed_file: flag(&backup, "file")?,
                backup_at: text(&backup, "at")?,
                label: text(it, "label")?,
                created: text(it, "created")?,
                network: field(it, "network"),
                reroot_of: match field(it, "rerootOf") {
                    None => None,
                    Some(Value::Str(x)) => Some(address(&x).ok_or("rerootOf")?),
                    Some(_) => return Err("rerootOf".into()),
                },
            };
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
        Ok(Book { current, lines, left, upgraded })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(author: Option<[u8; 20]>, grantee: Option<[u8; 20]>) -> Line {
        Line {
            id: zikaron::hexfmt::encode(&author.or(grantee).unwrap_or_default()),
            author,
            grantee,
            author_office: None,
            grantee_office: None,
            author_home: if author.is_some() { "/x/a".into() } else { UNSEATED.into() },
            grantee_home: if grantee.is_some() { "/x/g".into() } else { UNSEATED.into() },
            backed_words: false,
            backed_file: true,
            backup_at: NO_BACKUP_AT.into(),
            label: NO_LABEL.into(),
            created: NO_CREATED.into(),
            network: None,
            reroot_of: None,
        }
    }

    #[test]
    fn a_row_made_for_a_reroot_names_its_old_root() {
        let plain = Book { current: None, lines: vec![line(Some([0x11; 20]), None)], left: Vec::new(), upgraded: false };
        let before = plain.to_bytes();
        assert!(!String::from_utf8(before.clone()).expect("text").contains("rerootOf"));
        assert_eq!(Book::parse(&before).expect("reads").to_bytes(), before);
        let mut made = line(Some([0x22; 20]), None);
        made.reroot_of = Some([0x11; 20]);
        let book = Book { current: None, lines: vec![made], left: Vec::new(), upgraded: false };
        let bytes = book.to_bytes();
        assert_eq!(Book::parse(&bytes).expect("reads").lines[0].reroot_of, Some([0x11; 20]));
        let bad = String::from_utf8(bytes).expect("text").replace("\"rerootOf\":\"0x1111111111111111111111111111111111111111\"", "\"rerootOf\":7");
        assert_eq!(Book::parse(bad.as_bytes()).err(), Some("rerootOf".to_string()));
    }

    #[test]
    fn a_seat_names_its_root_generation_and_key_in_office() {
        let (a, g, k) = ([0x11; 20], [0x22; 20], [0x33; 20]);
        let mut words = line(Some(a), Some(g));
        words.author_office = Some(Office { generation: 2, current: k, next: None, former: Vec::new() });
        let book = Book { current: Some((words.id.clone(), SEATS[0])), lines: vec![words.clone(), line(None, Some([0x44; 20]))], left: Vec::new(), upgraded: false };
        let bytes = book.to_bytes();
        let text = String::from_utf8(bytes.clone()).expect("text");
        assert!(text.contains(SHAPE) && text.contains("\"generation\":2") && text.contains("\"root\":\"0x1111"), "{text}");
        let back = Book::parse(&bytes).expect("reads");
        assert!(!back.upgraded);
        assert_eq!(back.lines[0].office(SEATS[0]), Some(Office { generation: 2, current: k, next: None, former: Vec::new() }));
        assert_eq!(back.lines[0].address(SEATS[0]), Some(a), "the root, not the key in office");
        assert_eq!(back.lines[0].office(SEATS[1]), Some(Office::root(g)));
        assert_eq!(back.lines[1].office(SEATS[0]), None, "an empty seat has no office");
        assert_eq!(back.to_bytes(), bytes, "read and written alike");
        let v1 = text
            .replace(SHAPE, SHAPE_V1)
            .replace(&format!("{{\"current\":\"0x{}\",\"generation\":2,\"root\":\"0x{}\"}}", "33".repeat(20), "11".repeat(20)), &format!("\"0x{}\"", "11".repeat(20)))
            .replace(&format!("{{\"current\":\"0x{0}\",\"generation\":0,\"root\":\"0x{0}\"}}", "22".repeat(20)), &format!("\"0x{}\"", "22".repeat(20)))
            .replace(&format!("{{\"current\":\"0x{0}\",\"generation\":0,\"root\":\"0x{0}\"}}", "44".repeat(20)), &format!("\"0x{}\"", "44".repeat(20)));
        let old = Book::parse(v1.as_bytes()).expect("the earlier register reads");
        assert!(old.upgraded);
        assert_eq!(old.lines[0].office(SEATS[0]), Some(Office::root(a)));
        let once = old.to_bytes();
        let again = Book::parse(&once).expect("reads");
        assert!(!again.upgraded);
        assert_eq!(again.to_bytes(), once, "the second reading writes the same bytes");
        let zero_other = text.replace("\"generation\":2", "\"generation\":0");
        assert_eq!(Book::parse(zero_other.as_bytes()).unwrap_err(), "author.current");
        let later_root = text.replace(&format!("\"current\":\"0x{}\"", "33".repeat(20)), &format!("\"current\":\"0x{}\"", "11".repeat(20)));
        assert_eq!(Book::parse(later_root.as_bytes()).unwrap_err(), "author.current");
        assert_eq!(Book::parse(text.replace("\"generation\":2,", "").as_bytes()).unwrap_err(), "author.generation");
        let bare_here = v1.replace(SHAPE_V1, SHAPE);
        assert_eq!(Book::parse(bare_here.as_bytes()).unwrap_err(), "author");
        assert_eq!(Book::parse(text.replace(SHAPE, "zikaron-desk/identities/3").as_bytes()).unwrap_err(), "shape");
    }

    #[test]
    fn a_seat_names_its_next_and_former_keys() {
        let (a, g, k1, k2) = ([0x11; 20], [0x22; 20], [0x33; 20], [0x55; 20]);
        let mut words = line(Some(a), Some(g));
        words.author_office = Some(Office { generation: 2, current: k2, next: Some([0x66; 20]), former: vec![k1] });
        let book = Book { current: None, lines: vec![words], left: Vec::new(), upgraded: false };
        let bytes = book.to_bytes();
        let back = Book::parse(&bytes).expect("reads");
        assert_eq!(back.lines[0].office(SEATS[0]), Some(Office { generation: 2, current: k2, next: Some([0x66; 20]), former: vec![k1] }));
        assert_eq!(back.to_bytes(), bytes);
        let text = String::from_utf8(bytes).expect("text");
        let hex = |b: [u8; 20]| zikaron::hexfmt::encode(&b);
        assert_eq!(Book::parse(text.replace(&format!("\"next\":\"{}\"", hex([0x66; 20])), &format!("\"next\":\"{}\"", hex(a))).as_bytes()).unwrap_err(), "author.next", "next is the root");
        assert_eq!(Book::parse(text.replace(&format!("\"former\":[\"{}\"]", hex(k1)), &format!("\"former\":[\"{}\"]", hex(k2))).as_bytes()).unwrap_err(), "author.former", "former is the key in office");
        assert_eq!(Book::parse(text.replace(&format!("\"former\":[\"{}\"]", hex(k1)), "\"former\":7").as_bytes()).unwrap_err(), "author.former");
    }
}
