use crate::crypto;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zikaron_glue::addressed::{self as form, BadHead, HEAD, INFO, POINT, SEGMENT, TAG};

const KEY: usize = 32;
const PREFIX: usize = 19;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    NotAddressed,
    Head(String),
    NotRecipient,
    Broken(String),
    Recipient,
    Io(String),
}

pub mod words {
    pub const NOT_RECIPIENT: (&str, &str) = ("这份加密包不是给本机身份的", "This encrypted package is not for an identity on this machine");
    pub const BROKEN: (&str, &str) = ("加密记录包已损坏或不完整,没有解开", "The encrypted record package is damaged or incomplete; it was not opened");
    pub const HEAD: (&str, &str) = ("加密记录包的格式不认识,没有解开", "The encrypted record package is in a form not read here; it was not opened");
    pub const RECIPIENT: (&str, &str) = ("接收方公钥不是曲线上的点", "The recipient's public key is not a point of the curve");
    pub const ELSEWHERE: (&str, &str) = ("这是加密记录包,请在「核验记录」打开", "This is an encrypted record package. Open it under Verify record");
    pub const BOTH: (&str, &str) = ("记录接收方地址与公钥只填一个", "Give the recipient's address or public key, not both");
    pub const FOLDER: (&str, &str) = ("加密只用于单文件记录包", "Only a one-file record package is encrypted");
    pub const NOT_ANCHORED: (&str, &str) = ("导出加密记录失败:该地址未进行链上锚定", "Encrypted record package not exported: this address has not anchored on chain");
    pub const CHAIN_UNREAD: (&str, &str) = ("导出加密记录失败:读链失败", "Encrypted record package not exported: the chain could not be read");
    pub const UNPROVEN: (&str, &str) = ("导出加密记录失败:节点给的交易证不出自身,请换一台节点", "Encrypted record package not exported: a node gave a transaction that does not prove itself; use another node");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    NotRecipient,
    Broken,
    Head,
    Elsewhere,
    KeyShape,
    Both,
    Folder,
    NotAnchored,
    ChainUnread,
    Unproven,
}

impl Why {
    pub const ALL: [Why; 10] = [Why::NotRecipient, Why::Broken, Why::Head, Why::Elsewhere, Why::KeyShape, Why::Both, Why::Folder, Why::NotAnchored, Why::ChainUnread, Why::Unproven];

    pub fn code(self) -> &'static str {
        match self {
            Why::NotRecipient => "ADDRESSED_NOT_RECIPIENT",
            Why::Broken => "ADDRESSED_BROKEN",
            Why::Head => "ADDRESSED_HEAD",
            Why::Elsewhere => "ADDRESSED_ELSEWHERE",
            Why::KeyShape => "ADDRESSED_KEY_SHAPE",
            Why::Both => "ADDRESSED_BOTH",
            Why::Folder => "ADDRESSED_FOLDER",
            Why::NotAnchored => "ADDRESS_NOT_ANCHORED",
            Why::ChainUnread => "ADDRESS_CHAIN_UNREAD",
            Why::Unproven => "ADDRESS_KEY_UNPROVEN",
        }
    }

    pub fn words(self) -> (&'static str, &'static str) {
        match self {
            Why::NotRecipient => words::NOT_RECIPIENT,
            Why::Broken => words::BROKEN,
            Why::Head => words::HEAD,
            Why::Elsewhere => words::ELSEWHERE,
            Why::KeyShape => words::RECIPIENT,
            Why::Both => words::BOTH,
            Why::Folder => words::FOLDER,
            Why::NotAnchored => words::NOT_ANCHORED,
            Why::ChainUnread => words::CHAIN_UNREAD,
            Why::Unproven => words::UNPROVEN,
        }
    }

    pub fn unanswered(self) -> bool {
        self == Why::ChainUnread
    }

    pub fn of(r: &Refused) -> Option<Why> {
        match r {
            Refused::NotRecipient => Some(Why::NotRecipient),
            Refused::Broken(_) => Some(Why::Broken),
            Refused::Head(_) | Refused::NotAddressed => Some(Why::Head),
            Refused::Recipient => Some(Why::KeyShape),
            Refused::Io(_) => None,
        }
    }
}

impl Refused {
    pub fn words(&self) -> Option<(&'static str, &'static str)> {
        Why::of(self).map(Why::words)
    }

    pub fn code(&self) -> &'static str {
        match self {
            Refused::NotAddressed => "NOT_ADDRESSED",
            Refused::Head(_) => "HEAD",
            Refused::NotRecipient => "NOT_RECIPIENT",
            Refused::Broken(_) => "BROKEN",
            Refused::Recipient => "RECIPIENT",
            Refused::Io(_) => "IO",
        }
    }

    pub fn tail(&self) -> &str {
        match self {
            Refused::Head(s) | Refused::Broken(s) | Refused::Io(s) => s,
            _ => "",
        }
    }
}

pub fn key_text(text: &str) -> Option<[u8; POINT]> {
    let t = text.trim().to_ascii_lowercase();
    if !t.starts_with("0x") || !matches!(t.len(), 68 | 132) {
        return None;
    }
    crypto::point(&zikaron::hexfmt::decode(&t)?)
}

pub trait Recipient {
    fn public(&self) -> [u8; POINT];
    fn agree(&self, one_time: &[u8; POINT]) -> Option<[u8; 32]>;
}

pub struct Bytes(pub [u8; 32]);

impl Recipient for Bytes {
    fn public(&self) -> [u8; POINT] {
        crypto::public_of(&self.0).unwrap_or([0; POINT])
    }
    fn agree(&self, one_time: &[u8; POINT]) -> Option<[u8; 32]> {
        crypto::agree(&self.0, one_time)
    }
}

impl Drop for Bytes {
    fn drop(&mut self) {
        zikaron_os::wipe(&mut self.0);
    }
}

struct Schedule {
    key: [u8; KEY],
    prefix: [u8; PREFIX],
}

impl Schedule {
    fn of(shared_x: &[u8; 32], one_time: &[u8; POINT], recipient: &[u8; POINT]) -> Schedule {
        let mut salt = [0u8; 2 * POINT];
        salt[..POINT].copy_from_slice(one_time);
        salt[POINT..].copy_from_slice(recipient);
        let mut okm = [0u8; KEY + PREFIX];
        let _ = crypto::hkdf_sha256_salted(&salt, shared_x, INFO.as_bytes(), &mut okm);
        let mut s = Schedule { key: [0; KEY], prefix: [0; PREFIX] };
        s.key.copy_from_slice(&okm[..KEY]);
        s.prefix.copy_from_slice(&okm[KEY..]);
        zikaron_os::wipe(&mut okm);
        s
    }

    fn nonce(&self, i: u32, last: bool) -> [u8; 24] {
        let mut n = [0u8; 24];
        n[..PREFIX].copy_from_slice(&self.prefix);
        n[PREFIX..PREFIX + 4].copy_from_slice(&i.to_be_bytes());
        n[23] = u8::from(last);
        n
    }
}

impl Drop for Schedule {
    fn drop(&mut self) {
        zikaron_os::wipe(&mut self.key);
        zikaron_os::wipe(&mut self.prefix);
    }
}

fn fill(r: &mut dyn Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        match r.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(got)
}

fn io(what: &str, e: &std::io::Error) -> Refused {
    Refused::Io(format!("{what}: {e}"))
}

pub fn seal(recipient: &[u8], plain: &mut dyn Read, out: &mut dyn Write) -> Result<[u8; POINT], Refused> {
    let to = crypto::point(recipient).ok_or(Refused::Recipient)?;
    let mut e = crypto::fresh_secret().map_err(|t| Refused::Io(t.tail))?;
    let one_time = crypto::public_of(&e);
    let shared = crypto::agree(&e, &to);
    zikaron_os::wipe(&mut e);
    let (Some(one_time), Some(mut shared)) = (one_time, shared) else { return Err(Refused::Recipient) };
    let sched = Schedule::of(&shared, &one_time, &to);
    zikaron_os::wipe(&mut shared);
    let head = form::lay_head(&one_time);
    out.write_all(&head).map_err(|e| io("write", &e))?;
    let mut cur = vec![0u8; SEGMENT];
    let mut next = vec![0u8; SEGMENT];
    let mut n = fill(plain, &mut cur).map_err(|e| io("read", &e))?;
    let mut i: u32 = 0;
    loop {
        let m = if n == SEGMENT { fill(plain, &mut next).map_err(|e| io("read", &e))? } else { 0 };
        let last = m == 0;
        let sealed = crypto::xchacha_seal(&sched.key, &sched.nonce(i, last), &head, &cur[..n]).ok_or_else(|| Refused::Io("seal".into()))?;
        out.write_all(&sealed).map_err(|e| io("write", &e))?;
        if last {
            break;
        }
        i = i.checked_add(1).ok_or_else(|| Refused::Io("too many segments".into()))?;
        std::mem::swap(&mut cur, &mut next);
        n = m;
    }
    zikaron_os::wipe(&mut cur);
    zikaron_os::wipe(&mut next);
    out.flush().map_err(|e| io("write", &e))?;
    Ok(one_time)
}

#[derive(Debug)]
pub enum Sealing<E> {
    Lay(E),
    Refused(Refused),
    Land(zikaron_glue::landing::Trouble),
}

pub fn seal_out<T, E>(to: &[u8; POINT], out: &Path, lay: impl FnOnce(&Path) -> Result<T, E>) -> Result<(T, [u8; POINT]), Sealing<E>> {
    use zikaron_glue::landing::{land_stream, Readers, Streamed, Trouble};
    if out.exists() {
        return Err(Sealing::Land(Trouble::Occupied(out.display().to_string())));
    }
    let room = sealing_room().map_err(Sealing::Refused)?;
    let plain = room.0.join(zikaron_glue::addressed::default_name());
    let laid = lay(&plain).map_err(Sealing::Lay)?;
    let mut src = std::fs::File::open(&plain).map_err(|e| Sealing::Refused(io(&format!("open {}", plain.display()), &e)))?;
    let mut one_time = [0u8; POINT];
    land_stream(Readers::Anyone, out, |f| {
        let mut w = std::io::BufWriter::new(f);
        one_time = seal(to, &mut src, &mut w)?;
        w.flush().map_err(|e| io("write", &e))
    })
    .map_err(|s| match s {
        Streamed::Fill(r) => Sealing::Refused(r),
        Streamed::Land(t) => Sealing::Land(t),
    })?;
    Ok((laid, one_time))
}

struct SealingRoom(PathBuf);

impl Drop for SealingRoom {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sealing_room() -> Result<SealingRoom, Refused> {
    temp_folder("zikaron-sealing-").map(SealingRoom)
}

fn temp_folder(stem: &str) -> Result<PathBuf, Refused> {
    let mut n = [0u8; 8];
    crate::trouble::fill_random(&mut n).map_err(|t| Refused::Io(t.tail))?;
    let dir = std::env::temp_dir().join(format!("{stem}{}", zikaron::hexfmt::encode(&n).trim_start_matches("0x")));
    std::fs::create_dir(&dir).map_err(|e| io(&format!("create {}", dir.display()), &e))?;
    Ok(dir)
}

pub struct Opened {
    pub dir: PathBuf,
    pub file: PathBuf,
    pub key: usize,
}

impl Drop for Opened {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

const OPENED_NAME: &str = "kit.zkit";

#[cfg(test)]
const MAGIC_LEN: usize = zikaron_glue::addressed::MAGIC.len();

fn read_head(r: &mut dyn Read) -> Result<([u8; HEAD], [u8; POINT]), Refused> {
    let mut head = [0u8; HEAD];
    let n = fill(r, &mut head).map_err(|e| io("read", &e))?;
    let h = form::head(&head[..n]).map_err(|b| match b {
        BadHead::NotAddressed => Refused::NotAddressed,
        BadHead::Short => Refused::Head("short".into()),
        BadHead::Version(v) => Refused::Head(format!("version {v}")),
    })?;
    let p = crypto::point(&h.point).ok_or_else(|| Refused::Head("one-time key".into()))?;
    Ok((head, p))
}

fn next_segment(r: &mut dyn Read, held: &mut Option<u8>, buf: &mut Vec<u8>) -> std::io::Result<bool> {
    buf.clear();
    buf.resize(SEGMENT + TAG, 0);
    let mut at = 0;
    if let Some(b) = held.take() {
        buf[0] = b;
        at = 1;
    }
    let n = at + fill(r, &mut buf[at..])?;
    buf.truncate(n);
    if n < SEGMENT + TAG {
        return Ok(true);
    }
    let mut one = [0u8; 1];
    if fill(r, &mut one)? == 0 {
        return Ok(true);
    }
    *held = Some(one[0]);
    Ok(false)
}

pub fn open(file: &Path, keys: &[&dyn Recipient]) -> Result<Opened, Refused> {
    let f = std::fs::File::open(file).map_err(|e| io(&format!("open {}", file.display()), &e))?;
    let mut r = std::io::BufReader::new(f);
    let (head, one_time) = read_head(&mut r)?;
    let mut held = None;
    let mut seg = Vec::new();
    let mut last = next_segment(&mut r, &mut held, &mut seg).map_err(|e| io("read", &e))?;
    if seg.len() < TAG {
        return Err(Refused::Broken("0".into()));
    }
    let mut found = None;
    for (k, key) in keys.iter().enumerate() {
        let Some(mut x) = key.agree(&one_time) else { continue };
        let sched = Schedule::of(&x, &one_time, &key.public());
        zikaron_os::wipe(&mut x);
        if let Some(plain) = crypto::xchacha_open(&sched.key, &sched.nonce(0, last), &head, &seg) {
            found = Some((k, sched, plain));
            break;
        }
    }
    let Some((k, sched, first)) = found else { return Err(Refused::NotRecipient) };
    let opened = room()?;
    let wrote = (|| -> Result<(), Refused> {
        let mut o = zikaron_os::Options::new();
        o.write(true).create_new(true);
        zikaron_os::owner_only(&mut o);
        let out = o.open(&opened.file).map_err(|e| io("write", &e))?;
        let mut w = std::io::BufWriter::new(out);
        w.write_all(&first).map_err(|e| io("write", &e))?;
        let mut i: u32 = 0;
        while !last {
            i = i.checked_add(1).ok_or_else(|| Refused::Broken(i.to_string()))?;
            last = next_segment(&mut r, &mut held, &mut seg).map_err(|e| io("read", &e))?;
            let mut plain = crypto::xchacha_open(&sched.key, &sched.nonce(i, last), &head, &seg).ok_or_else(|| Refused::Broken(i.to_string()))?;
            w.write_all(&plain).map_err(|e| io("write", &e))?;
            zikaron_os::wipe(&mut plain);
        }
        let f = w.into_inner().map_err(|e| io("write", e.error()))?;
        f.sync_all().map_err(|e| io("write", &e))
    })();
    let mut opened = opened;
    match wrote {
        Ok(()) => {
            opened.key = k;
            Ok(opened)
        }
        Err(r) => Err(r),
    }
}

fn room() -> Result<Opened, Refused> {
    let dir = temp_folder("zikaron-opened-")?;
    let file = dir.join(OPENED_NAME);
    Ok(Opened { dir, file, key: usize::MAX })
}

pub fn recipient_among(file: &Path, keys: &[&dyn Recipient]) -> Result<Option<usize>, Refused> {
    let f = std::fs::File::open(file).map_err(|e| io(&format!("open {}", file.display()), &e))?;
    let mut r = std::io::BufReader::new(f);
    let (head, one_time) = read_head(&mut r)?;
    let mut held = None;
    let mut seg = Vec::new();
    let last = next_segment(&mut r, &mut held, &mut seg).map_err(|e| io("read", &e))?;
    for (k, key) in keys.iter().enumerate() {
        let Some(mut x) = key.agree(&one_time) else { continue };
        let sched = Schedule::of(&x, &one_time, &key.public());
        zikaron_os::wipe(&mut x);
        if let Some(mut p) = crypto::xchacha_open(&sched.key, &sched.nonce(0, last), &head, &seg) {
            zikaron_os::wipe(&mut p);
            return Ok(Some(k));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(b: u8) -> Bytes {
        Bytes([b; 32])
    }

    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn turn() -> std::sync::MutexGuard<'static, ()> {
        TURN.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn sealed_to(k: &Bytes, plain: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let _ = seal(&k.public(), &mut &plain[..], &mut out).unwrap();
        out
    }

    fn at(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    struct Scratch(PathBuf);

    impl std::ops::Deref for Scratch {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(tag: &str) -> Scratch {
        let d = std::env::temp_dir().join(format!("zk-addressed-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Scratch(d)
    }

    fn plain(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 31 % 251) as u8).collect()
    }

    fn opened_rooms() -> usize {
        std::fs::read_dir(std::env::temp_dir()).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with("zikaron-opened-")).count()
    }

    #[test]
    fn the_recipient_opens_the_very_bytes_sealed_at_every_segment_boundary() {
        let _turn = turn();
        let d = scratch("round");
        let k = key(3);
        for n in [0, 1, SEGMENT - 1, SEGMENT, SEGMENT + 1, 2 * SEGMENT, 3 * SEGMENT + 17] {
            let p = plain(n);
            let f = at(&d, "p.zkit", &sealed_to(&k, &p));
            let o = open(&f, &[&key(9), &k]).unwrap();
            assert_eq!(o.key, 1);
            assert_eq!(std::fs::read(&o.file).unwrap(), p, "{n} bytes");
        }
    }

    #[test]
    fn a_key_that_is_not_the_recipient_opens_nothing_and_leaves_nothing() {
        let _turn = turn();
        let d = scratch("stranger");
        let f = at(&d, "p.zkit", &sealed_to(&key(3), &plain(SEGMENT * 2 + 5)));
        let before = opened_rooms();
        assert_eq!(open(&f, &[&key(4), &key(5)]).err(), Some(Refused::NotRecipient));
        assert_eq!(open(&f, &[]).err(), Some(Refused::NotRecipient));
        assert_eq!(recipient_among(&f, &[&key(4)]), Ok(None));
        assert_eq!(recipient_among(&f, &[&key(4), &key(3)]), Ok(Some(1)));
        assert_eq!(opened_rooms(), before);
    }

    #[test]
    fn one_changed_byte_anywhere_is_refused_and_leaves_no_plaintext() {
        let _turn = turn();
        let d = scratch("flip");
        let k = key(3);
        let good = sealed_to(&k, &plain(2 * SEGMENT + 9));
        let before = opened_rooms();
        let cut = form::MAGIC.len();
        for i in [cut + 3, HEAD - 1, HEAD, HEAD + SEGMENT + TAG + 2, good.len() - 1] {
            let mut bad = good.clone();
            bad[i] ^= 1;
            let f = at(&d, "p.zkit", &bad);
            let r = open(&f, &[&k]).err().unwrap();
            if i < HEAD {
                assert!(matches!(r, Refused::NotRecipient | Refused::Head(_)), "{i}: {r:?}");
            } else if i < HEAD + SEGMENT + TAG {
                assert_eq!(r, Refused::NotRecipient, "{i}");
            } else {
                assert!(matches!(r, Refused::Broken(_)), "{i}: {r:?}");
            }
        }
        let mut v = good.clone();
        v[cut] = 9;
        assert_eq!(open(&at(&d, "v.zkit", &v), &[&k]).err(), Some(Refused::Head("version 9".into())));
        assert_eq!(opened_rooms(), before);
    }

    #[test]
    fn a_package_cut_short_reordered_or_lengthened_is_refused() {
        let _turn = turn();
        let d = scratch("order");
        let k = key(3);
        let good = sealed_to(&k, &plain(3 * SEGMENT + 100));
        let s = SEGMENT + TAG;
        let segs: Vec<&[u8]> = vec![&good[HEAD..HEAD + s], &good[HEAD + s..HEAD + 2 * s], &good[HEAD + 2 * s..HEAD + 3 * s], &good[HEAD + 3 * s..]];
        let before = opened_rooms();
        let cut = [&good[..HEAD], segs[0], segs[1]].concat();
        assert_eq!(open(&at(&d, "c.zkit", &cut), &[&k]).err(), Some(Refused::Broken("1".into())));
        assert!(matches!(open(&at(&d, "c2.zkit", &good[..good.len() - 3]), &[&k]).err(), Some(Refused::Broken(_))));
        let swapped = [&good[..HEAD], segs[0], segs[2], segs[1], segs[3]].concat();
        assert_eq!(open(&at(&d, "s.zkit", &swapped), &[&k]).err(), Some(Refused::Broken("1".into())));
        let longer = [&good[..], &[0u8; 20][..]].concat();
        assert!(matches!(open(&at(&d, "l.zkit", &longer), &[&k]).err(), Some(Refused::Broken(_))));
        assert_eq!(open(&at(&d, "h.zkit", &good[..HEAD]), &[&k]).err(), Some(Refused::Broken("0".into())));
        assert_eq!(opened_rooms(), before);
    }

    #[test]
    fn the_package_holds_no_byte_of_the_recipients_address_or_key() {
        let k = key(3);
        let p = k.public();
        let full = crypto::uncompressed(&p).unwrap();
        let addr = zikaron::cryptox::address_of_privkey(&k.0).unwrap();
        let out = sealed_to(&k, &plain(SEGMENT + 3));
        let has = |needle: &[u8]| out.windows(needle.len()).any(|w| w == needle);
        assert!(!has(&p) && !has(&full[1..]) && !has(&addr));
        let again = sealed_to(&k, &plain(SEGMENT + 3));
        assert_ne!(out[form::MAGIC.len() + 1..HEAD], again[form::MAGIC.len() + 1..HEAD]);
    }

    #[test]
    fn a_recipient_key_off_the_curve_is_refused_and_both_forms_are_taken() {
        let _turn = turn();
        let k = key(3);
        let mut out = Vec::new();
        let mut no_point = [0u8; 33];
        no_point[0] = 2;
        assert_eq!(seal(&no_point, &mut &b"x"[..], &mut out), Err(Refused::Recipient));
        assert_eq!(seal(&[4u8; 64], &mut &b"x"[..], &mut out), Err(Refused::Recipient));
        let mut bad = crypto::uncompressed(&k.public()).unwrap();
        bad[64] ^= 1;
        assert_eq!(seal(&bad, &mut &b"x"[..], &mut out), Err(Refused::Recipient));
        assert!(out.is_empty());
        let full = crypto::uncompressed(&k.public()).unwrap();
        seal(&full, &mut &b"abc"[..], &mut out).unwrap();
        let d = scratch("forms");
        let o = open(&at(&d, "f.zkit", &out), &[&k]).unwrap();
        assert_eq!(std::fs::read(&o.file).unwrap(), b"abc");
    }

    #[test]
    fn a_plain_package_is_not_addressed() {
        let _turn = turn();
        let d = scratch("plain");
        let f = at(&d, "p.zkit", zikaron_glue::container::MAGIC.as_bytes());
        assert_eq!(open(&f, &[&key(3)]).err(), Some(Refused::NotAddressed));
    }

    fn sealing_rooms() -> usize {
        std::fs::read_dir(std::env::temp_dir()).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with("zikaron-sealing-")).count()
    }

    #[test]
    fn one_table_of_codes_and_words_serves_both_ends() {
        let mut codes: Vec<&str> = Why::ALL.iter().map(|w| w.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), Why::ALL.len(), "each code once");
        assert_eq!(Why::ALL.iter().filter(|w| w.unanswered()).collect::<Vec<_>>(), vec![&Why::ChainUnread], "the one member that is no answer");
        assert_eq!(Why::of(&Refused::NotRecipient), Some(Why::NotRecipient));
        assert_eq!(Why::of(&Refused::Broken("2".into())), Some(Why::Broken));
        assert_eq!(Why::of(&Refused::NotAddressed), Some(Why::Head));
        assert_eq!(Why::of(&Refused::Recipient), Some(Why::KeyShape));
        assert_eq!(Why::of(&Refused::Io("disk".into())), None);
        assert_eq!(Refused::NotRecipient.words(), Some(words::NOT_RECIPIENT));
        for w in Why::ALL {
            let (zh, en) = w.words();
            assert!(!zh.is_empty() && !en.is_empty(), "{w:?}");
        }
    }

    #[test]
    fn a_package_is_sealed_out_whole_or_not_at_all_and_no_plain_copy_stays() {
        let _turn = turn();
        let d = scratch("seal-out");
        let k = key(3);
        let out = d.join("s.zkit");
        let rooms = sealing_rooms();
        let plain = plain(3 * SEGMENT + 5);
        let (laid, one_time) = seal_out(&k.public(), &out, |p| std::fs::write(p, &plain).map(|_| 7)).unwrap();
        assert_eq!(laid, 7);
        assert_eq!(&std::fs::read(&out).unwrap()[MAGIC_LEN..MAGIC_LEN + 1 + POINT][1..], &one_time[..], "the one-time key in the head");
        assert_eq!(std::fs::read(&open(&out, &[&k]).unwrap().file).unwrap(), plain, "opens to the bytes laid out");
        assert_eq!(sealing_rooms(), rooms, "the plain copy is gone");
        let before = std::fs::read(&out).unwrap();
        let mut laid_out = false;
        let taken = seal_out(&k.public(), &out, |_| {
            laid_out = true;
            Ok::<(), ()>(())
        });
        assert!(matches!(taken, Err(Sealing::Land(zikaron_glue::landing::Trouble::Occupied(_)))));
        assert!(!laid_out, "nothing laid out");
        assert_eq!(std::fs::read(&out).unwrap(), before);
        let other = d.join("t.zkit");
        let refused = seal_out(&k.public(), &other, |p| {
            std::fs::write(p, b"half").unwrap();
            Err::<(), &str>("refused")
        });
        assert!(matches!(refused, Err(Sealing::Lay("refused"))));
        assert!(!other.exists(), "nothing at out");
        assert_eq!(sealing_rooms(), rooms, "the plain copy is gone");
    }

    #[test]
    fn a_typed_key_is_read_once_for_both_ends() {
        let k = key(3).public();
        let full = crypto::uncompressed(&k).unwrap();
        assert_eq!(key_text(&zikaron::hexfmt::encode(&k)), Some(k));
        assert_eq!(key_text(&zikaron::hexfmt::encode(&full).to_ascii_uppercase().replacen("0X", "0x", 1)), Some(k), "uncompressed, any case");
        assert_eq!(key_text(&zikaron::hexfmt::encode(&k)[2..]), None, "without 0x");
        assert_eq!(key_text(&format!("0x02{}", "00".repeat(32))), None, "off the curve");
        assert_eq!(key_text("0x1234"), None);
    }
}
