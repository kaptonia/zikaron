//! The anchor key: generation, address, and moving in and out of the local key vault (`keybox`). A plaintext
//! private key never lands in any file of ours.
//!
//! ─── This layer invents no cryptography ───
//!
//! The curve order range, address derivation and digests all come from the core's `cryptox` (third-party
//! cryptography lives only there). Hex spelling comes from the core's `hexfmt` (one name, one home). This
//! file does only one thing itself: take thirty-two bytes from system entropy until they fall within the
//! order.
//!
//! ─── Plaintext goes only two places ───
//!
//! Memory (`Secret`, zeroed when it goes out of scope) and ciphertext in the key vault's file. There is no
//! third: this crate has no path that writes a `Secret`'s bytes to a file, and what the `keystore` branch
//! writes is ciphertext.
//!
//! ─── "Plaintext never lands on disk" is carried by types, not reminders ───
//!
//! A `pub(crate)` `Secret::bytes()` would let any module in the crate get the thirty-two bytes, and "nobody
//! write it to a file" would be a reminder, which cannot guard a module added later.
//!
//! So that accessor is private to this module, and `Secret` has only these exits, each handing out something
//! no longer plaintext:
//!
//! 1. [`Secret::address`]: a derived address (law §5.5), which cannot lead back to the key;
//! 2. [`Secret::with_sign_key`]: lends the bytes to the `sign` layer for one signature, the loan covering
//! exactly that call;
//! 3. [`Secret::ciphered`]: the only path toward a file, already through `cryptx::aes128_ctr` on the way out
//! (so the `keystore` branch can only write ciphertext);
//! 4. [`Secret::with_tx_key`]: lends the bytes to the anchoring crate for one anchoring transaction, the loan
//! covering exactly that call, with exactly one caller, and it leads to no file either.
//!
//! The fifth is [`reveal_once`], which takes the `Secret` away (so once-only is structural); the string it
//! hands out lives in interface state, and not one byte enters any file.
//!
//! So "write the private key bytes into a file" has no callable path in this crate: there are five exits in
//! all; the first hands out no plaintext, the third hands out ciphertext, the second and fourth lend once and
//! lead to no file, and the fifth goes to the screen once.

use crate::fault::{classify, Fault, Known};
use crate::keybox;
use zikaron::cryptox;
use zikaron::hexfmt;

/// The source of system entropy. One name, one home.
pub const ENTROPY: &str = "/dev/urandom";

/// The base of the key's slot name in the key vault comes from [`crate::places`]: it may be set only once,
/// and the statement that sets it lives only in the test hooks (the `drive` feature, off in normal builds).
/// The shipped build has no path to change it.

/// How many times this process lent the key for signing (observation only, read only by tests; nothing in
/// the product decides on it).
///
/// The two lending exits (`with_sign_key`, `with_tx_key`) are the only sources of signatures; each loan is
/// counted once, in one counter.
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
    /// Take thirty-two bytes. Outside the curve order is refused (law §5.7).
    pub fn take(bytes: [u8; 32]) -> Option<Secret> {
        if cryptox::in_range(&bytes) {
            Some(Secret(bytes))
        } else {
            None
        }
    }

    /// Private to this module. The exits are the three below; this accessor cannot leave this file, so
    /// "plaintext bytes reachable elsewhere in the crate" cannot be written.
    fn bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// This key's address (the same derivation as law §5.5, from the core).
    pub fn address(&self) -> Option<Address> {
        cryptox::address_of_privkey(&self.0).map(Address)
    }

    /// The signing exit. Lends the bytes to the `sign` layer once, the loan covering exactly that call.
    ///
    /// Why lend rather than sign here: every call site of the primitive that actually signs (the core's
    /// `cryptox::sign_digest`) is in `sign.rs`. "Who can sign" in code means "where its call sites are", and
    /// moving the primitive into this file would open another place outside the three signing entry points. So this
    /// layer only lends and does not sign.
    ///
    /// There is exactly one caller (counted by the self-check suite), and its next statement is that
    /// primitive.
    ///
    /// Visible only to the `sign` file. With `pub(crate)` any file in the crate could borrow the key, and
    /// since the signing primitive is the core's public surface, another signing exit taking free domain
    /// strings could be opened anywhere, so "a fourth domain has no form at compile time" would hold only
    /// inside `sign.rs`. With `pub(in crate::sign)` other files cannot even write the call: without the key,
    /// no second place can combine a domain and preimage and hand them to the primitive.
    pub(in crate::sign) fn with_sign_key<R>(&self, f: impl FnOnce(&[u8; 32]) -> R) -> R {
        lent();
        f(&self.0)
    }

    /// The signing exit for the anchoring transaction.
    ///
    /// The anchoring crate's `send::anchor` takes thirty-two bytes (Ethereum's signing rules are in that
    /// crate, not here), so this lends them once, for exactly that call: the caller cannot keep the
    /// reference, and no second place calls it (the self-check suite counts the callers: exactly one, whose
    /// next statement is `send::anchor`).
    ///
    /// This leads to no file. Writing to disk has only the `ciphered` path.
    ///
    /// Visible only to the `sign` file, and the closure runs the send itself. A `pub(crate)` version called
    /// as `with_tx_key(|k| *k)` would copy all thirty-two bytes out and move them into a background thread,
    /// contradicting "the loan covers exactly that call"; and any file in the crate could borrow once that
    /// way and call the core's public primitive directly, growing a general signing exit back. With `pub(in
    /// crate::sign)` other files cannot even write the call; the only caller (`sign::anchor_send`) runs the
    /// anchoring crate's `send::anchor` entirely in the closure, so the loan is exactly that send.
    pub(in crate::sign) fn with_tx_key<R>(&self, f: impl FnOnce(&[u8; 32]) -> R) -> R {
        lent();
        f(&self.0)
    }

    /// The only exit of the path to disk, and it leaves as ciphertext.
    ///
    /// Takes an AES key and an iv, runs the thirty-two bytes through `cryptx::aes128_ctr`, and returns the
    /// ciphertext. The plaintext is one copy inside this function, zeroed when the function ends. Everything
    /// the keystore branch writes can only come from here.
    pub(crate) fn ciphered(&self, key: &[u8; 16], iv: &[u8; 16]) -> Vec<u8> {
        let mut buf = Wipe(self.0);
        crate::cryptx::aes128_ctr(key, iv, &mut buf.0);
        buf.0.to_vec()
    }
}

/// A thirty-two-byte scratch buffer that zeroes itself. The copy in `ciphered` lives in it, so the plaintext
/// never stays on the stack as an ordinary array.
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

    /// Recognize an address from a string pasted back by a person. Case is ignored: wallets spell
    /// differently, and "the echo does not match" should not be said over a case difference.
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

/// Generate one. Take thirty-two bytes from system entropy, retrying while outside the order; if the retry
/// limit is reached, a named error, never silently a key outside the order.
pub fn generate() -> Result<Secret, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are marked too.
    crate::trace::mark(crate::feature::Feature::H2);
    use std::io::Read;
    let mut h = std::fs::File::open(ENTROPY).map_err(|e| classify(&e, ENTROPY))?;
    for _ in 0..64 {
        let mut b = [0u8; 32];
        h.read_exact(&mut b).map_err(|e| classify(&e, ENTROPY))?;
        if let Some(s) = Secret::take(b) {
            return Ok(s);
        }
    }
    Err(Fault::known(Known::Entropy, crate::lang::filln(crate::lang::Key::Tail162, &[&(ENTROPY).to_string()])))
}

/// Whether this identity's key for this seat is present now. "Absent" is not an error: first run asks exactly
/// this.
///
/// Which slot is answered by [`crate::identity::account_now`]: without a register, the slot at the account
/// base; with a register, the current identity's current seat's slot. An empty current seat answers "absent"
/// (that seat has no key).
pub fn present() -> Result<bool, Fault> {
    // This asks "present?", not "take it out": the vault's `present` reads only slot names in the book on
    // disk and does not touch the master key, so it can answer while the vault is locked or does not exist
    // yet. Using `get` would need the master key: the product asking "is there a key" at startup on a machine
    // without a passcode would get "the key vault is locked", landing in the trouble bar as a toast nobody
    // can act on. The presence question must not go through the lock.
    let Some(acct) = crate::identity::account_now()? else { return Ok(false) };
    keybox::present(&acct)
}

/// Put a key into the current slot. An empty current seat is refused by name (no slot, nowhere to put it).
pub fn install(s: &Secret) -> Result<(), Fault> {
    let acct = crate::identity::account_now()?
        .ok_or_else(|| Fault::known(Known::SeatUnseated, crate::lang::t(crate::lang::Key::IdSeatEmpty).to_string()))?;
    keybox::put(&acct, s.bytes())
}

/// Put a key into a named slot (used by the identity layer when creating identities; slot names are assembled
/// only by `places`).
pub(crate) fn install_at(account: &str, s: &Secret) -> Result<(), Fault> {
    keybox::put(account, s.bytes())
}

/// Both seat keys of a recovery-word identity as vault slots (slot name, key bytes), for a vault being built
/// anew (a restore): the bytes go straight to `keybox::build_new` to be sealed, as `install_at` hands them to
/// `keybox::put`.
pub(crate) fn seat_slots(entropy: &[u8; crate::family::ENTROPY_BYTES]) -> Option<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    for role in crate::roles::Role::ALL {
        let s = derived(entropy, role)?;
        let a = s.address()?;
        out.push((crate::places::key_slot(&a), s.bytes().to_vec()));
    }
    Some(out)
}

/// Derive this seat's key from entropy along the family path (`cryptx` produces the bytes; this file wraps
/// them as a `Secret`).
pub(crate) fn derived(entropy: &[u8; crate::family::ENTROPY_BYTES], role: crate::roles::Role) -> Option<Secret> {
    let mut raw = crate::cryptx::derive(entropy, &crate::family::path(role))?;
    let s = Secret::take(raw);
    for b in raw.iter_mut() {
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    s
}

/// Recognize a private key from pasted hex (optional `0x`, case ignored). `None` when unrecognized.
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

/// Load the current slot's key from the key vault. A locked vault is refused by name as `LOCKED`; bytes of
/// the wrong shape are refused by name, never forced.
pub fn load() -> Result<Option<Secret>, Fault> {
    let Some(acct) = crate::identity::account_now()? else { return Ok(None) };
    load_at(&acct)
}

/// Load the key from a named slot.
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
    // Wipe both copies. This is the hot path taken on every signature: the part taken from the vault (`raw`)
    // and this stack copy (`b`) are both the plaintext key. Without wiping, "locking means no key remains in
    // memory" would hold only for the master key, while the key itself stayed in the returned heap buffer and
    // on this frame's stack (`address_of`, `from_hex` and `words_of` already wipe this way).
    for x in raw.iter_mut() {
        unsafe { std::ptr::write_volatile(x, 0) };
    }
    for x in b.iter_mut() {
        unsafe { std::ptr::write_volatile(x, 0) };
    }
    got.ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::filln(crate::lang::Key::Tail164, &[&(acct).to_string()])))
        .map(Some)
}

/// Read bytes as a key and compute its address (used by migration to check each slot's address). `None` for
/// the wrong shape.
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

/// Recover the vault with a private key (the existing-key identity path: the key decrypted from a keystore
/// file). The bytes do not leave this file.
pub(crate) fn recover_with(s: &Secret, new_pin: &str) -> Result<(), Fault> {
    let a = s
        .address()
        .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string()))?;
    keybox::recover(s.bytes(), new_pin, &a.hex(), &[crate::places::key_slot(&a)])
}

/// Record a recovery seal for this identity (an existing-key identity recovers with its private key). The
/// bytes do not leave this file: handed to `keybox` to seal, and the file it writes holds ciphertext.
pub(crate) fn seal_recovery(id: &str, s: &Secret) -> Result<bool, Fault> {
    keybox::add_recovery(id, keybox::PrimaryKind::KeyFile, s.bytes())
}

/// Show a raw private key once. This function takes the `Secret` away (the caller's slot becomes empty and
/// the bytes are zeroed at once) and returns a hex string for the person to copy.
///
/// Once-only is thus structural: on a second call the slot is already `None`, with nothing to take. The
/// string on screen lives in interface state and is cleared when the person clicks "copied"; not one byte
/// enters any file.
pub fn reveal_once(slot: &mut Option<Secret>) -> Option<String> {
    let s = slot.take()?;
    Some(hexfmt::encode(s.bytes()))
}

/// One sweep's reading: the result for each slot and the enumeration recheck.
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

/// Remove this family of slots: the one at the account base and every one starting with "account base-" (key
/// slots, seed slots).
///
/// Which to remove is enumerated from the slots in the key vault now, not from names recorded when this
/// process created them: identities created in the window and slots left by an earlier unfinished sweep are
/// all in this family and found by one enumeration. After removal it enumerates again to recheck, recording
/// what remains; any slot that cannot be removed and any enumeration error is recorded for the caller to
/// report, never dropped.
///
/// The shipped account base [`crate::places::ACCOUNT`] family is never swept (returns an empty reading): this
/// broom is only for the test account. The shipped build has no caller (deleting an identity goes
/// through `identity::delete`, removing only that identity's slots).
pub fn forget() -> Swept {
    let base = crate::places::key_account().to_string();
    let mut s = Swept::default();
    if base == crate::places::ACCOUNT {
        return s;
    }
    let prefix = crate::places::family_prefix();
    let mine = |a: &str| a == base || a.starts_with(&prefix);
    // Locked, a shape 3 vault keeps its account names sealed. A test account's vault file is its own (named by
    // the account), so every slot in it is this family's: dropped whole, then counted again.
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
