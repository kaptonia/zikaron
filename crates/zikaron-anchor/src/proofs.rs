use crate::ahead::{Header, Want};
use crate::rlp;
use crate::rpc::Endpoint;
use crate::wire::W;
use zikaron::json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorProof {
    pub chain_id: u64,
    pub hash: [u8; 32],
    pub emitter: [u8; 20],
    pub header: Vec<u8>,
    pub parent: Option<Vec<u8>>,
    pub tx: Vec<u8>,
    pub tx_proof: Vec<Vec<u8>>,
    pub receipt: Vec<u8>,
    pub receipt_proof: Vec<Vec<u8>>,
    pub index: u64,
    pub sender_at: Option<Vec<Vec<u8>>>,
    pub sender_before: Option<Vec<Vec<u8>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gap {
    Chain,
    Header,
    Parent,
    Tx,
    TxRoot,
    ReceiptRoot,
    Receipt,
    AccountProof,
    Unbuilt,
}

#[derive(Clone, Debug)]
pub struct Proved {
    pub header: Header,
    pub parent: Option<([u8; 32], Header)>,
    pub tx: crate::tx::Tx,
    pub code_at: Option<bool>,
    pub code_before: Option<bool>,
}

struct Fields {
    header: Header,
    tx_root: [u8; 32],
    receipt_root: [u8; 32],
}

fn h32(item: &rlp::Item) -> Option<[u8; 32]> {
    item.bytes().and_then(|b| <[u8; 32]>::try_from(b).ok())
}

fn fields(rlp_bytes: &[u8]) -> Option<Fields> {
    let item = rlp::decode_all(rlp_bytes)?;
    let l = item.list().filter(|l| l.len() >= crate::ahead::HEADER_REQUIRED)?;
    Some(Fields {
        header: Header { number: l[8].u64()?, timestamp: l[11].u64()?, state_root: h32(&l[3])?, parent: h32(&l[0])? },
        tx_root: h32(&l[4])?,
        receipt_root: h32(&l[5])?,
    })
}

fn receipt_parts(bytes: &[u8]) -> Option<(Option<u64>, Vec<([u8; 20], Vec<[u8; 32]>, Vec<u8>)>)> {
    let body = match bytes.first() {
        Some(t) if *t < 0x80 => &bytes[1..],
        _ => bytes,
    };
    let item = rlp::decode_all(body)?;
    let l = item.list().filter(|l| l.len() == 4)?;
    let status = l[0].bytes().filter(|b| b.len() <= 1).map(|b| b.first().copied().unwrap_or(0) as u64);
    let logs = l[3]
        .list()?
        .iter()
        .map(|log| {
            let p = log.list().filter(|p| p.len() == 3)?;
            let at = <[u8; 20]>::try_from(p[0].bytes()?).ok()?;
            let topics: Option<Vec<[u8; 32]>> = p[1].list()?.iter().map(h32).collect();
            Some((at, topics?, p[2].bytes()?.to_vec()))
        })
        .collect::<Option<Vec<_>>>()?;
    Some((status, logs))
}

pub fn check(p: &AnchorProof, chain: u64, want: &Want) -> Result<Proved, Gap> {
    if p.chain_id != chain {
        return Err(Gap::Chain);
    }
    if zikaron::cryptox::keccak256(&p.header) != want.block {
        return Err(Gap::Header);
    }
    let f = fields(&p.header).filter(|f| f.header.number == want.number).ok_or(Gap::Header)?;
    let parent = match (&p.parent, f.header.number) {
        (None, 0) => None,
        (Some(bytes), n) if n > 0 => {
            if zikaron::cryptox::keccak256(bytes) != f.header.parent {
                return Err(Gap::Parent);
            }
            let pf = fields(bytes).filter(|pf| pf.header.number + 1 == n).ok_or(Gap::Parent)?;
            Some((f.header.parent, pf.header))
        }
        _ => return Err(Gap::Parent),
    };
    let tx = crate::tx::from_raw(&p.tx).ok_or(Gap::Tx)?;
    if tx.hash != want.tx || tx.chain_id != Some(chain) || tx.sender != Some(want.who) || tx.to.is_none() {
        return Err(Gap::Tx);
    }
    let key = crate::trie::index_key(p.index);
    if crate::ahead::verify(&f.tx_root, &key, &p.tx_proof) != Ok(Some(p.tx.clone())) {
        return Err(Gap::TxRoot);
    }
    if crate::ahead::verify(&f.receipt_root, &key, &p.receipt_proof) != Ok(Some(p.receipt.clone())) {
        return Err(Gap::ReceiptRoot);
    }
    let (status, logs) = receipt_parts(&p.receipt).ok_or(Gap::Receipt)?;
    let mut sender_topic = [0u8; 32];
    sender_topic[12..].copy_from_slice(&want.who);
    let form = [crate::scan::topic0(), sender_topic, p.hash];
    let anchored = logs.iter().any(|(at, topics, data)| *at == p.emitter && topics.as_slice() == form && data.is_empty());
    if status != Some(1) || !anchored || !crate::scan::calldata_carries(&tx.input, &p.hash) {
        return Err(Gap::Receipt);
    }
    let code = |root: &[u8; 32], proof: &Option<Vec<Vec<u8>>>| match proof {
        None => Ok(None),
        Some(nodes) => crate::ahead::code_in(root, &want.who, nodes).map(Some).map_err(|_| Gap::AccountProof),
    };
    let code_at = code(&f.header.state_root, &p.sender_at)?;
    let code_before = match &parent {
        Some((_, ph)) => code(&ph.state_root, &p.sender_before)?,
        None if p.sender_before.is_some() => return Err(Gap::AccountProof),
        None => None,
    };
    Ok(Proved { header: f.header, parent, tx, code_at, code_before })
}

pub fn receipt_bytes(r: &W) -> Option<Vec<u8>> {
    use crate::tx::{data, qty, qty_u64};
    let kind = qty_u64(r, "type").unwrap_or(0);
    let status = qty(r, "status")?;
    let logs: Vec<Vec<u8>> = r
        .member("logs")?
        .as_arr()?
        .iter()
        .map(|log| {
            let topics: Option<Vec<Vec<u8>>> = log.member("topics")?.as_arr()?.iter().map(|t| t.as_str().and_then(zikaron::hexfmt::decode).map(|b| rlp::bytes(&b))).collect();
            Some(rlp::list(&[rlp::bytes(&data(log, "address")?), rlp::list(&topics?), rlp::bytes(&data(log, "data")?)]))
        })
        .collect::<Option<Vec<_>>>()?;
    let body = rlp::list(&[rlp::scalar(&status), rlp::scalar(&qty(r, "cumulativeGasUsed")?), rlp::bytes(&data(r, "logsBloom")?), rlp::list(&logs)]);
    if kind == 0 {
        return Some(body);
    }
    let mut out = vec![u8::try_from(kind).ok()?];
    out.extend_from_slice(&body);
    Some(out)
}

fn account_proof(ep: &mut dyn Endpoint, who: &[u8; 20], number: u64) -> Option<Vec<Vec<u8>>> {
    let params = Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(who)), Value::Arr(Vec::new()), Value::Str(crate::scan::hex_quantity(number))]);
    let answer = ep.call("eth_getProof", &params).ok()?;
    answer.member("accountProof")?.as_arr()?.iter().map(|x| x.as_str().and_then(zikaron::hexfmt::decode)).collect()
}

#[allow(clippy::too_many_arguments)]
pub fn build(ep: &mut dyn Endpoint, chain: u64, block: &[u8; 32], tx: &[u8; 32], who: &[u8; 20], hash: &[u8; 32], emitter: &[u8; 20]) -> Result<AnchorProof, Gap> {
    let full = ep.call("eth_getBlockByHash", &Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(block)), Value::Bool(true)])).map_err(|_| Gap::Unbuilt)?;
    let header = crate::ahead::header_rlp(&full).filter(|h| zikaron::cryptox::keccak256(h) == *block).ok_or(Gap::Unbuilt)?;
    let f = fields(&header).ok_or(Gap::Unbuilt)?;
    let txs: Vec<crate::tx::Tx> = full
        .member("transactions")
        .and_then(W::as_arr)
        .ok_or(Gap::Unbuilt)?
        .iter()
        .map(|t| crate::tx::read(t).ok().filter(|t| !t.raw.is_empty()))
        .collect::<Option<Vec<_>>>()
        .ok_or(Gap::Unbuilt)?;
    let index = txs.iter().position(|t| t.hash == *tx).ok_or(Gap::Unbuilt)?;
    let raws: Vec<Vec<u8>> = txs.iter().map(|t| t.raw.clone()).collect();
    let (tx_root, tx_proof) = crate::trie::root_and_proof(&raws, index).ok_or(Gap::Unbuilt)?;
    if tx_root != f.tx_root {
        return Err(Gap::Unbuilt);
    }
    let receipts_answer = ep.call("eth_getBlockReceipts", &Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(block))])).map_err(|_| Gap::Unbuilt)?;
    let receipts: Vec<Vec<u8>> = receipts_answer.as_arr().ok_or(Gap::Unbuilt)?.iter().map(receipt_bytes).collect::<Option<Vec<_>>>().ok_or(Gap::Unbuilt)?;
    if receipts.len() != raws.len() {
        return Err(Gap::Unbuilt);
    }
    let (receipt_root, receipt_proof) = crate::trie::root_and_proof(&receipts, index).ok_or(Gap::Unbuilt)?;
    if receipt_root != f.receipt_root {
        return Err(Gap::Unbuilt);
    }
    let parent = if f.header.number == 0 {
        None
    } else {
        let p = ep.call("eth_getBlockByNumber", &Value::Arr(vec![Value::Str(crate::scan::hex_quantity(f.header.number - 1)), Value::Bool(false)])).map_err(|_| Gap::Unbuilt)?;
        Some(crate::ahead::header_rlp(&p).filter(|h| zikaron::cryptox::keccak256(h) == f.header.parent).ok_or(Gap::Unbuilt)?)
    };
    let mut proof = AnchorProof {
        chain_id: chain,
        hash: *hash,
        emitter: *emitter,
        header,
        parent,
        tx: raws[index].clone(),
        tx_proof,
        receipt: receipts[index].clone(),
        receipt_proof,
        index: index as u64,
        sender_at: account_proof(ep, who, f.header.number),
        sender_before: f.header.number.checked_sub(1).and_then(|n| account_proof(ep, who, n)),
    };
    let want = Want { tx: *tx, block: *block, number: f.header.number, who: *who, hash: *hash };
    if matches!(check(&proof, chain, &want), Err(Gap::AccountProof)) {
        let at_holds = proof.sender_at.as_ref().is_none_or(|nodes| crate::ahead::code_in(&f.header.state_root, who, nodes).is_ok());
        if !at_holds {
            proof.sender_at = None;
        }
        let before_root = proof.parent.as_ref().and_then(|b| fields(b)).map(|pf| pf.header.state_root);
        let before_holds = match (&proof.sender_before, before_root) {
            (Some(nodes), Some(root)) => crate::ahead::code_in(&root, who, nodes).is_ok(),
            _ => false,
        };
        if !before_holds {
            proof.sender_before = None;
        }
    }
    check(&proof, chain, &want).map_err(|_| Gap::Unbuilt)?;
    Ok(proof)
}

fn hex_list(nodes: &[Vec<u8>]) -> Value {
    Value::Arr(nodes.iter().map(|n| Value::Str(zikaron::hexfmt::encode(n))).collect())
}

fn list_of(v: &Value) -> Option<Vec<Vec<u8>>> {
    match v {
        Value::Arr(items) => items.iter().map(|x| x.as_str().and_then(zikaron::hexfmt::decode)).collect(),
        _ => None,
    }
}

impl AnchorProof {
    pub fn to_value(&self) -> Value {
        let s = |b: &[u8]| Value::Str(zikaron::hexfmt::encode(b));
        let maybe = |n: &Option<Vec<Vec<u8>>>| n.as_ref().map(|n| hex_list(n)).unwrap_or(Value::Null);
        Value::Obj(vec![
            ("chainId".into(), Value::Int(self.chain_id)),
            ("emitter".into(), s(&self.emitter)),
            ("hash".into(), s(&self.hash)),
            ("header".into(), s(&self.header)),
            ("index".into(), Value::Int(self.index)),
            ("parent".into(), self.parent.as_ref().map(|p| s(p)).unwrap_or(Value::Null)),
            ("receipt".into(), s(&self.receipt)),
            ("receiptProof".into(), hex_list(&self.receipt_proof)),
            ("senderProof".into(), Value::Obj(vec![("at".into(), maybe(&self.sender_at)), ("before".into(), maybe(&self.sender_before))])),
            ("tx".into(), s(&self.tx)),
            ("txProof".into(), hex_list(&self.tx_proof)),
        ])
    }

    pub fn of_value(v: &Value) -> Option<AnchorProof> {
        let Value::Obj(ms) = v else { return None };
        let names: Vec<&str> = ms.iter().map(|(n, _)| n.as_str()).collect();
        if names != ["chainId", "emitter", "hash", "header", "index", "parent", "receipt", "receiptProof", "senderProof", "tx", "txProof"] {
            return None;
        }
        let get = |k: &str| ms.iter().find(|(n, _)| n == k).map(|(_, x)| x);
        let bytes = |k: &str| get(k).and_then(Value::as_str).and_then(zikaron::hexfmt::decode);
        let int = |k: &str| match get(k) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        };
        let Some(Value::Obj(sp)) = get("senderProof") else { return None };
        let sp_names: Vec<&str> = sp.iter().map(|(n, _)| n.as_str()).collect();
        if sp_names != ["at", "before"] {
            return None;
        }
        let maybe = |x: &Value| match x {
            Value::Null => Some(None),
            other => list_of(other).map(Some),
        };
        Some(AnchorProof {
            chain_id: int("chainId")?,
            emitter: bytes("emitter")?.try_into().ok()?,
            hash: bytes("hash")?.try_into().ok()?,
            header: bytes("header")?,
            index: int("index")?,
            parent: match get("parent")? {
                Value::Null => None,
                Value::Str(x) => Some(zikaron::hexfmt::decode(x)?),
                _ => return None,
            },
            receipt: bytes("receipt")?,
            receipt_proof: list_of(get("receiptProof")?)?,
            sender_at: maybe(&sp[0].1)?,
            sender_before: maybe(&sp[1].1)?,
            tx: bytes("tx")?,
            tx_proof: list_of(get("txProof")?)?,
        })
    }

    pub fn tx_hash(&self) -> [u8; 32] {
        zikaron::cryptox::keccak256(&self.tx)
    }
}

#[cfg(test)]
mod tests;
