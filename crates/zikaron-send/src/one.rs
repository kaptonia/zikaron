use crate::node::{self, Endpoint, NodeAddr};
use crate::{gate, Code, Trouble, Why};
use zikaron::json::Value;
use zikaron_anchor::rpc;
use zikaron_anchor::send::Fees;

pub trait PlainSigner {
    fn from(&self) -> Option<[u8; 20]>;
    fn sign_plain(&self, chain: u64, to: [u8; 20], value: u128, data: &[u8], fees: Fees, nonce: u64) -> Result<(Vec<u8>, [u8; 32]), rpc::Trouble>;
    fn seal_at(&self, dir: &std::path::Path) -> Option<crate::sent::Seal>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub chain: u64,
    pub to: [u8; 20],
    pub value: u128,
    pub data: Vec<u8>,
    pub again: Again,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Again {
    Same,
    Another,
}

impl Call {
    pub fn transfer(chain: u64, to: [u8; 20], value: u128) -> Call {
        Call { chain, to, value, data: Vec::new(), again: Again::Another }
    }

    pub fn book(chain: u64, at: [u8; 20], call: zikaron_anchor::book::Call, who: &[u8; 20]) -> Call {
        let again = if call == zikaron_anchor::book::Call::Accede { Again::Same } else { Again::Another };
        Call { chain, to: at, value: 0, data: call.data(who), again }
    }

    pub fn notice(chain: u64, at: [u8; 20], entry: &[u8; 32], grant: &[u8; 32]) -> Call {
        Call { chain, to: at, value: 0, data: zikaron_anchor::revoke::calldata(entry, grant), again: Again::Same }
    }

    pub fn same_once_included(&self) -> bool {
        self.again == Again::Same
    }

    fn asked(&self, from: &[u8; 20]) -> Value {
        let mut m = vec![("from".to_string(), Value::Str(zikaron::hexfmt::encode(from))), ("to".to_string(), Value::Str(zikaron::hexfmt::encode(&self.to)))];
        if !self.data.is_empty() {
            m.insert(0, ("data".to_string(), Value::Str(zikaron::hexfmt::encode(&self.data))));
        }
        if self.value > 0 {
            m.push(("value".to_string(), Value::Str(format!("0x{:x}", self.value))));
        }
        Value::Obj(m)
    }
}

pub fn estimate(eps: &[Endpoint], from: &[u8; 20], call: &Call) -> Result<(u64, Fees), Trouble> {
    let _read = node::OneRead::open();
    use zikaron_anchor::send::NoGas;
    let asked = call.asked(from);
    node::ask_ahead(eps, call.chain, &|_, lowest, _| {
        let at = Value::Str(format!("0x{lowest:x}"));
        vec![
            ("eth_estimateGas", Value::Arr(vec![asked.clone(), at.clone()])),
            ("eth_getBlockByNumber", Value::Arr(vec![at, Value::Bool(false)])),
            ("eth_feeHistory", zikaron_anchor::send::tip_params(lowest)),
        ]
    });
    let (eps, left) = crate::batch::serving_eps(eps, call.chain)?;
    let eps = &eps;
    let gas = zikaron_anchor::send::estimate_plain_gas(
        || node::head_block(eps, call.chain).map(|(h, _)| h).map_err(|t| (t, false)),
        |params| node::ask_call(eps, "eth_estimateGas", params).map(|r| r.value),
        asked,
        |(_, about): &(Trouble, bool)| !about,
    )
    .map_err(|no| match no {
        NoGas::Network((t, _)) => crate::batch::with_left(t, &left),
        NoGas::Refused((t, _)) => Trouble::new(Code::GasRefused, t.why),
        NoGas::NotText(v) => Trouble::new(Code::ChainShape, Why::GasNotText(format!("{v:?}"))),
        NoGas::Unreadable(h) => Trouble::new(Code::ChainShape, Why::GasNotQty(h)),
        NoGas::OverCap(n) => Trouble::text(Code::GasRefused, format!("{n} > {}", zikaron_anchor::send::PLAIN_GAS_MAX)),
    })?;
    let (fees, _) = node::fees(eps, call.chain);
    Ok((gas, fees.with_plain_estimate(gas)))
}

pub const DATA_FEE_ORACLE: [u8; 20] = [0x42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0f];

pub fn data_fee(eps: &[Endpoint], tx: &zikaron_anchor::send::Unsigned) -> Result<u128, Trouble> {
    let shape = |w: &Value| Trouble::new(Code::ChainShape, Why::Text(format!("{w:?}")));
    let at = zikaron::hexfmt::encode(&DATA_FEE_ORACLE);
    let code = node::ask(eps, "eth_getCode", &Value::Arr(vec![Value::Str(at), Value::Str("latest".into())]))?;
    let code = code.value.as_str().and_then(zikaron::hexfmt::decode).ok_or_else(|| shape(&code.value))?;
    if code.is_empty() {
        return Ok(0);
    }
    let bytes = tx.payload();
    let word = |n: usize| {
        let mut w = [0u8; 32];
        w[24..].copy_from_slice(&(n as u64).to_be_bytes());
        w
    };
    let mut data = zikaron::cryptox::keccak256(b"getL1Fee(bytes)")[..4].to_vec();
    data.extend_from_slice(&word(32));
    data.extend_from_slice(&word(bytes.len()));
    data.extend_from_slice(&bytes);
    data.resize(4 + 64 + bytes.len().div_ceil(32) * 32, 0);
    let fee = node::ask(eps, "eth_call", &zikaron_anchor::book::call_params(&DATA_FEE_ORACLE, &data, "latest"))?;
    let answered = fee.value.as_str().and_then(zikaron::hexfmt::decode).filter(|w| w.len() == 32 && w[..16].iter().all(|b| *b == 0));
    answered.map(|w| u128::from_be_bytes(w[16..].try_into().unwrap_or([0; 16]))).ok_or_else(|| shape(&fee.value))
}

#[derive(Clone, Copy, Debug)]
pub enum Through<'a> {
    Gate(&'a gate::Pass),
    Folder,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fell {
    pub tx: String,
    pub block: u64,
    pub status: u64,
    pub gas_used: Option<u64>,
}

pub fn send(_through: Through, urls: &[NodeAddr], call: &Call, signer: &dyn PlainSigner, fees: Fees, wait: std::time::Duration) -> Result<Fell, Trouble> {
    let from = signer.from().ok_or_else(|| Trouble::new(Code::KeyMalformed, Why::NoAddress))?;
    let (urls, _) = crate::batch::serving_urls(urls, call.chain)?;
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(call.chain, u.clone())).collect();
    let first = urls.first().map(NodeAddr::for_transport).unwrap_or_default().to_string();
    let unread = |said: String| Trouble::new(Code::Unreachable, Why::Text(said));
    let dir = crate::sent::machine().map(|m| crate::sent::place(&m, call.chain, &from, &call.to, &call.data)).map_err(unread)?;
    let seal = signer.seal_at(&dir).ok_or_else(|| Trouble::new(Code::KeyMalformed, Why::NoAddress))?;
    let before = crate::sent::read(&dir, &seal).map_err(unread)?;
    let mut at_nonce: Option<u64> = None;
    if !before.is_empty() {
        let mut ep = node::endpoint_at(&first).ok_or_else(|| Trouble::new(Code::Unreachable, Why::NoNodes))?;
        match zikaron_anchor::send::earlier(ep.as_mut(), &from, &before) {
            zikaron_anchor::send::Earlier::Included { tx, status, block_number } if status != 1 || call.same_once_included() => return reverted_void(&dir, &seal, Ok(fell(&urls, &tx, status, block_number))),
            zikaron_anchor::send::Earlier::Included { .. } => crate::sent::void(&dir, &seal, &before).map_err(unread)?,
            zikaron_anchor::send::Earlier::Held { tx } => return reverted_void(&dir, &seal, confirmed(&urls, &tx, wait)),
            zikaron_anchor::send::Earlier::Unread { tx, said } => return Err(Trouble::text(Code::Unreachable, format!("{} {said}", zikaron::hexfmt::encode(&tx)))),
            zikaron_anchor::send::Earlier::Unused { nonce } => at_nonce = Some(nonce),
            zikaron_anchor::send::Earlier::Void => crate::sent::void(&dir, &seal, &before).map_err(unread)?,
        }
    }
    let (have, _) = node::balance(&eps, call.chain, &from)?;
    let need = fees.cap_wei().saturating_add(call.value);
    crate::batch::funds_check(need, have)?;
    let nonce = match at_nonce {
        Some(n) => n,
        None => {
            let r = node::ask(&eps, "eth_getTransactionCount", &zikaron_anchor::send::nonce_params(&from))?;
            r.value.as_str().and_then(node::wei).and_then(|n| u64::try_from(n).ok()).ok_or_else(|| Trouble::new(Code::Unreachable, Why::NonceUnread { node: urls.first().map(NodeAddr::said).unwrap_or_default() }))?
        }
    };
    let (raw, hash) = signer.sign_plain(call.chain, call.to, call.value, &call.data, fees, nonce).map_err(|t| node::said_trouble(urls.first().map(NodeAddr::for_transport).unwrap_or_default(), &t))?;
    crate::sent::land(&dir, &seal, nonce, &hash).map_err(unread)?;
    let mut sent = false;
    let mut last: Option<Trouble> = None;
    for u in &urls {
        let Some(mut ep) = node::endpoint_at(u.for_transport()) else { continue };
        match zikaron_anchor::send::submit(ep.as_mut(), &raw, &hash) {
            Ok(()) => {
                sent = true;
                break;
            }
            Err(e) => {
                let t = node::said_trouble(u.for_transport(), &e);
                if node::already_taken(&t) && node::tx_known(&urls, call.chain, &hash) {
                    sent = true;
                    break;
                }
                if node::next_after(&t) == node::Next::Stop {
                    return Err(t);
                }
                last = Some(t);
            }
        }
    }
    if !sent && !node::tx_known(&urls, call.chain, &hash) {
        return Err(last.unwrap_or_else(|| Trouble::new(Code::Unreachable, Why::NoNodes)));
    }
    reverted_void(&dir, &seal, confirmed(&urls, &hash, wait))
}

fn reverted_void(dir: &std::path::Path, seal: &crate::sent::Seal, f: Result<Fell, Trouble>) -> Result<Fell, Trouble> {
    if matches!(&f, Ok(x) if x.status != 1) {
        let _ = crate::sent::read(dir, seal).and_then(|held| crate::sent::void(dir, seal, &held));
    }
    f
}

fn confirmed(urls: &[NodeAddr], hash: &[u8; 32], wait: std::time::Duration) -> Result<Fell, Trouble> {
    let mut opened: Vec<Box<dyn rpc::Endpoint + Send>> = urls.iter().filter_map(|u| node::endpoint_at(u.for_transport())).collect();
    let mut refs: Vec<&mut dyn rpc::Endpoint> = opened.iter_mut().map(|b| b.as_mut() as &mut dyn rpc::Endpoint).collect();
    match zikaron_anchor::send::confirm_each(&mut refs, hash, wait, &node::SEND_BACKOFF) {
        zikaron_anchor::send::Confirm::Included { status, block_number } => Ok(fell(urls, hash, status, block_number)),
        _ => Err(Trouble::text(Code::NotIncluded, zikaron::hexfmt::encode(hash))),
    }
}

fn fell(urls: &[NodeAddr], hash: &[u8; 32], status: u64, block: u64) -> Fell {
    let tx = zikaron::hexfmt::encode(hash);
    let gas_used = urls.iter().find_map(|u| {
        let mut ep = node::endpoint_at(u.for_transport())?;
        let w = zikaron_anchor::patience::ask(ep.as_mut(), "eth_getTransactionReceipt", &Value::Arr(vec![Value::Str(tx.clone())])).ok()?;
        w.member("gasUsed").and_then(|g| g.as_str()).and_then(node::wei).and_then(|n| u64::try_from(n).ok())
    });
    Fell { tx, block, status, gas_used }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_landed_transfer_is_not_the_next_one() {
        assert!(!Call::transfer(1, [0x22; 20], 5).same_once_included(), "a payment each time");
        assert!(!Call::book(1, [0x33; 20], zikaron_anchor::book::Call::Declare, &[0x44; 20]).same_once_included(), "a declaration names the pending key again");
        assert!(Call::book(1, [0x33; 20], zikaron_anchor::book::Call::Accede, &[0x44; 20]).same_once_included(), "the seat is taken once");
        assert!(Call::notice(1, [0x55; 20], &[1; 32], &[2; 32]).same_once_included(), "a notice is kept once done");
    }
}
