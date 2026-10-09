//! The anchor key: generation, address, and moving in and out of the local key vault (`keybox`). A plaintext
//! private key never lands in any file of ours.
//!
//! No cryptography is invented here: the curve order check, address derivation and digests come from the
//! core's `cryptox`, and hex spelling from the core's `hexfmt`. The only thing done here is drawing thirty-two
//! bytes from system entropy until they fall within the curve order.
//!
//! Plaintext goes only two places: memory (`Secret`, zeroed when dropped) and ciphertext in the key vault's
//! file. This crate has no path that writes a `Secret`'s bytes to a file, and the `keystore` module writes
//! only ciphertext.
//!
//! This is enforced by types, not by convention. A `pub(crate)` `Secret::bytes()` would let any module read
//! the thirty-two bytes, and "never write them to a file" would be a reminder that cannot guard modules added
//! later. So that accessor is private to this module, and `Secret` has only these exits, none of which hands
//! out plaintext to a file:
//!
//! 1. [`Secret::address`]: a derived address (law §5.5), which cannot lead back to the key;
//! 2. [`Secret::with_sign_key`]: lends the bytes to the `sign` module for exactly one signature;
//! 3. [`Secret::ciphered`]: the only path toward a file, already encrypted with `cryptx::aes128_ctr` (so
//!    the `keystore` module can only write ciphertext);
//! 4. [`Secret::with_tx_key`]: lends the bytes to the anchoring crate for exactly one anchoring transaction,
//!    with exactly one caller, and leads to no file;
//! 5. [`reveal_once`]: consumes the `Secret` (so once-only is structural); the string it returns lives in UI
//!    state and never enters a file.

use crate::fault::{classify, Fault, Known};
use crate::keybox;
use zikaron::cryptox;
use zikaron::hexfmt;

/// The source of system entropy, as the operating-system crate names it (for the words of a refusal).
pub const ENTROPY: &str = zikaron_os::ENTROPY_SOURCE;

/// Fill `buf` from the system's entropy source: the one way this app reads randomness. When it cannot be
/// read it is refused by name, never replaced by a weaker source.
pub fn fill_random(buf: &mut [u8]) -> Result<(), Fault> {
    zikaron_os::fill_random(buf).map_err(|e| classify(&e, ENTROPY))
}

/// `n` bytes from the system's entropy source (see [`fill_random`]).
pub fn random(n: usize) -> Result<Vec<u8>, Fault> {
    let mut b = vec![0u8; n];
    fill_random(&mut b)?;
    Ok(b)
}

// The base of the key's slot name in the key vault comes from [`crate::places`]: it can be set only once, and
// only by test hooks behind a cargo feature that is off in normal builds. The shipped build cannot change it.

/// How many times this process lent the key for signing (observation only, read by tests; no product
/// decision depends on it). The two lending exits (`with_sign_key`, `with_tx_key`) are the only sources of
/// signatures, and each loan is counted once.
static LENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn lent() {
    LENT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// How many times this process has lent the key for signing so far.
pub fn signed_count() -> u64 {
    LENT.load(std::sync::atomic::Ordering::SeqCst)
}

/// A plaintext private key. No `Debug`, no `Clone`, no `Display`: it cannot be printed or copied, and its
/// bytes are zeroed when it goes out of scope.
pub struct Secret([u8; 32]);

impl Drop for Secret {
    fn drop(&mut self) {
        for b in self.0.iter_mut() {
            unsafe { std::ptr::write_volatile(b, 0) };
        }
    }
}

impl Secret {
    /// Takes thirty-two bytes. Values outside the curve order are refused (law §5.7).
    pub fn take(bytes: [u8; 32]) -> Option<Secret> {
        if cryptox::in_range(&bytes) {
            Some(Secret(bytes))
        } else {
            None
        }
    }

    /// Private to this module, so plaintext bytes cannot be reached elsewhere in the crate.
    fn bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// This key's address (law §5.5, derived by the core).
    pub fn address(&self) -> Option<Address> {
        cryptox::address_of_privkey(&self.0).map(Address)
    }

    /// The signing exit: lends the bytes to the `sign` module for exactly one call.
    ///
    /// It lends rather than signs so that every call site of the actual signing primitive (the core's
    /// `cryptox::sign_digest`) stays in `sign.rs`: "who can sign" is "where the call sites are", and signing
    /// here would add a place outside the three signing entry points. There is exactly one caller (counted by
    /// the self-check suite), and its next statement is that primitive.
    ///
    /// Visible only to `sign`. With `pub(crate)` any file could borrow the key and, since the primitive is the
    /// core's public API, open another signing exit with free-form domain strings. With
    /// `pub(in crate::sign)` other files cannot even write the call, so no second place can pair a domain and
    /// preimage and sign them.
    pub(in crate::sign) fn with_sign_key<R>(&self, f: impl FnOnce(&[u8; 32]) -> R) -> R {
        lent();
        f(&self.0)
    }

    /// The signing exit for the anchoring transaction.
    ///
    /// The anchoring crate's `send::anchor` takes the thirty-two bytes (Ethereum signing lives in that crate),
    /// so this lends them for exactly that call: the caller cannot keep the reference, and the self-check
    /// suite counts exactly one caller, whose next statement is `send::anchor`. It leads to no file; the only
    /// path to disk is `ciphered`.
    ///
    /// Visible only to `sign`, and the closure runs the send itself. A `pub(crate)` version could be called
    /// as `with_tx_key(|k| *k)`, copying the bytes out (e.g. into a background thread) and letting any file
    /// call the core's signing primitive directly. With `pub(in crate::sign)`, the only caller
    /// (`sign::anchor_send`) runs `send::anchor` entirely inside the closure, so the loan is exactly that send.
    pub(in crate::sign) fn with_tx_key<R>(&self, f: impl FnOnce(&[u8; 32]) -> R) -> R {
        lent();
        f(&self.0)
    }

    /// The only exit toward disk, and it leaves as ciphertext.
    ///
    /// Runs the thirty-two bytes through `cryptx::aes128_ctr` with the given key and iv and returns the
    /// ciphertext. The one plaintext copy made here is zeroed when the function returns. Everything the
    /// keystore module writes comes from here.
    pub(crate) fn ciphered(&self, key: &[u8; 16], iv: &[u8; 16]) -> Vec<u8> {
        let mut buf = Wipe(self.0);
        crate::cryptx::aes128_ctr(key, iv, &mut buf.0);
        buf.0.to_vec()
    }
}

/// A thirty-two-byte scratch buffer that zeroes itself on drop, so the copy in `ciphered` never lingers on
/// the stack as an ordinary array.
struct Wipe([u8; 32]);

impl Drop for Wipe {
    fn drop(&mut self) {
        for b in self.0.iter_mut() {
            unsafe { std::ptr::write_volatile(b, 0) };
        }
    }
}

/// A twenty-byte address.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Address(pub [u8; 20]);

impl Address {
    /// `0x` plus forty lowercase hex digits (spelling from the core's `hexfmt`).
    pub fn hex(&self) -> String {
        hexfmt::encode(&self.0)
    }

    /// Parses an address pasted back by a user. Case is ignored: wallets spell it differently, and a case
    /// difference should not be reported as a mismatch.
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

/// Generates a key: draws thirty-two bytes from system entropy, retrying while outside the curve order. If
/// the retry limit is reached it returns a named error, never a key outside the order.
pub fn generate() -> Result<Secret, Fault> {
    // Mark the trace here too, so direct calls that bypass `apply` (tests, the CLI) are traced.
    crate::trace::mark(crate::feature::Feature::H2);
    for _ in 0..64 {
        let mut b = [0u8; 32];
        fill_random(&mut b)?;
        if let Some(s) = Secret::take(b) {
            return Ok(s);
        }
    }
    Err(Fault::known(Known::Entropy, crate::lang::filln(crate::lang::Key::Tail162, &[&(ENTROPY).to_string()])))
}

/// Whether this identity's key for this seat is present now. "Absent" is not an error: first run asks exactly
/// this.
///
/// `acct` is the current slot (`identity::account_now`, read by the caller from the register): without a
/// register, the account-base slot; with one, the current identity's current seat's slot. An empty current
/// seat (`None`) answers "absent".
pub fn present(acct: Option<&str>) -> Result<bool, Fault> {
    // Use the vault's `present`, not `get`: it reads only slot names on disk and does not need the master
    // key, so it answers while the vault is locked or does not exist yet. With `get`, asking "is there a key"
    // at startup on a machine without a passcode would raise an unactionable "the key vault is locked" toast.
    let Some(acct) = acct else { return Ok(false) };
    keybox::present(acct)
}

/// Puts a key into the current slot (`acct`, as [`present`] reads it). An empty current seat is refused by
/// name (there is no slot to put it in).
pub fn install(acct: Option<&str>, s: &Secret) -> Result<(), Fault> {
    let acct = acct
        .ok_or_else(|| Fault::known(Known::SeatUnseated, crate::lang::t(crate::lang::Key::IdSeatEmpty).to_string()))?;
    keybox::put(acct, s.bytes())
}

/// Puts a key into a named slot (used by the identity layer when creating identities; slot names are built
/// only by `places`).
pub(crate) fn install_at(account: &str, s: &Secret) -> Result<(), Fault> {
    keybox::put(account, s.bytes())
}

/// Both seat keys of a recovery-word identity as vault slots (slot name, key bytes), for a vault being built
/// anew (a restore): the bytes go straight to `keybox::build_new` to be sealed, as `install_at` hands them
/// to `keybox::put`.
pub(crate) fn seat_slots(entropy: &[u8; crate::family::ENTROPY_BYTES]) -> Option<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    for role in crate::roles::Role::ALL {
        let s = derived(entropy, role)?;
        let a = s.address()?;
        out.push((crate::places::key_slot(&a), s.bytes().to_vec()));
    }
    Some(out)
}

/// Derives this seat's key from entropy along the family path (`cryptx` produces the bytes; this wraps them
/// as a `Secret`).
pub(crate) fn derived(entropy: &[u8; crate::family::ENTROPY_BYTES], role: crate::roles::Role) -> Option<Secret> {
    let mut raw = crate::cryptx::derive(entropy, &crate::family::path(role))?;
    let s = Secret::take(raw);
    for b in raw.iter_mut() {
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    s
}

/// Parses a private key from pasted hex (optional `0x`, case ignored). `None` when unrecognized.
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
    for x in b.iter_mut() {
        unsafe { std::ptr::write_volatile(x, 0) };
    }
    let s = Secret::take(raw);
    for x in raw.iter_mut() {
        unsafe { std::ptr::write_volatile(x, 0) };
    }
    s
}

/// Loads the current slot's key (`acct`, as [`present`] reads it) from the key vault. A locked vault is
/// refused as `LOCKED`; bytes of the wrong shape are refused by name, never forced into a key.
pub fn load(acct: Option<&str>) -> Result<Option<Secret>, Fault> {
    let Some(acct) = acct else { return Ok(None) };
    load_at(acct)
}

/// Loads the key from a named slot.
pub(crate) fn load_at(acct: &str) -> Result<Option<Secret>, Fault> {
    let Some(mut raw) = keybox::get(acct)? else {
        return Ok(None);
    };
    if raw.len() != 32 {
        for x in raw.iter_mut() {
            unsafe { std::ptr::write_volatile(x, 0) };
        }
        return Err(Fault::known(
            Known::KeyMalformed,
            crate::lang::filln(crate::lang::Key::Tail163, &[&(acct).to_string(), &(raw.len()).to_string()]),
        ));
    }
    let mut b = [0u8; 32];
    b.copy_from_slice(&raw);
    let got = Secret::take(b);
    // Wipe both plaintext copies: `raw` from the vault and the stack copy `b`. This runs on every signature;
    // without it, locking would clear the master key but leave this key in a heap buffer and on the stack.
    for x in raw.iter_mut() {
        unsafe { std::ptr::write_volatile(x, 0) };
    }
    for x in b.iter_mut() {
        unsafe { std::ptr::write_volatile(x, 0) };
    }
    got.ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::filln(crate::lang::Key::Tail164, &[&(acct).to_string()])))
        .map(Some)
}

/// Reads bytes as a key and computes its address (used by migration to check each slot). `None` for the
/// wrong shape.
pub(crate) fn address_of(raw: &[u8]) -> Option<Address> {
    if raw.len() != 32 {
        return None;
    }
    let mut b = [0u8; 32];
    b.copy_from_slice(raw);
    let s = Secret::take(b)?;
    for x in b.iter_mut() {
        unsafe { std::ptr::write_volatile(x, 0) };
    }
    s.address()
}

/// Address check for tests (the same algorithm as `address_of`, public for the test hooks).
pub fn address_of_probe(raw: &[u8]) -> Option<Address> {
    address_of(raw)
}

/// Recovers the vault with a private key (the existing-key identity path: the key decrypted from a keystore
/// file). The bytes do not leave this module.
pub(crate) fn recover_with(s: &Secret, new_pin: &str) -> Result<(), Fault> {
    let a = s
        .address()
        .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string()))?;
    keybox::recover(s.bytes(), new_pin, &a.hex(), &[crate::places::key_slot(&a)])
}

/// Records a recovery seal for this identity (an existing-key identity recovers with its private key). The
/// bytes go only to `keybox` to be sealed, and the file it writes holds ciphertext.
pub(crate) fn seal_recovery(id: &str, s: &Secret) -> Result<bool, Fault> {
    keybox::add_recovery(id, keybox::PrimaryKind::KeyFile, s.bytes())
}

/// Shows a raw private key once. Takes the `Secret` out of the slot (leaving `None`; the key's bytes are
/// zeroed when it drops) and returns a hex string for the user to copy.
///
/// Once-only is structural: a second call finds the slot empty. The string lives in UI state and is cleared
/// when the user clicks "copied"; not one byte enters any file.
pub fn reveal_once(slot: &mut Option<Secret>) -> Option<String> {
    let s = slot.take()?;
    Some(hexfmt::encode(s.bytes()))
}

/// The result of one sweep: the outcome for each slot and the enumeration recheck.
#[derive(Clone, Debug, Default)]
pub struct Swept {
    /// Slots removed.
    pub dropped: Vec<String>,
    /// Slots created this pass that were already gone at sweep time (someone removed them first).
    pub missing: Vec<String>,
    /// Slots that could not be removed, with the key vault's sentence.
    pub failed: Vec<(String, String)>,
    /// Slots of this family still present in the enumeration recheck after removal.
    pub left: Vec<String>,
    /// The error sentence if the enumeration recheck itself failed.
    pub recheck: Option<String>,
}

impl Swept {
    /// All removed, and the recheck found no leftovers and no error.
    pub fn clean(&self) -> bool {
        self.missing.is_empty() && self.failed.is_empty() && self.left.is_empty() && self.recheck.is_none()
    }
}

/// Removes this family of slots: the account-base slot and every one starting with "account base-" (key
/// slots, seed slots).
///
/// What to remove is enumerated from the vault now, not from names recorded when this process created them,
/// so identities created in the window and slots left by an earlier unfinished sweep are all found. After
/// removal it enumerates again and records what remains; any slot that cannot be removed and any enumeration
/// error is recorded for the caller to report, never dropped.
///
/// The shipped account base [`crate::places::ACCOUNT`] is never swept (returns an empty result): this is
/// only for test accounts. The shipped build has no caller (deleting an identity goes through
/// `identity::delete`, which removes only that identity's slots).
pub fn forget() -> Swept {
    let base = crate::places::key_account().to_string();
    let mut s = Swept::default();
    if base == crate::places::ACCOUNT {
        return s;
    }
    let prefix = crate::places::family_prefix();
    let mine = |a: &str| a == base || a.starts_with(&prefix);
    // While locked, a shape 3 vault keeps its account names sealed. A test account's vault file is its own
    // (named by the account), so every slot in it belongs to this family: drop them all, then count again.
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
