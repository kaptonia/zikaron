use crate::fault::{Fault, Known};

pub fn set_primary(id: &str, pin: &str) -> Result<(), Fault> {
    crate::trace::mark(crate::feature::Feature::H4);
    let reg = crate::register::read()?.ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?;
    let row = reg.find(id).ok_or_else(|| Fault::known(Known::NoIdentity, id.to_string()))?.clone();
    if let Some((p, _)) = crate::keybox::primary()? {
        if p.eq_ignore_ascii_case(&row.id) {
            return Err(Fault::known(Known::PrimaryAlready, row.id.clone()));
        }
    }
    let (kind, secret_slot) = match row.kind() {
        crate::identity::Kind::Words => (crate::keybox::PrimaryKind::Words, crate::places::seed_slot(&row.id)),
        crate::identity::Kind::Existing => {
            if !row.backed_file {
                return Err(Fault::known(Known::PrimaryNoKeyFile, row.id.clone()));
            }
            let acct = row.accounts().into_iter().next().ok_or_else(|| Fault::known(Known::KeychainMissing, row.id.clone()))?;
            (crate::keybox::PrimaryKind::KeyFile, acct)
        }
    };
    let mut secret = crate::keybox::get(&secret_slot)?.ok_or_else(|| Fault::known(Known::KeychainMissing, secret_slot.clone()))?;
    let mut slots: Vec<(String, Vec<u8>)> = Vec::new();
    for a in crate::keybox::accounts()? {
        if let Some(b) = crate::keybox::get(&a)? {
            slots.push((a, b));
        }
    }
    let nb = crate::keybox::build_new(crate::keybox::PinFor::New(pin), Some((&row.id, kind, &secret)), &slots);
    zikaron_os::wipe(&mut secret);
    for (_, b) in slots.iter_mut() {
        zikaron_os::wipe(b);
    }
    let nb = nb?;
    let new = nb.local_key();
    let old = crate::keybox::local_key()?;
    let staged = (|| -> Result<(), Fault> {
        crate::keybox::stage_new(&nb)?;
        crate::local::restage(&old, &new, &nb.name_key())?;
        Ok(())
    })();
    let undo = || {
        let _ = crate::keybox::drop_staged();
        let _ = crate::local::drop_change();
    };
    if let Err(f) = staged {
        undo();
        return Err(f);
    }
    crate::local::cut_point(crate::local::Cut::BeforeCommit)?;
    if let Err(f) = crate::keybox::commit_new(nb) {
        undo();
        return Err(f);
    }
    crate::local::cut_point(crate::local::Cut::AfterCommit)?;
    if let Err(f) = crate::local::settle_pending() {
        crate::local::note_trouble(f);
    }
    Ok(())
}
