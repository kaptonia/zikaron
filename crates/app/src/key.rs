use crate::fault::{classify, Fault, Known};
use crate::keybox;
use zikaron::cryptox;
use zikaron::hexfmt;

pub const ENTROPY: &str = zikaron_os::ENTROPY_SOURCE;

pub fn fill_random(buf: &mut [u8]) -> Result<(), Fault> {
    zikaron_os::fill_random(buf).map_err(|e| classify(&e, ENTROPY))
}

pub fn random(n: usize) -> Result<Vec<u8>, Fault> {
    let mut b = vec![0u8; n];
    fill_random(&mut b)?;
    Ok(b)
}

static LENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn lent() {
    LENT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

pub fn signed_count() -> u64 {
    LENT.load(std::sync::atomic::Ordering::SeqCst)
}

pub struct Secret([u8; 32]);

impl Drop for Secret {
    fn drop(&mut self) {
        zikaron_os::wipe(&mut self.0);
    }
}

impl Secret {
    pub fn take(bytes: [u8; 32]) -> Option<Secret> {
        if cryptox::in_range(&bytes) {
            Some(Secret(bytes))
        } else {
            None
        }
    }

    fn bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn address(&self) -> Option<Address> {
        cryptox::address_of_privkey(&self.0).map(Address)
    }

    pub fn public(&self) -> Option<[u8; 33]> {
        zikaron_seal::crypto::public_of(&self.0)
    }

    pub(in crate::sign) fn with_sign_key<R>(&self, f: impl FnOnce(&[u8; 32]) -> R) -> R {
        lent();
        f(&self.0)
    }

    pub(in crate::sign) fn with_tx_key<R>(&self, f: impl FnOnce(&[u8; 32]) -> R) -> R {
        lent();
        f(&self.0)
    }

    pub(in crate::sign) fn with_plain_key<R>(&self, f: impl FnOnce(&[u8; 32]) -> R) -> R {
        lent();
        f(&self.0)
    }

    pub(crate) fn ciphered(&self, key: &[u8; 16], iv: &[u8; 16]) -> Vec<u8> {
        let mut buf = Wipe(self.0);
        crate::cryptx::aes128_ctr(key, iv, &mut buf.0);
        buf.0.to_vec()
    }
}

impl zikaron_seal::addressed::Recipient for Secret {
    fn public(&self) -> [u8; 33] {
        Secret::public(self).unwrap_or([0; 33])
    }

    fn agree(&self, one_time: &[u8; 33]) -> Option<[u8; 32]> {
        zikaron_seal::crypto::agree(&self.0, one_time)
    }
}

struct Wipe([u8; 32]);

impl Drop for Wipe {
    fn drop(&mut self) {
        zikaron_os::wipe(&mut self.0);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Address(pub [u8; 20]);

impl Address {
    pub fn hex(&self) -> String {
        hexfmt::encode(&self.0)
    }

    pub fn parse(s: &str) -> Option<Address> {
        let t = s.trim();
        let low = t.to_ascii_lowercase();
        if !hexfmt::is_hex20(&low) {
            return None;
        }
        let b = hexfmt::decode(&low)?;
        let mut a = [0u8; 20];
        a.copy_from_slice(&b);
        Some(Address(a))
    }
}

pub fn generate() -> Result<Secret, Fault> {
    crate::trace::mark(crate::feature::Feature::H2);
    for _ in 0..64 {
        let mut b = [0u8; 32];
        fill_random(&mut b)?;
        if let Some(s) = Secret::take(b) {
            return Ok(s);
        }
    }
    Err(Fault::known(Known::Entropy, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail162, &[&(ENTROPY).to_string()]))))
}

pub fn present(acct: Option<&str>) -> Result<bool, Fault> {
    let Some(acct) = acct else { return Ok(false) };
    keybox::present(acct)
}

pub fn install(acct: Option<&str>, s: &Secret) -> Result<(), Fault> {
    let acct = acct
        .ok_or_else(|| Fault::known(Known::SeatUnseated, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::IdSeatEmpty).to_string())))?;
    keybox::put(acct, s.bytes())
}

pub(crate) fn install_at(account: &str, s: &Secret) -> Result<(), Fault> {
    keybox::put(account, s.bytes())
}

pub(crate) fn seat_slots(entropy: &[u8; crate::family::ENTROPY_BYTES]) -> Option<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    for role in crate::roles::Role::ALL {
        let s = derived(entropy, role)?;
        let a = s.address()?;
        out.push((crate::places::key_slot(&a), s.bytes().to_vec()));
    }
    Some(out)
}

pub(crate) fn derived(entropy: &[u8; crate::family::ENTROPY_BYTES], role: crate::roles::Role) -> Option<Secret> {
    derived_at(entropy, role, crate::family::GROUP)
}

pub(crate) fn derived_at(entropy: &[u8; crate::family::ENTROPY_BYTES], role: crate::roles::Role, group: u32) -> Option<Secret> {
    let mut raw = crate::cryptx::derive(entropy, &crate::family::path_at(role, group))?;
    let s = Secret::take(raw);
    zikaron_os::wipe(&mut raw);
    s
}

pub(crate) fn from_hex(text: &str) -> Option<Secret> {
    let t = text.trim().to_ascii_lowercase();
    let t = if t.starts_with("0x") { t } else { format!("0x{t}") };
    let mut b = hexfmt::decode(&t)?;
    if b.len() != 32 {
        for x in b.iter_mut() {
            *x = 0;
        }
        return None;
    }
    let mut raw = [0u8; 32];
    raw.copy_from_slice(&b);
    zikaron_os::wipe(&mut b);
    let s = Secret::take(raw);
    zikaron_os::wipe(&mut raw);
    s
}

pub fn load(acct: Option<&str>) -> Result<Option<Secret>, Fault> {
    let Some(acct) = acct else { return Ok(None) };
    load_at(acct)
}

pub(crate) fn load_at(acct: &str) -> Result<Option<Secret>, Fault> {
    let Some(mut raw) = keybox::get(acct)? else {
        return Ok(None);
    };
    if raw.len() != 32 {
        zikaron_os::wipe(&mut raw);
        return Err(Fault::known(
            Known::KeyMalformed,
            crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail163, &[&(acct).to_string(), &(raw.len()).to_string()])),
        ));
    }
    let mut b = [0u8; 32];
    b.copy_from_slice(&raw);
    let got = Secret::take(b);
    zikaron_os::wipe(&mut raw);
    zikaron_os::wipe(&mut b);
    let got = got.ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail164, &[&(acct).to_string()]))))?;
    if let Some(want) = crate::places::slot_address(acct) {
        if got.address() != Some(want) {
            return Err(Fault::known(Known::KeyMalformed, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::TailKeyNotSlots, &[&(acct).to_string()]))));
        }
    }
    Ok(Some(got))
}

pub(crate) fn address_of(raw: &[u8]) -> Option<Address> {
    if raw.len() != 32 {
        return None;
    }
    let mut b = [0u8; 32];
    b.copy_from_slice(raw);
    let s = Secret::take(b)?;
    zikaron_os::wipe(&mut b);
    s.address()
}

pub fn address_of_probe(raw: &[u8]) -> Option<Address> {
    address_of(raw)
}

pub(crate) fn recover_with(s: &Secret, new_pin: &str) -> Result<(), Fault> {
    s.address().ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail013).to_string())))?;
    keybox::recover(s.bytes(), new_pin)
}

pub(crate) fn seal_recovery(id: &str, s: &Secret) -> Result<bool, Fault> {
    keybox::add_recovery(id, keybox::PrimaryKind::KeyFile, s.bytes())
}

pub fn reveal_once(slot: &mut Option<Secret>) -> Option<String> {
    let s = slot.take()?;
    Some(hexfmt::encode(s.bytes()))
}

#[derive(Clone, Debug, Default)]
pub struct Swept {
    pub dropped: Vec<String>,
    pub missing: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub left: Vec<String>,
    pub recheck: Option<String>,
}

impl Swept {
    pub fn clean(&self) -> bool {
        self.missing.is_empty() && self.failed.is_empty() && self.left.is_empty() && self.recheck.is_none()
    }
}

pub fn forget() -> Swept {
    let base = crate::places::key_account().to_string();
    let mut s = Swept::default();
    if base == crate::places::ACCOUNT {
        return s;
    }
    let prefix = crate::places::family_prefix();
    let mine = |a: &str| a == base || a.starts_with(&prefix);
    if let Err(f) = keybox::accounts() {
        if f.which() != Some(crate::fault::Known::Locked) {
            s.recheck = Some(f.said().to_string());
            return s;
        }
        match keybox::drop_all_slots() {
            Ok(n) => s.dropped = (0..n).map(|i| format!("{base} slot #{i}")).collect(),
            Err(f) => s.failed.push((base.clone(), f.said().to_string())),
        }
        match keybox::slot_count() {
            Ok(0) => {}
            Ok(n) => s.left = vec![format!("{base} {n} slots")],
            Err(f) => s.recheck = Some(f.said().to_string()),
        }
        return s;
    }
    match keybox::accounts() {
        Ok(all) => {
            for acct in all.into_iter().filter(|a| mine(a)) {
                match keybox::drop_item(&acct) {
                    Ok(true) => s.dropped.push(acct),
                    Ok(false) => s.missing.push(acct),
                    Err(f) => s.failed.push((acct, f.said().to_string())),
                }
            }
        }
        Err(f) => {
            s.recheck = Some(f.said().to_string());
            return s;
        }
    }
    match keybox::accounts() {
        Ok(all) => s.left = all.into_iter().filter(|a| mine(a)).collect(),
        Err(f) => s.recheck = Some(f.said().to_string()),
    }
    s
}
