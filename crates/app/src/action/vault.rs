use super::*;

pub(super) fn import_grant(shell: &mut Shell, typed: &str) -> Result<Imported, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let taken = crate::payloadx::take_full(typed)?;
    let all = &taken.hops;
    let mut fresh: Vec<&Vec<u8>> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut end = String::new();
    for b in all {
        let e = crate::vaultx::admit(b)?;
        let id = e.id_hex();
        end = id.clone();
        let at = crate::vaultx::path_of(home, &id)?;
        if at.exists() {
            let have = crate::local::read(&at, crate::local::Doc::Held)?.unwrap_or_default();
            if have == **b {
                skipped.push(id);
                continue;
            }
            return Err(crate::fault::Fault::known(crate::fault::Known::Occupied, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail058, &[&(id).to_string()]))));
        }
        fresh.push(b);
    }
    if fresh.is_empty() {
        let refused = || crate::fault::Fault::known(crate::fault::Known::AlreadyHeld, skipped.last().cloned().unwrap_or_default());
        let (Some((_, o)), Some(bytes)) = (taken.file.as_ref(), taken.package_bytes()) else { return Err(refused()) };
        let Some(last) = all.last() else { return Err(refused()) };
        if !crate::vaultx::newer_than_held(home, last, &o.ledger) {
            return Err(refused());
        }
        crate::grantfilex::keep(home, &bytes?, &o.kit_id)?;
        relist_held(shell);
        return Ok(Imported::Renewed { grant: end });
    }
    if let (Some((_, o)), Some(bytes)) = (taken.file.as_ref(), taken.package_bytes()) {
        crate::grantfilex::keep(home, &bytes?, &o.kit_id)?;
    }
    let ids = crate::vaultx::store_all(home, &fresh)?;
    relist_held(shell);
    Ok(Imported::Stored { ids, grant: end })
}

#[derive(Debug)]
pub(super) enum Imported {
    Stored { ids: Vec<String>, grant: String },
    Renewed { grant: String },
}

pub(super) fn import_grant_dir(shell: &mut Shell, dir: &str) -> Result<(Vec<String>, usize, Vec<crate::fault::Fault>), crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let d = dir.trim();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(d)
        .map_err(|e| crate::fault::classify(&e, d))?
        .filter_map(|x| x.ok().map(|x| x.path()))
        .filter(|p| p.is_file())
        .filter(|p| p.file_name().map(|n| !crate::home::is_side_file(&n.to_string_lossy())).unwrap_or(false))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(Fault::known(Known::GrantDirEmpty, d.to_string()));
    }
    let mut ids = Vec::new();
    let mut renewed = 0;
    let mut refused = Vec::new();
    for p in files {
        match import_grant(shell, &p.display().to_string()) {
            Ok(Imported::Stored { ids: mut got, .. }) => ids.append(&mut got),
            Ok(Imported::Renewed { .. }) => renewed += 1,
            Err(f) if f.which() == Some(Known::AlreadyHeld) => {}
            Err(f) => refused.push(f),
        }
    }
    Ok((ids, renewed, refused))
}

pub(super) fn list_held(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    let root = shell
        .home
        .as_ref()
        .map(|h| h.root().to_path_buf())
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    Ok(shell.tasks.spawn(Kind::Held, move || {
        let home = crate::home::Home::open(&root)?;
        let (held, rejected) = crate::vaultx::held_all(&home)?;
        Ok(Done::Held { held, rejected })
    }))
}

pub(super) fn set_upstream(shell: &mut Shell, grant: &str, dir: &str) -> Result<String, crate::fault::Fault> {
    let g = grant.trim().to_string();
    if !zikaron::hexfmt::is_hex32(&g) {
        return Err(crate::fault::Fault::known(crate::fault::Known::ContentShape, g));
    }
    let d = if dir.trim().is_empty() { String::new() } else { crate::home::kept(dir)?.display().to_string() };
    shell.commit_settings(|s| {
        s.upstreams.retain(|(x, _)| *x != g);
        if !d.is_empty() {
            s.upstreams.push((g.clone(), d));
        }
    })?;
    Ok(g)
}

pub(super) fn note_held(shell: &mut Shell, grant: &str, note: &str, issuer_note: &str) -> Result<String, crate::fault::Fault> {
    let g = crate::lastread::grant_form(grant);
    if !zikaron::hexfmt::is_hex32(&g) {
        return Err(crate::fault::Fault::known(crate::fault::Known::ContentShape, grant.trim().to_string()));
    }
    let (note, issuer_note) = (note.trim().to_string(), issuer_note.trim().to_string());
    if note.is_empty() && issuer_note.is_empty() {
        return Ok(g);
    }
    let author = if issuer_note.is_empty() {
        None
    } else {
        let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
        let held = crate::vaultx::held(home)?;
        let a = held.iter().find(|h| crate::lastread::grant_form(&h.id) == g).map(|h| crate::lastread::issuer_form(&h.author));
        Some(a.ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::GrantNotHeld, g.clone()))?)
    };
    shell.commit_settings(|s| {
        if !note.is_empty() {
            s.grant_notes.retain(|(x, _)| *x != g);
            s.grant_notes.push((g.clone(), note.clone()));
        }
        if let Some(a) = &author {
            s.issuer_notes.retain(|(x, _)| x != a);
            s.issuer_notes.push((a.clone(), issuer_note.clone()));
        }
    })?;
    Ok(g)
}

pub(super) fn review(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    let tried = review_start(shell);
    if !matches!(tried, Ok(Spawned::InFlight)) {
        shell.vault_changed = false;
    }
    tried
}

fn review_start(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::NoHome, String::new())
    })?;
    let at = home.root().to_path_buf();
    let ground = ground_bare(shell);
    let eps = shell.endpoints.clone();
    let shelf = shelf_of(shell, "");
    let chain = shell.settings.chain_id;
    let root = home.root().to_path_buf();
    let spawned = shell.tasks.spawn(Kind::Review, move || {
        let picked = crate::vaultx::held(&crate::home::Home::open(&at)?)?;
        let nets = read_nets_now()?;
        crate::task::stage_at(Kind::Review, 0);
        let now = chain.and_then(|c| crate::chainx::head_time(&eps, c).ok()).map(|(t, _, _)| t);
        crate::task::stage_at(Kind::Review, 1);
        let mut items = Vec::with_capacity(picked.len());
        let mut from = Vec::with_capacity(picked.len());
        let mut kept = Vec::with_capacity(picked.len());
        for (i, h) in picked.iter().enumerate() {
            crate::task::count(Kind::Review, i as u64, picked.len() as u64);
            let found = crate::supplyx::find(&shelf, crate::supplyx::Want::Hop(&h.bytes), 0);
            let up: Vec<Vec<u8>> = found.supply.as_ref().map(|s| s.items.clone()).unwrap_or_default();
            from.push((found.supply.as_ref().map(|s| (s.level, s.place.clone())), found.misses.first().map(|(_, f)| f.evidence())));
            kept.push(found.supply.as_ref().is_none_or(|s| s.kept));
            items.push((h.clone(), up));
        }
        let main = ground.as_ref().ok().filter(|_| !eps.is_empty() || !nets.is_empty()).map(|g| (&eps[..], g));
        let mut scan = |senders: &[String], pick: &zikaron_anchor::scan::Pick| -> Result<crate::vaultx::LineageRead, String> {
            let w = crate::widex::scan_picked(main, &nets, senders, crate::widex::Ask::First, pick).map_err(|f| f.evidence())?;
            Ok(crate::vaultx::LineageRead { fragment: crate::widex::with_unread_windows(&w.fragment, &w.missed, senders), anchors: w.anchors, missed: w.missed })
        };
        let read: Option<&mut dyn FnMut(&[String], &zikaron_anchor::scan::Pick) -> Result<crate::vaultx::LineageRead, String>> = if main.is_some() || !nets.is_empty() { Some(&mut scan) } else { None };
        let offer = crate::readerx::book_offer(main, &nets);
        let book = |root: &str| crate::readerx::book_key(root, &offer);
        let all_eps = crate::readnets::all_endpoints(&eps, &nets);
        let grants: Vec<&[u8]> = items.iter().map(|(h, _)| h.bytes.as_slice()).collect();
        let notes = crate::noticex::for_grants(&grants, &all_eps);
        let no_chain = read.is_none();
        let mut cards = crate::vaultx::review_all(&items, now, read, Some(&book), &notes);
        for c in cards.iter_mut() {
            c.chain_unread = no_chain;
        }
        let home = crate::home::Home::open(&root).ok();
        for (card, ((level, miss), ((h, up), kept))) in cards.iter_mut().zip(from.into_iter().zip(items.iter().zip(kept.into_iter()))) {
            card.from = level;
            if card.said.is_empty() && up.is_empty() {
                if let Some(said) = miss {
                    card.said = said;
                }
            }
            if let (Some(home), false) = (home.as_ref(), kept) {
                if let Err(f) = crate::vaultx::keep_newer(home, h, up) {
                    if card.said.is_empty() {
                        card.said = f.evidence();
                    }
                }
            }
        }
        Ok(Done::Reviewed { cards, now })
    });
    Ok(spawned)
}
