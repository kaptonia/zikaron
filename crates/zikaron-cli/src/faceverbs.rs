use crate::args::Args;
use crate::codes::{Key, Reason};
use crate::out::{self, s, Answer, Said};
use crate::verbs;
use zikaron::json::Value;
use zikaron_glue::door::Own;
use zikaron_glue::facegate::{self as gate, Barred, Consented};
use zikaron_send::face::{Built, Lane, NotRead};
use zikaron_send::node::{Endpoint, NodeAddr};

struct Chains {
    lanes: Vec<Lane>,
    eps: Vec<Endpoint>,
}

fn given_chains(a: &Args) -> Chains {
    let endpoints = a.many_at("endpoint");
    if endpoints.is_empty() {
        out::misuse(Reason::Args, out::Subject::flag("endpoint"), Said::FaceChains);
    }
    let specs = crate::chain::endpoint_specs(&endpoints);
    let mut lanes = Vec::new();
    for c in crate::chain::chains_of(&specs) {
        let Some((row, registry)) = zikaron_anchor::deploy::ROWS.iter().find(|r| r.chain_id == c).and_then(|r| Some((r, zikaron_seal::identities::address(r.registry)?))) else {
            out::misuse(Reason::Args, out::Subject::flag("endpoint"), Said::ChainNotDeployed);
        };
        lanes.push(Lane { chain: c, registry, from_block: row.from_block });
    }
    let eps = specs.iter().map(|(c, u)| Endpoint::at(*c, NodeAddr::new(u.as_str()))).collect();
    verbs::proxy_from(a);
    Chains { lanes, eps }
}

fn home_chains(a: &Args, ground: &zikaron_seal::ground::Ground, cli: Result<zikaron_seal::machine::Cli, zikaron_seal::Trouble>) -> Result<Chains, Answer> {
    if !a.many_at("endpoint").is_empty() {
        out::misuse(Reason::Args, out::Subject::flag("endpoint"), Said::HomeSettingsSay);
    }
    if a.one("proxy").is_some() {
        verbs::proxy_from(a);
    } else {
        verbs::rest_keys_from(a);
        let cli = cli.map_err(|t| crate::sealed::refused(&t))?;
        zikaron_send::node::take_machine_route(cli.proxy.as_deref());
    }
    let mut lanes = Vec::new();
    if let (Some(chain), Some(registry)) = (ground.chain_id, ground.registry) {
        lanes.push(Lane { chain, registry, from_block: ground.from_block });
        lanes.extend(ground.other_chains().into_iter().filter(|c| *c != chain).filter_map(|c| ground.window_of(c).map(|(registry, from_block)| Lane { chain: c, registry, from_block })));
    }
    let eps = lanes.iter().flat_map(|l| crate::sealedsend::nodes_of(ground, l.chain)).collect();
    Ok(Chains { lanes, eps })
}

pub(crate) fn sealed_here(home: &crate::sealed::Opened, ground: &zikaron_seal::ground::Ground) -> Option<Answer> {
    let root = root_of(&home.items)?;
    let mut lanes = Vec::new();
    if let (Some(chain), Some(registry)) = (ground.chain_id, ground.registry) {
        lanes.push(Lane { chain, registry, from_block: ground.from_block });
        lanes.extend(ground.other_chains().into_iter().filter(|c| *c != chain).filter_map(|c| ground.window_of(c).map(|(registry, from_block)| Lane { chain: c, registry, from_block })));
    }
    let eps: Vec<Endpoint> = lanes.iter().flat_map(|l| crate::sealedsend::nodes_of(ground, l.chain)).collect();
    let b = zikaron_send::face::input(&eps, &root, &lanes, &home.items, &home.checked()).ok()?;
    let barred = gate::sealed(&b.table).err()?;
    let seated = gate::seated_ids(&b.table);
    let held: Vec<String> = gate::ledger_faces(&home.items).into_iter().filter(|f| !seated.contains(&f.id)).map(|f| f.id).collect();
    let _ = home.amend_queue(|q| {
        let ids: Vec<String> = q.items.iter().filter(|x| held.iter().any(|h| h.eq_ignore_ascii_case(&x.id))).map(|x| x.id.clone()).collect();
        q.drop_ids(&ids)
    });
    Some(self::barred(barred))
}

fn unread(detail: &str) -> Answer {
    crate::named::of(Own::Face, "FACE_UNREAD", detail, true, "")
}

fn barred(b: Barred) -> Answer {
    crate::named::of(Own::Face, b.code(), &b.detail(), b.unanswered(), "")
}

fn root_of(items: &[Vec<u8>]) -> Option<[u8; 20]> {
    zikaron_glue::book::root_of(items).ok().and_then(|r| zikaron_seal::identities::address(&r))
}

fn read(chains: &Chains, root: &[u8; 20], pile: &[Vec<u8>], facts: &dyn zikaron_send::Facts) -> Result<Built, NotRead> {
    zikaron_send::face::input(&chains.eps, root, &chains.lanes, pile, facts)
}

pub(crate) fn face_table(a: &Args) -> Answer {
    let built = if let Some(r) = a.one("root") {
        let Some(root) = zikaron_seal::identities::address(&r) else { out::misuse(Reason::Args, out::Subject::flag("root"), Said::Not20) };
        let chains = given_chains(a);
        let facts = crate::hopin::bare_record();
        let facts: &dyn zikaron_send::Facts = match &facts {
            Some(f) => f,
            None => &zikaron_send::Bare,
        };
        read(&chains, &root, &[], facts)
    } else {
        let at = a.need("ledger");
        if crate::sealed::is_home(&at) {
            let r = match crate::sealed::read(&at, verbs::now_secs()) {
                Ok(r) => r,
                Err(t) => return crate::sealed::refused(&t),
            };
            let ground = match r.ground() {
                Ok(g) => g,
                Err(t) => return crate::sealed::refused(&t),
            };
            let chains = match home_chains(a, &ground, r.machine_settings()) {
                Ok(c) => c,
                Err(answer) => return answer,
            };
            let Some(root) = root_of(&r.items) else { return unread(zikaron_send::face::NO_ROOT) };
            read(&chains, &root, &r.items, &r.checked())
        } else {
            let dir = match crate::ledger::open(&at) {
                Ok(d) => d,
                Err(t) => return verbs::ledger_trouble(&t),
            };
            let items = match crate::ledger::pile(&dir) {
                Ok(x) => x,
                Err(t) => return verbs::ledger_trouble(&t),
            };
            let chains = given_chains(a);
            let Some(root) = root_of(&items) else { return unread(zikaron_send::face::NO_ROOT) };
            let facts = crate::hopin::bare_record();
            let facts: &dyn zikaron_send::Facts = match &facts {
                Some(f) => f,
                None => &zikaron_send::Bare,
            };
            read(&chains, &root, &items, facts)
        }
    };
    match built {
        Ok(b) => table_answer(&zikaron::json::canon_bytes(&b.table.value), b.moment, &zikaron_send::face::omitted(&b.left)),
        Err(n) => unread(&n.said()),
    }
}

pub(crate) fn table_answer(table: &[u8], moment: u64, omitted: &[String]) -> Answer {
    let table = match zikaron::json::parse(table) {
        Ok(v) => v,
        Err(_) => return unread("NO_TABLE"),
    };
    out::affirmed(vec![(Key::Table, table), (Key::Moment, Value::Int(moment)), (Key::Omitted, Value::Arr(omitted.iter().map(|x| s(x)).collect()))])
}

enum Write {
    Home(u64),
    Account { platform: String, handle: String, note: String },
    Key { address: String, role: String, sig: String, note: String },
    Name(String),
    Release(String),
    Reroot(String),
    Origin(String),
}

fn text(a: &Args, f: &'static str) -> String {
    a.one(f).unwrap_or_default()
}

pub(crate) fn face_home(a: &Args) -> Answer {
    face_write(a, Write::Home(a.within_ceiling("chain").unwrap_or(0)))
}

pub(crate) fn face_account(a: &Args) -> Answer {
    face_write(a, Write::Account { platform: text(a, "platform"), handle: text(a, "handle"), note: text(a, "note") })
}

pub(crate) fn face_key(a: &Args) -> Answer {
    face_write(a, Write::Key { address: text(a, "address"), role: text(a, "role"), sig: text(a, "sig"), note: text(a, "note") })
}

pub(crate) fn face_name(a: &Args) -> Answer {
    face_write(a, Write::Name(text(a, "name")))
}

pub(crate) fn face_release(a: &Args) -> Answer {
    face_write(a, Write::Release(text(a, "subject")))
}

pub(crate) fn face_reroot(a: &Args) -> Answer {
    face_write(a, Write::Reroot(text(a, "to")))
}

pub(crate) fn face_origin(a: &Args) -> Answer {
    face_write(a, Write::Origin(text(a, "from")))
}

pub(crate) fn face_joined(a: &Args) -> Answer {
    let addr = |f: &'static str| zikaron_seal::identities::address(&a.need(f)).unwrap_or_else(|| out::misuse(Reason::Args, out::Subject::flag(f), Said::Not20));
    let (first, second) = (addr("root"), addr("to"));
    let chains = match a.one("ledger") {
        Some(at) if crate::sealed::is_home(&at) => {
            let r = match crate::sealed::read(&at, verbs::now_secs()) {
                Ok(r) => r,
                Err(t) => return crate::sealed::refused(&t),
            };
            let ground = match r.ground() {
                Ok(g) => g,
                Err(t) => return crate::sealed::refused(&t),
            };
            match home_chains(a, &ground, r.machine_settings()) {
                Ok(c) => c,
                Err(answer) => return answer,
            }
        }
        _ => given_chains(a),
    };
    let facts = crate::hopin::bare_record();
    let facts: &dyn zikaron_send::Facts = match &facts {
        Some(f) => f,
        None => &zikaron_send::Bare,
    };
    let mut read_to = Vec::new();
    let mut tables = Vec::new();
    for root in [first, second] {
        match read(&chains, &root, &[], facts) {
            Ok(b) => {
                read_to.push((zikaron::hexfmt::encode(&root), b.moment, zikaron_send::face::omitted(&b.left)));
                tables.push(b.table);
            }
            Err(n) => return unread(&format!("root {} {}", zikaron::hexfmt::encode(&root), n.said())),
        }
    }
    joined_answer(zikaron_glue::face::joined(&tables[0], &tables[1]), &read_to)
}

pub(crate) fn joined_answer(joined: bool, read_to: &[(String, u64, Vec<String>)]) -> Answer {
    let one = |(root, moment, omitted): &(String, u64, Vec<String>)| {
        out::obj(vec![(Key::Root, s(root)), (Key::Moment, Value::Int(*moment)), (Key::Omitted, Value::Arr(omitted.iter().map(|x| s(x)).collect()))])
    };
    out::affirmed(vec![(Key::Joined, Value::Bool(joined)), (Key::ReadTo, Value::Arr(read_to.iter().map(one).collect()))])
}

fn gated(w: &Write, items: Vec<Vec<u8>>, table: Result<zikaron_glue::face::Table, String>, key: &[u8; 20], chains: &Chains, sent: Vec<String>) -> Result<Value, Answer> {
    let r = gate::ready(items, table, &zikaron::hexfmt::encode(key)).map_err(barred)?.with_sent(sent);
    match w {
        Write::Home(chain) => {
            let basis: Vec<u64> = chains.lanes.iter().map(|l| l.chain).collect();
            let body = r.opening(*chain, &basis).map_err(barred)?;
            let root: [u8; 20] = zikaron::hexfmt::decode(r.writer.root()).and_then(|b| b.try_into().ok()).unwrap_or([0; 20]);
            let eps: Vec<Endpoint> = chains.eps.iter().filter(|e| e.chain == *chain).cloned().collect();
            zikaron_send::book::seated(&eps, *chain, &root, key, &gate::lineage_of(&r.items)).map_err(|t| crate::named::of_trouble(Own::Face, &t, ""))?;
            Ok(body)
        }
        Write::Account { platform, handle, note } => r.account(platform, handle, note).map_err(barred),
        Write::Key { address, role, sig, note } => {
            r.home().map_err(barred)?;
            let c = match r.consent(address, role, sig).map_err(barred)? {
                Consented::Have(c) => c,
                Consented::SignHere { .. } => return Err(barred(Barred::Consent("MISSING".to_string()))),
            };
            r.key(&c, note).map_err(barred)
        }
        Write::Name(name) => r.persona(name).map_err(barred),
        Write::Release(subject) => r.release(subject).map_err(barred),
        Write::Reroot(to) => r.reroot(to, "").map_err(barred),
        Write::Origin(from) => r.origin(from, "").map_err(barred),
    }
}

fn folder_sent(chains: &Chains, items: &[Vec<u8>], table: &Result<zikaron_glue::face::Table, String>, signer: &[u8; 20]) -> Vec<String> {
    let (Ok(t), Some((entry, chain))) = (table, gate::last_home_entry(items)) else { return Vec::new() };
    if t.home == Some(chain) || gate::read_in(t, &entry) {
        return Vec::new();
    }
    let (Some(lane), Some(hash)) = (chains.lanes.iter().find(|l| l.chain == chain), zikaron::hexfmt::decode(&entry).and_then(|b| <[u8; 32]>::try_from(b).ok())) else { return Vec::new() };
    let urls: Vec<NodeAddr> = chains.eps.iter().filter(|e| e.chain == chain).map(|e| e.url.clone()).collect();
    match zikaron_send::batch::on_chain(&urls, chain, &lane.registry, signer, &[hash], lane.from_block, &gate::lineage_of(items)) {
        Err(t) if t.code == zikaron_send::Code::OnChain => vec![entry],
        _ => Vec::new(),
    }
}

fn table_for(chains: &Chains, items: &[Vec<u8>], facts: &dyn zikaron_send::Facts) -> Result<zikaron_glue::face::Table, String> {
    let root = root_of(items).ok_or_else(|| zikaron_send::face::NO_ROOT.to_string())?;
    read(chains, &root, items, facts).map(|b| b.table).map_err(|n| n.said())
}

fn face_write(a: &Args, w: Write) -> Answer {
    let at = a.need("ledger");
    let entry_type = zikaron_glue::face::ENTRY_TYPE;
    if crate::sealed::is_home(&at) {
        if a.one("key").is_some() || a.one(crate::args::KEY_FILE).is_some() {
            out::misuse(Reason::Args, out::Subject::Flags(vec!["key", crate::args::KEY_FILE]), Said::SealedNoKey);
        }
        let now = verbs::now_secs();
        let (home, (seq, prev)) = match crate::sealedsend::open_to_write(&at, now, |h| verbs::next_link(a, &h.items)) {
            Ok(x) => x,
            Err(answer) => return answer,
        };
        let ground = match home.ground() {
            Ok(g) => g,
            Err(t) => return crate::sealed::refused(&t),
        };
        let chains = match home_chains(a, &ground, home.machine_settings()) {
            Ok(c) => c,
            Err(answer) => return answer,
        };
        let table = table_for(&chains, &home.items, &home.checked());
        let key = zikaron::cryptox::address_of_privkey(&home.key).unwrap_or([0; 20]);
        let sent = match home.read_queue() {
            Ok(q) => q.sent_ids(),
            Err(t) => return crate::sealed::refused(&t),
        };
        let body = match gated(&w, home.items.clone(), table, &key, &chains, sent) {
            Ok(b) => b,
            Err(answer) => return answer,
        };
        return verbs::seal_and_append(verbs::Landing::Sealed(&home, now, verbs::Then::Nothing), &home.items, &home.author.clone(), entry_type, seq, prev.as_deref(), body, &home.key, a.one("root").as_deref());
    }
    let key = a.key("key");
    let Some(author) = crate::entry::address(&key) else { return out::denied(Reason::Key, vec![]) };
    let dir = match crate::ledger::open(&at) {
        Ok(d) => d,
        Err(t) => return verbs::ledger_trouble(&t),
    };
    let items = match crate::ledger::pile(&dir) {
        Ok(x) => x,
        Err(t) => return verbs::ledger_trouble(&t),
    };
    let (seq, prev) = match verbs::next_link(a, &items) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
    let chains = given_chains(a);
    let facts = crate::hopin::bare_record();
    let facts: &dyn zikaron_send::Facts = match &facts {
        Some(f) => f,
        None => &zikaron_send::Bare,
    };
    let table = table_for(&chains, &items, facts);
    let signer = zikaron::cryptox::address_of_privkey(&key).unwrap_or([0; 20]);
    let sent = folder_sent(&chains, &items, &table, &signer);
    let body = match gated(&w, items.clone(), table, &signer, &chains, sent) {
        Ok(b) => b,
        Err(answer) => return answer,
    };
    verbs::seal_and_append(verbs::Landing::Dir(&dir), &items, &author, entry_type, seq, prev.as_deref(), body, &key, a.one("root").as_deref())
}
