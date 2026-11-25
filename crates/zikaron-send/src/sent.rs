use std::path::{Path, PathBuf};
use zikaron::json::Value;

pub const DIR: &str = "cli-sent";

const NONCE: &str = "nonce";
const TX: &str = "tx";
const VOID: &str = "void";

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

pub struct Seal([u8; 32]);

pub const SEAL_INFO: &str = "zikaron cli-sent/1";

impl Seal {
    pub fn of(secret: &[u8; 32], dir: &Path) -> Seal {
        let mut info = SEAL_INFO.as_bytes().to_vec();
        info.extend_from_slice(dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().as_bytes());
        let mut k = [0u8; 32];
        let _ = zikaron_seal::crypto::hkdf_sha256(secret, &info, &mut k);
        Seal(k)
    }

    fn seal(&self, name: &str, plain: &[u8]) -> Result<Vec<u8>, String> {
        let mut nonce = [0u8; 24];
        zikaron_os::fill_random(&mut nonce).map_err(|e| format!("{}: {e}", zikaron_os::ENTROPY_SOURCE))?;
        let body = zikaron_seal::crypto::xchacha_seal(&self.0, &nonce, name.as_bytes(), plain).ok_or_else(|| "seal".to_string())?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&body);
        Ok(out)
    }

    pub fn open(&self, name: &str, bytes: &[u8]) -> Option<Vec<u8>> {
        if zikaron::json::parse(bytes).is_ok() {
            return Some(bytes.to_vec());
        }
        let (nonce, body) = bytes.split_at_checked(24)?;
        zikaron_seal::crypto::xchacha_open(&self.0, nonce.try_into().ok()?, name.as_bytes(), body)
    }
}

impl Drop for Seal {
    fn drop(&mut self) {
        zikaron_os::wipe(&mut self.0);
    }
}

pub fn machine() -> Result<PathBuf, String> {
    match zikaron_os::machine::here() {
        Ok((m, _)) => Ok(m),
        Err(e) => Err(format!("{e:?}")),
    }
}

pub fn place(machine: &Path, chain: u64, from: &[u8; 20], to: &[u8; 20], data: &[u8]) -> PathBuf {
    let mut b = chain.to_be_bytes().to_vec();
    b.extend_from_slice(from);
    b.extend_from_slice(to);
    b.extend_from_slice(data);
    let d = zikaron::cryptox::sha256(&b);
    machine.join(DIR).join(zikaron::hexfmt::encode(&d[..16]).trim_start_matches("0x"))
}

pub fn read(dir: &Path, seal: &Seal) -> Result<Vec<(u64, [u8; 32])>, String> {
    let f = files(dir, seal)?;
    Ok(f.sends.into_iter().filter(|(_, tx)| !f.void.contains(tx)).collect())
}

struct Files {
    sends: Vec<(u64, [u8; 32])>,
    void: Vec<[u8; 32]>,
    count: usize,
}

fn files(dir: &Path, seal: &Seal) -> Result<Files, String> {
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
        if name.starts_with('.') {
            continue;
        }
        let bad = || format!("{}: not a record", at.display());
        let n: u64 = name.parse().map_err(|_| bad())?;
        let bytes = std::fs::read(&at).map_err(|e| format!("{}: {e}", at.display()))?;
        let plain = seal.open(&name, &bytes).ok_or_else(bad)?;
        let v = zikaron::json::parse(&plain).map_err(|_| bad())?;
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

pub fn land(dir: &Path, seal: &Seal, nonce: u64, tx: &[u8; 32]) -> Result<(), String> {
    let body = Value::Obj(vec![(NONCE.into(), Value::Str(nonce_text(nonce))), (TX.into(), Value::Str(zikaron::hexfmt::encode(tx)))]);
    land_next(dir, seal, &body)
}

pub fn void(dir: &Path, seal: &Seal, txs: &[(u64, [u8; 32])]) -> Result<(), String> {
    for (_, tx) in txs {
        land_next(dir, seal, &Value::Obj(vec![(VOID.into(), Value::Str(zikaron::hexfmt::encode(tx)))]))?;
    }
    Ok(())
}

fn land_next(dir: &Path, seal: &Seal, body: &Value) -> Result<(), String> {
    let said = |t: zikaron_glue::landing::Trouble| format!("{} {} {}", t.code(), t.subject(), t.said().unwrap_or_default());
    zikaron_glue::landing::mkdir(dir).map_err(said)?;
    let next = files(dir, seal)?.count.to_string();
    let sealed = seal.seal(&next, &zikaron::json::canon_bytes(body))?;
    zikaron_glue::landing::land_bytes(&dir.join(&next), &sealed).map_err(said)
}

const LOGS_LOCK: &str = "logs";

pub const LOGS_INFO: &str = "zikaron cli-sent-logs/1";

pub struct Logs {
    path: PathBuf,
    seal: Seal,
}

impl Logs {
    fn part_path(&self, part: &str) -> Option<PathBuf> {
        let base = self.path.file_name()?.to_string_lossy().into_owned();
        Some(self.path.with_file_name(format!("{base}-{part}")))
    }
}

pub fn logs_at(machine: &Path, chain: u64, from: &[u8; 20], registry: &[u8; 20], secret: &[u8; 32]) -> Logs {
    let mut b = chain.to_be_bytes().to_vec();
    b.extend_from_slice(from);
    b.extend_from_slice(registry);
    let d = zikaron::cryptox::sha256(&b);
    let path = machine.join(DIR).join(format!("logs-{}", zikaron::hexfmt::encode(&d[..16]).trim_start_matches("0x")));
    let mut info = LOGS_INFO.as_bytes().to_vec();
    info.extend_from_slice(path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().as_bytes());
    let mut k = [0u8; 32];
    let _ = zikaron_seal::crypto::hkdf_sha256(secret, &info, &mut k);
    let seal = Seal(k);
    zikaron_os::wipe(&mut k);
    Logs { path, seal }
}

pub fn forget_logs(machine: &Path) -> std::io::Result<bool> {
    let mut had = false;
    let plain = crate::checked::Plain(machine.to_path_buf()).dir();
    {
        let _held = crate::checked::hold_at(&plain, crate::checked::LOGS);
        had |= crate::checked::forget_logs_in(&plain)?;
    }
    {
        let _held = crate::checked::hold_at(&plain, crate::checked::FILE);
        match std::fs::remove_file(plain.join(crate::checked::FILE)) {
            Ok(()) => had = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    let dir = machine.join(DIR);
    let _held = crate::checked::hold_at(&dir, LOGS_LOCK);
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(had),
        Err(e) => return Err(e),
    };
    for e in entries {
        let e = e?;
        if e.file_name().to_string_lossy().starts_with("logs-") && e.file_type()?.is_file() {
            std::fs::remove_file(e.path())?;
            had = true;
        }
    }
    Ok(had)
}

impl zikaron_anchor::settled::Keep for Logs {
    fn load(&self, part: &str) -> Option<Vec<u8>> {
        let path = self.part_path(part)?;
        let name = path.file_name()?.to_string_lossy().into_owned();
        let bytes = std::fs::read(&path).ok()?;
        let (nonce, body) = bytes.split_at_checked(24)?;
        zikaron_seal::crypto::xchacha_open(&self.seal.0, nonce.try_into().ok()?, name.as_bytes(), body)
    }

    fn save(&self, part: &str, plain: &[u8]) -> Result<(), zikaron_anchor::settled::Unkept> {
        let path = self.part_path(part).ok_or_else(|| format!("no place for {part}"))?;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| format!("no name for {part}"))?;
        let sealed = self.seal.seal(&name, plain)?;
        let dir = path.parent().ok_or_else(|| format!("no folder for {part}"))?;
        zikaron_seal::land::put_at(dir, &name, &sealed).map_err(|t| crate::checked::unkept_of(&t))?;
        let _ = std::fs::remove_file(&self.path);
        Ok(())
    }

    fn parts(&self) -> Vec<String> {
        let (Some(dir), Some(base)) = (self.path.parent(), self.path.file_name().map(|n| format!("{}-", n.to_string_lossy()))) else { return Vec::new() };
        crate::checked::files_in(dir).iter().filter_map(|f| f.strip_prefix(&base).map(str::to_string)).collect()
    }

    fn hold(&self) -> Option<Box<dyn std::any::Any>> {
        crate::checked::hold_at(self.path.parent()?, LOGS_LOCK).map(|l| Box::new(l) as Box<dyn std::any::Any>)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_settled_log_reads_are_forgotten_and_the_records_of_sends_stay() {
        let machine = std::env::temp_dir().join(format!("zk-sent-forget-{}", std::process::id()));
        let dir = machine.join(super::DIR);
        let record = dir.join("0123456789abcdef0123456789abcdef");
        std::fs::create_dir_all(&record).expect("a record folder");
        std::fs::write(record.join("0"), b"sealed").expect("a record file");
        std::fs::write(dir.join("logs-aaaa"), b"sealed").expect("a log read");
        std::fs::write(dir.join("logs-bbbb.tmp"), b"half").expect("a write cut short");
        assert_eq!(super::forget_logs(&machine).ok(), Some(true));
        assert!(!dir.join("logs-aaaa").exists() && !dir.join("logs-bbbb.tmp").exists(), "the reads are gone");
        assert!(record.join("0").exists(), "the record of what was sent stays");
        assert_eq!(super::forget_logs(&machine).ok(), Some(false), "nothing left to forget");
        assert_eq!(super::forget_logs(&machine.join("none")).ok(), Some(false), "no folder: nothing there");
        let _ = std::fs::remove_dir_all(&machine);
    }
}
