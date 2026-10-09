//! What the direct `anchor` (no `--home`) sent, kept so a rerun asks about it first.
//!
//! The desktop keeps what it sent in its home's queue; the command line without `--home` has no home, so it
//! keeps its own record in this user's machine folder (`zikaron_os::machine`), the folder the command line
//! already reads to find the desktop's local endpoint. One anchoring is known by what it sends: the chain, the
//! account it goes from, where it goes and the bytes it carries ([`place`]); the same command run again is the
//! same anchoring. Each transaction signed for it is one small file there, landed after it is signed and before
//! it is broadcast (`zikaron_anchor::send::anchor_landed`): its nonce and its hash. Nothing else is kept (no
//! key, no endpoint), and nothing is ever removed. A transaction judged void (no node holds it, its nonce used
//! by another transaction) is marked so by one more file (`void` and its hash); a marked transaction is no
//! longer waited on, so the next run sends the anchoring afresh. Where the transactions stand on a rerun is
//! judged by the anchoring crate (`zikaron_anchor::send::earlier`, by the rule the app's queue uses too).
//!
//! Every path of this record is spelled here only.

use std::path::{Path, PathBuf};
use zikaron::json::Value;

/// The folder in the machine folder that holds this record.
pub const DIR: &str = "cli-sent";

/// The two members of one record file. The nonce is written as a `0x` quantity (a nonce is any 64-bit number,
/// past the ceiling of the law's integers), the hash as `0x` and 64 hex digits.
const NONCE: &str = "nonce";
const TX: &str = "tx";
const VOID: &str = "void";

/// A nonce as the record writes it, and read back the one way (`0x`, one to sixteen hex digits).
fn nonce_text(n: u64) -> String {
    format!("0x{n:x}")
}

fn nonce_of(x: &str) -> Option<u64> {
    let body = x.strip_prefix("0x")?;
    if body.is_empty() || body.len() > 16 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(body, 16).ok()
}

/// The machine folder, or the words for why it does not read.
pub fn machine() -> Result<PathBuf, String> {
    match zikaron_os::machine::here() {
        Ok((m, _)) => Ok(m),
        Err(e) => Err(format!("{e:?}")),
    }
}

/// Where the record of one anchoring lives: a folder named by the first sixteen bytes of the sha256 of the chain
/// id (eight bytes, big-endian), the sending account, the recipient and the call data, so the same command finds
/// the same folder and the name says none of them.
pub fn place(machine: &Path, chain: u64, from: &[u8; 20], to: &[u8; 20], data: &[u8]) -> PathBuf {
    let mut b = chain.to_be_bytes().to_vec();
    b.extend_from_slice(from);
    b.extend_from_slice(to);
    b.extend_from_slice(data);
    let d = zikaron::cryptox::sha256(&b);
    machine.join(DIR).join(zikaron::hexfmt::encode(&d[..16]).trim_start_matches("0x"))
}

/// The transactions recorded at `dir` and not marked void, each its nonce and hash, in the order sent (files named
/// by their number). No folder is none sent. A file that does not read as one record or one void mark refuses the
/// whole reading by its name: what was sent is never guessed.
pub fn read(dir: &Path) -> Result<Vec<(u64, [u8; 32])>, String> {
    let f = files(dir)?;
    Ok(f.sends.into_iter().filter(|(_, tx)| !f.void.contains(tx)).collect())
}

/// What the files at `dir` say: the transactions sent and the hashes marked void, and how many files there are.
struct Files {
    sends: Vec<(u64, [u8; 32])>,
    void: Vec<[u8; 32]>,
    count: usize,
}

fn files(dir: &Path) -> Result<Files, String> {
    let listing = match std::fs::read_dir(dir) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Files { sends: Vec::new(), void: Vec::new(), count: 0 }),
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    let mut got: Vec<(u64, u64, [u8; 32])> = Vec::new();
    let mut void: Vec<[u8; 32]> = Vec::new();
    let mut count = 0usize;
    for item in listing {
        let item = item.map_err(|e| format!("{}: {e}", dir.display()))?;
        let at = item.path();
        let name = item.file_name().to_string_lossy().into_owned();
        // The landing's own temporary siblings are not records; nor is a hidden file (a name starting with `.`:
        // this record never writes one; a file browser or a sync tool does, `.DS_Store`). Any other name that
        // does not read as a record still refuses the whole reading.
        if zikaron_glue::landing::is_beside_name(&name) || name.starts_with('.') {
            continue;
        }
        let bad = || format!("{}: not a record", at.display());
        let n: u64 = name.parse().map_err(|_| bad())?;
        let bytes = std::fs::read(&at).map_err(|e| format!("{}: {e}", at.display()))?;
        let v = zikaron::json::parse(&bytes).map_err(|_| bad())?;
        count += 1;
        let hash = |k: &str| -> Option<[u8; 32]> { v.member(k).and_then(|x| x.as_str()).and_then(zikaron::hexfmt::decode).and_then(|b| b.try_into().ok()) };
        if let Some(h) = hash(VOID) {
            void.push(h);
            continue;
        }
        let nonce = v.member(NONCE).and_then(|x| x.as_str()).and_then(nonce_of).ok_or_else(bad)?;
        let tx: [u8; 32] = v.member(TX).and_then(|x| x.as_str()).and_then(zikaron::hexfmt::decode).and_then(|b| b.try_into().ok()).ok_or_else(bad)?;
        got.push((n, nonce, tx));
    }
    got.sort_by_key(|(n, _, _)| *n);
    Ok(Files { sends: got.into_iter().map(|(_, nonce, tx)| (nonce, tx)).collect(), void, count })
}

/// Land one more record at `dir`: the transaction signed at `nonce` with hash `tx`, as the next file. A record
/// that cannot be landed says so (the landing's code and subject, and the system's words).
pub fn land(dir: &Path, nonce: u64, tx: &[u8; 32]) -> Result<(), String> {
    let body = Value::Obj(vec![(NONCE.into(), Value::Str(nonce_text(nonce))), (TX.into(), Value::Str(zikaron::hexfmt::encode(tx)))]);
    land_next(dir, &body)
}

/// Mark the transactions `txs` void at `dir` (one more file each): they are no longer read as sent.
pub fn void(dir: &Path, txs: &[(u64, [u8; 32])]) -> Result<(), String> {
    for (_, tx) in txs {
        land_next(dir, &Value::Obj(vec![(VOID.into(), Value::Str(zikaron::hexfmt::encode(tx)))]))?;
    }
    Ok(())
}

/// Land one more file at `dir`, named by the next number.
fn land_next(dir: &Path, body: &Value) -> Result<(), String> {
    let said = |t: zikaron_glue::landing::Trouble| format!("{} {} {}", t.code(), t.subject(), t.said().unwrap_or_default());
    zikaron_glue::landing::mkdir(dir).map_err(said)?;
    let next = files(dir)?.count;
    zikaron_glue::landing::land_bytes(&dir.join(next.to_string()), &zikaron::json::canon_bytes(body)).map_err(said)
}
