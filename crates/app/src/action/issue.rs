use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub tx: String,
    pub back: String,
    pub withdrawn: bool,
}

fn seat_here(shell: &Shell) -> Result<zikaron_send::issue::Seat, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let home = shell.home.as_ref().ok_or_else(|| Fault::known(Known::NoHome, String::new()))?;
    let pile = home.ledger()?.pile()?.items;
    zikaron_send::issue::seat_of(&pile).ok_or_else(|| Fault::known(Known::NoGenesis, String::new()))
}

fn issue_refused() -> crate::fault::Owned {
    crate::fault::Owned::Reason(zikaron_glue::door::Own::IssueRefused)
}

fn send_refused() -> crate::fault::Owned {
    crate::fault::Owned::Reason(zikaron_glue::door::Own::SendRefused)
}

fn nodes_of(shell: &Shell, chain: u64) -> Vec<crate::chainx::Endpoint> {
    shell.endpoints.iter().filter(|e| e.chain == chain).cloned().collect()
}

pub(super) fn grant_first(shell: &mut Shell, a: Action) -> Applied {
    use crate::fault::{Fault, Known};
    let Action::DraftGrant { draft, terms_file, .. } = &a else { return shell.trouble(Fault::unknown("grant")) };
    if let Err(f) = shell.may_write_entries() {
        return shell.trouble(f);
    }
    let waiting = match grant_rules(shell, draft, terms_file.as_deref(), true) {
        Ok(w) => w,
        Err(f) => return shell.trouble(f),
    };
    let Some(chain) = shell.settings.chain_id else { return shell.trouble(Fault::known(Known::NoChainId, String::new()).owned(issue_refused())) };
    let settle = match settle_of(shell, &waiting) {
        Ok(s) => s,
        Err(f) => return shell.trouble(f),
    };
    check_first_settling(shell, a, chain, settle)
}

pub(super) fn revoke_first(shell: &mut Shell, grant: &str, case: &str) -> Applied {
    if let Err(f) = shell.may_write_entries() {
        return shell.trouble(f);
    }
    if let Err(f) = revoke_rules(shell, grant) {
        return shell.trouble(f);
    }
    let pair = match pair_of(shell, grant) {
        Ok(p) => p,
        Err(f) => return shell.trouble(f),
    };
    match pair {
        None => match revoke(shell, grant, case) {
            Ok((id, n)) => Applied::Revoked { id, queued: n.queued, next: n.next, notice: None },
            Err(f) => shell.trouble(f),
        },
        Some(p) => check_first(shell, Action::Revoke { grant: grant.trim().to_string(), case: case.to_string() }, p.member.chain),
    }
}

fn pair_of(shell: &Shell, grant: &str) -> Result<Option<zikaron_send::withdraw::Pair>, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    Ok(zikaron_send::withdraw::pair(&home.ledger()?.pile()?.items, grant))
}

struct Settle {
    pile: Vec<Vec<u8>>,
    clash: Vec<String>,
    waiting: Vec<String>,
    chains: Vec<(u64, Vec<crate::chainx::Endpoint>)>,
}

fn settle_of(shell: &Shell, clash: &[String]) -> Result<Option<Settle>, crate::fault::Fault> {
    if clash.is_empty() {
        return Ok(None);
    }
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let pile = home.ledger()?.pile()?.items;
    let waiting = zikaron_send::withdraw::unsettled(&pile, clash);
    let mut chains: Vec<u64> = waiting.iter().filter_map(|g| zikaron_send::withdraw::member_chain(&pile, g)).collect();
    chains.sort_unstable();
    chains.dedup();
    let chains = chains.into_iter().map(|c| (c, nodes_of(shell, c))).collect();
    Ok(Some(Settle { pile, clash: clash.to_vec(), waiting, chains }))
}

fn check_first(shell: &mut Shell, then: Action, chain: u64) -> Applied {
    check_first_settling(shell, then, chain, None)
}

fn check_first_settling(shell: &mut Shell, then: Action, chain: u64, settle: Option<Settle>) -> Applied {
    let seat = match seat_here(shell) {
        Ok(s) => s,
        Err(f) => return shell.trouble(f),
    };
    let eps = nodes_of(shell, chain);
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf());
    let sealed = matches!(&shell.persona, crate::facex::State::Read(b) if zikaron_glue::facegate::sealed(&b.table).is_err())
        || shell.home.as_ref().and_then(|h| h.ledger().ok()).and_then(|l| l.pile().ok()).is_some_and(|p| zikaron_glue::facegate::written_reroot(&p.items).is_some());
    match shell.tasks.spawn(Kind::Gate, move || {
        crate::chainx::speak();
        if let Some(st) = settle {
            let eps: Vec<crate::chainx::Endpoint> = st.chains.iter().flat_map(|(c, eps)| eps.iter().filter(|e| e.chain == *c).cloned()).collect();
            let unread = zikaron_send::withdraw::read_back(&eps, &st.pile, &st.waiting, &crate::sendx::record::noticed(), crate::sendx::record::add_noticed)?;
            let standing = zikaron_send::withdraw::standing(&st.pile, &crate::sendx::record::noticed());
            let still: Vec<String> = st.clash.iter().filter(|g| zikaron_send::withdraw::written(&standing, g).is_none()).cloned().collect();
            if let zikaron_seal::terms::Withheld::Undecided(ids) = zikaron_send::withdraw::undecided(zikaron_seal::terms::Withheld::Clash(still), &st.pile, &unread) {
                return Err(super::grant::undecided(&ids));
            }
        }
        let member = zikaron_send::issue::check(&eps, chain, &seat).map_err(|t| {
            let f = crate::fault::Fault::from(t).owned(issue_refused());
            if sealed && f.which() == Some(crate::fault::Known::RootNotInBook) {
                f.worded(None, Some(crate::lang::Key::FaultNextRootNotInBookSealed))
            } else {
                f
            }
        })?;
        Ok(Done::Issued { root, then: Box::new(then), member })
    }) {
        Spawned::Started => Applied::Started(Kind::Gate),
        Spawned::InFlight => Applied::Refused(Kind::Gate),
    }
}

pub fn issued(shell: &mut Shell, root: Option<std::path::PathBuf>, then: Action, member: zikaron_send::issue::Member, stale: bool) -> Option<Applied> {
    let here = shell.home.as_ref().map(|h| h.root().to_path_buf());
    if stale || here != root {
        if shell.tasks.in_flight(Kind::Gate) {
            return None;
        }
        return Some(apply(shell, then));
    }
    trace::mark(then.feature());
    if let Some(k) = held_back(shell, &then) {
        return Some(shell.trouble(crate::fault::Fault::known(k, String::new())));
    }
    Some(match then {
        Action::DraftGrant { draft, exclusive, terms_file } => match grant_rules(shell, &draft, terms_file.as_deref(), false).and_then(|_| sign_grant(shell, &draft, exclusive, terms_file.as_deref(), member)) {
            Ok((id, q)) => Applied::Granted { id, queued: q.queued, next: q.next },
            Err(f) => shell.trouble(f),
        },
        Action::MigrateGrants => match migrate(shell, member) {
            Ok((count, q)) => Applied::GrantsMigrated { count, queued: q.queued, next: q.next },
            Err(f) => shell.trouble(f),
        },
        Action::Revoke { grant, case } => match notice_first(shell, &grant, &case) {
            Ok(a) => a,
            Err(f) => shell.trouble(f),
        },
        other => apply(shell, other),
    })
}

fn notice_first(shell: &mut Shell, grant: &str, case: &str) -> Result<Applied, crate::fault::Fault> {
    let pair = pair_of(shell, grant)?.ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::SubjectMissing, grant.to_string()))?;
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let pile = home.ledger()?.pile()?.items;
    let ledger = home.dir(crate::home::Slot::Ledger).display().to_string();
    let written = |of: &str| zikaron_send::withdraw::written(&pile, of);
    let member_id = zikaron::hexfmt::encode(&zikaron::cryptox::sha256(&pair.grant));
    if let Some(old) = pair.old.as_deref() {
        if written(old).is_none() {
            revoke(shell, old, case)?;
        }
    }
    let entry = match written(&member_id) {
        Some(id) => id,
        None => seal_revocation(shell, &member_id, case)?,
    };
    let entry_b: [u8; 32] = zikaron::hexfmt::decode(&entry).and_then(|b| b.try_into().ok()).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::ContentShape, entry.clone()))?;
    let secret = signing_key(shell, crate::sign::Use::Anchor)?;
    let ask = crate::exitgate::ask_of(shell).map_err(|f| f.owned(send_refused()))?;
    let chain = pair.member.chain;
    let eps = nodes_of(shell, chain);
    let urls: Vec<crate::chainx::NodeAddr> = eps.iter().map(|e| e.url.clone()).collect();
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf());
    let g = pair.grant;
    let grant_id = member_id;
    match shell.tasks.spawn(Kind::Gate, move || {
        crate::chainx::speak();
        let pass = crate::exitgate::pass(&ask).map_err(|f| f.owned(send_refused()))?;
        let noticed = zikaron_send::withdraw::notice(zikaron_send::one::Through::Gate(&pass), &eps, &urls, chain, &g, &entry_b, &secret, std::time::Duration::from_secs(60)).map_err(|t| {
            let included = t.code != zikaron_send::Code::NotIncluded;
            let f = crate::fault::Fault::from(t);
            if included {
                f.owned(send_refused())
            } else {
                f.owned(crate::fault::Owned::NotIncluded { entry: entry.clone(), ledger: ledger.clone() })
            }
        })?;
        Ok(Done::Withdrew { root, entry, grant: grant_id, chain, noticed })
    }) {
        Spawned::Started => Ok(Applied::Started(Kind::Gate)),
        Spawned::InFlight => Ok(Applied::Refused(Kind::Gate)),
    }
}

pub fn withdrew(shell: &mut Shell, root: Option<std::path::PathBuf>, entry: String, grant: String, chain: u64, noticed: zikaron_send::withdraw::Noticed) -> Option<Applied> {
    let here = shell.home.as_ref().map(|h| h.root().to_path_buf());
    if here != root {
        return None;
    }
    let withdrawn = noticed.back == zikaron_send::withdraw::Back::Counted;
    let h32 = |s: &str| -> Option<[u8; 32]> { zikaron::hexfmt::decode(s)?.try_into().ok() };
    if noticed.fell.status == 1 {
        if let Some(home) = shell.home.as_ref() {
            let at = zikaron_send::keep::Inclusion { tx: noticed.fell.tx.clone(), chain, block: noticed.fell.block };
            match crate::queue::settle(home, std::slice::from_ref(&entry), true, Some(&at)) {
                Ok((_, q)) => shell.queue = q,
                Err(f) => shell.faults.push(f),
            }
        }
    }
    if withdrawn {
        if let (Some(g), Some(tx)) = (h32(&grant), h32(&noticed.fell.tx)) {
            if let Err(f) = crate::sendx::record::add_noticed(zikaron_send::checked::Notice { grant: g, chain, tx, block: noticed.fell.block }) {
                shell.faults.push(f);
            }
        }
    }
    shell.reread(crate::shell::Reread::WroteGrant);
    Some(Applied::Revoked { id: entry, queued: shell.queue.len(), next: Next::Held, notice: Some(Notice { tx: noticed.fell.tx, back: noticed.back.code().to_string(), withdrawn }) })
}

fn migrate(shell: &mut Shell, member: zikaron_send::issue::Member) -> Result<(usize, Enqueued), crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let olds = migratable(home, shell.chain_now())?;
    let mut last = Enqueued { queued: shell.queue.len(), next: Next::Held };
    let mut count = 0usize;
    for (old, body, exclusive) in olds {
        let d = draft_again(&old, &body);
        grant_rules(shell, &d, None, false)?;
        let (_, q) = sign_grant(shell, &d, exclusive, None, member)?;
        last = q;
        count += 1;
    }
    Ok((count, last))
}

fn draft_again(old: &str, body: &zikaron::json::Value) -> crate::grantx::Draft {
    let s = |k: &str| body.member(k).and_then(zikaron::json::Value::as_str).unwrap_or_default().to_string();
    let (from, to) = crate::grantx::window_of(body).map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or_default();
    crate::grantx::Draft { grantee: s("grantee"), work: s("work"), terms: s("terms"), history: s("history"), from, to, scope_md: s("scope_md"), upstream: s("upstream"), supersedes: old.to_string(), ..Default::default() }
}

pub fn migratable(home: &crate::home::Home, now: Option<u64>) -> Result<Vec<(String, zikaron::json::Value, bool)>, crate::fault::Fault> {
    let rows = crate::grantx::table(home)?;
    let pile = home.ledger()?.pile()?.items;
    Ok(pile
        .iter()
        .filter_map(|b| zikaron_glue::book::check(b).ok())
        .filter_map(|e| {
            let id = zikaron::hexfmt::encode(&e.id);
            let r = rows.iter().find(|r| r.id == id)?;
            r.migratable(now).then(|| (id, e.body.clone(), r.exclusive))
        })
        .collect())
}

pub(super) fn migrate_first(shell: &mut Shell) -> Applied {
    use crate::fault::{Fault, Known};
    if let Err(f) = shell.may_write_entries() {
        return shell.trouble(f);
    }
    let Some(home) = shell.home.as_ref() else { return shell.trouble(Fault::known(Known::NoHome, String::new())) };
    let olds = match migratable(home, shell.chain_now()) {
        Ok(v) if v.is_empty() => return Applied::GrantsMigrated { count: 0, queued: shell.queue.len(), next: Next::Held },
        Ok(v) => v,
        Err(f) => return shell.trouble(f),
    };
    for (old, body, _) in &olds {
        if let Err(f) = grant_rules(shell, &draft_again(old, body), None, false) {
            return shell.trouble(f);
        }
    }
    let Some(chain) = shell.settings.chain_id else { return shell.trouble(Fault::known(Known::NoChainId, String::new())) };
    check_first(shell, Action::MigrateGrants, chain)
}
