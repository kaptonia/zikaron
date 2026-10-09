use super::*;

/// Everything a grant file export can be refused for without reading the chain: the id's shape, a home, the
/// folder given (empty means the home's kits folder) and the grant's chain in this home. Checked before the
/// exit gate starts and again by the export after it passes; this is the only place that decides.
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

/// Exports a grant file to the chosen folder (empty means the home's kits folder); the publish address pointer
/// comes from settings.
pub(super) fn export_grant_file(shell: &mut Shell, id: &str, to: &str, pass: &crate::exitgate::Pass) -> Result<crate::grantfilex::Exported, crate::fault::Fault> {
    let (id, folder) = grant_file_plan(shell, id, to)?;
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    // The exit gate passed in the background just before this runs (`gate_first`).
    crate::grantfilex::export(pass, home, &id, shell.settings.publish.as_deref(), &folder)
}

/// Sets the publish address. Only `https://` is accepted; anything else is `REMOTE_NOT_HTTPS` and the settings
/// file is left untouched. Empty clears it.
pub(super) fn set_publish(shell: &mut Shell, url: &str) -> Result<Option<String>, crate::fault::Fault> {
    let t = url.trim();
    let v = if t.is_empty() { None } else { Some(crate::fetchx::base_of(t)?.as_str().to_string()) };
    let keep = v.clone();
    shell.commit_settings(|s| s.publish = keep)?;
    // The publish base changed: re-link this home's bundles that have no hand-filled `link` to the new base.
    if let Some(home) = shell.home.as_ref() {
        let room = home.dir(crate::home::Slot::Kits);
        let r = crate::ledgerx::root_of(home).and_then(|root| crate::home::machine_dir().and_then(|m| crate::kitsindex::relink(&m, &root, &room, v.as_deref())));
        // A ledger without genesis has exported no bundles; that is not an error.
        if let Err(f) = r.map(|_| ()) {
            if f.which() != Some(crate::fault::Known::Ledger) {
                shell.faults.push(f);
            }
        }
    }
    shell.reread_kits();
    Ok(v)
}

/// Fetches the ledger and checks its tail. Only for a restored identity's home (marked not yet fetched): takes
/// this identity's full ledger from the whole-machine backup at `from`, lands it in this home (skipping
/// existing entries), then scans this identity's anchors on chain and checks that every anchored digest is in
/// this ledger. A missing node or basis, or an unreachable chain, is refused by name and the mark stays
/// (writing stays closed until the tail check passes).
pub(super) fn fetch_ledger(shell: &mut Shell, from: &str, password: crate::secret::Secret) -> Result<Spawned, crate::fault::Fault> {
    let f = fetch_from(shell, from)?;
    if !shell.tasks.in_flight(Kind::Fetch) {
        shell.fetch_checks_tail = false;
    }
    Ok(shell.tasks.spawn(Kind::Fetch, move || {
        let items = crate::backup::ledger_of(&f.at, password.expose(), &f.id, f.seat)?;
        let home = crate::home::Home::open(&f.root)?;
        // Reconcile before landing: this home's entries plus the fetched ones go through one offline
        // reconciliation. If they break together while the fetched ledger is sound on its own, that is a
        // conflict (same chain position, different contents): nothing lands and the person is asked (setting
        // this home aside with `fetch_aside` is the way through). A fetched ledger broken on its own is refused.
        let have = home.ledger()?.pile()?.items;
        if let Some(c) = at_odds(&have, &items)? {
            let rows = offline_rows(&home, &items)?;
            return Ok(Done::FetchConflict { root: f.root.clone(), offline: c, fetched: items.len(), rows });
        }
        let landed = crate::restorex::land(&home, &items)?;
        let pile = home.ledger()?.pile()?.items;
        let tail = crate::exitgate::tail(&f.ask(&f.root))?;
        Ok(Done::Fetched { root: f.root, landed, entries: pile.len(), tail, from: None })
    }))
}

/// Checks the tail of this identity's seats against the chain, wherever their ledgers came from (fetched from a
/// backup, adopted in place, or created here). For each seat home with the not-fetched mark, the exit gate's
/// own reading (`exitgate::tail`: that ledger's lineage and that seat's key, nodes agreeing, on that home's
/// own chain settings and nodes) says whether every anchored digest is in that home's ledger. Passing removes
/// the mark and opens writing; an anchor missing from the ledger turns the mark into "newer entries elsewhere"
/// (`Shell::fetch_landed`). Basis and nodes are checked first: without them it is refused by name and every
/// mark stays. Pollers record the attempt before asking (`Shell::take_tail_due`), so a refusal is not retried
/// every frame.
pub(super) fn check_tail(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    // The open home's settings are checked first: without them nothing can be checked.
    crate::exitgate::ask_of(shell)?;
    let view = crate::register::view(shell.settings.role)?;
    let (row, _) = crate::identity::now_row(&view).ok_or_else(|| Fault::known(Known::NoIdentity, String::new()))?;
    let open = shell.home.as_ref().map(|h| h.root().to_path_buf());
    // Each seat of this identity whose home has the mark (an unreadable mark counts as present), with what the
    // gate needs for that home.
    let mut asks: Vec<crate::exitgate::Ask> = Vec::new();
    for seat in row.seats() {
        let (Some(root), Some(addr)) = (row.home(seat), row.address(seat)) else { continue };
        let Ok(h) = crate::home::Home::open(&root) else { continue };
        if matches!(crate::restorex::read(&h), Ok(None)) {
            continue;
        }
        let is_open = open.as_deref().map(|o| crate::home::same_place(o, &root)).unwrap_or(false);
        // Another seat's home is checked on its own network if it has one (it may differ from the open home's):
        // a setting or node missing there keeps its mark and is reported by name, never read on the open
        // home's network instead. Only a home with no network of its own uses the open home's.
        let own_net = if is_open { Ok(false) } else { crate::settings::Settings::read(&h).map(|s| s.chain_id.is_some()) };
        match own_net {
            Ok(false) => {
                let mut a = crate::exitgate::ask_of(shell)?;
                a.root = root.clone();
                a.own = Some(addr.hex());
                asks.push(a);
            }
            _ => match crate::exitgate::ask_for_home(&h, Some(addr.hex())) {
                Ok(a) => asks.push(a),
                Err(f) => shell.faults.push(f),
            },
        }
    }
    if asks.is_empty() {
        return Err(Fault::known(Known::SubjectMissing, crate::lang::t(crate::lang::Key::Tail215).to_string()));
    }
    let spawned = shell.tasks.spawn(Kind::Fetch, move || {
        let mut checked = Vec::new();
        // Each home is checked separately: one whose chain cannot be read does not hold back the others.
        for ask in asks {
            let tail = crate::exitgate::tail(&ask);
            checked.push((ask.root, tail));
        }
        Ok(Done::TailChecked { checked })
    });
    if spawned == Spawned::Started {
        shell.fetch_checks_tail = true;
    }
    Ok(spawned)
}

/// Starts the tail check when it is due (`Shell::tail_due`). Called wherever it becomes due (an identity landing
/// with marked homes, nodes or basis set, a ledger adopted in place); the window's timer covers the rest.
pub(super) fn tail_if_due(shell: &mut Shell) {
    if shell.take_tail_due() {
        if let Err(f) = check_tail(shell) {
            shell.faults.push(f);
        }
    }
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

impl FetchFrom {
    /// What the gate needs to judge the tail of the ledger landed at `root` (this seat's key, these settings).
    fn ask(&self, root: &std::path::Path) -> crate::exitgate::Ask {
        crate::exitgate::Ask {
            root: root.to_path_buf(),
            eps: self.eps.iter().filter(|e| e.chain == self.g.chain).cloned().collect(),
            chain: self.g.chain,
            registry: self.g.registry,
            from_block: self.g.from_block,
            own: self.g.senders.first().cloned(),
        }
    }
}

/// Whether this home's ledger and the fetched one conflict: an entry here and a fetched entry with the same
/// sequence number but different contents. Returns `None` if not; otherwise the number of this home's entries
/// the fetched ledger lacks (entries that would remain only in the old data). A fetched ledger broken on its
/// own, or the two broken together in another way, is refused by name.
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

/// This home's entries the fetched ledger lacks, as table rows with their queue time (entries recorded here
/// offline, which would remain only in the old data if this home is set aside).
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

/// The checks fetching runs first, and what it needs.
fn fetch_from(shell: &mut Shell, from: &str) -> Result<FetchFrom, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    // Handover and the writer lock are checked first (fetching writes entries into the ledger). A broken chain
    // and a held pen are not: fetching is how those are repaired.
    if let Some(to) = shell.handed.as_ref() {
        return Err(Fault::known(Known::HandedOver, to.clone()));
    }
    match shell.lock.as_ref() {
        Some(l) if l.mode().writable() => {}
        Some(l) => return Err(Fault::known(Known::ReadOnly, crate::lang::filln(crate::lang::Key::Tail006, &[&(l.holder()).to_string()]))),
        None => return Err(Fault::known(Known::NoHome, crate::lang::t(crate::lang::Key::Tail005).to_string())),
    }
    let home = shell.home.as_ref().ok_or_else(|| Fault::known(Known::NoHome, String::new()))?;
    // An unreadable mark is still treated as "not yet fetched" (landing replaces it with a readable one or
    // removes it), so it never locks this home forever.
    if matches!(crate::restorex::read(home), Ok(None)) {
        return Err(Fault::known(Known::SubjectMissing, crate::lang::t(crate::lang::Key::Tail215).to_string()));
    }
    let root = home.root().to_path_buf();
    let who = shell.anchor.ok_or_else(|| Fault::known(Known::KeyNotStored, String::new()))?;
    // The source is a whole-machine backup: this identity's ledger for this seat is taken from it.
    let seat = shell.settings.role;
    let id = crate::identity::now_row(&crate::register::view(seat)?).map(|(r, _)| r.id).ok_or_else(|| Fault::known(Known::NoIdentity, String::new()))?;
    let at = crate::home::landing(from)?;
    let mut g = ground_bare(shell)?;
    g.senders = vec![who.hex()];
    let eps = shell.endpoints.clone();
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::t(crate::lang::Key::Tail055).to_string()));
    }
    Ok(FetchFrom { root, at, id, seat, g, eps })
}

/// The person accepted the conflict: this seat's home is set aside whole (`local::set_home_aside`: kept in the
/// machine directory under a number and listed among this machine's data folders, so it can still be opened
/// and read; it keeps its read-only mark, and the exit gate refuses it since the chain holds what it lacks). A
/// fresh home in the same place, with its settings carried over, receives the fetched ledger, and the tail is
/// checked as in `fetch_ledger`. Nothing is deleted. The first checks are the same as for fetching.
pub(super) fn fetch_aside(shell: &mut Shell, from: &str, password: crate::secret::Secret) -> Result<Spawned, crate::fault::Fault> {
    let f = fetch_from(shell, from)?;
    if !shell.tasks.in_flight(Kind::Fetch) {
        shell.fetch_checks_tail = false;
    }
    Ok(shell.tasks.spawn(Kind::Fetch, move || {
        let items = crate::backup::ledger_of(&f.at, password.expose(), &f.id, f.seat)?;
        // Everything is prepared beside this home before touching it: a fetched ledger broken on its own is
        // refused, the fresh home is staged (other folders carried over, mark placed), the fetched ledger lands
        // there and its tail is checked against the chain. Any refusal up to here leaves this home unchanged.
        at_odds(&[], &items)?;
        let staged = crate::local::stage_fresh_home(&f.root)?;
        let done = (|| -> Result<(usize, usize, crate::restorex::Tail), crate::fault::Fault> {
            let landed = crate::restorex::land(&staged, &items)?;
            let pile = staged.ledger()?.pile()?.items;
            Ok((landed, pile.len(), crate::exitgate::tail(&f.ask(staged.root()))?))
        })();
        let (landed, entries, tail) = match done {
            Ok(x) => x,
            Err(e) => {
                let _ = std::fs::remove_dir_all(crate::local::staged_home(&f.root));
                return Err(e);
            }
        };
        // Then swap, all or nothing (`local::swap_aside`).
        let aside = match crate::local::swap_aside(&f.root) {
            Ok(a) => a,
            Err(e) => {
                // Roll forward if the old data is intact in its place, otherwise back (as an interrupted swap is
                // settled at the next unlock); the shell then reopens this home as it is.
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

/// Checks publication on a background thread. The local record bundle (chosen by the person; empty means the
/// latest in the home's kits folder) is compared file by file with what is fetched from the publish address.
/// The app never uploads for the person; this only reads. A missing publish address is refused by name.
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
        zikaron_glue::pack::gather(&dir, "", &mut local).map_err(|t| crate::fault::Fault::of_landing(t))?;
        let read = crate::fetchx::compare(&base, &local)?;
        Ok(Done::Published { url: base.as_str().to_string(), read })
    }))
}
