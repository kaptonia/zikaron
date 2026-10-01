use super::*;

/// Everything an export of a grant file can be refused for without reading the chain: the id's shape, a
/// home, the folder the person gave (empty means the home's kits) and the grant's chain in this home. Asked
/// before the exit gate starts and again by the export where the gate landed; the judgment lives only here.
pub(super) fn grant_file_plan(shell: &Shell, id: &str, to: &str) -> Result<(String, std::path::PathBuf), crate::fault::Fault> {
    let id = id.trim().to_string();
    if !zikaron::hexfmt::is_hex32(&id) {
        return Err(crate::fault::Fault::known(crate::fault::Known::ContentShape, id));
    }
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let folder = if to.trim().is_empty() { home.dir(crate::home::Slot::Kits) } else { crate::home::landing(to)? };
    crate::badgex::chain_for(&crate::badgex::pool(home)?, &id)?;
    Ok((id, folder))
}

/// Export a grant file. It lands in the folder the person chose (empty means the home's kits); the publish
/// address pointer comes from the settings cell.
pub(super) fn export_grant_file(shell: &mut Shell, id: &str, to: &str) -> Result<crate::grantfilex::Exported, crate::fault::Fault> {
    let (id, folder) = grant_file_plan(shell, id, to)?;
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    // The exit gate passed in the background just before this runs (`gate_first`).
    crate::grantfilex::export(home, &id, shell.settings.publish.as_deref(), &folder)
}

/// Set the publish address. `https://` only; unrecognized is `REMOTE_NOT_HTTPS`, and not one byte of the
/// settings file changes. Empty clears it.
pub(super) fn set_publish(shell: &mut Shell, url: &str) -> Result<Option<String>, crate::fault::Fault> {
    let t = url.trim();
    let v = if t.is_empty() { None } else { Some(crate::fetchx::base_of(t)?.as_str().to_string()) };
    let keep = v.clone();
    shell.commit_settings(|s| s.publish = keep)?;
    // The publish base changed: bundles of this home without a hand-filled `link` are reassembled on the new
    // base.
    if let Some(home) = shell.home.as_ref() {
        let room = home.dir(crate::home::Slot::Kits);
        let r = crate::ledgerx::root_of(home).and_then(|root| crate::home::machine_dir().and_then(|m| crate::kitsindex::relink(&m, &root, &room, v.as_deref())));
        // A ledger without genesis has exported no bundles, which is not a trouble.
        if let Err(f) = r.map(|_| ()) {
            if f.which() != Some(crate::fault::Known::Ledger) {
                shell.faults.push(f);
            }
        }
    }
    shell.reread_kits();
    Ok(v)
}

/// Fetch the ledger and check its tail. Only for a restored identity's home (with the mark): fetch this
/// identity's full ledger through the four levels (`from` is the person's place for the last two), land it in
/// this home (skipping existing entries), then scan this identity's anchors on chain and check "every
/// anchored digest is in this ledger". Node or basis not configured, or chain unreachable, is refused by name
/// and the mark stays (without a tail check, writing does not open).
pub(super) fn fetch_ledger(shell: &mut Shell, from: &str, password: crate::secret::Secret) -> Result<Spawned, crate::fault::Fault> {
    let f = fetch_from(shell, from)?;
    Ok(shell.tasks.spawn(Kind::Fetch, move || {
        let items = crate::backup::ledger_of(&f.at, password.expose(), &f.id, f.seat)?;
        let home = crate::home::Home::open(&f.root)?;
        // Reconcile before landing: this home's existing entries plus the fetched ones go to the core for one
        // offline reconciliation. Broken together while the fetched ledger holds on its own is a conflict
        // (the same place in the chain, other contents): nothing lands, and the person is asked (setting this
        // home aside is the one way through, `fetch_aside`). A fetched ledger broken on its own is refused.
        let have = home.ledger()?.pile()?.items;
        if let Some(c) = at_odds(&have, &items)? {
            let rows = offline_rows(&home, &items)?;
            return Ok(Done::FetchConflict { root: f.root.clone(), offline: c, fetched: items.len(), rows });
        }
        let landed = crate::restorex::land(&home, &items)?;
        let pile = home.ledger()?.pile()?.items;
        let scanned = crate::auditx::scan_once(&f.eps, &to_head(&f.eps, f.g)?)?;
        let tail = crate::restorex::tail_check(&pile, &scanned.fragment);
        Ok(Done::Fetched { root: f.root, landed, entries: pile.len(), tail, from: None })
    }))
}

/// Where fetching takes its ledger from and what it checks against, asked before the background pass.
struct FetchFrom {
    root: std::path::PathBuf,
    at: std::path::PathBuf,
    id: String,
    seat: crate::roles::Role,
    g: crate::auditx::Ground,
    eps: Vec<crate::chainx::Endpoint>,
}

/// Whether this home's ledger and the fetched one are at odds: an entry here and a fetched entry at the same
/// place in the chain (the same sequence number) with other contents. `None` when there is none; the number of
/// this home's entries the fetched ledger lacks when there is (entries recorded here that would stay only in
/// the old data). A fetched ledger broken on its own, or the two broken together some other way, is refused by
/// name.
fn at_odds(have: &[Vec<u8>], items: &[Vec<u8>]) -> Result<Option<usize>, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let broken = |all: &[Vec<u8>]| -> Result<bool, Fault> {
        if all.is_empty() {
            return Ok(false);
        }
        Ok(crate::auditx::offline_items(all)?.label == zikaron::tokens::Label::BrokenChain.as_str())
    };
    let refused = || Fault::known(Known::Broken, zikaron::tokens::Label::BrokenChain.as_str().to_string());
    if broken(items)? {
        return Err(refused());
    }
    let fetched: Vec<zikaron::entry::Entry> = items.iter().filter_map(|b| zikaron::entry::check(b).ok()).collect();
    let here: Vec<zikaron::entry::Entry> = have.iter().filter_map(|b| zikaron::entry::check(b).ok()).collect();
    let clash = here.iter().any(|h| fetched.iter().any(|f| f.seq == h.seq && f.id != h.id));
    if clash {
        return Ok(Some(here.iter().filter(|h| !fetched.iter().any(|f| f.id == h.id)).count()));
    }
    let mut all = have.to_vec();
    all.extend(items.iter().cloned());
    if broken(&all)? {
        return Err(refused());
    }
    Ok(None)
}

/// This home's entries the fetched ledger lacks, as table rows, each with when it was queued (the entries
/// recorded here offline: they stay only in the old data when this home is set aside).
fn offline_rows(home: &crate::home::Home, items: &[Vec<u8>]) -> Result<Vec<(crate::ledgerx::Row, Option<u64>)>, crate::fault::Fault> {
    let fetched: Vec<String> = items.iter().filter_map(|b| zikaron::entry::check(b).ok()).map(|e| e.id_hex().to_ascii_lowercase()).collect();
    let queue = crate::queue::Queue::read(home).unwrap_or_default();
    let table = crate::ledgerx::table_with(home, None, &queue)?;
    Ok(table
        .rows
        .into_iter()
        .filter(|r| !fetched.iter().any(|x| x.eq_ignore_ascii_case(&r.id)))
        .map(|r| {
            let at = queue.items.iter().find(|q| q.id.eq_ignore_ascii_case(&r.id)).map(|q| q.at);
            (r, at)
        })
        .collect())
}

/// The checks fetching asks first, and what it needs.
fn fetch_from(shell: &mut Shell, from: &str) -> Result<FetchFrom, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    // Handover and the writer lock are asked first (fetching lands entries in the ledger); broken chain and pen
    // are not asked here: fetching is exactly the way to repair those two.
    if let Some(to) = shell.handed.as_ref() {
        return Err(Fault::known(Known::HandedOver, to.clone()));
    }
    match shell.lock.as_ref() {
        Some(l) if l.mode().writable() => {}
        Some(l) => return Err(Fault::known(Known::ReadOnly, crate::lang::filln(crate::lang::Key::Tail006, &[&(l.holder()).to_string()]))),
        None => return Err(Fault::known(Known::NoHome, crate::lang::t(crate::lang::Key::Tail005).to_string())),
    }
    let home = shell.home.as_ref().ok_or_else(|| Fault::known(Known::NoHome, String::new()))?;
    // An unreadable mark is still treated as "not yet fetched" (landing overwrites it with a readable form or
    // removes it), never locking this home forever.
    if matches!(crate::restorex::read(home), Ok(None)) {
        return Err(Fault::known(Known::SubjectMissing, crate::lang::t(crate::lang::Key::Tail215).to_string()));
    }
    let root = home.root().to_path_buf();
    let who = shell.anchor.ok_or_else(|| Fault::known(Known::KeyNotStored, String::new()))?;
    // The source is a whole-machine backup: this identity's ledger for this seat is taken from it.
    let seat = shell.settings.role;
    let id = crate::identity::now_row(seat)?.map(|(r, _)| r.id).ok_or_else(|| Fault::known(Known::NoIdentity, String::new()))?;
    let at = crate::home::landing(from)?;
    let mut g = ground_bare(shell)?;
    g.senders = vec![who.hex()];
    let eps = shell.endpoints.clone();
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::t(crate::lang::Key::Tail055).to_string()));
    }
    Ok(FetchFrom { root, at, id, seat, g, eps })
}

/// The person said yes to the conflict: this seat's home is set aside whole (`local::set_home_aside`: kept
/// in the machine directory under a number, listed among this machine's data folders so it can be opened and
/// read; its read-only mark stays, and the exit gate refuses it since the chain holds what it lacks); a fresh
/// home in its place (the same place, its settings carried over) receives the fetched ledger, and the tail is
/// checked as for `fetch_ledger`. Nothing is deleted. The same first checks as fetching.
pub(super) fn fetch_aside(shell: &mut Shell, from: &str, password: crate::secret::Secret) -> Result<Spawned, crate::fault::Fault> {
    let f = fetch_from(shell, from)?;
    Ok(shell.tasks.spawn(Kind::Fetch, move || {
        let items = crate::backup::ledger_of(&f.at, password.expose(), &f.id, f.seat)?;
        // Everything is done beside this home before it is touched: a fetched ledger broken on its own is
        // refused, the fresh home is staged (its other rooms carried, its mark placed), the fetched ledger lands
        // there and its tail is checked against the chain. Any refusal up to here leaves this home as it was.
        at_odds(&[], &items)?;
        let staged = crate::local::stage_fresh_home(&f.root)?;
        let done = (|| -> Result<(usize, usize, crate::restorex::Tail), crate::fault::Fault> {
            let landed = crate::restorex::land(&staged, &items)?;
            let pile = staged.ledger()?.pile()?.items;
            let scanned = crate::auditx::scan_once(&f.eps, &to_head(&f.eps, f.g.clone())?)?;
            Ok((landed, pile.len(), crate::restorex::tail_check(&pile, &scanned.fragment)))
        })();
        let (landed, entries, tail) = match done {
            Ok(x) => x,
            Err(e) => {
                let _ = std::fs::remove_dir_all(crate::local::staged_home(&f.root));
                return Err(e);
            }
        };
        // Then the swap, all or nothing (`local::swap_aside`).
        let aside = match crate::local::swap_aside(&f.root) {
            Ok(a) => a,
            Err(e) => {
                // Forward when the old data is whole in its place, back otherwise (as a cut is at the next
                // unlock); the shell opens this home again as it then is.
                if !crate::local::is_cut(&e) {
                    if let Err(s) = crate::local::settle_swap() {
                        crate::local::note_trouble(s);
                    }
                }
                return Err(e);
            }
        };
        Ok(Done::FetchedAside { aside, fetched: Box::new(Done::Fetched { root: f.root, landed, entries, tail, from: None }) })
    }))
}

/// Check publication. Runs on a background thread. The local record bundle (chosen by the person; empty means
/// the latest in the home's kits) is compared file by file with what is fetched from the publish address; the
/// product does not upload for the person, and this step only reads. No publish address configured is refused
/// by name.
pub(super) fn check_published(shell: &mut Shell, local: &str) -> Result<Spawned, crate::fault::Fault> {
    let url = shell.settings.publish.clone().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::FieldMissing, crate::lang::t(crate::lang::Key::PublishNoUrl).to_string())
    })?;
    let base = crate::fetchx::base_of(&url)?;
    let dir = if local.trim().is_empty() {
        let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
        crate::kitx::latest_kit(&home.dir(crate::home::Slot::Kits))
            .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::PublishNoKit, home.dir(crate::home::Slot::Kits).display().to_string()))?
    } else {
        std::path::PathBuf::from(local.trim())
    };
    if !dir.join(zikaron_glue::names::MANIFEST).is_file() {
        return Err(crate::fault::Fault::known(crate::fault::Known::NotAdoptable, crate::lang::filln(crate::lang::Key::Tail027, &[&dir.display().to_string()])));
    }
    shell.published = None;
    Ok(shell.tasks.spawn(Kind::Publish, move || {
        crate::task::stage_at(Kind::Publish, 0);
        let mut local: Vec<(String, Vec<u8>)> = Vec::new();
        zikaron_glue::pack::gather(&dir, "", &mut local).map_err(|t| crate::fault::Fault::landing(t.code(), t.subject()))?;
        let read = crate::fetchx::compare(&base, &local)?;
        Ok(Done::Published { url: base.as_str().to_string(), read })
    }))
}
