//! The command line's credential: the one thing the command line is given to open this machine's sealed homes
//! without the passcode. The desktop issues it when "write from the command line alone" is turned on (after the
//! passcode), removes it when the switch goes off, at a new master key, or when it is found expired.
//!
//! The file sits in the machine directory, owner-only (`zikaron_os::put_at`), and is a closed table of four
//! members: a fresh random key K, the master key sealed under K (the one authenticated cipher of every sealed
//! file), when it was issued, and when it expires (`null`: never). Nothing else is in it, and a file with any
//! other member is refused. A changed passcode leaves the master key as it was, so the credential keeps
//! working; a new master key makes it worthless, which is why the desktop removes it then.
//!
//! Its expiry is checked at every opening: an expired credential is removed on the way out and refused by name
//! (`CredentialExpired`). The clock it is checked against is the caller's (`now`, seconds since the epoch).

use crate::keybox::Plain;
use crate::places::{credential_file_account, credential_file_for};
use crate::trouble::{random, Code, Trouble};
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};

/// The credential's members, in canonical order. Closed: a file with another member, or without one, is refused.
pub mod member {
    pub const EXPIRES: &str = "expires";
    pub const ISSUED: &str = "issued";
    pub const KEY: &str = "key";
    pub const MASTER: &str = "master";
    pub const ALL: [&str; 4] = [EXPIRES, ISSUED, KEY, MASTER];
}

/// The additional data the master key is sealed with, so the sealed bytes open as nothing but a credential.
pub const DOMAIN: &[u8] = b"zikaron/cli-credential/1";

/// When a credential was issued and when it stops working (`None`: never), in seconds since the epoch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Term {
    pub issued: u64,
    pub expires: Option<u64>,
}

impl Term {
    /// Whether it has expired at `now`.
    pub fn expired(&self, now: u64) -> bool {
        self.expires.is_some_and(|e| now >= e)
    }
}

/// The credential file's path for this account base.
pub fn path(dir: &Path, account: &str) -> PathBuf {
    dir.join(credential_file_for(account))
}

/// Issue a credential for the master key `mk`: a fresh K, `mk` sealed under it, written whole and owner-only,
/// replacing any earlier one.
pub fn issue(dir: &Path, account: &str, mk: &[u8; 32], term: Term) -> Result<(), Trouble> {
    if term.expires.is_some_and(|e| e <= term.issued) || term.issued > json::MAX_INT || term.expires.is_some_and(|e| e > json::MAX_INT) {
        return Err(Trouble::new(Code::CredentialShape, format!("{} {:?}", term.issued, term.expires)));
    }
    let key = Plain::new(random(32)?);
    let nonce = random(crate::envelope::NONCE)?;
    let k: &[u8; 32] = key.bytes().try_into().map_err(|_| Trouble::new(Code::CredentialShape, member::KEY))?;
    let n: &[u8; 24] = nonce.as_slice().try_into().map_err(|_| Trouble::new(Code::CredentialShape, member::MASTER))?;
    let sealed = crate::crypto::xchacha_seal(k, n, DOMAIN, mk).ok_or_else(|| Trouble::new(Code::CredentialShape, member::MASTER))?;
    let mut held = nonce.clone();
    held.extend_from_slice(&sealed);
    let bytes = json::canon_bytes(&Value::Obj(vec![
        (member::EXPIRES.to_string(), term.expires.map(Value::Int).unwrap_or(Value::Null)),
        (member::ISSUED.to_string(), Value::Int(term.issued)),
        (member::KEY.to_string(), Value::Str(zikaron::hexfmt::encode(key.bytes()))),
        (member::MASTER.to_string(), Value::Str(zikaron::hexfmt::encode(&held))),
    ]));
    crate::land::put_at(dir, &credential_file_for(account), &bytes)
}

/// Remove the credential; true when there was one.
pub fn remove(dir: &Path, account: &str) -> Result<bool, Trouble> {
    let p = path(dir, account);
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Trouble::io(&e, &p.display().to_string())),
    }
}

/// The account base of the one credential in `dir`: the command line finds whose machine files it opens this
/// way. None there is `CredentialMissing`; more than one is `CredentialShape` with their names.
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

/// A credential as read: its term, K and the sealed master key.
struct Read {
    term: Term,
    key: Plain,
    master: Vec<u8>,
}

fn read(dir: &Path, account: &str) -> Result<Read, Trouble> {
    let p = path(dir, account);
    let at = p.display().to_string();
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Trouble::new(Code::CredentialMissing, at)),
        Err(e) => return Err(Trouble::io(&e, &at)),
    };
    let shape = |what: &str| Trouble::new(Code::CredentialShape, format!("{at}: {what}"));
    let v = json::parse(&bytes).map_err(|t| shape(&format!("{t:?}")))?;
    let Value::Obj(m) = &v else { return Err(shape("{}")) };
    let names: Vec<&str> = m.iter().map(|(k, _)| k.as_str()).collect();
    if names != member::ALL {
        return Err(shape(&names.join(" ")));
    }
    let get = |k: &str| v.member(k).ok_or_else(|| shape(k));
    let issued = match get(member::ISSUED)? {
        Value::Int(n) => *n,
        _ => return Err(shape(member::ISSUED)),
    };
    let expires = match get(member::EXPIRES)? {
        Value::Int(n) if *n > issued => Some(*n),
        Value::Null => None,
        _ => return Err(shape(member::EXPIRES)),
    };
    let hex = |k: &str| match get(k)? {
        Value::Str(s) => zikaron::hexfmt::decode(s).ok_or_else(|| shape(k)),
        _ => Err(shape(k)),
    };
    let key = Plain::new(hex(member::KEY)?);
    if key.bytes().len() != 32 {
        return Err(shape(member::KEY));
    }
    let master = hex(member::MASTER)?;
    if master.len() != crate::envelope::NONCE + 32 + 16 {
        return Err(shape(member::MASTER));
    }
    Ok(Read { term: Term { issued, expires }, key, master })
}

/// The credential's term, without opening it (the desktop shows when it expires).
pub fn term(dir: &Path, account: &str) -> Result<Term, Trouble> {
    Ok(read(dir, account)?.term)
}

/// Open the credential at `now`: the master key it holds. Missing is `CredentialMissing`; not in its shape, or
/// not opening, is `CredentialShape`; expired is removed and refused as `CredentialExpired` (the expiry in the
/// evidence).
pub fn open(dir: &Path, account: &str, now: u64) -> Result<Plain, Trouble> {
    let r = read(dir, account)?;
    if r.term.expired(now) {
        remove(dir, account)?;
        let expires = r.term.expires.unwrap_or_default();
        return Err(Trouble::new(Code::CredentialExpired, format!("{} {expires}", path(dir, account).display())));
    }
    let shape = || Trouble::new(Code::CredentialShape, format!("{}: {}", path(dir, account).display(), member::MASTER));
    let k: &[u8; 32] = r.key.bytes().try_into().map_err(|_| shape())?;
    let (nonce, sealed) = r.master.split_at(crate::envelope::NONCE);
    let n: &[u8; 24] = nonce.try_into().map_err(|_| shape())?;
    let mk = crate::crypto::xchacha_open(k, n, DOMAIN, sealed).ok_or_else(shape)?;
    if mk.len() != 32 {
        return Err(shape());
    }
    Ok(Plain::new(mk))
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

    #[test]
    fn a_credential_is_four_members_owner_only_and_opens_to_the_master_key() {
        let d = dir("closed");
        let mk = [7u8; 32];
        issue(&d, "t", &mk, Term { issued: 100, expires: Some(200) }).expect("issued");
        let p = path(&d, "t");
        assert!(zikaron_os::is_owner_only(&p).expect("checked"));
        let v = json::parse(&std::fs::read(&p).expect("read")).expect("json");
        let Value::Obj(m) = v else { panic!("object") };
        assert_eq!(m.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), member::ALL);
        assert_eq!(open(&d, "t", 150).expect("opens").bytes(), &mk);
        assert_eq!(find(&d).expect("found"), "t");
        assert_eq!(term(&d, "t").expect("term"), Term { issued: 100, expires: Some(200) });
    }

    #[test]
    fn an_expired_credential_is_refused_by_name_and_removed() {
        let d = dir("expired");
        issue(&d, "t", &[1u8; 32], Term { issued: 100, expires: Some(200) }).expect("issued");
        assert_eq!(open(&d, "t", 200).err().map(|t| t.code), Some(Code::CredentialExpired));
        assert!(!path(&d, "t").exists());
        assert_eq!(open(&d, "t", 150).err().map(|t| t.code), Some(Code::CredentialMissing));
    }

    #[test]
    fn a_credential_that_never_expires_opens_at_any_time() {
        let d = dir("never");
        issue(&d, "t", &[2u8; 32], Term { issued: 100, expires: None }).expect("issued");
        assert_eq!(open(&d, "t", u64::MAX).expect("opens").bytes(), &[2u8; 32]);
    }

    #[test]
    fn a_credential_with_another_member_or_a_changed_byte_is_refused() {
        let d = dir("shape");
        issue(&d, "t", &[3u8; 32], Term { issued: 100, expires: None }).expect("issued");
        let p = path(&d, "t");
        let text = String::from_utf8(std::fs::read(&p).expect("read")).expect("utf8");
        std::fs::write(&p, text.replacen('{', "{\"extra\":1,", 1)).expect("written");
        assert_eq!(open(&d, "t", 150).err().map(|t| t.code), Some(Code::CredentialShape));
        issue(&d, "t", &[3u8; 32], Term { issued: 100, expires: None }).expect("issued");
        let mut b = std::fs::read(&p).expect("read");
        let at = b.len() - 4;
        b[at] = if b[at] == b'0' { b'1' } else { b'0' };
        std::fs::write(&p, b).expect("written");
        assert_eq!(open(&d, "t", 150).err().map(|t| t.code), Some(Code::CredentialShape));
    }
}
