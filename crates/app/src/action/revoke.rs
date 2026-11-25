use super::*;

pub(super) fn revoke_rules(shell: &mut Shell, grant: &str) -> Result<(), crate::fault::Fault> {
    crate::grantx::revocation_body(grant, "")?;
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let queued: Vec<String> = shell.queue.items.iter().map(|q| q.id.clone()).collect();
    let rows = crate::ledgerx::table(home, None, &queued)?.rows;
    zikaron_glue::retraction::revocation::may_revoke(&crate::retractx::lines(&rows), grant).map_err(|why| {
        let f = match why {
            zikaron_glue::retraction::revocation::Invalid::NotInLedger => crate::fault::Fault::known(crate::fault::Known::SubjectMissing, grant.trim().to_string()),
            zikaron_glue::retraction::revocation::Invalid::NotAGrant => crate::fault::Fault::known(crate::fault::Known::SubjectMissing, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail048, &[&(grant.trim()).to_string()]))),
        };
        f.ruled(zikaron_glue::door::Rule::Revocation, Some(why.token()), Some(grant.trim().to_string()), None)
    })?;
    Ok(())
}

pub(super) fn revoke(shell: &mut Shell, grant: &str, case: &str) -> Result<(String, Enqueued), crate::fault::Fault> {
    revoke_rules(shell, grant)?;
    let id = seal_revocation(shell, grant, case)?;
    let n = queue_it(shell, &id);
    Ok((id, n))
}

pub(super) fn seal_revocation(shell: &mut Shell, grant: &str, case: &str) -> Result<String, crate::fault::Fault> {
    let body = crate::grantx::revocation_body(grant, case)?;
    let (head, secret) = ready_to_append(shell)?;
    let sealed = match crate::entryx::seal(
        &secret,
        zikaron_glue::book::Kind::Revocation.as_str(),
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
    shell.reread(crate::shell::Reread::WroteGrant);
    Ok(id)
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
