use super::*;

pub(super) fn look_at_key(shell: &mut Shell, to: &str) -> Result<Spawned, crate::fault::Fault> {
    let who = Address::parse(to).ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::AddressShape, to.trim().to_string())
    })?;
    let g = ground(shell)?;
    let eps = shell.endpoints.clone();
    shell.sighting = None;
    Ok(shell.tasks.spawn(Kind::Sighting, move || {
        let s = crate::succeedx::look_at(&eps, &g, &who)?;
        Ok(Done::Sighting { to: who.hex(), anchors: s.anchors, asked: s.asked })
    }))
}

pub(super) fn succeed(
    shell: &mut Shell,
    to: &str,
    kind: &str,
    effective: &str,
    statement_md: &str,
) -> Result<(String, Enqueued), crate::fault::Fault> {
    // If left empty, the effective time is filled with now from `Shell::clock` (injectable): the format
    // requires the field, but no rule compares against it, since the key changes when the succession entry is
    // recorded. Values the person gave are used as given. An empty note becomes the method's plain wording
    // (methods without one are refused by name for the missing field). Both are filled in `succeedx::fill`.
    let (effective, statement_md) = crate::succeedx::fill(kind, effective, statement_md, (shell.clock)());
    let body = crate::succeedx::succession_body(to, kind, &effective, &statement_md)?;
    // Refuse to sign if the new key has already sent anchors: one key, one ledger. The person runs the scan
    // first; if it has not run, say so by name rather than treating "no reading" as "clean".
    match shell.sighting.as_ref() {
        Some((who, anchors, _)) if who.eq_ignore_ascii_case(to.trim()) => {
            if *anchors > 0 {
                return Err(crate::fault::Fault::known(
                    crate::fault::Known::AlreadyRooted,
                    crate::lang::filln(crate::lang::Key::Tail051, &[&(who).to_string(), &(anchors).to_string()]),
                ));
            }
        }
        _ => {
            return Err(crate::fault::Fault::known(
                crate::fault::Known::NotAudited,
                crate::lang::t(crate::lang::Key::Tail052).to_string(),
            ))
        }
    }
    let (head, secret) = ready_to_append(shell)?;
    let sealed = match crate::entryx::seal(
        &secret,
        zikaron::tokens::EntryType::Succession.as_str(),
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
    // Mark the ledger handed over as soon as the entry is recorded. `handed` is otherwise computed when the
    // table is read, so clearing only the table would leave it `None` until the ledger page was revisited,
    // and the old key could keep appending from other pages. From this entry on, only the new key writes.
    shell.handed = Some(to.trim().to_string());
    shell.stale_rows();
    Ok((id, n))
}
