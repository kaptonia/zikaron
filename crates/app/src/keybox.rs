//! The key vault, as this app wires it: the vault itself (its file, seals, master key, slots, the new-master-key
//! change) is the sealed layer's (`zikaron_seal::keybox`), one implementation with the command line. What stays
//! here is where it lives (this machine directory and this run's account base, `crate::places`), the trace mark
//! of a passcode action (`H2`), the task ticket a background write is checked against (`task::ticket_void`), the
//! seat keys a recovery-word primary recovers with (derived from its entropy, `key::seat_slots`), and this app's
//! fault table.

use crate::fault::{Fault, Known};
use zikaron_seal::keybox as vault;

pub use vault::{
    begin_lock, below_floor, floor, lock, member, params, pin_digits_only, pin_trouble, set_light_kdf, Held, LocalKey, NewBook, PinFor, PinTrouble, Plain, PrimaryKind, State, BIND_DOMAIN, IV_LEN, LOCAL_INFO, MEMBER_INFO, NAMES_INFO, NEXT, PIN_LEN, PROBE_KDF, SHAPE, SHAPE_V1, SHAPE_V2,
    WRONG_LIMIT,
};

/// This machine's vault: the machine directory, and the vault and lock file names of this run's account base.
fn here() -> Result<vault::Vault, Fault> {
    Ok(vault::Vault::new(crate::home::machine_dir()?, crate::places::keybox_file(), crate::places::keybox_lock_file()))
}

/// Before a change to the vault: a background worker whose home is no longer the one it started for writes
/// nothing and ends with that refusal (`task::ticket_void`).
fn may_write() -> Result<vault::Vault, Fault> {
    let v = here()?;
    if crate::task::ticket_void() {
        return Err(crate::task::void_ticket_fault(v.dir()));
    }
    Ok(v)
}

/// The passcode actions' trace mark, set here too so direct calls that bypass `apply` (tests, the CLI) are traced.
fn mark() {
    crate::trace::mark(crate::feature::Feature::H2);
}

/// Every secret a primary of `kind` recovers with, in the vault's label order: `secret` itself (entropy or an
/// existing key), then for a recovery-word identity the two seat keys derived from it. Wiped when dropped.
fn recovery_secrets(kind: PrimaryKind, secret: &[u8]) -> Result<Vec<Plain>, Fault> {
    let mut out = vec![Plain::new(secret.to_vec())];
    if kind == PrimaryKind::Words {
        let entropy: &[u8; crate::family::ENTROPY_BYTES] = secret.try_into().map_err(|_| Fault::known(Known::KeyboxShape, member::RECOVERY.to_string()))?;
        // The seat keys in `Role::ALL` order, as `key::seat_slots` hands them to be sealed.
        let seats = crate::key::seat_slots(entropy).ok_or_else(|| Fault::known(Known::KeyMalformed, member::RECOVERY.to_string()))?;
        out.extend(seats.into_iter().map(|(_, k)| Plain::new(k)));
    }
    Ok(out)
}

/// Where the vault file lives: the machine directory, named per account base.
pub fn path() -> Result<std::path::PathBuf, Fault> {
    Ok(here()?.path())
}

pub fn state() -> Result<State, Fault> {
    Ok(vault::state(&here()?)?)
}

pub fn tries_left() -> Result<u64, Fault> {
    Ok(vault::tries_left(&here()?)?)
}

pub fn set_pin(pin: &str) -> Result<(), Fault> {
    mark();
    Ok(vault::set_pin(&may_write()?, pin)?)
}

pub fn unlock(pin: &str) -> Result<(), Fault> {
    mark();
    Ok(vault::unlock(&may_write()?, pin)?)
}

pub fn check_pin(pin: &str) -> Result<(), Fault> {
    mark();
    Ok(vault::check_pin(&may_write()?, pin)?)
}

pub fn reseal(pin: &str) -> Result<(), Fault> {
    Ok(vault::reseal(&may_write()?, pin)?)
}

/// The trouble when a reseal did not land (only after a successful open); empty after taking.
pub fn take_reseal_trouble() -> Option<Fault> {
    vault::take_reseal_trouble().map(Fault::from)
}

pub fn change_pin(old: &str, new: &str) -> Result<(), Fault> {
    mark();
    Ok(vault::change_pin(&may_write()?, old, new)?)
}

pub fn recover(secret: &[u8], new_pin: &str) -> Result<(), Fault> {
    Ok(vault::recover(&may_write()?, secret, new_pin)?)
}

pub fn primary() -> Result<Option<(String, PrimaryKind)>, Fault> {
    Ok(vault::primary(&here()?)?)
}

pub fn primary_kind() -> Result<Option<PrimaryKind>, Fault> {
    Ok(vault::primary_kind(&here()?)?)
}

/// Record the primary identity's recovery seals: one per secret it recovers with (see [`recovery_secrets`]).
pub fn add_recovery(id: &str, kind: PrimaryKind, secret: &[u8]) -> Result<bool, Fault> {
    let secrets = recovery_secrets(kind, secret)?;
    Ok(vault::add_recovery(&may_write()?, id, kind, &secrets)?)
}

pub fn recoverable() -> Result<bool, Fault> {
    Ok(vault::recoverable(&here()?)?)
}

pub fn reset_empty() -> Result<(), Fault> {
    Ok(vault::reset_empty(&may_write()?)?)
}

pub fn has_recovery(id: &str) -> Result<bool, Fault> {
    Ok(vault::has_recovery(&here()?, id)?)
}

pub fn drop_recovery(id: &str) -> Result<(), Fault> {
    Ok(vault::drop_recovery(&may_write()?, id)?)
}

/// The local data key D, derived from the master key now; `LOCKED` while locked.
pub fn local_key() -> Result<LocalKey, Fault> {
    Ok(vault::local_key()?)
}

/// The names key NK, derived from the master key now; `LOCKED` while locked.
pub fn name_key() -> Result<crate::names::NameKey, Fault> {
    Ok(vault::name_key()?)
}

/// Build a vault in memory with a new master key (`primary`: id, kind, and the secret it recovers with).
pub fn build_new(pin: PinFor, primary: Option<(&str, PrimaryKind, &[u8])>, slots: &[(String, Vec<u8>)]) -> Result<NewBook, Fault> {
    let secrets = match primary {
        Some((_, kind, secret)) => Some(recovery_secrets(kind, secret)?),
        None => None,
    };
    let primary = primary.zip(secrets.as_deref()).map(|((id, kind, _), s)| (id, kind, s));
    Ok(vault::build_new(&here()?, pin, primary, slots)?)
}

pub fn stage_new(nb: &NewBook) -> Result<(), Fault> {
    Ok(vault::stage_new(&may_write()?, nb)?)
}

pub fn commit_new(nb: NewBook) -> Result<(), Fault> {
    Ok(vault::commit_new(&may_write()?, nb)?)
}

pub fn drop_staged() -> Result<bool, Fault> {
    Ok(vault::drop_staged(&may_write()?)?)
}

pub fn slot_name_of(account: &str) -> Result<Option<String>, Fault> {
    Ok(vault::slot_name_of(&here()?, account)?)
}

pub fn put(account: &str, secret: &[u8]) -> Result<(), Fault> {
    Ok(vault::put(&may_write()?, account, secret)?)
}

pub fn get(account: &str) -> Result<Option<Vec<u8>>, Fault> {
    Ok(vault::get(&here()?, account)?)
}

pub fn drop_item(account: &str) -> Result<bool, Fault> {
    Ok(vault::drop_item(&may_write()?, account)?)
}

pub fn drop_all_slots() -> Result<usize, Fault> {
    Ok(vault::drop_all_slots(&may_write()?)?)
}

pub fn slot_count() -> Result<usize, Fault> {
    Ok(vault::slot_count(&here()?)?)
}

pub fn accounts() -> Result<Vec<String>, Fault> {
    Ok(vault::accounts(&here()?)?)
}

pub fn present(account: &str) -> Result<bool, Fault> {
    Ok(vault::present(&here()?, account)?)
}

/// The command line's credential (`zikaron_seal::credential`), in this machine directory under this run's
/// account base: issued with the master key in hand (the vault open), so the command line can open this
/// machine's sealed data alone until `term` ends.
pub fn issue_credential(term: zikaron_seal::credential::Term) -> Result<(), Fault> {
    let v = may_write()?;
    let account = crate::places::key_account();
    Ok(vault::with_master(|mk| zikaron_seal::credential::issue(v.dir(), account, mk, term))??)
}

/// Remove the command line's credential; true when there was one.
pub fn drop_credential() -> Result<bool, Fault> {
    Ok(zikaron_seal::credential::remove(&crate::home::machine_dir()?, crate::places::key_account())?)
}

/// The command line's credential's term (`None` without one).
pub fn credential_term() -> Result<Option<zikaron_seal::credential::Term>, Fault> {
    match zikaron_seal::credential::term(&crate::home::machine_dir()?, crate::places::key_account()) {
        Ok(t) => Ok(Some(t)),
        Err(t) if t.code == zikaron_seal::Code::CredentialMissing => Ok(None),
        Err(t) => Err(t.into()),
    }
}

/// Where the command line's credential stands against the vault now open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Credential {
    /// None on this machine.
    None,
    /// In force until its term ends.
    Live(zikaron_seal::credential::Term),
    /// Its time is up (at `now`).
    Expired,
    /// It holds another master key than this vault's: issued before a new master key took effect.
    Stale,
}

/// Read the command line's credential at `now` and open it against the master key in hand (the vault open).
/// Opening an expired one removes it (`zikaron_seal::credential::open`).
pub fn credential_now(now: u64) -> Result<Credential, Fault> {
    let dir = crate::home::machine_dir()?;
    let account = crate::places::key_account();
    let Some(term) = credential_term()? else { return Ok(Credential::None) };
    if term.expired(now) {
        return Ok(Credential::Expired);
    }
    let held = zikaron_seal::credential::open(&dir, account, now)?;
    let same = vault::with_master(|mk| held.bytes() == mk.as_slice())?;
    Ok(if same { Credential::Live(term) } else { Credential::Stale })
}
