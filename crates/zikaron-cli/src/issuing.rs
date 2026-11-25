use crate::args::Args;
use crate::codes::{Key, Reason};
use crate::out::{self, Answer};
use zikaron::json::Value;
use zikaron_send::issue::{Member, Seat};
use zikaron_send::node::Endpoint;
use zikaron_send::{Code, Trouble};

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

pub enum Road<'a> {
    Folder,
    Home(&'a crate::sealed::Opened),
}

pub fn refused(t: &Trouble) -> Answer {
    refused_named(t.code.as_str(), &crate::sealedsend::said(&t.why), t.network())
}

pub fn refused_named(code: &str, detail: &str, network: bool) -> Answer {
    crate::named::of(zikaron_glue::door::Own::IssueRefused, code, detail, network, "")
}

fn given(a: &Args) -> Vec<Endpoint> {
    let given = a.many_at("endpoint");
    if given.is_empty() {
        return Vec::new();
    }
    crate::chain::endpoint_specs(&given).iter().map(|(c, u)| Endpoint::at(*c, zikaron_send::node::NodeAddr::new(u.as_str()))).collect()
}

pub fn venue(a: &Args, road: &Road, want: Option<u64>) -> Result<(u64, Vec<Endpoint>), Answer> {
    let eps = given(a);
    let mut chains: Vec<u64> = eps.iter().map(|e| e.chain).collect();
    chains.sort_unstable();
    chains.dedup();
    if chains.len() > 1 {
        out::misuse(Reason::Args, out::Subject::flag("endpoint"), out::Said::OneChainNodes);
    }
    match road {
        Road::Folder => {
            crate::verbs::proxy_from(a);
            let Some(&chain) = chains.first() else { return Err(refused(&Trouble::text(Code::NoEndpoint, want.map(|c| c.to_string()).unwrap_or_default()))) };
            match want {
                Some(w) if w != chain => Err(refused(&Trouble::text(Code::NoEndpoint, w.to_string()))),
                _ => Ok((chain, eps)),
            }
        }
        Road::Home(home) => {
            let ground = home.ground().map_err(|t| crate::sealed::refused(&t))?;
            let cli = home.machine_settings().map_err(|t| crate::sealed::refused(&t))?;
            zikaron_send::node::take_machine_route(cli.proxy.as_deref());
            if a.one("proxy").is_some() || a.one("rest-keys").is_some() {
                crate::verbs::proxy_from(a);
            }
            let chain = match want.or(ground.chain_id) {
                Some(c) => c,
                None => return Err(refused(&Trouble::text(Code::NoChainId, home.home().display().to_string()))),
            };
            if !eps.is_empty() {
                return if chains.first() == Some(&chain) { Ok((chain, eps)) } else { Err(refused(&Trouble::text(Code::NoEndpoint, chain.to_string()))) };
            }
            Ok((chain, crate::sealedsend::nodes_of(&ground, chain)))
        }
    }
}

fn seat(items: &[Vec<u8>]) -> Result<Seat, Answer> {
    zikaron_send::issue::seat_of(items).ok_or_else(|| out::denied(Reason::TipAbsent, vec![]))
}

pub fn member(a: &Args, road: &Road, items: &[Vec<u8>]) -> Result<Member, Answer> {
    let seat = seat(items)?;
    let (chain, eps) = venue(a, road, None)?;
    zikaron_send::issue::check(&eps, chain, &seat).map_err(|t| refused(&t))
}

#[allow(clippy::too_many_arguments)]
pub fn revoke(
    a: &Args,
    road: &Road,
    items: &[Vec<u8>],
    pair: zikaron_send::withdraw::Pair,
    case: Option<String>,
    key: &[u8; 32],
    link: (u64, Option<String>),
    land: &mut dyn FnMut(bool, Value, u64, Option<&str>) -> Result<(String, String, bool), Answer>,
) -> Answer {
    let seat = match seat(items) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
    let (chain, eps) = match venue(a, road, Some(pair.member.chain)) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
    if let Err(t) = zikaron_send::issue::check(&eps, chain, &seat) {
        return refused(&t);
    }
    let body = |grant: &str| {
        let mut m = vec![("grant".to_string(), s(grant))];
        if let Some(c) = &case {
            m.push(("case".to_string(), s(c)));
        }
        m.sort_by(|x, y| x.0.cmp(&y.0));
        Value::Obj(m)
    };
    let (mut seq, mut prev) = link;
    if let Some(old) = pair.old.as_deref() {
        if zikaron_send::withdraw::written(items, old).is_none() {
            match land(false, body(old), seq, prev.as_deref()) {
                Ok((id, _, _)) => {
                    seq += 1;
                    prev = Some(id);
                }
                Err(answer) => return answer,
            }
        }
    }
    let member_id = zikaron::hexfmt::encode(&zikaron::cryptox::sha256(&pair.grant));
    let (entry, ledger, written) = match zikaron_send::withdraw::written(items, &member_id) {
        Some(id) => (id, ledger_of(road, a), false),
        None => match land(true, body(&member_id), seq, prev.as_deref()) {
            Ok(x) => x,
            Err(answer) => return answer,
        },
    };
    let Some(entry_b) = zikaron::hexfmt::decode(&entry).and_then(|b| <[u8; 32]>::try_from(b).ok()) else { return refused(&Trouble::text(Code::ContentShape, entry)) };
    let pass;
    let through = match road {
        Road::Folder => zikaron_send::one::Through::Folder,
        Road::Home(home) => {
            pass = match crate::sealedsend::notice_pass(home) {
                Ok(p) => p,
                Err(answer) => return answer,
            };
            zikaron_send::one::Through::Gate(&pass)
        }
    };
    let urls: Vec<zikaron_send::node::NodeAddr> = eps.iter().filter(|e| e.chain == chain).map(|e| e.url.clone()).collect();
    if let (Road::Folder, Ok(m), Some(at), Some(from)) = (road, zikaron_send::sent::machine(), zikaron_anchor::revoke::at(chain), zikaron::cryptox::address_of_privkey(key)) {
        zikaron_anchor::action::keep_in(std::sync::Arc::new(zikaron_send::sent::logs_at(&m, chain, &from, &at, key)));
    }
    let signer = Signing(*key);
    let noticed = match zikaron_send::withdraw::notice(through, &eps, &urls, chain, &pair.grant, &entry_b, &signer, std::time::Duration::from_secs(a.u64_of("wait-secs").unwrap_or(90))) {
        Ok(n) => n,
        Err(t) if t.code == Code::NotIncluded => return not_noticed(&entry, Some(&crate::sealedsend::said(&t.why)), zikaron_send::withdraw::NOT_INCLUDED),
        Err(t) => return crate::sealedsend::send_refused(t.code.as_str(), &crate::sealedsend::said(&t.why), t.network(), out::Said::SendRefused.text().to_string()),
    };
    if noticed.back != zikaron_send::withdraw::Back::Counted {
        return not_noticed(&entry, Some(&noticed.fell.tx), noticed.back.code());
    }
    if let Road::Home(home) = road {
        let _ = keep_noticed(home, &member_id, chain, &noticed.fell);
        let _ = home.amend_queue(|q| q.included_out(std::slice::from_ref(&entry), &noticed.fell.tx, chain, noticed.fell.block));
    }
    let _ = written;
    noticed_answer(&entry, &ledger, &noticed.fell.tx, noticed.back.code(), true)
}

pub fn noticed_answer(entry: &str, ledger: &str, tx: &str, notice: &str, withdrawn: bool) -> Answer {
    if !withdrawn {
        return not_noticed(entry, Some(tx).filter(|t| !t.is_empty()), notice);
    }
    out::affirmed(vec![(Key::EntryId, s(entry)), (Key::Ledger, s(ledger)), (Key::Notice, s(notice)), (Key::Tx, s(tx))])
}

fn ledger_of(road: &Road, a: &Args) -> String {
    match road {
        Road::Home(home) => home.ledger_root(),
        Road::Folder => a.one("ledger").unwrap_or_default(),
    }
}

fn not_noticed(entry: &str, tx: Option<&str>, notice: &str) -> Answer {
    let mut m = vec![(Key::EntryId, s(entry)), (Key::Notice, s(notice))];
    if let Some(t) = tx {
        m.push((Key::Tx, s(t)));
    }
    out::unanswered(Reason::NotNoticed, m)
}

fn keep_noticed(home: &crate::sealed::Opened, grant: &str, chain: u64, fell: &zikaron_send::one::Fell) -> Option<bool> {
    let g = zikaron::hexfmt::decode(grant)?.try_into().ok()?;
    let tx = zikaron::hexfmt::decode(&fell.tx)?.try_into().ok()?;
    zikaron_send::checked::add_noticed(&home.checked().0, zikaron_send::checked::Notice { grant: g, chain, tx, block: fell.block }).ok()
}

struct Signing([u8; 32]);

impl zikaron_send::one::PlainSigner for Signing {
    fn from(&self) -> Option<[u8; 20]> {
        zikaron::cryptox::address_of_privkey(&self.0)
    }

    fn sign_plain(&self, chain: u64, to: [u8; 20], value: u128, data: &[u8], fees: zikaron_anchor::send::Fees, nonce: u64) -> Result<(Vec<u8>, [u8; 32]), zikaron_anchor::rpc::Trouble> {
        zikaron_anchor::send::sign_plain(&self.0, chain, to, value, data, fees, nonce)
    }

    fn seal_at(&self, dir: &std::path::Path) -> Option<zikaron_send::sent::Seal> {
        Some(zikaron_send::sent::Seal::of(&self.0, dir))
    }
}

impl Drop for Signing {
    fn drop(&mut self) {
        zikaron_os::wipe(&mut self.0);
    }
}
