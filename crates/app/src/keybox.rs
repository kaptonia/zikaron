use crate::fault::{Fault, Known};
use zikaron_seal::keybox as vault;

pub use vault::{
    begin_lock, below_floor, floor, lock, member, params, pin_digits_only, pin_trouble, set_light_kdf, Held, LocalKey, NewBook, PinFor, PinTrouble, Plain, PrimaryKind, State, BIND_DOMAIN, IV_LEN, LOCAL_INFO, MEMBER_INFO, NAMES_INFO, NEXT, PIN_LEN, PROBE_KDF, SHAPE, SHAPE_V1, SHAPE_V2,
    WRONG_LIMIT,
};

fn here() -> Result<vault::Vault, Fault> {
    Ok(vault::Vault::new(crate::home::machine_dir()?, crate::places::keybox_file(), crate::places::keybox_lock_file()))
}

fn may_write() -> Result<vault::Vault, Fault> {
    let v = here()?;
    if crate::task::ticket_void() {
        return Err(crate::task::void_ticket_fault(v.dir()));
    }
    Ok(v)
}

fn mark() {
    crate::trace::mark(crate::feature::Feature::H2);
}

fn recovery_secrets(kind: PrimaryKind, secret: &[u8]) -> Result<Vec<Plain>, Fault> {
    let mut out = vec![Plain::new(secret.to_vec())];
    if kind == PrimaryKind::Words {
        let entropy: &[u8; crate::family::ENTROPY_BYTES] = secret.try_into().map_err(|_| Fault::known(Known::KeyboxShape, member::RECOVERY.to_string()))?;
        let seats = crate::key::seat_slots(entropy).ok_or_else(|| Fault::known(Known::KeyMalformed, member::RECOVERY.to_string()))?;
        out.extend(seats.into_iter().map(|(_, k)| Plain::new(k)));
    }
    Ok(out)
}

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

pub fn local_key() -> Result<LocalKey, Fault> {
    Ok(vault::local_key()?)
}

pub fn name_key() -> Result<crate::names::NameKey, Fault> {
    Ok(vault::name_key()?)
}

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

static CREDENTIAL_DROPPED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn commit_new(nb: NewBook) -> Result<(), Fault> {
    let v = may_write()?;
    if drop_credential()? {
        CREDENTIAL_DROPPED.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    Ok(vault::commit_new(&v, nb)?)
}

pub fn take_credential_dropped() -> bool {
    CREDENTIAL_DROPPED.swap(false, std::sync::atomic::Ordering::SeqCst)
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

pub fn issue_credential(term: zikaron_seal::credential::Term) -> Result<(), Fault> {
    let v = may_write()?;
    Ok(zikaron_seal::credential::issue(v.dir(), crate::places::key_account(), term)?)
}

pub fn add_pass(note: &str, now: u64) -> Result<(crate::secret::Secret, zikaron_seal::credential::Row), Fault> {
    let v = may_write()?;
    let account = crate::places::key_account();
    let (pass, row) = vault::with_master(|mk| zikaron_seal::credential::add(v.dir(), account, mk, note, now))??;
    Ok((crate::secret::Secret::from(pass.text()), row))
}

pub fn drop_pass(tag: &str) -> Result<(), Fault> {
    let v = may_write()?;
    Ok(zikaron_seal::credential::drop_row(v.dir(), crate::places::key_account(), tag)?)
}

pub fn drop_credential() -> Result<bool, Fault> {
    Ok(zikaron_seal::credential::remove(&crate::home::machine_dir()?, crate::places::key_account())?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Credential {
    None,
    Live(zikaron_seal::credential::Status),
    Expired,
    Unformed,
}

pub fn credential_now(now: u64) -> Result<Credential, Fault> {
    match zikaron_seal::credential::status(&crate::home::machine_dir()?, crate::places::key_account()) {
        Ok(st) if st.term.expired(now) => Ok(Credential::Expired),
        Ok(st) => Ok(Credential::Live(st)),
        Err(t) if t.code == zikaron_seal::Code::CredentialMissing => Ok(Credential::None),
        Err(t) if t.code == zikaron_seal::Code::CredentialShape => Ok(Credential::Unformed),
        Err(t) => Err(t.into()),
    }
}
