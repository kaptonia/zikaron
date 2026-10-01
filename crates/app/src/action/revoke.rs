use super::*;

pub(super) fn revoke(shell: &mut Shell, grant: &str, case: &str) -> Result<(String, Enqueued), crate::fault::Fault> {
    let body = crate::grantx::revocation_body(grant, case)?;
    // The revoked grant must be in this ledger. The zikaron/1 law (§6.4) says `grant` is "the entry_id of a
    // grant in this ledger"; a revocation pointing nowhere is a valid entry with a dangling reference (§11),
    // and that is not what the person wants: it is refused by name at once, so it never lands on chain.
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    // Read that entry now, not through the register table: the two have separate owners, and this question
    // only needs "is it in this ledger, and is it a grant".
    let d = crate::ledgerx::detail(home, grant).map_err(|_| {
        crate::fault::Fault::known(crate::fault::Known::SubjectMissing, grant.trim().to_string())
    })?;
    if d.kind != zikaron::tokens::EntryType::Grant {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::SubjectMissing,
            crate::lang::filln(crate::lang::Key::Tail048, &[&(grant.trim()).to_string()]),
        ));
    }
    let (head, secret) = ready_to_append(shell)?;
    let sealed = match crate::entryx::seal(
        &secret,
        zikaron::tokens::EntryType::Revocation.as_str(),
        head.0 + 1,
        Some(&head.1),
        body,
    ) {
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
    // The register row's state changes too: clear it so it is reread.
    shell.stale_grants();
    shell.stale_rows();
    Ok((id, n))
}

pub(super) fn read_story(shell: &mut Shell, grant: &str) -> Result<usize, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let s = crate::grantx::story(home, grant)?;
    let n = s.revocations.len();
    shell.story = Some((grant.trim().to_string(), s.grant, s.revocations));
    Ok(n)
}
