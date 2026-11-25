use super::*;
use crate::fault::{Fault, Known};
use zikaron_glue::face::Table;
use zikaron_glue::facegate::{self as gate, Barred, Consented, Reseat};

pub(crate) fn barred(b: Barred) -> Fault {
    let k = match b.code() {
        "NO_GENESIS" => Known::NoGenesis,
        "FACE_UNWRITABLE" => Known::FaceUnwritable,
        "FACE_UNREAD" => Known::FaceUnread,
        "FACE_ROTATING" => Known::FaceRotating,
        "FACE_NO_HOME" => Known::FaceNoHome,
        "FACE_HOME_CHAIN" => Known::FaceHomeChain,
        "FACE_ROW" => Known::FaceRow,
        "CONSENT_REFUSED" => Known::ConsentRefused,
        "ADDRESS_SHAPE" => Known::AddressShape,
        "CONTENT_SHAPE" => Known::ContentShape,
        "FACE_SEALED" => Known::FaceSealed,
        _ => Known::SubjectMissing,
    };
    let f = Fault::known(k, b.detail()).owned(crate::fault::Owned::Reason(zikaron_glue::door::Own::Face));
    match b {
        Barred::NoHome { unsent: true, .. } => f.worded(None, Some(crate::lang::Key::FaultNextFaceUnsent)),
        Barred::HomeUnread { .. } => f.worded(Some(crate::lang::Key::FaultWhatFaceHomeUnread), Some(crate::lang::Key::FaultNextFaceHomeUnread)).no_answer(),
        _ => f,
    }
}

fn table_of(shell: &Shell) -> Result<Table, String> {
    match &shell.persona {
        crate::facex::State::Read(b) => Ok(b.table.clone()),
        crate::facex::State::Unread => Err("unread".to_string()),
        crate::facex::State::NotRead(m) => Err(crate::facex::missing_said(m)),
    }
}

pub(super) fn ready(shell: &Shell) -> Result<gate::Ready, Fault> {
    shell.may_write_entries()?;
    let home = shell.home.as_ref().ok_or_else(|| Fault::known(Known::NoHome, String::new()))?;
    let items = home.ledger()?.pile()?.items;
    let key = shell.anchor.ok_or_else(|| Fault::known(Known::KeychainMissing, String::new()))?;
    let machine = match &shell.persona {
        crate::facex::State::Read(_) => None,
        crate::facex::State::Unread => Some("unread".to_string()),
        crate::facex::State::NotRead(m) => Some(crate::facex::missing_machine(m)),
    };
    let sent = shell.queue.sent_ids();
    gate::ready(items, table_of(shell), &key.hex()).map(|r| r.with_sent(sent)).map_err(|b| {
        let unread = matches!(b, gate::Barred::Unread(_));
        match (unread, machine) {
            (true, Some(m)) => barred(b).in_machine_words(m),
            _ => barred(b),
        }
    })
}

fn write(shell: &mut Shell, body: zikaron::json::Value) -> Result<(String, Enqueued), Fault> {
    let (head, secret) = ready_to_append(shell)?;
    let sealed = crate::entryx::seal(&secret, zikaron_glue::face::ENTRY_TYPE, head.0 + 1, Some(&head.1), body)?;
    drop(secret);
    let id = land_sealed(shell, sealed)?;
    let n = queue_it(shell, &id);
    Ok((id, n))
}

pub(super) fn face_name(shell: &mut Shell, name: &str) -> Result<(String, Enqueued), Fault> {
    let body = ready(shell)?.persona(name).map_err(barred)?;
    write(shell, body)
}

pub(super) fn face_account(shell: &mut Shell, platform: &str, handle: &str, note: &str) -> Result<(String, Enqueued), Fault> {
    let body = ready(shell)?.account(platform, handle, note).map_err(barred)?;
    write(shell, body)
}

pub(super) fn face_key(shell: &mut Shell, address: &str, role: &str, note: &str, consent: &str, sign_here: bool) -> Result<(String, Enqueued), Fault> {
    let r = ready(shell)?;
    r.home().map_err(barred)?;
    let c = match r.consent(address, role, consent).map_err(barred)? {
        Consented::Have(c) => c,
        Consented::SignHere { address, role } => {
            let key = if sign_here { consent_key(shell, &Address(address))? } else { None };
            let Some(k) = key else { return Err(barred(Barred::Consent("MISSING".to_string()))) };
            let root: [u8; 20] = zikaron::hexfmt::decode(r.writer.root()).and_then(|b| b.try_into().ok()).ok_or_else(|| Fault::known(Known::NoGenesis, String::new()))?;
            r.holding(crate::sign::sign_consent(&k, &root, &role)?).map_err(barred)?
        }
    };
    let body = r.key(&c, note).map_err(barred)?;
    write(shell, body)
}

pub(super) fn face_release(shell: &mut Shell, subject: &str) -> Result<(String, Enqueued), Fault> {
    let body = ready(shell)?.release(subject).map_err(barred)?;
    write(shell, body)
}

pub(super) fn home_first(shell: &mut Shell, chain: u64) -> Applied {
    let r = match home_gates(shell, chain) {
        Ok((r, _)) => r,
        Err(f) => return shell.trouble(f),
    };
    let eps: Vec<crate::chainx::Endpoint> = shell.endpoints.iter().filter(|e| e.chain == chain).cloned().collect();
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf());
    let seat_root: [u8; 20] = zikaron::hexfmt::decode(r.writer.root()).and_then(|b| b.try_into().ok()).unwrap_or([0; 20]);
    let key = shell.anchor.map(|a| a.0).unwrap_or([0; 20]);
    let lineage = gate::lineage_of(&r.items);
    match shell.tasks.spawn(Kind::Gate, move || {
        crate::chainx::speak();
        crate::chainx::book::seated(&eps, chain, &seat_root, &key, &lineage).map_err(|t| Fault::from(t).owned(crate::fault::Owned::Reason(zikaron_glue::door::Own::Face)))?;
        Ok(Done::FaceCleared { root, then: Box::new(Action::FaceHome { chain }) })
    }) {
        Spawned::Started => Applied::Started(Kind::Gate),
        Spawned::InFlight => Applied::Refused(Kind::Gate),
    }
}

fn home_gates(shell: &Shell, chain: u64) -> Result<(gate::Ready, zikaron::json::Value), Fault> {
    let r = ready(shell)?;
    let basis: Vec<u64> = shell.basis_chains().iter().map(|(c, _, _)| *c).collect();
    let body = r.opening(chain, &basis).map_err(barred)?;
    Ok((r, body))
}

pub fn face_cleared(shell: &mut Shell, root: Option<std::path::PathBuf>, then: Action) -> Option<Applied> {
    let here = shell.home.as_ref().map(|h| h.root().to_path_buf());
    if here != root {
        return None;
    }
    let Action::FaceHome { chain } = then else { return Some(apply(shell, then)) };
    trace::mark(Feature::H2);
    let written = home_gates(shell, chain).and_then(|(_, body)| write(shell, body));
    Some(match written {
        Ok((id, q)) => Applied::Queued { id, queued: q.queued, next: q.next },
        Err(f) => shell.trouble(f),
    })
}

pub fn unseated(items: &[Vec<u8>], state: &crate::facex::State) -> Option<usize> {
    unseated_of(&face_entries(items), state)
}

pub fn face_entries(items: &[Vec<u8>]) -> Vec<gate::LedgerFace> {
    gate::ledger_faces(items)
}

pub fn unseated_of(faces: &[gate::LedgerFace], state: &crate::facex::State) -> Option<usize> {
    let table = match state {
        crate::facex::State::Read(b) => Some(&b.table),
        _ => None,
    };
    gate::unseated(faces, table)
}

pub(super) fn face_reseat(shell: &mut Shell, entry: &str) -> Result<Applied, Fault> {
    let r = ready(shell)?;
    let queue = shell.queue.clone();
    let again = r.reseat(entry, |id| queue.has(id)).map_err(barred)?;
    let id = match &again {
        Reseat::Home { id, .. } | Reseat::Body { id, .. } => id.clone(),
    };
    let office_signed = r.items.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).any(|e| e.id_hex().eq_ignore_ascii_case(&id) && e.author.eq_ignore_ascii_case(&r.table.office));
    let on_chain = shell.queue.anchored_here(&id) || shell.audit.as_ref().is_some_and(|a| crate::ledgerx::anchored_of(&a.report).iter().any(|(h, _)| h.eq_ignore_ascii_case(&id)));
    if office_signed && !on_chain {
        let (landed, q) = super::ledger::enqueue(shell, &id);
        if landed {
            return Ok(Applied::Queued { id, queued: q.queued, next: q.next });
        }
    }
    if let Some(home) = shell.home.as_ref() {
        let (_, q) = crate::queue::amend(home, |q| q.drop_ids(std::slice::from_ref(&id)))?;
        shell.queue = q;
    }
    match again {
        Reseat::Home { chain, .. } => Ok(home_first(shell, chain)),
        Reseat::Body { body, .. } => {
            let (id, q) = write(shell, body)?;
            Ok(Applied::Queued { id, queued: q.queued, next: q.next })
        }
    }
}

pub(super) fn face_consent(shell: &mut Shell, identity: &str, role: &str) -> Result<String, Fault> {
    let text = identity.trim();
    let hex = text.strip_prefix(zikaron_glue::face::MARKER_PREFIX).unwrap_or(text);
    let root = Address::parse(hex).ok_or_else(|| Fault::known(Known::ConsentRefused, "FORM root".to_string()))?;
    let key = signing_key(shell, crate::sign::Use::Sign(crate::sign::Face::Consent))?;
    Ok(crate::sign::sign_consent(&key, &root.0, role.trim())?.text())
}

pub(super) fn face_reroot(shell: &mut Shell, to: &str) -> Result<(String, Enqueued), Fault> {
    let body = ready(shell)?.reroot(to, "").map_err(barred)?;
    write(shell, body)
}

pub(super) fn face_origin(shell: &mut Shell, from: &str) -> Result<(String, Enqueued), Fault> {
    let body = ready(shell)?.origin(from, "").map_err(barred)?;
    write(shell, body)
}

pub fn reroot_row(chain: u64) -> Result<&'static crate::deploy::Deployment, Fault> {
    crate::deploy::KNOWN.iter().find(|d| d.chain_id == chain && zikaron_anchor::book::at(chain).is_some()).ok_or_else(|| barred(Barred::HomeChain { outside: true, chain }))
}

pub fn reroot_nodes(settings: &crate::settings::Settings, chain: u64) -> Result<Vec<String>, Fault> {
    let row = reroot_row(chain)?;
    Ok(super::archive::chain_nodes(settings, chain, row))
}

pub(super) fn reroot_make(shell: &mut Shell, answers: &[(usize, crate::secret::Secret)], label: &str, chain: u64) -> Result<Applied, Fault> {
    let row = reroot_row(chain)?;
    let r = ready(shell)?;
    let old = Address::parse(r.writer.root()).ok_or_else(|| Fault::known(Known::NoGenesis, String::new()))?;
    if let Some(made) = shell.identities.as_ref().and_then(|reg| reg.rows.iter().find(|x| x.reroot_of == Some(old)).map(|x| x.id.clone())) {
        return Err(barred(Barred::Row(format!("made {made}"))));
    }
    let nodes = reroot_nodes(&shell.settings, chain)?;
    let Some(f) = shell.new_words.as_ref() else {
        return Err(Fault::known(Known::PhraseConfirm, String::new()));
    };
    crate::identity::confirm(f, answers)?;
    let f = f.twin();
    let label = label.trim().to_string();
    let name = row.name;
    Ok(vault(shell, move || {
        let made = crate::register::change(|reg| {
            let was = reg.current.clone();
            let made = crate::identity::add_words(reg, &f, true, &label)?;
            crate::identity::choose_network(reg, &made.id, name)?;
            let row = reg.rows.iter_mut().find(|x| x.id == made.id).ok_or_else(|| Fault::known(Known::NoIdentity, made.id.clone()))?;
            row.reroot_of = Some(old);
            if !label.is_empty() {
                row.label = label.clone();
            }
            let made = row.clone();
            reg.current = was;
            Ok(made)
        })?;
        if let Some(dir) = made.home(crate::roles::Role::Author) {
            let home = crate::home::Home::open_or_create(&dir)?;
            let mut s = crate::settings::Settings::read(&home).unwrap_or_default();
            super::archive::reroot_network(&mut s, row, nodes);
            s.write(&home)?;
        }
        Ok(crate::task::Vault::RerootMade { id: made.id })
    }))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeReading {
    pub chain: u64,
    pub to: Address,
    pub has: u128,
    pub need: u128,
    pub give: u128,
    pub office_has: u128,
    pub cost: u128,
    pub sent: Option<String>,
}

impl FeeReading {
    pub fn met(&self) -> bool {
        self.has >= self.need
    }

    pub fn can_send(&self) -> bool {
        self.office_has >= self.give.saturating_add(self.cost)
    }
}

pub(super) fn reroot_fee(shell: &mut Shell, go: bool) -> Result<Spawned, Fault> {
    let pair = shell.face_pair.clone().filter(|p| p.side == crate::facex::Side::Old).ok_or_else(|| barred(Barred::Unread("pair".to_string())))?;
    let chain = pair.other_chain.ok_or_else(|| barred(Barred::Unread("chain".to_string())))?;
    let registry = reroot_row(chain)?.registry_address();
    let eps: Vec<crate::chainx::Endpoint> = reroot_nodes(&shell.settings, chain)?.iter().filter_map(|x| crate::chainx::Endpoint::parse(x)).collect();
    let urls: Vec<crate::chainx::NodeAddr> = eps.iter().map(|e| e.url.clone()).collect();
    let r = ready_or_sealed(shell)?;
    let entry = gate::written_reroot(&r.0).and_then(|(id, _)| r.0.iter().find(|b| zikaron::hexfmt::encode(&zikaron::cryptox::sha256(b)) == id).cloned()).ok_or_else(|| barred(Barred::NoEntry("reroot".to_string())))?;
    let calls = 2 + r.1;
    let office = shell.anchor.ok_or_else(|| Fault::known(Known::KeychainMissing, String::new()))?;
    let to = pair.other;
    let (current, ask) = if go { (Some(signing_key(shell, crate::sign::Use::Anchor)?), Some(crate::exitgate::ask_of(shell)?)) } else { (None, None) };
    Ok(shell.tasks.spawn(Kind::RerootFee, move || {
        crate::chainx::speak();
        let read = || -> Result<FeeReading, Fault> {
            let call = zikaron_send::one::Call { chain, to: registry.0, value: 0, data: zikaron_anchor::send::carried_calldata(&entry), again: zikaron_send::one::Again::Same };
            let (_, fees) = zikaron_send::one::estimate(&eps, &to.0, &call)?;
            let need = u128::from(fees.gas_limit).saturating_mul(u128::from(fees.max_fee)).saturating_mul(2).saturating_mul(calls as u128);
            let (has, _) = crate::chainx::balance(&eps, chain, &to)?;
            let (office_has, _) = crate::chainx::balance(&eps, chain, &office)?;
            let give = need.saturating_add(need / 4).saturating_sub(has);
            let transfer = zikaron_send::one::Call::transfer(chain, to.0, give.max(1));
            let (_, tf) = zikaron_send::one::estimate(&eps, &office.0, &transfer)?;
            let cost = u128::from(tf.gas_limit).saturating_mul(u128::from(tf.max_fee));
            Ok(FeeReading { chain, to, has, need, give, office_has, cost, sent: None })
        };
        let now = read()?;
        let (Some(current), Some(ask)) = (current, ask) else { return Ok(Done::RerootFee(now)) };
        if now.met() || now.give == 0 {
            return Ok(Done::RerootFee(now));
        }
        if !now.can_send() {
            return Err(Fault::known(Known::InsufficientFunds, format!("{chain} {} < {}", now.office_has, now.give.saturating_add(now.cost))));
        }
        let pass = crate::exitgate::pass(&ask)?;
        let fund = zikaron_send::one::Call::transfer(chain, to.0, now.give);
        let (_, ff) = zikaron_send::one::estimate(&eps, &office.0, &fund)?;
        let fell = zikaron_send::one::send(zikaron_send::one::Through::Gate(&pass), &urls, &fund, &current, ff, std::time::Duration::from_secs(60))?;
        if fell.status != 1 {
            return Err(super::rotate::failed_in_block(&fell, ff.gas_limit));
        }
        let mut after = read()?;
        after.sent = Some(fell.tx.clone());
        Ok(Done::RerootFee(after))
    }))
}

fn ready_or_sealed(shell: &Shell) -> Result<(Vec<Vec<u8>>, usize), Fault> {
    let home = shell.home.as_ref().ok_or_else(|| Fault::known(Known::NoHome, String::new()))?;
    let items = home.ledger()?.pile()?.items;
    let t = table_of(shell).map_err(|w| barred(Barred::Unread(w)))?;
    Ok((items, usize::from(t.persona.is_some()) + t.faces.iter().filter(|r| r.bound && !matches!(r.named, zikaron_glue::face::Named::Persona(_))).count()))
}

pub(super) fn joined_read(shell: &mut Shell, root: &str, to: &str) -> Result<Spawned, Fault> {
    let a = Address::parse(root.trim()).ok_or_else(|| barred(Barred::Address(root.trim().to_string())))?;
    let b = Address::parse(to.trim()).ok_or_else(|| barred(Barred::Address(to.trim().to_string())))?;
    let eps = shell.endpoints.clone();
    let chains = shell.basis_chains();
    let local = |x: &Address| shell.identities.as_ref().and_then(|reg| reg.rows.iter().find(|r| r.address(crate::roles::Role::Author) == Some(*x)).map(|r| r.id.clone()));
    let (ia, ib) = (local(&a), local(&b));
    Ok(shell.tasks.spawn(Kind::FaceJoined, move || {
        let first = crate::facex::pair_read(&eps, &chains, crate::facex::Side::New, a, ia);
        let second = crate::facex::pair_read(&eps, &chains, crate::facex::Side::Old, b, ib);
        Ok(Done::FaceJoined(Box::new(crate::facex::Joined { first, second })))
    }))
}
