use super::*;

pub(super) fn grant_rules(shell: &mut Shell, d: &crate::grantx::Draft, terms_file: Option<&str>, waits: bool) -> Result<Vec<String>, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let raw = home.ledger()?.pile()?.items;
    let pile = zikaron_send::withdraw::standing(&raw, &crate::sendx::record::noticed());
    let window = window_of(&d.from, &d.to)?;
    let mut waiting = Vec::new();
    match zikaron_seal::terms::may_grant(&pile, &crate::termsx::records(home)?, &d.work, window, Some(d.supersedes.as_str()).filter(|s| !s.trim().is_empty())) {
        Ok(()) => {}
        Err(zikaron_seal::terms::Withheld::Clash(ids)) if waits && !zikaron_send::withdraw::unsettled(&raw, &ids).is_empty() => waiting = ids,
        Err(w) => {
            let token = Some(w.token());
            match w {
                zikaron_seal::terms::Withheld::Deleted => {
                    return Err(crate::fault::Fault::known(crate::fault::Known::EntryRefused, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail231).to_string())).ruled(zikaron_glue::door::Rule::Withheld, token, None, Some(Vec::new())));
                }
                zikaron_seal::terms::Withheld::Undecided(ids) => return Err(undecided(&ids)),
                zikaron_seal::terms::Withheld::Clash(ids) => {
                    let n = ids.len();
                    let clash: Vec<crate::grantx::Row> = crate::grantx::table(home)?.into_iter().filter(|r| ids.contains(&r.id)).collect();
                    shell.clash = clash;
                    return Err(crate::fault::Fault::known(crate::fault::Known::Conflict, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail043, &[&(n).to_string()]))).ruled(zikaron_glue::door::Rule::Withheld, token, None, Some(ids)));
                }
            }
        }
    }
    shell.clash.clear();
    let doc = terms_file.map(str::trim).filter(|p| !p.is_empty()).map(std::path::PathBuf::from);
    if let Some(p) = doc.as_ref() {
        let c = crate::anchorx::of_file(p)?;
        if !c.hex().eq_ignore_ascii_case(d.terms.trim()) {
            return Err(crate::fault::Fault::known(crate::fault::Known::TermsMismatch, format!("{} ≠ {}", c.hex(), d.terms.trim())));
        }
    }
    Ok(waiting)
}

pub(super) fn undecided(ids: &[String]) -> crate::fault::Fault {
    let w = zikaron_seal::terms::Withheld::Undecided(ids.to_vec());
    crate::fault::Fault::known(crate::fault::Known::EntryRefused, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::TailWithdrawalUnread, &[&ids.len().to_string()]))).ruled(zikaron_glue::door::Rule::Withheld, Some(w.token()), None, Some(ids.to_vec()))
}

pub(super) fn sign_grant(
    shell: &mut Shell,
    d: &crate::grantx::Draft,
    exclusive: bool,
    terms_file: Option<&str>,
    member: zikaron_send::issue::Member,
) -> Result<(String, Enqueued), crate::fault::Fault> {
    let doc = terms_file.map(str::trim).filter(|p| !p.is_empty()).map(std::path::PathBuf::from);
    let d = &crate::grantx::Draft { revocation: Some(member), ..d.clone() };
    let (head, secret) = ready_to_append(shell)?;
    let root = match shell.home.as_ref() {
        Some(home) => crate::auditx::root_of(&home.ledger()?.pile()?.items)?,
        None => return Err(crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())),
    };
    let sealed = match crate::grantx::draft(&secret, d, head, &root) {
        Ok(x) => {
            shell.flow.sign = crate::anchorx::Step::Done;
            x
        }
        Err(f) => {
            shell.flow.sign = crate::anchorx::Step::Failed;
            return Err(f);
        }
    };
    let id = land_sealed(shell, sealed)?;
    let n = queue_it(shell, &id);
    if let Some(home) = shell.home.as_ref() {
        if let Err(f) = crate::termsx::keep(home, &id, &d.terms, exclusive, doc.as_deref()) {
            shell.faults.push(f);
        }
    }
    shell.reread(crate::shell::Reread::Issued);
    Ok((id, n))
}

pub(super) fn window_of(from: &str, to: &str) -> Result<Option<(u64, u64)>, crate::fault::Fault> {
    let (f, t) = (from.trim(), to.trim());
    match (f.is_empty(), t.is_empty()) {
        (true, true) => Ok(None),
        (false, false) => {
            let num = |s: &str| crate::fault::whole_within_ceiling(s, crate::lang::Key::Tail044);
            Ok(Some((num(f)?, num(t)?)))
        }
        _ => Err(crate::fault::Fault::known(
            crate::fault::Known::FieldMissing,
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail045).to_string()),
        )),
    }
}
