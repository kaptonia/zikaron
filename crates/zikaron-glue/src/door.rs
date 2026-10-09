//! Message format for the desktop's local IPC endpoint (the "door"), shared by both sides: the CLI asks, the
//! desktop answers.
//!
//! The CLI parses its arguments and rejects misuse as usual, then sends the verb and parsed flags as one
//! [`Request`]. The desktop runs it through its own action layer and returns one [`Reply`] of facts, never the
//! CLI's answer (the CLI builds its answer from them where it builds every answer). Each message is one frame:
//! a 4-byte big-endian length, then that many bytes of `zikaron/1` canonical JSON. Frames over [`CAP`] are not
//! read.
//!
//! The endpoint's location ([`place`]) depends on the machine directory and the open home, so each home has its
//! own endpoint: the CLI names the home (`--home`), and a home the desktop does not have open has none.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};

/// Protocol identifier carried in every frame; a frame with another value is not read.
pub const FORM: &str = "zikaron-door/1";

/// Maximum frame body size in bytes.
pub const CAP: usize = 1 << 20;

/// The endpoint for `home` under machine directory `machine`, named by the first eight bytes of the sha256 of
/// both canonicalized paths (so links such as `/tmp` vs `/private/tmp` resolve alike). Both sides derive the
/// same name, and the name never reveals the paths.
pub fn place(machine: &Path, home: &Path) -> PathBuf {
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let mut bytes = real(machine).into_os_string().into_encoded_bytes();
    bytes.push(0);
    bytes.extend(real(home).into_os_string().into_encoded_bytes());
    let digest = zikaron::cryptox::sha256(&bytes);
    zikaron_os::door::place(machine, &zikaron::hexfmt::encode(&digest[..8]))
}

/// Member names of a frame (closed set, spelled only here).
mod member {
    pub const FORM: &str = "form";
    pub const VERB: &str = "verb";
    pub const HOME: &str = "home";
    pub const ARGS: &str = "args";
    pub const REPLY: &str = "reply";
    pub const ENTRY_ID: &str = "entryId";
    pub const LEDGER: &str = "ledger";
    pub const SEQ: &str = "seq";
    pub const TX: &str = "tx";
    pub const BLOCK: &str = "block";
    pub const STATUS: &str = "status";
    pub const WAITED: &str = "waited";
    pub const DETAIL: &str = "detail";
    pub const KIT_ID: &str = "kitId";
    pub const PATH: &str = "path";
    pub const ENTRIES: &str = "entries";
    pub const FILES: &str = "files";
    pub const DROPPED: &str = "dropped";
    pub const COUNT: &str = "count";
    pub const CODE: &str = "code";
    pub const TAIL: &str = "tail";
    pub const NETWORK: &str = "network";
    pub const ZH: &str = "zh";
    pub const EN: &str = "en";
}

/// A CLI request: the verb, the named home, and each parsed flag with its value in the order given (a repeated
/// flag appears more than once).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub verb: String,
    pub home: String,
    pub args: Vec<(String, String)>,
}

impl Request {
    /// The value of a single-use flag (the CLI has already rejected repeats); `None` when absent.
    pub fn one(&self, flag: &str) -> Option<&str> {
        self.args.iter().find(|(k, _)| k == flag).map(|(_, v)| v.as_str())
    }

    /// Every value of a flag, in order.
    pub fn many(&self, flag: &str) -> Vec<&str> {
        self.args.iter().filter(|(k, _)| k == flag).map(|(_, v)| v.as_str()).collect()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let args = self.args.iter().map(|(k, v)| Value::Arr(vec![Value::Str(k.clone()), Value::Str(v.clone())])).collect();
        json::canon_bytes(&Value::Obj(vec![
            (member::ARGS.into(), Value::Arr(args)),
            (member::FORM.into(), Value::Str(FORM.into())),
            (member::HOME.into(), Value::Str(self.home.clone())),
            (member::VERB.into(), Value::Str(self.verb.clone())),
        ]))
    }

    /// Parse a request; `None` when the bytes are not a request of this form.
    pub fn of_bytes(b: &[u8]) -> Option<Request> {
        let v = form_of(b)?;
        let mut args = Vec::new();
        for a in v.member(member::ARGS)?.as_arr()? {
            match a.as_arr().map(|x| x.as_slice()) {
                Some([Value::Str(k), Value::Str(x)]) => args.push((k.clone(), x.clone())),
                _ => return None,
            }
        }
        Some(Request { verb: v.member(member::VERB)?.as_str()?.to_string(), home: v.member(member::HOME)?.as_str()?.to_string(), args })
    }
}

/// The outcome of a request, as facts. Closed set: the CLI maps each to its documented answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    /// An entry was written: its id, the ledger folder it went into, its sequence number.
    Wrote { entry_id: String, ledger: String, seq: u64 },
    /// The batch was sent and included with status 1 at this block.
    Anchored { tx: String, block: u64 },
    /// Included, with another status (the chain said the call failed).
    Reverted { tx: String, status: u64 },
    /// Sent but not seen in a block within the desktop's wait (the desktop keeps waiting).
    NotYet { tx: String, waited: u64 },
    /// Sent, but no node answered the receipt query within the wait; `detail` is what was heard (the desktop
    /// keeps waiting).
    Unheard { tx: String, detail: String },
    /// Sent and void: no node knows it and its nonce was used by another transaction, so it will never be
    /// included; its entries return to the queue (the batch's last transaction).
    Voided { tx: String },
    /// A record package was written.
    Kit { kit_id: String, path: String, entries: u64, files: u64, dropped: Vec<String> },
    /// Nothing was sent: the entries wait in the queue for the user to send them from the desktop.
    Queued { count: u64 },
    /// The action needs the passcode, so it is done on the desktop, never over IPC.
    OnDesktop,
    /// The desktop's action layer refused: its code, the evidence, whether it is a network failure (no answer
    /// rather than a negative one), and the message in both languages.
    Refused { code: String, tail: String, network: bool, zh: String, en: String },
    /// The desktop locked, closed the home or quit before the request was answered: a request not yet begun was
    /// not done; one begun may have been (the desktop shows what it did).
    Closed,
}

mod kind {
    pub const WROTE: &str = "wrote";
    pub const ANCHORED: &str = "anchored";
    pub const REVERTED: &str = "reverted";
    pub const NOT_YET: &str = "notYet";
    pub const UNHEARD: &str = "unheard";
    pub const VOIDED: &str = "voided";
    pub const KIT: &str = "kit";
    pub const QUEUED: &str = "queued";
    pub const ON_DESKTOP: &str = "onDesktop";
    pub const REFUSED: &str = "refused";
    pub const CLOSED: &str = "closed";
}

impl Reply {
    pub fn to_bytes(&self) -> Vec<u8> {
        let s = |x: &str| Value::Str(x.to_string());
        let mut ms: Vec<(String, Value)> = vec![(member::FORM.into(), s(FORM))];
        let mut put = |k: &str, v: Value| ms.push((k.to_string(), v));
        match self {
            Reply::Wrote { entry_id, ledger, seq } => {
                put(member::REPLY, s(kind::WROTE));
                put(member::ENTRY_ID, s(entry_id));
                put(member::LEDGER, s(ledger));
                put(member::SEQ, Value::Int(*seq));
            }
            Reply::Anchored { tx, block } => {
                put(member::REPLY, s(kind::ANCHORED));
                put(member::TX, s(tx));
                put(member::BLOCK, Value::Int(*block));
            }
            Reply::Reverted { tx, status } => {
                put(member::REPLY, s(kind::REVERTED));
                put(member::TX, s(tx));
                put(member::STATUS, Value::Int(*status));
            }
            Reply::NotYet { tx, waited } => {
                put(member::REPLY, s(kind::NOT_YET));
                put(member::TX, s(tx));
                put(member::WAITED, Value::Int(*waited));
            }
            Reply::Unheard { tx, detail } => {
                put(member::REPLY, s(kind::UNHEARD));
                put(member::TX, s(tx));
                put(member::DETAIL, s(detail));
            }
            Reply::Voided { tx } => {
                put(member::REPLY, s(kind::VOIDED));
                put(member::TX, s(tx));
            }
            Reply::Kit { kit_id, path, entries, files, dropped } => {
                put(member::REPLY, s(kind::KIT));
                put(member::KIT_ID, s(kit_id));
                put(member::PATH, s(path));
                put(member::ENTRIES, Value::Int(*entries));
                put(member::FILES, Value::Int(*files));
                put(member::DROPPED, Value::Arr(dropped.iter().map(|x| s(x)).collect()));
            }
            Reply::Queued { count } => {
                put(member::REPLY, s(kind::QUEUED));
                put(member::COUNT, Value::Int(*count));
            }
            Reply::OnDesktop => put(member::REPLY, s(kind::ON_DESKTOP)),
            Reply::Refused { code, tail, network, zh, en } => {
                put(member::REPLY, s(kind::REFUSED));
                put(member::CODE, s(code));
                put(member::TAIL, s(tail));
                put(member::NETWORK, Value::Bool(*network));
                put(member::ZH, s(zh));
                put(member::EN, s(en));
            }
            Reply::Closed => put(member::REPLY, s(kind::CLOSED)),
        }
        ms.sort_by(|a, b| a.0.cmp(&b.0));
        json::canon_bytes(&Value::Obj(ms))
    }

    /// Read a reply; `None` when the bytes are not one in this form.
    pub fn of_bytes(b: &[u8]) -> Option<Reply> {
        let v = form_of(b)?;
        let s = |k: &str| v.member(k).and_then(|x| x.as_str()).map(str::to_string);
        let n = |k: &str| match v.member(k) {
            Some(Value::Int(x)) => Some(*x),
            _ => None,
        };
        Some(match v.member(member::REPLY)?.as_str()? {
            kind::WROTE => Reply::Wrote { entry_id: s(member::ENTRY_ID)?, ledger: s(member::LEDGER)?, seq: n(member::SEQ)? },
            kind::ANCHORED => Reply::Anchored { tx: s(member::TX)?, block: n(member::BLOCK)? },
            kind::REVERTED => Reply::Reverted { tx: s(member::TX)?, status: n(member::STATUS)? },
            kind::NOT_YET => Reply::NotYet { tx: s(member::TX)?, waited: n(member::WAITED)? },
            kind::UNHEARD => Reply::Unheard { tx: s(member::TX)?, detail: s(member::DETAIL)? },
            kind::VOIDED => Reply::Voided { tx: s(member::TX)? },
            kind::KIT => Reply::Kit {
                kit_id: s(member::KIT_ID)?,
                path: s(member::PATH)?,
                entries: n(member::ENTRIES)?,
                files: n(member::FILES)?,
                dropped: v.member(member::DROPPED)?.as_arr()?.iter().map(|x| x.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?,
            },
            kind::QUEUED => Reply::Queued { count: n(member::COUNT)? },
            kind::ON_DESKTOP => Reply::OnDesktop,
            kind::REFUSED => Reply::Refused {
                code: s(member::CODE)?,
                tail: s(member::TAIL)?,
                network: match v.member(member::NETWORK)? {
                    Value::Bool(b) => *b,
                    _ => return None,
                },
                zh: s(member::ZH)?,
                en: s(member::EN)?,
            },
            kind::CLOSED => Reply::Closed,
            _ => return None,
        })
    }
}

/// Parse bytes as a value of this form (the `zikaron/1` JSON reader, then the form member).
fn form_of(b: &[u8]) -> Option<Value> {
    let v = json::parse(b).ok()?;
    (v.member(member::FORM)?.as_str()? == FORM).then_some(v)
}

/// Why a frame was not read. Closed set.
#[derive(Debug)]
pub enum Unread {
    /// The peer closed before a whole frame arrived (or sent nothing).
    Short,
    /// The declared length exceeds [`CAP`].
    TooLarge(usize),
    /// An I/O error (including a read timeout).
    Io(std::io::Error),
}

/// Write one frame.
pub fn put(w: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    let n = u32::try_from(bytes.len()).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "a frame over four gigabytes"))?;
    w.write_all(&n.to_be_bytes())?;
    w.write_all(bytes)?;
    w.flush()
}

/// Read one frame of at most [`CAP`] bytes.
pub fn take(r: &mut impl Read) -> Result<Vec<u8>, Unread> {
    let mut head = [0u8; 4];
    read_all(r, &mut head)?;
    let n = u32::from_be_bytes(head) as usize;
    if n > CAP {
        return Err(Unread::TooLarge(n));
    }
    let mut body = vec![0u8; n];
    read_all(r, &mut body)?;
    Ok(body)
}

fn read_all(r: &mut impl Read, buf: &mut [u8]) -> Result<(), Unread> {
    match r.read_exact(buf) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Err(Unread::Short),
        Err(e) => Err(Unread::Io(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every reply and a request round-trip; another form or non-JSON does not parse.
    #[test]
    fn requests_and_replies_read_back_as_written() {
        let q = Request { verb: "annotate".into(), home: "/h".into(), args: vec![("subject".into(), "ab".into()), ("note".into(), "x y\n".into()), ("note".into(), "again".into())] };
        assert_eq!(Request::of_bytes(&q.to_bytes()), Some(q.clone()));
        assert_eq!(q.one("subject"), Some("ab"));
        assert_eq!(q.many("note"), vec!["x y\n", "again"]);
        let all = [
            Reply::Wrote { entry_id: "e".into(), ledger: "/l".into(), seq: 3 },
            Reply::Anchored { tx: "t".into(), block: 9 },
            Reply::Reverted { tx: "t".into(), status: 0 },
            Reply::NotYet { tx: "t".into(), waited: 30 },
            Reply::Unheard { tx: "t".into(), detail: "no node".into() },
            Reply::Voided { tx: "t".into() },
            Reply::Kit { kit_id: "k".into(), path: "/o".into(), entries: 2, files: 0, dropped: vec![".DS_Store".into()] },
            Reply::Queued { count: 4 },
            Reply::OnDesktop,
            Reply::Refused { code: "LOCKED".into(), tail: "t".into(), network: true, zh: "锁".into(), en: "locked".into() },
            Reply::Closed,
        ];
        for r in all {
            assert_eq!(Reply::of_bytes(&r.to_bytes()), Some(r.clone()), "{r:?}");
            assert_eq!(Request::of_bytes(&r.to_bytes()), None, "a reply is not a request");
        }
        assert_eq!(Reply::of_bytes(br#"{"form":"zikaron-door/2","reply":"closed"}"#), None, "another form");
        assert_eq!(Reply::of_bytes(br#"{"form":"zikaron-door/1","reply":"other"}"#), None, "not a reply this form has");
        assert_eq!(Reply::of_bytes(b"not json"), None);
        assert_eq!(Request::of_bytes(br#"{"args":[["a"]],"form":"zikaron-door/1","home":"/h","verb":"v"}"#), None, "an argument is a pair");
    }

    /// A frame reads back; one cut short, or empty, is `Short`; one longer than the cap is not read.
    #[test]
    fn a_frame_reads_whole_or_not_at_all() {
        let mut buf = Vec::new();
        put(&mut buf, b"hello").expect("put");
        assert_eq!(take(&mut buf.as_slice()).expect("take"), b"hello");
        assert!(matches!(take(&mut &buf[..6]), Err(Unread::Short)));
        assert!(matches!(take(&mut &b""[..]), Err(Unread::Short)));
        let over = ((CAP + 1) as u32).to_be_bytes();
        assert!(matches!(take(&mut &over[..]), Err(Unread::TooLarge(n)) if n == CAP + 1));
        let at = (CAP as u32).to_be_bytes();
        assert!(matches!(take(&mut &at[..]), Err(Unread::Short)), "the cap itself is taken (here cut short)");
    }

    /// The same location however spelled gives the same endpoint; another home gives another.
    #[test]
    fn a_door_is_named_by_its_places_as_the_system_resolves_them() {
        let d = std::env::temp_dir().join(format!("zkg-door-{}", std::process::id()));
        let (m, h, o) = (d.join("m"), d.join("h"), d.join("o"));
        for x in [&m, &h, &o] {
            std::fs::create_dir_all(x).expect("dir");
        }
        let a = place(&m, &h);
        assert_eq!(a, place(&m, &h.join(".").join("..").join("h")), "the same place spelled otherwise");
        assert_ne!(a, place(&m, &o), "another home");
        let name = a.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        assert!(name.starts_with(zikaron_os::door::NAME_HEAD) && !name.contains("zkg-door"), "the name never says the paths: {name}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
