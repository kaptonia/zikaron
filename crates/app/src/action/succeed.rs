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
    // The effective time is filled by this desk with now: law §6.7 requires this cell, while §7.6 has no
    // clock and no rule compares against it; the seat moves by §7.3's lineage when the succession entry
    // is recorded. The clock comes from the shell (`Shell::clock`, injectable). What the person gave is used
    // as given. The note may be empty: empty writes the method's plain words (tokens beyond the two have none
    // and are still refused by name for the missing cell). Both cells are filled in one place,
    // `succeedx::fill`.
    let (effective, statement_md) = crate::succeedx::fill(kind, effective, statement_md, (shell.clock)());
    let body = crate::succeedx::succession_body(to, kind, &effective, &statement_md)?;
    // Do not sign when the new key is seen to have sent anchors. One key, one ledger (law §7.5); the scan is
    // run by the person first, and when it has not run this says so by name, never treating "no reading" as
    // "clean".
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
    // Hand over as soon as it is recorded. `handed` is computed by the table read, so clearing only the table
    // would leave `handed` at `None` in the shell until the person revisited the ledger page, and the old key
    // could keep appending on other pages (law §7.3: from then on the new key writes this ledger). This
    // closes the "still writable between signing and rereading" form.
    shell.handed = Some(to.trim().to_string());
    shell.stale_rows();
    Ok((id, n))
}
