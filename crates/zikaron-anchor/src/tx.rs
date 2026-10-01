//! Transactions: recompute a node's transaction object into bytes, check its hash, recover its sender (law
//! §9.1).
//!
//! §9.1 defines the sender as the address recovered from the transaction's own secp256k1 signature. The
//! node's `from` is the node's claim, not a chain fact. So each transaction is re-encoded, its signer
//! recovered, and the re-encoded bytes keccak-hashed and compared with `hash`; a mismatch means the endpoint
//! gave something other than what it claims, and reading stops.
//!
//! Three kinds have no sender in this grammar: a type outside {0x0, 0x1, 0x2, 0x3, 0x4} (deposit type 0x7e,
//! future types) has no rebuildable signing payload; `r` and `s` both zero means no signature; a legacy `v`
//! of 27 or 28 (outside EIP-155) names no chain. §9.1 rules on each: such a transaction is no anchor in
//! either form, its logs are no anchors whatever their topics, and it enters no evidence record (§9.2).

use crate::rlp;
use zikaron::cryptox;
use crate::wire::W;

/// A transaction as read.
#[derive(Clone, Debug)]
pub struct Tx {
    pub hash: [u8; 32],
    /// The chain the signature names; `None` when it names none (legacy, outside EIP-155).
    pub chain_id: Option<u64>,
    /// The §9.1 sender; `None` when this grammar gives it none.
    pub sender: Option<[u8; 20]>,
    /// Recipient; `None` for a contract creation.
    pub to: Option<[u8; 20]>,
    /// Calldata in the §9.1 sense: the top-level `data` field, the bytes the sender signed.
    pub input: Vec<u8>,
    /// Containing block; `None` when not yet included.
    pub block_number: Option<u64>,
    pub tx_type: u64,
    /// The re-encoded full bytes (a §9.7 kit pins them to the block's transaction root). Empty for types
    /// whose signing payload cannot be rebuilt.
    pub raw: Vec<u8>,
}

/// Where reading a transaction cannot go on. Each means the endpoint's data does not hold together; none is a
/// chain verdict.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Bad {
    /// A required member is missing or has the wrong shape.
    Shape(&'static str),
    /// The re-encoded bytes do not hash to the claimed hash.
    HashMismatch,
}

fn s<'a>(v: &'a W, k: &str) -> Option<&'a str> {
    v.member(k)?.as_str()
}

/// A hex quantity to u64.
pub fn hex_qty(w: &W) -> Option<u64> {
    let x = w.as_str()?;
    let body = x.strip_prefix("0x").or_else(|| x.strip_prefix("0X"))?;
    if body.is_empty() || body.len() > 16 {
        return None;
    }
    u64::from_str_radix(body, 16).ok()
}

/// A hex quantity to big-endian bytes without leading zeros.
pub fn qty(v: &W, k: &str) -> Option<Vec<u8>> {
    let x = s(v, k)?;
    let body = x.strip_prefix("0x").or_else(|| x.strip_prefix("0X"))?;
    if body.is_empty() {
        return None;
    }
    let padded = if body.len() % 2 == 1 { format!("0{body}") } else { body.to_string() };
    let mut out = Vec::with_capacity(padded.len() / 2);
    let b = padded.as_bytes();
    for i in (0..b.len()).step_by(2) {
        let hi = (b[i] as char).to_digit(16)?;
        let lo = (b[i + 1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
    }
    let mut i = 0;
    while i + 1 < out.len() && out[i] == 0 {
        i += 1;
    }
    Some(out[i..].to_vec())
}

/// A hex quantity to u64 (within the §3.1 int bound).
pub fn qty_u64(v: &W, k: &str) -> Option<u64> {
    let b = qty(v, k)?;
    if b.len() > 8 {
        return None;
    }
    let mut x = 0u64;
    for byte in &b {
        x = (x << 8) | *byte as u64;
    }
    Some(x)
}

/// A hex byte string to bytes.
pub fn data(v: &W, k: &str) -> Option<Vec<u8>> {
    let x = s(v, k)?;
    zikaron::hexfmt::decode(x)
}

fn addr(v: &W, k: &str) -> Option<[u8; 20]> {
    let b = data(v, k)?;
    if b.len() != 20 {
        return None;
    }
    let mut a = [0u8; 20];
    a.copy_from_slice(&b);
    Some(a)
}

fn scalar32(v: &W, k: &str) -> Option<[u8; 32]> {
    let b = qty(v, k)?;
    if b.len() > 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out[32 - b.len()..].copy_from_slice(&b);
    Some(out)
}

fn is_zero(x: &[u8; 32]) -> bool {
    x.iter().all(|b| *b == 0)
}

fn access_list(v: &W) -> Vec<Vec<u8>> {
    let Some(items) = v.member("accessList").and_then(|x| x.as_arr()) else {
        return Vec::new();
    };
    items
        .iter()
        .map(|e| {
            let a = data(e, "address").unwrap_or_default();
            let keys: Vec<Vec<u8>> = e
                .member("storageKeys")
                .and_then(|x| x.as_arr())
                .map(|ks| {
                    ks.iter()
                        .map(|k| rlp::bytes(&k.as_str().and_then(zikaron::hexfmt::decode).unwrap_or_default()))
                        .collect()
                })
                .unwrap_or_default();
            rlp::list(&[rlp::bytes(&a), rlp::list(&keys)])
        })
        .collect()
}

fn authorization_list(v: &W) -> Vec<Vec<u8>> {
    let Some(items) = v.member("authorizationList").and_then(|x| x.as_arr()) else {
        return Vec::new();
    };
    items
        .iter()
        .map(|e| {
            rlp::list(&[
                rlp::scalar(&qty(e, "chainId").unwrap_or_default()),
                rlp::bytes(&data(e, "address").unwrap_or_default()),
                rlp::scalar(&qty(e, "nonce").unwrap_or_default()),
                rlp::scalar(&qty(e, "yParity").or_else(|| qty(e, "v")).unwrap_or_default()),
                rlp::scalar(&qty(e, "r").unwrap_or_default()),
                rlp::scalar(&qty(e, "s").unwrap_or_default()),
            ])
        })
        .collect()
}

fn to_field(v: &W) -> Vec<u8> {
    match addr(v, "to") {
        Some(a) => rlp::bytes(&a),
        None => rlp::bytes(&[]),
    }
}

/// Read a transaction: re-encode, check the hash, recover the sender.
pub fn read(v: &W) -> Result<Tx, Bad> {
    let hash = {
        let b = data(v, "hash").ok_or(Bad::Shape("hash"))?;
        if b.len() != 32 {
            return Err(Bad::Shape("hash"));
        }
        let mut h = [0u8; 32];
        h.copy_from_slice(&b);
        h
    };
    let tx_type = qty_u64(v, "type").unwrap_or(0);
    let input = data(v, "input").ok_or(Bad::Shape("input"))?;
    let to = addr(v, "to");
    let block_number = qty_u64(v, "blockNumber");
    let r = scalar32(v, "r").ok_or(Bad::Shape("r"))?;
    let sig_s = scalar32(v, "s").ok_or(Bad::Shape("s"))?;

    let nothing = Tx { hash, chain_id: None, sender: None, to, input: input.clone(), block_number, tx_type, raw: Vec::new() };

    // No signature (system or deposit types): no sender in this grammar.
    if is_zero(&r) && is_zero(&sig_s) {
        return Ok(nothing);
    }
    // A type whose signing payload cannot be rebuilt: no sender, and no hash to recompute.
    if !matches!(tx_type, 0 | 1 | 2 | 3 | 4) {
        return Ok(nothing);
    }

    let nonce = rlp::scalar(&qty(v, "nonce").unwrap_or_default());
    let gas = rlp::scalar(&qty(v, "gas").unwrap_or_default());
    let value = rlp::scalar(&qty(v, "value").unwrap_or_default());
    let dat = rlp::bytes(&input);
    let dst = to_field(v);
    let al = rlp::list(&access_list(v));

    let (payload, raw, chain_id, recid) = if tx_type == 0 {
        let gas_price = rlp::scalar(&qty(v, "gasPrice").unwrap_or_default());
        let vq = qty_u64(v, "v").ok_or(Bad::Shape("v"))?;
        // EIP-155: v = 2 × chainId + 35 + recid. 27 and 28 are the legacy form whose signature names no
        // chain.
        let (chain, recid) = if vq == 27 || vq == 28 {
            // The lawful case: the signature names no chain (legacy, outside EIP-155).
            (None, (vq - 27) as u8)
        } else if vq >= 37 {
            let chain = (vq - 35) / 2;
            let recid = (vq - 35 - 2 * chain) as u8;
            if recid > 1 {
                // A v this formula cannot produce means the endpoint's answer does not hold, not that the law
                // gives no sender.
                return Err(Bad::Shape("v"));
            }
            (Some(chain), recid)
        } else {
            return Err(Bad::Shape("v"));
        };
        let base = vec![nonce, gas_price, gas, dst, value, dat];
        let payload = match chain {
            Some(c) => {
                let mut p = base.clone();
                p.push(rlp::quantity(c));
                p.push(rlp::bytes(&[]));
                p.push(rlp::bytes(&[]));
                rlp::list(&p)
            }
            None => rlp::list(&base),
        };
        let mut full = base;
        full.push(rlp::quantity(vq));
        full.push(rlp::scalar(&r));
        full.push(rlp::scalar(&sig_s));
        (payload, rlp::list(&full), chain, recid)
    } else {
        let chain = qty_u64(v, "chainId").ok_or(Bad::Shape("chainId"))?;
        let y = qty_u64(v, "yParity").or_else(|| qty_u64(v, "v")).unwrap_or(0);
        if y > 1 {
            // Likewise: typed transactions have yParity 0 or 1; any other value means the endpoint's answer
            // does not hold.
            return Err(Bad::Shape("yParity"));
        }
        let mut fields: Vec<Vec<u8>> = vec![rlp::quantity(chain), nonce];
        match tx_type {
            1 => {
                fields.push(rlp::scalar(&qty(v, "gasPrice").unwrap_or_default()));
                fields.push(gas);
                fields.push(dst);
                fields.push(value);
                fields.push(dat);
                fields.push(al);
            }
            _ => {
                fields.push(rlp::scalar(&qty(v, "maxPriorityFeePerGas").unwrap_or_default()));
                fields.push(rlp::scalar(&qty(v, "maxFeePerGas").unwrap_or_default()));
                fields.push(gas);
                fields.push(dst);
                fields.push(value);
                fields.push(dat);
                fields.push(al);
                if tx_type == 3 {
                    fields.push(rlp::scalar(&qty(v, "maxFeePerBlobGas").unwrap_or_default()));
                    let hashes: Vec<Vec<u8>> = v
                        .member("blobVersionedHashes")
                        .and_then(|x| x.as_arr())
                        .map(|hs| {
                            hs.iter()
                                .map(|h| rlp::bytes(&h.as_str().and_then(zikaron::hexfmt::decode).unwrap_or_default()))
                                .collect()
                        })
                        .unwrap_or_default();
                    fields.push(rlp::list(&hashes));
                }
                if tx_type == 4 {
                    fields.push(rlp::list(&authorization_list(v)));
                }
            }
        }
        let mut payload = vec![tx_type as u8];
        payload.extend_from_slice(&rlp::list(&fields));
        let mut full = fields;
        full.push(rlp::scalar(&[y as u8]));
        full.push(rlp::scalar(&r));
        full.push(rlp::scalar(&sig_s));
        let mut raw = vec![tx_type as u8];
        raw.extend_from_slice(&rlp::list(&full));
        (payload, raw, Some(chain), y as u8)
    };

    if cryptox::keccak256(&raw) != hash {
        return Err(Bad::HashMismatch);
    }
    let digest = cryptox::keccak256(&payload);
    let sender = cryptox::recover_address(&digest, &r, &sig_s, recid);
    Ok(Tx { hash, chain_id, sender, to, input, block_number, tx_type, raw })
}


/// Read a transaction from its full bytes (a §9.7 kit carries bytes, not objects).
///
/// The signing payload has one rule: drop the last three signature members and re-encode with the type
/// prefix. Both entry points (node object, kit bytes) therefore compute the same payload.
pub fn from_raw(raw: &[u8]) -> Option<Tx> {
    let hash = cryptox::keccak256(raw);
    let (tx_type, list) = if raw.first().copied()? >= 0xc0 {
        (0u64, crate::rlp::decode_all(raw)?)
    } else {
        (raw[0] as u64, crate::rlp::decode_all(&raw[1..])?)
    };
    let items = list.list()?.to_vec();
    if items.len() < 4 {
        return None;
    }
    let body = &items[..items.len() - 3];
    let v_item = &items[items.len() - 3];
    let r = scalar_of(&items[items.len() - 2])?;
    let s = scalar_of(&items[items.len() - 1])?;
    // Field positions: legacy is [nonce, gasPrice, gas, to, value, data]; typed transactions start with
    // chainId.
    let (to_at, data_at) = match tx_type {
        0 => (3, 5),
        1 => (4, 6),
        _ => (5, 7),
    };
    let to = body.get(to_at).and_then(|x| x.bytes()).and_then(|b| {
        if b.len() == 20 {
            let mut a = [0u8; 20];
            a.copy_from_slice(b);
            Some(a)
        } else {
            None
        }
    });
    let input = body.get(data_at).and_then(|x| x.bytes()).map(<[u8]>::to_vec).unwrap_or_default();
    let nothing = Tx { hash, chain_id: None, sender: None, to, input: input.clone(), block_number: None, tx_type, raw: raw.to_vec() };
    if r.iter().all(|x| *x == 0) && s.iter().all(|x| *x == 0) {
        return Some(nothing);
    }
    if !matches!(tx_type, 0 | 1 | 2 | 3 | 4) {
        return Some(nothing);
    }
    let (payload, chain_id, recid) = if tx_type == 0 {
        let vq = v_item.u64()?;
        let (chain, recid) = if vq == 27 || vq == 28 {
            (None, (vq - 27) as u8)
        } else if vq >= 37 {
            let chain = (vq - 35) / 2;
            let recid = (vq - 35 - 2 * chain) as u8;
            if recid > 1 {
                return Some(nothing);
            }
            (Some(chain), recid)
        } else {
            return Some(nothing);
        };
        let mut fields: Vec<Vec<u8>> = body.iter().map(reencode).collect::<Option<_>>()?;
        if let Some(c) = chain {
            fields.push(crate::rlp::quantity(c));
            fields.push(crate::rlp::bytes(&[]));
            fields.push(crate::rlp::bytes(&[]));
        }
        (crate::rlp::list(&fields), chain, recid)
    } else {
        let chain = body.first()?.u64()?;
        let y = v_item.u64()?;
        if y > 1 {
            return Some(nothing);
        }
        let fields: Vec<Vec<u8>> = body.iter().map(reencode).collect::<Option<_>>()?;
        let mut p = vec![tx_type as u8];
        p.extend_from_slice(&crate::rlp::list(&fields));
        (p, Some(chain), y as u8)
    };
    let digest = cryptox::keccak256(&payload);
    let sender = cryptox::recover_address(&digest, &r, &s, recid);
    Some(Tx { hash, chain_id, sender, to, input, block_number: None, tx_type, raw: raw.to_vec() })
}

fn scalar_of(item: &crate::rlp::Item) -> Option<[u8; 32]> {
    let b = item.bytes()?;
    if b.len() > 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out[32 - b.len()..].copy_from_slice(b);
    Some(out)
}

/// Re-encode a decoded item as RLP, unchanged.
pub fn reencode(item: &crate::rlp::Item) -> Option<Vec<u8>> {
    Some(match item {
        crate::rlp::Item::Bytes(b) => crate::rlp::bytes(b),
        crate::rlp::Item::List(items) => {
            let inner: Vec<Vec<u8>> = items.iter().map(reencode).collect::<Option<_>>()?;
            crate::rlp::list(&inner)
        }
    })
}
