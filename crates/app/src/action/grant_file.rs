use crate::queue::Kept as _;
use super::*;

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

pub(super) fn export_grant_file(shell: &mut Shell, id: &str, to: &str, original: &str, key: Option<&str>, pass: &crate::exitgate::Pass) -> Result<crate::grantfilex::Exported, crate::fault::Fault> {
    let (id, folder) = grant_file_plan(shell, id, to)?;
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let original = original.trim();
    let at = std::path::PathBuf::from(original);
    let pick = match original {
        "" | "none" => crate::grantfilex::Pick::No,
        crate::door::ORIGINALS_ALL => crate::grantfilex::Pick::Listed,
        _ => crate::grantfilex::Pick::At(&at),
    };
    let key: Option<[u8; 33]> = match key {
        Some(k) => Some(zikaron::hexfmt::decode(k).and_then(|b| b.try_into().ok()).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::AddressedKeyShape, k.to_string()))?),
        None => None,
    };
    let x = crate::grantfilex::export(pass, home, &id, &folder, pick, key.as_ref())?;
    if !x.sealed {
        if let Err(f) = crate::exports::record_plain(&x.path, crate::exports::Kind::GrantPackage) {
            shell.faults.push(f);
        }
    }
    Ok(x)
}

pub(super) fn set_publish(shell: &mut Shell, url: &str) -> Result<Option<String>, crate::fault::Fault> {
    let t = url.trim();
    let v = if t.is_empty() { None } else { Some(crate::fetchx::base_of(t)?.as_str().to_string()) };
    let keep = v.clone();
    shell.commit_settings(|s| s.publish = keep)?;
    if let Some(home) = shell.home.as_ref() {
        let room = home.dir(crate::home::Slot::Kits);
        let r = crate::ledgerx::root_of(home).and_then(|root| crate::home::machine_dir().and_then(|m| crate::kitsindex::relink(&m, &root, &room, v.as_deref())));
        if let Err(f) = r.map(|_| ()) {
            if f.which() != Some(crate::fault::Known::Ledger) {
                shell.faults.push(f);
            }
        }
    }
    shell.reread_kits();
    Ok(v)
}

pub(super) fn fetch_ledger(shell: &mut Shell, from: &str, password: crate::secret::Secret) -> Result<Spawned, crate::fault::Fault> {
    let f = fetch_from(shell, from)?;
    if !shell.tasks.in_flight(Kind::Fetch) {
        shell.fetch_checks_tail = false;
    }
    Ok(shell.tasks.spawn(Kind::Fetch, move || {
        let items = crate::backup::ledger_of(&f.at, password.expose(), &f.id, f.seat)?;
        let home = crate::home::Home::open(&f.root)?;
        let have = home.ledger()?.pile()?.items;
        if let Some(c) = at_odds(&have, &items)? {
            let rows = offline_rows(&home, &items)?;
            return Ok(Done::FetchConflict { root: f.root.clone(), offline: c, fetched: items.len(), rows });
        }
        let landed = crate::restorex::land(&home, &items)?;
        let pile = home.ledger()?.pile()?.items;
        let ask = f.ask(&f.root)?;
        let tail = crate::exitgate::tail(&ask)?;
        Ok(Done::Fetched { root: f.root, landed, entries: pile.len(), tail, from: None, on: Some(crate::restorex::On::asked(&ask)) })
    }))
}

pub(super) fn check_tail(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let open_ask = crate::exitgate::ask_of(shell)?;
    let reg = shell.identities.as_ref().ok_or_else(|| Fault::known(Known::NoIdentity, String::new()))?;
    let (row, _) = crate::identity::now_row(reg).ok_or_else(|| Fault::known(Known::NoIdentity, String::new()))?;
    let seats: Vec<(std::path::PathBuf, String)> = row.seats().into_iter().filter_map(|seat| Some((row.home(seat)?, row.address(seat)?.hex()))).collect();
    let open = shell.home.as_ref().map(|h| h.root().to_path_buf());
    let wanted = shell.tail_wanted;
    let spawned = shell.tasks.spawn(Kind::Fetch, move || {
        let mut asks: Vec<crate::exitgate::Ask> = Vec::new();
        let mut checked = Vec::new();
        for (root, addr) in seats {
            let Ok(h) = crate::home::Home::open(&root) else { continue };
            let is_open = open.as_deref().map(|o| crate::home::same_place(o, &root)).unwrap_or(false);
            if matches!(crate::restorex::read(&h), Ok(None)) && !(is_open && wanted) {
                continue;
            }
            let own_net = if is_open { Ok(false) } else { crate::settings::Settings::read(&h).map(|s| s.chain_id.is_some()) };
            match own_net {
                Ok(false) => {
                    let mut a = open_ask.clone();
                    a.root = root.clone();
                    a.own = Some(addr);
                    asks.push(a);
                }
                _ => match crate::exitgate::ask_for_home(&h, Some(addr)) {
                    Ok(a) => asks.push(a),
                    Err(f) => checked.push((root, Err(f), None)),
                },
            }
        }
        if asks.is_empty() && checked.is_empty() {
            return Err(Fault::known(Known::SubjectMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail215).to_string())));
        }
        for ask in asks {
            let tail = crate::exitgate::tail(&ask);
            let on = crate::restorex::On::asked(&ask);
            checked.push((ask.root, tail, Some(on)));
        }
        Ok(Done::TailChecked { checked, unwritten: false })
    });
    if spawned == Spawned::Started {
        shell.fetch_checks_tail = true;
    }
    Ok(spawned)
}

pub fn ask_unwritten(shell: &mut Shell) {
    let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else { return };
    let Ok(ask) = crate::exitgate::ask_of(shell) else { return };
    let spawned = shell.tasks.spawn(Kind::Fetch, move || {
        let recorded = crate::home::Home::open(&root).map(|h| !matches!(crate::restorex::read_on(&h), Ok(None))).unwrap_or(true);
        let checked = if recorded {
            Vec::new()
        } else {
            let on = crate::restorex::On::asked(&ask);
            vec![(root, crate::exitgate::tail(&ask), Some(on))]
        };
        Ok(Done::TailChecked { checked, unwritten: true })
    });
    if spawned == Spawned::Started {
        shell.fetch_checks_tail = true;
    }
}

pub(super) fn tail_if_due(shell: &mut Shell) {
    if shell.take_tail_due() {
        if let Err(f) = check_tail(shell) {
            shell.faults.push(f);
        }
    }
}

struct FetchFrom {
    root: std::path::PathBuf,
    at: std::path::PathBuf,
    id: String,
    seat: crate::roles::Role,
    g: crate::auditx::Ground,
    eps: Vec<crate::chainx::Endpoint>,
}

impl FetchFrom {
    fn ask(&self, root: &std::path::Path) -> Result<crate::exitgate::Ask, crate::fault::Fault> {
        let ground = zikaron_seal::ground::Ground {
            chain_id: Some(self.g.chain),
            registry: Some(self.g.registry.0),
            from_block: self.g.from_block,
            endpoints: Vec::new(),
            earlier: self.g.earlier.iter().map(|w| zikaron_seal::ground::Earlier { chain_id: w.chain, registry: w.registry.0, from_block: w.from_block }).collect(),
            publish: None,
            rest: Vec::new(),
            rest_on: false,
            log_asks: None,
        };
        Ok(zikaron_send::gate::ask(root.to_path_buf(), &ground, &self.eps, self.g.senders.first().cloned())?)
    }
}

fn at_odds(have: &[Vec<u8>], items: &[Vec<u8>]) -> Result<Option<usize>, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let broken = |all: &[Vec<u8>]| -> Result<bool, Fault> {
        if all.is_empty() {
            return Ok(false);
        }
        Ok(crate::auditx::offline_items(all)?.label == zikaron::v2::tokens::Label::BrokenChain.as_str())
    };
    let refused = || Fault::known(Known::Broken, zikaron::v2::tokens::Label::BrokenChain.as_str().to_string());
    if broken(items)? {
        return Err(refused());
    }
    let fetched: Vec<zikaron_glue::book::Entry> = items.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).collect();
    let here: Vec<zikaron_glue::book::Entry> = have.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).collect();
    let clash = here.iter().any(|h| fetched.iter().any(|f| f.law == h.law && f.seq == h.seq && f.id != h.id));
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

fn offline_rows(home: &crate::home::Home, items: &[Vec<u8>]) -> Result<Vec<(crate::ledgerx::Row, Option<u64>)>, crate::fault::Fault> {
    let fetched: Vec<String> = items.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).map(|e| e.id_hex().to_ascii_lowercase()).collect();
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

fn fetch_from(shell: &mut Shell, from: &str) -> Result<FetchFrom, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    if let Some(to) = shell.handed.as_ref() {
        return Err(Fault::known(Known::HandedOver, to.clone()));
    }
    match shell.lock.as_ref() {
        Some(l) if l.mode().writable() => {}
        Some(l) => return Err(Fault::known(Known::ReadOnly, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail006, &[&(l.holder()).to_string()])))),
        None => return Err(Fault::known(Known::NoHome, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail005).to_string()))),
    }
    let home = shell.home.as_ref().ok_or_else(|| Fault::known(Known::NoHome, String::new()))?;
    if matches!(crate::restorex::read(home), Ok(None)) {
        return Err(Fault::known(Known::SubjectMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail215).to_string())));
    }
    let root = home.root().to_path_buf();
    let who = shell.anchor.ok_or_else(|| Fault::known(Known::KeyNotStored, String::new()))?;
    let seat = shell.settings.role;
    let id = crate::identity::now_row(&crate::register::view()?).map(|(r, _)| r.id).ok_or_else(|| Fault::known(Known::NoIdentity, String::new()))?;
    let at = crate::home::landing(from)?;
    let mut g = ground_bare(shell)?;
    g.senders = vec![who.hex()];
    let eps = shell.endpoints.clone();
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail055).to_string())));
    }
    Ok(FetchFrom { root, at, id, seat, g, eps })
}

pub(super) fn fetch_aside(shell: &mut Shell, from: &str, password: crate::secret::Secret) -> Result<Spawned, crate::fault::Fault> {
    let f = fetch_from(shell, from)?;
    if !shell.tasks.in_flight(Kind::Fetch) {
        shell.fetch_checks_tail = false;
    }
    Ok(shell.tasks.spawn(Kind::Fetch, move || {
        let items = crate::backup::ledger_of(&f.at, password.expose(), &f.id, f.seat)?;
        at_odds(&[], &items)?;
        let staged = crate::local::stage_fresh_home(&f.root)?;
        let done = (|| -> Result<(usize, usize, crate::restorex::Tail, crate::restorex::On), crate::fault::Fault> {
            let landed = crate::restorex::land(&staged, &items)?;
            let pile = staged.ledger()?.pile()?.items;
            let ask = f.ask(staged.root())?;
            Ok((landed, pile.len(), crate::exitgate::tail(&ask)?, crate::restorex::On::asked(&ask)))
        })();
        let (landed, entries, tail, on) = match done {
            Ok(x) => x,
            Err(e) => {
                let _ = std::fs::remove_dir_all(crate::local::staged_home(&f.root));
                return Err(e);
            }
        };
        let aside = match crate::local::swap_aside(&f.root) {
            Ok(a) => a,
            Err(e) => {
                if !crate::local::is_cut(&e) {
                    if let Err(s) = crate::local::settle_swap() {
                        crate::local::note_trouble(s);
                    }
                }
                return Err(e);
            }
        };
        Ok(Done::FetchedAside { aside, fetched: Box::new(Done::Fetched { root: f.root, landed, entries, tail, from: None, on: Some(on) }) })
    }))
}

pub(super) fn check_published(shell: &mut Shell, local: &str) -> Result<Spawned, crate::fault::Fault> {
    let url = shell.settings.publish.clone().ok_or_else(|| {
        crate::fault::Fault::known(crate::fault::Known::FieldMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::PublishNoUrl).to_string()))
    })?;
    let base = crate::fetchx::base_of(&url)?;
    let dir = if local.trim().is_empty() {
        let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
        crate::kitx::latest_kit(&home.dir(crate::home::Slot::Kits))
            .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::PublishNoKit, home.dir(crate::home::Slot::Kits).display().to_string()))?
    } else {
        std::path::PathBuf::from(local.trim())
    };
    if zikaron_glue::container::is_container_at(&dir) {
        return Err(crate::fault::Fault::known(crate::fault::Known::NotAdoptable, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::PublishFileKit).to_string())));
    }
    if !dir.join(zikaron_glue::names::MANIFEST).is_file() {
        return Err(crate::fault::Fault::known(crate::fault::Known::NotAdoptable, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail027, &[&dir.display().to_string()]))));
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

#[cfg(test)]
mod tests {
    use super::at_odds;
    use zikaron::json::Value;
    use zikaron_glue::book::{self, Law};

    #[test]
    fn positions_are_each_laws_own() {
        let s = crate::key::Secret::take([0x59; 32]).expect("a key");
        let author = s.address().expect("an address").hex();
        let text = |t: &str| Value::Str(t.to_string());
        let genesis_v1 = crate::entryx::seal_under(Law::One, &s, "genesis", 0, None, Value::Obj(vec![("statement_md".to_string(), text("old"))])).expect("seals");
        let closing = crate::entryx::seal_under(Law::One, &s, "succession", 1, Some(&genesis_v1.id), book::closing_body(&author, 100, book::CLOSING_KIND)).expect("seals");
        let genesis_v2 = crate::entryx::seal_under(Law::Two, &s, "genesis", 0, None, book::genesis_body("new", Some((&author, &closing.id)))).expect("seals");
        let ledger = vec![genesis_v1.bytes.clone(), closing.bytes.clone(), genesis_v2.bytes.clone()];
        assert_eq!(at_odds(&ledger, &ledger).ok(), Some(None), "one ledger, two segments");
        let note = |seq, prev: &str, said: &str| crate::entryx::seal_under(Law::Two, &s, "annotation", seq, Some(prev), Value::Obj(vec![("note_md".to_string(), text(said))])).expect("seals");
        let (here, there) = (note(1, &genesis_v2.id, "here"), note(1, &genesis_v2.id, "there"));
        let mut have = ledger.clone();
        have.push(here.bytes.clone());
        let mut fetched = ledger.clone();
        fetched.push(there.bytes.clone());
        assert_eq!(at_odds(&have, &fetched).ok(), Some(Some(1)), "two entries at one place of one law, one of them here alone");
    }
}
