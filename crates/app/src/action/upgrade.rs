use super::*;

pub(super) fn upgrade(shell: &mut Shell) -> Result<(crate::upgradex::Stage, Enqueued), crate::fault::Fault> {
    use crate::upgradex::Stage;
    shell.may_write_entries()?;
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let items = home.ledger()?.pile()?.items;
    let fragment = shell.audit.as_ref().map(|a| a.fragment.clone());
    let me = shell.anchor.map(|a| a.hex());
    let stage = crate::upgradex::stage(&items, fragment.as_ref(), me.as_deref(), &shell.queue);
    let held = Enqueued { queued: shell.queue.len(), next: Next::Held };
    let queue = |shell: &mut Shell, ids: &[String]| -> Enqueued {
        let mut n = Enqueued { queued: shell.queue.len(), next: Next::Held };
        for id in ids {
            let (landed, e) = enqueue(shell, id);
            if landed {
                n = Enqueued { queued: e.queued, next: Next::Send };
            }
        }
        n
    };
    let Some(me) = me else { return Ok((stage, held)) };
    match &stage {
        Stage::Nothing | Stage::Done => Ok((stage, held)),
        Stage::Handed => Err(crate::fault::Fault::known(crate::fault::Known::UpgradeHanded, me)),
        Stage::Anchor { .. } => {
            let owed = crate::upgradex::owed(&items, fragment.as_ref(), &me, &shell.queue);
            let n = queue(shell, &owed);
            Ok((stage, n))
        }
        Stage::Closing { id } | Stage::Continuing { id } => {
            let id = id.clone();
            let n = queue(shell, &[id]);
            Ok((stage, n))
        }
        Stage::Close | Stage::Genesis => {
            let empty = zikaron_glue::book::empty_fragment();
            let read = zikaron_glue::book::read(&items, fragment.as_ref().unwrap_or(&empty)).map_err(|u| crate::fault::Fault::known(crate::fault::Known::Ledger, format!("{u:?}")))?;
            let secret = signing_key(shell, crate::sign::Use::Sign(crate::sign::Face::Entry))?;
            let next = match zikaron_glue::book::next_move(&read, &me, now_secs(), None) {
                Ok(m) => m,
                Err(zikaron_glue::book::NoMove::Moved) => return Ok((stage, held)),
                Err(zikaron_glue::book::NoMove::TipAbsent) => return Err(crate::fault::Fault::known(crate::fault::Known::NoGenesis, String::new())),
                Err(zikaron_glue::book::NoMove::TipForked) => return Err(crate::fault::Fault::known(crate::fault::Known::Ledger, "TIP_FORKED".to_string())),
            };
            let sealed = match next {
                zikaron_glue::book::Move::Close { seq, prev, body } => crate::entryx::seal_under(zikaron_glue::book::Law::One, &secret, zikaron_glue::book::Kind::Succession.as_str(), seq, Some(&prev), body)?,
                zikaron_glue::book::Move::Genesis { body, .. } => crate::entryx::seal_under(zikaron_glue::book::Law::Two, &secret, zikaron_glue::book::Kind::Genesis.as_str(), 0, None, body)?,
            };
            let id = land_sealed(shell, sealed)?;
            let n = queue(shell, &[id]);
            Ok((stage, n))
        }
    }
}

pub(super) fn upgrade_cost(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    use crate::upgradex::Stage;
    let stage = shell.upgrade.clone().unwrap_or(Stage::Nothing);
    let from = shell.anchor.ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::KeychainMissing, String::new()))?;
    let basis = shell.basis_chains();
    let (chain, registry, _) = *basis.first().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoChainId, String::new()))?;
    let eps: Vec<crate::chainx::Endpoint> = shell.endpoints.iter().filter(|e| e.chain == chain).cloned().collect();
    let sends = crate::upgradex::sends_from(&stage, basis.len(), crate::upgradex::leg(&stage, &shell.queue));
    Ok(super::rotate::spawn_rotate(shell, move || {
        crate::chainx::speak();
        let (mut txs, mut wei) = (0usize, 0u128);
        for (hashes, times) in sends {
            let data = zikaron_anchor::send::registry_calldata(&vec![[0u8; 32]; hashes.max(1)]);
            let call = zikaron_anchor::send::estimate_call(&from.0, &registry.0, &data);
            let e = zikaron_send::batch::estimate(&zikaron_send::batch::GasAsk { chain, call, data, eps: eps.clone() })?;
            txs += times;
            wei = wei.saturating_add((e.gas as u128).saturating_mul(e.fees.max_fee as u128).saturating_mul(times as u128));
        }
        Ok(Done::UpgradeCost { stage, txs, wei })
    }))
}
