use super::*;

pub(super) fn read_book(shell: &mut Shell, address: &str, dir: &str) -> Result<Spawned, crate::fault::Fault> {
    let who = crate::readerx::who(address)?;
    let dir = one_place(dir)?;
    let g = ground_read(shell)?;
    let eps = shell.endpoints.clone();
    let shelf = shelf_of(shell, dir);
    let nets = read_nets_now()?;
    shell.book = None;
    shell.others_asked = Some(OthersAsk { diligence: false, who, eps: eps.clone(), ground: g.clone(), nets: nets.clone(), work: String::new(), window: None });
    Ok(shell.tasks.spawn(Kind::Book, move || {
        let mut found = crate::supplyx::find_book(&shelf, &who.hex());
        crate::readerx::copy_under_book(&mut found, &who, &crate::readerx::book_offer(Some((&eps, &g)), &nets));
        let bytes = found.supply.as_ref().map(|s| s.items.clone()).unwrap_or_default();
        crate::task::stage_at(Kind::Book, 0);
        let b = crate::readerx::read_copy(&who, &bytes)?;
        crate::task::stage_at(Kind::Book, 1);
        let face = crate::facex::other(&eps, &g, &nets, &[], &who, &bytes);
        let moves = crate::facex::other_moves(&eps, &g, &nets, &[], &who, &face);
        let audit_key = crate::readerx::audit_key("book", &who.hex(), &bytes, Some(&g), &nets);
        Ok(Done::Book {
            face,
            moves,
            who: b.who,
            anchors: 0,
            asked: 0,
            entries: b.entries,
            label: String::new(),
            timeline: b.timeline,
            grants: b.grants,
            latest: None,
            from: found.supply.as_ref().map(|s| (s.level, s.place.clone())),
            files: found.supply.as_ref().and_then(|s| s.files),
            misses: found.misses,
            missed: Vec::new(),
            pile: bytes,
            audit_key,
        })
    }))
}

#[derive(Clone, Debug)]
pub struct OthersAsk {
    pub diligence: bool,
    pub who: crate::key::Address,
    pub eps: Vec<crate::chainx::Endpoint>,
    pub ground: crate::auditx::Ground,
    pub nets: Vec<crate::readnets::Net>,
    pub work: String,
    pub window: Option<(u64, u64)>,
}

pub(super) fn audit_others(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    let grantee = shell.others_asked.as_ref().is_some_and(|a| a.diligence);
    let read = if grantee {
        shell.diligence.as_ref().map(|r| (r.audit_key, r.pile.clone()))
    } else {
        match shell.book.as_ref() {
            Some(Done::Book { audit_key, pile, .. }) => Some((*audit_key, pile.clone())),
            _ => None,
        }
    };
    let (Some((key, bytes)), Some(asked)) = (read, shell.others_asked.clone()) else {
        return Err(crate::fault::Fault::known(crate::fault::Known::FieldMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::U3OthersNotRead).to_string())));
    };
    let whole = shell.others_whole.clone();
    let work = move || -> Result<crate::checkx::Audit, crate::fault::Fault> {
        let OthersAsk { who, eps, ground, nets, work, window, .. } = asked;
        let (g, scanned, missed, book) = crate::readerx::audit_read(&eps, ground, &who, &bytes, &nets)?;
        let entries = crate::auditx::read(&bytes, &scanned.fragment).map(|l| l.entries.len()).unwrap_or(0);
        let (chain, block) = crate::verifyx::basis_read(&scanned.fragment).first().map(|b| (b.chain, b.to_block)).unwrap_or((g.chain, 0));
        let at = crate::chainx::head_time(&eps, g.chain).ok().map(|(t, _, _)| t);
        let label = book.label.clone();
        let seen = if grantee {
            let mut r = crate::diligx::assemble(book, &scanned.fragment, &bytes, &work, window)?;
            r.missed = missed;
            r.now = at;
            r.pile = bytes.clone();
            r.audit_key = key;
            let seen = crate::readerx::Audited::with_diligence(&r);
            *whole.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, r));
            seen
        } else {
            crate::readerx::Audited::of_book(&book, &missed)
        };
        Ok(crate::checkx::Audit { verdict: String::new(), label, entries, chain, block, at, value: seen.to_value(), anchored: Vec::new(), ledgers: Vec::new() })
    };
    shell.audits.ask(key, crate::sendx::behind::Job { keeper: Kind::keeper(), work: Box::new(work) });
    super::check::behind_turn(shell);
    Ok(())
}

pub(super) fn face_reread(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf()).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let eps = shell.endpoints.clone();
    let chains = shell.basis_chains();
    let reg = shell.identities.clone();
    let here = shell.identities.as_ref().and_then(|r| crate::identity::now_row(r).and_then(|(row, _)| row.address(crate::roles::Role::Author)));
    Ok(shell.tasks.spawn(Kind::Persona, move || {
        let items = crate::home::Home::open(&root)?.ledger()?.pile()?.items;
        let own = crate::facex::own(&eps, &chains, &items);
        let me = zikaron_glue::book::root_of(&items).ok().and_then(|r| crate::key::Address::parse(&r)).or(here);
        let table = match &own {
            crate::facex::State::Read(b) => Some(&b.table),
            _ => None,
        };
        let pair = me
            .and_then(|me| crate::facex::other_of(table, &items, &me, reg.as_ref()))
            .map(|(side, other, id)| crate::facex::pair_read(&eps, &chains, side, other, id));
        Ok(Done::Persona(own, pair))
    }))
}

pub(super) fn face_books(shell: &mut Shell) -> Result<Spawned, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let root = ledger_root(shell).ok_or_else(|| Fault::known(Known::NoGenesis, String::new()))?;
    let office = shell.anchor.ok_or_else(|| Fault::known(Known::KeychainMissing, String::new()))?;
    let eps = shell.endpoints.clone();
    let chains: Vec<u64> = shell.basis_chains().into_iter().map(|(c, _, _)| c).collect();
    Ok(shell.tasks.spawn(Kind::FaceBooks, move || {
        crate::chainx::speak();
        let root_key_here = crate::key::present(Some(&crate::places::key_slot(&root))).unwrap_or(false);
        let mut out = Vec::new();
        for chain in chains {
            let mine: Vec<crate::chainx::Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
            let book = match zikaron_send::book::state(&mine, chain) {
                Err(t) => crate::facex::Book::Unread(Fault::from(t)),
                Ok(zikaron_send::book::State::Off) => crate::facex::Book::Off,
                Ok(zikaron_send::book::State::Wrong) => crate::facex::Book::Wrong,
                Ok(zikaron_send::book::State::At(at)) => match enrolment_read(&mine, chain, &at, &root, &office) {
                    Ok((held, plan)) => crate::facex::Book::Read { under: held.key_under(&root.0), plan },
                    Err(f) => crate::facex::Book::Unread(f),
                },
            };
            out.push((chain, book));
        }
        Ok(Done::FaceBooks(crate::facex::Books { root, office, root_key_here, chains: out }))
    }))
}

pub(super) fn book_address(shell: &mut Shell, address: &str, on: bool) -> Result<String, crate::fault::Fault> {
    let a = crate::readerx::who(address)?.hex();
    shell.commit_settings(|s| {
        if on {
            if !s.book.iter().any(|x| *x == a) {
                s.book.push(a.clone());
            }
        } else {
            s.book.retain(|x| *x != a);
        }
    })?;
    Ok(a)
}
