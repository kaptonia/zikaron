use super::*;

pub(super) fn verify_anchors(shell: &mut Shell, rows: &str) -> Result<Spawned, crate::fault::Fault> {
    // Clear the previous result first, so a refusal partway through (bad text, no node configured) cannot
    // leave an earlier "all passed" on screen.
    shell.proofs = None;
    let parsed = crate::adoptx::rows_of(rows)?;
    let eps = shell.endpoints.clone();
    if eps.is_empty() {
        return Err(crate::fault::Fault::known(
            crate::fault::Known::NoEndpoint,
            crate::lang::t(crate::lang::Key::Tail049).to_string(),
        ));
    }
    // Declared senders: this ledger's lineage (the same algorithm as self-audit).
    let senders = match shell.home.as_ref() {
        Some(h) => {
            let pile = h
                .ledger()?
                .pile()?;
            crate::auditx::senders_of(&pile.items)
        }
        None => Vec::new(),
    };
    let checked = rows.trim().to_string();
    Ok(shell.tasks.spawn(Kind::Adopt, move || {
        let mut out = Vec::with_capacity(parsed.len());
        for r in &parsed {
            out.push(crate::adoptx::verify_row(&eps, r, &senders)?);
        }
        Ok(Done::Adopt { proofs: out, rows: checked })
    }))
}

/// Co-signing, step three: verify the co-signature locally. Only a passing co-signature goes into the body.
pub(super) fn cosign(
    shell: &mut Shell,
    rows: &str,
    attestor: &str,
    attestation: &str,
) -> Result<String, crate::fault::Fault> {
    let parsed = crate::adoptx::rows_of(rows)?;
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    // The preimage includes `prev`, so the co-signature must be made against the current head; once the head
    // changes it no longer matches.
    let head = crate::ledgerx::head(home)?.ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoGenesis, crate::lang::t(crate::lang::Key::Tail050).to_string())
    })?;
    let who = shell.anchor.ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::KeychainMissing, crate::lang::t(crate::lang::Key::SetNoKey).to_string())
    })?;
    crate::adoptx::cosigned(&who, &parsed, &head.1, attestor, attestation)?;
    Ok(attestor.trim().to_ascii_lowercase())
}

pub(super) fn adopt_anchors(
    shell: &mut Shell,
    rows: &str,
    attestor: &str,
    attestation: &str,
) -> Result<(String, bool, Enqueued), crate::fault::Fault> {
    let parsed = crate::adoptx::rows_of(rows)?;
    // A co-signature is both fields or neither; this one check serves the page and the command line alike.
    // A half-filled one is refused by naming the missing field. With both, it is verified; a failure is
    // reported at once, and clearing both still lets the person record the entry without it.
    let both = match (attestor.trim().is_empty(), attestation.trim().is_empty()) {
        (true, true) => false,
        (false, false) => true,
        (false, true) => return Err(crate::fault::Fault::known(crate::fault::Known::FieldMissing, "attestation".to_string())),
        (true, false) => return Err(crate::fault::Fault::known(crate::fault::Known::FieldMissing, "attestor".to_string())),
    };
    if both {
        cosign(shell, rows, attestor, attestation)?;
    }
    let cos = if both { Some((attestor.trim(), attestation.trim())) } else { None };
    let body = crate::adoptx::adoption_body(&parsed, cos);
    let (head, secret) = ready_to_append(shell)?;
    let sealed = match crate::entryx::seal(
        &secret,
        zikaron::tokens::EntryType::Adoption.as_str(),
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
    shell.stale_rows();
    Ok((id, both, n))
}

/// Lists the anchors a key sent. For this key (`address` empty) it uses the fragment the audit fetched when
/// the home opened and lists only anchors not yet in the ledger; for another key it scans the chain once, using
/// this home's chain, registry contract and start block with that key as the only sender. Each row not in the
/// ledger is verified on chain (this key against this ledger's lineage, another key against itself).
pub(super) fn list_key_anchors(shell: &mut Shell, address: &str) -> Result<Spawned, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    shell.key_anchors = None;
    let eps = shell.endpoints.clone();
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::t(crate::lang::Key::Tail049).to_string()));
    }
    let home = shell.home.as_ref().ok_or_else(|| Fault::known(Known::NoHome, String::new()))?;
    let pile = home.ledger()?.pile()?;
    let ids: Vec<String> = pile.items.iter().filter_map(|b| zikaron::entry::check(b).ok()).map(|e| e.id_hex()).collect();
    let lineage = crate::auditx::senders_of(&pile.items);
    let asked = address.trim().to_ascii_lowercase();
    let job: Result<(Vec<crate::adoptx::KeyAnchor>, Vec<String>, Option<crate::auditx::Ground>), Fault> = if asked.is_empty() {
        let me = shell.anchor.ok_or_else(|| Fault::known(Known::KeychainMissing, crate::lang::t(crate::lang::Key::SetNoKey).to_string()))?;
        let a = shell.audit.as_ref().ok_or_else(|| Fault::known(Known::NotAudited, String::new()))?;
        let rows: Vec<crate::adoptx::KeyAnchor> = crate::adoptx::anchors_in(&a.fragment, &me.hex(), &ids).into_iter().filter(|k| !k.in_ledger).collect();
        Ok((rows, lineage, None))
    } else {
        let who = Address::parse(&asked).ok_or_else(|| Fault::known(Known::AddressShape, asked.clone()))?;
        let mut g = ground_bare(shell)?;
        g.senders = vec![who.hex()];
        Ok((Vec::new(), vec![who.hex()], Some(g)))
    };
    let (own, senders, scan) = job?;
    Ok(shell.tasks.spawn(Kind::Adopt, move || {
        let mut rows = own;
        if let Some(g) = scan {
            let g = to_head(&eps, g)?;
            let scanned = crate::auditx::scan_once(&eps, &g)?;
            rows = crate::adoptx::anchors_in(&scanned.fragment, &asked, &ids);
        }
        for r in rows.iter_mut() {
            if !r.in_ledger {
                r.proof = Some(crate::adoptx::verify_row(&eps, &r.row, &senders)?);
            }
        }
        Ok(Done::KeyAnchors { address: asked, rows })
    }))
}

/// Reads a claim text (when signing a claim for someone else) and places it on the shell. With a network
/// configured, each anchor's block number is fetched in the background.
pub(super) fn read_claim(shell: &mut Shell, text: &str) -> Result<Option<Spawned>, crate::fault::Fault> {
    shell.claim = None;
    shell.attested = None;
    let claim = crate::adoptx::claim_of(text)?;
    let eps = shell.endpoints.clone();
    let blocks = vec![None; claim.rows.len()];
    if eps.is_empty() {
        shell.claim = Some((claim, blocks));
        return Ok(None);
    }
    Ok(Some(shell.tasks.spawn(Kind::Adopt, move || {
        let blocks: Vec<Option<u64>> = claim.rows.iter().map(|r| crate::adoptx::block_of(&eps, r)).collect();
        Ok(Done::Claim { claim, blocks })
    })))
}

/// Signs a claim for someone else and returns (signer, signature). The passcode gate has already been passed
/// in `apply`. This desk's key signs the claim's preimage (`sign::sign_adoption`) exactly as given in the text.
pub(super) fn attest_for(shell: &mut Shell, text: &str) -> Result<(String, String), crate::fault::Fault> {
    let claim = crate::adoptx::claim_of(text)?;
    let missing = || crate::fault::Fault::known(crate::fault::Known::KeychainMissing, crate::lang::t(crate::lang::Key::SetNoKey).to_string());
    let secret = crate::key::load(crate::register::account_now()?.as_deref())?.ok_or_else(missing)?;
    let who = secret.address().ok_or_else(missing)?;
    let sig = crate::sign::sign_adoption(&secret, &claim.preimage)?;
    shell.attested = Some((who.hex(), sig.clone()));
    Ok((who.hex(), sig))
}
