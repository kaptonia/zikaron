use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};

pub const FORM: &str = "zikaron-door/1";

pub const CAP: usize = 1 << 20;

pub fn place(machine: &Path, home: &Path) -> PathBuf {
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let mut bytes = real(machine).into_os_string().into_encoded_bytes();
    bytes.push(0);
    bytes.extend(real(home).into_os_string().into_encoded_bytes());
    let digest = zikaron::cryptox::sha256(&bytes);
    zikaron_os::door::place(machine, &zikaron::hexfmt::encode(&digest[..8]))
}

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
    pub const ORIGINALS: &str = "originals";
    pub const LEFT_OUT: &str = "leftOut";
    pub const COUNT: &str = "count";
    pub const CODE: &str = "code";
    pub const TAIL: &str = "tail";
    pub const NETWORK: &str = "network";
    pub const ZH: &str = "zh";
    pub const EN: &str = "en";
    pub const NAMES: &str = "names";
    pub const RECORDS: &str = "records";
    pub const CONTENT: &str = "content";
    pub const ORIGINAL: &str = "original";
    pub const DELETED: &str = "deleted";
    pub const ANCHORED: &str = "anchored";
    pub const FIRST_AT: &str = "firstAt";
    pub const TAKEN: &str = "taken";
    pub const UNANSWERED: &str = "unanswered";
    pub const VERDICT: &str = "verdict";
    pub const SUBJECT: &str = "subject";
    pub const DONE: &str = "done";
    pub const REMOVED: &str = "removed";
    pub const KEPT: &str = "kept";
    pub const KIND: &str = "kind";
    pub const LINK: &str = "link";
    pub const WHY: &str = "why";
    pub const SEALED: &str = "sealed";
    pub const RULE: &str = "rule";
    pub const OWN: &str = "own";
    pub const TOKEN: &str = "token";
    pub const NOTICE: &str = "notice";
    pub const WITHDRAWN: &str = "withdrawn";
    pub const TABLE: &str = "table";
    pub const MOMENT: &str = "moment";
    pub const OMITTED: &str = "omitted";
    pub const JOINED: &str = "joined";
    pub const READ_TO: &str = "readTo";
    pub const ROOT_OF: &str = "root";
}

pub use crate::doorrules::{desk_reason, Own, Rule, DESK, RETRACT_NOT_AUDITED, TIP_NOT_MOVED};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub verb: String,
    pub home: String,
    pub args: Vec<(String, String)>,
}

impl Request {
    pub fn one(&self, flag: &str) -> Option<&str> {
        self.args.iter().find(|(k, _)| k == flag).map(|(_, v)| v.as_str())
    }

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Wrote { entry_id: String, ledger: String, seq: u64 },
    Noticed { entry_id: String, ledger: String, tx: String, notice: String, withdrawn: bool },
    Anchored { tx: String, block: u64 },
    Reverted { tx: String, status: u64 },
    NotYet { tx: String, waited: u64 },
    Unheard { tx: String, detail: String },
    Voided { tx: String },
    OnChain { names: Vec<String>, tx: String, block: u64 },
    Kit { kit_id: String, path: String, entries: u64, files: u64, dropped: Vec<String>, originals: Vec<String>, left_out: Vec<String> },
    Verified { kit_id: String, entries: u64, records: Vec<Record>, taken: Option<Taken>, unanswered: Vec<String> },
    KitRefused { verdict: String, subject: Option<String> },
    Queued { count: u64 },
    OnDesktop,
    Refused { code: String, tail: String, network: bool, zh: String, en: String, own: Option<Own> },
    RuleRefused { rule: Rule, token: Option<String>, entry_id: Option<String>, names: Option<Vec<String>>, zh: String, en: String },
    Closed,
    GrantPackage { path: String, kit_id: String, files: u64, originals: Vec<String>, left_out: Vec<String>, sealed: bool },
    Cleaned { done: bool, removed: Vec<Cleared>, kept: Vec<Cleared> },
    Face { table: String, moment: u64, omitted: Vec<String> },
    Joined { joined: bool, read: Vec<ReadOf> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadOf {
    pub root: String,
    pub moment: u64,
    pub omitted: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cleared {
    pub kind: String,
    pub path: String,
    pub link: Option<String>,
    pub why: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub entry_id: String,
    pub content: String,
    pub original: crate::recordcheck::Original,
    pub chain: crate::recordcheck::OnChain,
    pub deleted: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Taken {
    pub path: String,
    pub names: Vec<String>,
}

mod kind {
    pub const WROTE: &str = "wrote";
    pub const NOTICED: &str = "noticed";
    pub const ANCHORED: &str = "anchored";
    pub const REVERTED: &str = "reverted";
    pub const NOT_YET: &str = "notYet";
    pub const UNHEARD: &str = "unheard";
    pub const VOIDED: &str = "voided";
    pub const ON_CHAIN: &str = "onChain";
    pub const KIT: &str = "kit";
    pub const VERIFIED: &str = "verified";
    pub const KIT_REFUSED: &str = "kitRefused";
    pub const QUEUED: &str = "queued";
    pub const ON_DESKTOP: &str = "onDesktop";
    pub const REFUSED: &str = "refused";
    pub const RULE_REFUSED: &str = "ruleRefused";
    pub const CLOSED: &str = "closed";
    pub const CLEANED: &str = "cleaned";
    pub const GRANT_PACKAGE: &str = "grantPackage";
    pub const FACE: &str = "face";
    pub const JOINED: &str = "joined";
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
            Reply::Noticed { entry_id, ledger, tx, notice, withdrawn } => {
                put(member::REPLY, s(kind::NOTICED));
                put(member::ENTRY_ID, s(entry_id));
                put(member::LEDGER, s(ledger));
                put(member::TX, s(tx));
                put(member::NOTICE, s(notice));
                put(member::WITHDRAWN, Value::Bool(*withdrawn));
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
            Reply::OnChain { names, tx, block } => {
                put(member::REPLY, s(kind::ON_CHAIN));
                put(member::NAMES, Value::Arr(names.iter().map(|x| s(x)).collect()));
                put(member::TX, s(tx));
                put(member::BLOCK, Value::Int(*block));
            }
            Reply::Kit { kit_id, path, entries, files, dropped, originals, left_out } => {
                put(member::REPLY, s(kind::KIT));
                put(member::KIT_ID, s(kit_id));
                put(member::PATH, s(path));
                put(member::ENTRIES, Value::Int(*entries));
                put(member::FILES, Value::Int(*files));
                put(member::DROPPED, Value::Arr(dropped.iter().map(|x| s(x)).collect()));
                put(member::ORIGINALS, Value::Arr(originals.iter().map(|x| s(x)).collect()));
                put(member::LEFT_OUT, Value::Arr(left_out.iter().map(|x| s(x)).collect()));
            }
            Reply::Verified { kit_id, entries, records, taken, unanswered } => {
                put(member::REPLY, s(kind::VERIFIED));
                put(member::KIT_ID, s(kit_id));
                put(member::ENTRIES, Value::Int(*entries));
                let row = |r: &Record| {
                    let mut m = vec![
                        (member::ANCHORED.to_string(), s(r.chain.word())),
                        (member::CONTENT.to_string(), s(&r.content)),
                        (member::ENTRY_ID.to_string(), s(&r.entry_id)),
                    ];
                    if let Some(t) = r.chain.first_at() {
                        m.push((member::FIRST_AT.to_string(), Value::Int(t)));
                    }
                    m.push((member::ORIGINAL.to_string(), s(r.original.word())));
                    if let Some(by) = &r.deleted {
                        m.push((member::DELETED.to_string(), s(by)));
                    }
                    Value::Obj(m)
                };
                put(member::RECORDS, Value::Arr(records.iter().map(row).collect()));
                if let Some(t) = taken {
                    put(member::TAKEN, Value::Obj(vec![(member::NAMES.to_string(), Value::Arr(t.names.iter().map(|x| s(x)).collect())), (member::PATH.to_string(), s(&t.path))]));
                }
                put(member::UNANSWERED, Value::Arr(unanswered.iter().map(|x| s(x)).collect()));
            }
            Reply::KitRefused { verdict, subject } => {
                put(member::REPLY, s(kind::KIT_REFUSED));
                put(member::VERDICT, s(verdict));
                if let Some(x) = subject {
                    put(member::SUBJECT, s(x));
                }
            }
            Reply::Queued { count } => {
                put(member::REPLY, s(kind::QUEUED));
                put(member::COUNT, Value::Int(*count));
            }
            Reply::OnDesktop => put(member::REPLY, s(kind::ON_DESKTOP)),
            Reply::Refused { code, tail, network, zh, en, own } => {
                put(member::REPLY, s(kind::REFUSED));
                put(member::CODE, s(code));
                put(member::TAIL, s(tail));
                put(member::NETWORK, Value::Bool(*network));
                put(member::ZH, s(zh));
                put(member::EN, s(en));
                if let Some(o) = own {
                    put(member::OWN, s(o.as_str()));
                }
            }
            Reply::RuleRefused { rule, token, entry_id, names, zh, en } => {
                put(member::REPLY, s(kind::RULE_REFUSED));
                put(member::RULE, s(rule.as_str()));
                if let Some(t) = token {
                    put(member::TOKEN, s(t));
                }
                if let Some(e) = entry_id {
                    put(member::ENTRY_ID, s(e));
                }
                if let Some(ns) = names {
                    put(member::NAMES, Value::Arr(ns.iter().map(|x| s(x)).collect()));
                }
                put(member::ZH, s(zh));
                put(member::EN, s(en));
            }
            Reply::Closed => put(member::REPLY, s(kind::CLOSED)),
            Reply::GrantPackage { path, kit_id, files, originals, left_out, sealed } => {
                put(member::REPLY, s(kind::GRANT_PACKAGE));
                put(member::FILES, Value::Int(*files));
                put(member::KIT_ID, s(kit_id));
                put(member::ORIGINALS, Value::Arr(originals.iter().map(|x| s(x)).collect()));
                put(member::LEFT_OUT, Value::Arr(left_out.iter().map(|x| s(x)).collect()));
                put(member::PATH, s(path));
                put(member::SEALED, Value::Bool(*sealed));
            }
            Reply::Face { table, moment, omitted } => {
                put(member::REPLY, s(kind::FACE));
                put(member::TABLE, s(table));
                put(member::MOMENT, Value::Int(*moment));
                put(member::OMITTED, Value::Arr(omitted.iter().map(|x| s(x)).collect()));
            }
            Reply::Joined { joined, read } => {
                put(member::REPLY, s(kind::JOINED));
                put(member::JOINED, Value::Bool(*joined));
                let one = |r: &ReadOf| {
                    Value::Obj(vec![
                        (member::MOMENT.to_string(), Value::Int(r.moment)),
                        (member::OMITTED.to_string(), Value::Arr(r.omitted.iter().map(|x| s(x)).collect())),
                        (member::ROOT_OF.to_string(), s(&r.root)),
                    ])
                };
                put(member::READ_TO, Value::Arr(read.iter().map(one).collect()));
            }
            Reply::Cleaned { done, removed, kept } => {
                let row = |c: &Cleared| {
                    let mut m = vec![(member::KIND.to_string(), s(&c.kind))];
                    if let Some(l) = &c.link {
                        m.push((member::LINK.to_string(), s(l)));
                    }
                    m.push((member::PATH.to_string(), s(&c.path)));
                    if let Some(w) = &c.why {
                        m.push((member::WHY.to_string(), s(w)));
                    }
                    Value::Obj(m)
                };
                put(member::REPLY, s(kind::CLEANED));
                put(member::DONE, Value::Bool(*done));
                put(member::KEPT, Value::Arr(kept.iter().map(row).collect()));
                put(member::REMOVED, Value::Arr(removed.iter().map(row).collect()));
            }
        }
        ms.sort_by(|a, b| a.0.cmp(&b.0));
        json::canon_bytes(&Value::Obj(ms))
    }

    pub fn of_bytes(b: &[u8]) -> Option<Reply> {
        let v = form_of(b)?;
        let s = |k: &str| v.member(k).and_then(|x| x.as_str()).map(str::to_string);
        let n = |k: &str| match v.member(k) {
            Some(Value::Int(x)) => Some(*x),
            _ => None,
        };
        Some(match v.member(member::REPLY)?.as_str()? {
            kind::WROTE => Reply::Wrote { entry_id: s(member::ENTRY_ID)?, ledger: s(member::LEDGER)?, seq: n(member::SEQ)? },
            kind::NOTICED => Reply::Noticed {
                entry_id: s(member::ENTRY_ID)?,
                ledger: s(member::LEDGER)?,
                tx: s(member::TX)?,
                notice: s(member::NOTICE)?,
                withdrawn: match v.member(member::WITHDRAWN)? {
                    Value::Bool(b) => *b,
                    _ => return None,
                },
            },
            kind::ANCHORED => Reply::Anchored { tx: s(member::TX)?, block: n(member::BLOCK)? },
            kind::REVERTED => Reply::Reverted { tx: s(member::TX)?, status: n(member::STATUS)? },
            kind::NOT_YET => Reply::NotYet { tx: s(member::TX)?, waited: n(member::WAITED)? },
            kind::UNHEARD => Reply::Unheard { tx: s(member::TX)?, detail: s(member::DETAIL)? },
            kind::VOIDED => Reply::Voided { tx: s(member::TX)? },
            kind::ON_CHAIN => Reply::OnChain {
                names: v.member(member::NAMES)?.as_arr()?.iter().map(|x| x.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?,
                tx: s(member::TX)?,
                block: n(member::BLOCK)?,
            },
            kind::KIT => Reply::Kit {
                kit_id: s(member::KIT_ID)?,
                path: s(member::PATH)?,
                entries: n(member::ENTRIES)?,
                files: n(member::FILES)?,
                dropped: v.member(member::DROPPED)?.as_arr()?.iter().map(|x| x.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?,
                originals: v.member(member::ORIGINALS)?.as_arr()?.iter().map(|x| x.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?,
                left_out: v.member(member::LEFT_OUT)?.as_arr()?.iter().map(|x| x.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?,
            },
            kind::VERIFIED => {
                let strs = |v: &Value| v.as_arr().and_then(|a| a.iter().map(|x| x.as_str().map(str::to_string)).collect::<Option<Vec<_>>>());
                let record = |r: &Value| -> Option<Record> {
                    let t = |k: &str| r.member(k).and_then(|x| x.as_str()).map(str::to_string);
                    let first = match r.member(member::FIRST_AT) {
                        None => None,
                        Some(Value::Int(x)) => Some(*x),
                        Some(_) => return None,
                    };
                    let deleted = match r.member(member::DELETED) {
                        None => None,
                        Some(Value::Str(by)) => Some(by.clone()),
                        Some(_) => return None,
                    };
                    Some(Record {
                        entry_id: t(member::ENTRY_ID)?,
                        content: t(member::CONTENT)?,
                        original: crate::recordcheck::Original::of_word(&t(member::ORIGINAL)?)?,
                        chain: crate::recordcheck::OnChain::of_word(&t(member::ANCHORED)?, first)?,
                        deleted,
                    })
                };
                let taken = match v.member(member::TAKEN) {
                    None => None,
                    Some(t) => Some(Taken { path: t.member(member::PATH)?.as_str()?.to_string(), names: strs(t.member(member::NAMES)?)? }),
                };
                Reply::Verified {
                    kit_id: s(member::KIT_ID)?,
                    entries: n(member::ENTRIES)?,
                    records: v.member(member::RECORDS)?.as_arr()?.iter().map(record).collect::<Option<Vec<_>>>()?,
                    taken,
                    unanswered: strs(v.member(member::UNANSWERED)?)?,
                }
            }
            kind::KIT_REFUSED => Reply::KitRefused { verdict: s(member::VERDICT)?, subject: s(member::SUBJECT) },
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
                own: match s(member::OWN) {
                    None => None,
                    Some(o) => Some(Own::of_str(&o)?),
                },
            },
            kind::RULE_REFUSED => Reply::RuleRefused {
                rule: Rule::of_str(&s(member::RULE)?)?,
                token: s(member::TOKEN),
                entry_id: s(member::ENTRY_ID),
                names: match v.member(member::NAMES) {
                    None => None,
                    Some(x) => Some(x.as_arr()?.iter().map(|y| y.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?),
                },
                zh: s(member::ZH)?,
                en: s(member::EN)?,
            },
            kind::CLOSED => Reply::Closed,
            kind::GRANT_PACKAGE => {
                let b = |k: &str| match v.member(k) {
                    Some(Value::Bool(x)) => Some(*x),
                    _ => None,
                };
                Reply::GrantPackage {
                    path: s(member::PATH)?,
                    kit_id: s(member::KIT_ID)?,
                    files: n(member::FILES)?,
                    originals: v.member(member::ORIGINALS)?.as_arr()?.iter().map(|x| x.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?,
                    left_out: v.member(member::LEFT_OUT)?.as_arr()?.iter().map(|x| x.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?,
                    sealed: b(member::SEALED)?,
                }
            }
            kind::FACE => Reply::Face {
                table: s(member::TABLE)?,
                moment: n(member::MOMENT)?,
                omitted: v.member(member::OMITTED)?.as_arr()?.iter().map(|x| x.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?,
            },
            kind::JOINED => Reply::Joined {
                joined: match v.member(member::JOINED)? {
                    Value::Bool(b) => *b,
                    _ => return None,
                },
                read: v
                    .member(member::READ_TO)?
                    .as_arr()?
                    .iter()
                    .map(|r| {
                        Some(ReadOf {
                            root: r.member(member::ROOT_OF)?.as_str()?.to_string(),
                            moment: r.member(member::MOMENT)?.as_int()?,
                            omitted: r.member(member::OMITTED)?.as_arr()?.iter().map(|x| x.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
            },
            kind::CLEANED => {
                let row = |r: &Value| -> Option<Cleared> {
                    let t = |k: &str| r.member(k).and_then(|x| x.as_str()).map(str::to_string);
                    Some(Cleared { kind: t(member::KIND)?, path: t(member::PATH)?, link: t(member::LINK), why: t(member::WHY) })
                };
                let rows = |k: &str| v.member(k)?.as_arr()?.iter().map(row).collect::<Option<Vec<_>>>();
                Reply::Cleaned {
                    done: match v.member(member::DONE)? {
                        Value::Bool(b) => *b,
                        _ => return None,
                    },
                    removed: rows(member::REMOVED)?,
                    kept: rows(member::KEPT)?,
                }
            }
            _ => return None,
        })
    }
}

fn form_of(b: &[u8]) -> Option<Value> {
    let v = json::parse(b).ok()?;
    (v.member(member::FORM)?.as_str()? == FORM).then_some(v)
}

#[derive(Debug)]
pub enum Unread {
    Short,
    TooLarge(usize),
    Io(std::io::Error),
}

pub fn put(w: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    let n = u32::try_from(bytes.len()).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "a frame over four gigabytes"))?;
    w.write_all(&n.to_be_bytes())?;
    w.write_all(bytes)?;
    w.flush()
}

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

    #[test]
    fn requests_and_replies_read_back_as_written() {
        let q = Request { verb: "annotate".into(), home: "/h".into(), args: vec![("subject".into(), "ab".into()), ("note".into(), "x y\n".into()), ("note".into(), "again".into())] };
        assert_eq!(Request::of_bytes(&q.to_bytes()), Some(q.clone()));
        assert_eq!(q.one("subject"), Some("ab"));
        assert_eq!(q.many("note"), vec!["x y\n", "again"]);
        let all = [
            Reply::Wrote { entry_id: "e".into(), ledger: "/l".into(), seq: 3 },
            Reply::Noticed { entry_id: "e".into(), ledger: "/l".into(), tx: "0x1".into(), notice: "n".into(), withdrawn: true },
            Reply::Anchored { tx: "t".into(), block: 9 },
            Reply::Reverted { tx: "t".into(), status: 0 },
            Reply::NotYet { tx: "t".into(), waited: 30 },
            Reply::Unheard { tx: "t".into(), detail: "no node".into() },
            Reply::Voided { tx: "t".into() },
            Reply::OnChain { names: vec!["h1".into(), "h2".into()], tx: "t".into(), block: 7 },
            Reply::Kit { kit_id: "k".into(), path: "/o".into(), entries: 2, files: 1, dropped: vec![".DS_Store".into()], originals: vec!["a.txt".into()], left_out: vec!["/gone/b.txt".into()] },
            Reply::Verified {
                kit_id: "k".into(),
                entries: 3,
                records: vec![
                    Record { entry_id: "e1".into(), content: "c1".into(), original: crate::recordcheck::Original::Match, chain: crate::recordcheck::OnChain::Anchored { first_at: 9 }, deleted: None },
                    Record { entry_id: "e2".into(), content: "c2".into(), original: crate::recordcheck::Original::Missing, chain: crate::recordcheck::OnChain::Unread, deleted: Some("0xd".into()) },
                ],
                taken: Some(Taken { path: "/t".into(), names: vec!["a 原件.pdf".into()] }),
                unanswered: vec!["eth_getLogs · 节点 a 答:范围过宽".into()],
            },
            Reply::Verified { kit_id: "k".into(), entries: 0, records: Vec::new(), taken: None, unanswered: Vec::new() },
            Reply::KitRefused { verdict: "v1".into(), subject: Some("a".into()) },
            Reply::KitRefused { verdict: "v2".into(), subject: None },
            Reply::Queued { count: 4 },
            Reply::GrantPackage { path: "/o/grant-1.zkgrant".into(), kit_id: "0x1".into(), files: 3, originals: vec!["work.txt".into()], left_out: vec!["/gone/work.txt".into()], sealed: false },
            Reply::Cleaned {
                done: true,
                removed: vec![Cleared { kind: "kit".into(), path: "/a".into(), link: Some("https://x/a".into()), why: None }],
                kept: vec![Cleared { kind: "mirror".into(), path: "/m".into(), link: None, why: Some("changed".into()) }],
            },
            Reply::OnDesktop,
            Reply::Refused { code: "LOCKED".into(), tail: "t".into(), network: true, zh: "锁".into(), en: "locked".into(), own: None },
            Reply::Refused { code: "BOOK_CODE".into(), tail: "11155111".into(), network: false, zh: "簿".into(), en: "book".into(), own: Some(Own::IssueRefused) },
            Reply::Refused { code: "LEDGER_NOT_FETCHED".into(), tail: String::new(), network: false, zh: "未取".into(), en: "unfetched".into(), own: Some(Own::ReadOnly) },
            Reply::Refused { code: "HANDED_OFF".into(), tail: "0xab".into(), network: false, zh: "交".into(), en: "handed".into(), own: Some(Own::HandedOff) },
            Reply::Refused { code: "UNREACHABLE".into(), tail: "n".into(), network: true, zh: "达".into(), en: "reach".into(), own: Some(Own::SendRefused) },
            Reply::RuleRefused { rule: Rule::Retraction, token: Some("E_RETRACT_REPEATED".into()), entry_id: None, names: None, zh: "删过".into(), en: "deleted".into() },
            Reply::RuleRefused { rule: Rule::Withheld, token: Some("E_GRANT_WORK_DELETED".into()), entry_id: None, names: Some(Vec::new()), zh: "删".into(), en: "gone".into() },
            Reply::RuleRefused { rule: Rule::WouldBreak, token: None, entry_id: Some("0xe".into()), names: Some(vec!["GAP".into()]), zh: "断".into(), en: "break".into() },
            Reply::Closed,
            Reply::Face { table: "{\"root\":\"0x11\"}".into(), moment: 1_800_000_000, omitted: vec!["10 NO_BOOK".into()] },
            Reply::Joined { joined: true, read: vec![ReadOf { root: "0x11".into(), moment: 9, omitted: Vec::new() }, ReadOf { root: "0x22".into(), moment: 8, omitted: vec!["10 NO_BOOK".into()] }] },
            Reply::Refused { code: "DISAGREE".into(), tail: "2 differ".into(), network: true, zh: "异".into(), en: "differ".into(), own: Some(Own::ChainUnread) },
            Reply::Refused { code: "FACE_ROW".into(), tail: "bound account x a".into(), network: false, zh: "行".into(), en: "row".into(), own: Some(Own::Face) },
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

    #[test]
    fn every_own_reason_reads_back_by_its_word() {
        for o in Own::ALL {
            assert_eq!(Own::of_str(o.as_str()), Some(o));
            let back = Reply::Refused { code: "C".into(), tail: "t".into(), network: false, zh: "z".into(), en: "e".into(), own: Some(o) };
            assert_eq!(Reply::of_bytes(&back.to_bytes()), Some(back.clone()));
        }
        let words: std::collections::BTreeSet<&str> = Own::ALL.iter().map(|o| o.as_str()).collect();
        assert_eq!(words.len(), Own::ALL.len());
        assert_eq!(Reply::of_bytes(br#"{"code":"C","en":"e","form":"zikaron-door/1","network":false,"own":"other","reply":"refused","tail":"t","zh":"z"}"#), None, "a reason this version does not know");
    }

    #[test]
    fn every_rule_reads_back_by_its_word() {
        for r in Rule::ALL {
            assert_eq!(Rule::of_str(r.as_str()), Some(r));
            let back = Reply::RuleRefused { rule: r, token: None, entry_id: None, names: None, zh: "z".into(), en: "e".into() };
            assert_eq!(Reply::of_bytes(&back.to_bytes()), Some(back.clone()));
        }
        let words: std::collections::BTreeSet<&str> = Rule::ALL.iter().map(|r| r.as_str()).collect();
        assert_eq!(words.len(), Rule::ALL.len());
        assert_eq!(Rule::of_str("ledger"), None);
    }

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
