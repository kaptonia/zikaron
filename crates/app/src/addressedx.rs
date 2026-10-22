//! The record package addressed to one key, the desktop's side: who a package is sealed to (an address whose key is
//! found on chain, or a key given), sealing an exported package, and opening one with this machine's identities. The
//! package itself is the sealed layer's (`zikaron_seal::addressed`); the key search the sender's
//! (`chainx::recipient_key`); the agreement each identity lends is `key::Secret`'s one exit.
//!
//! Nothing here records the recipient: not the export's result, not the kits index, not a trace mark.

use crate::fault::{Fault, Known};
use std::path::Path;
use zikaron_seal::addressed::{Opened, Refused};

/// Who a package goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum To {
    /// An address: its key is looked for on chain (`key_for`).
    Address([u8; 20]),
    /// A public key given (compressed, judged a point of the curve).
    Key([u8; 33]),
}

/// Whether text is an address in its one form (`0x` and forty hex digits).
pub fn address_ok(text: &str) -> bool {
    crate::key::Address::parse(text).is_some()
}

/// A public key as typed (`0x` and 66 or 130 hex digits), judged a point of the curve; compressed.
pub fn key_of(text: &str) -> Option<[u8; 33]> {
    zikaron_seal::addressed::key_text(text)
}

/// The recipient the two fields give: none for a plain package; one of them for an addressed one; both given, an
/// address not in its form, or a key off the curve, refused by name.
pub fn to_of(address: &str, key: &str) -> Result<Option<To>, Fault> {
    match (address.trim(), key.trim()) {
        ("", "") => Ok(None),
        (a, "") => crate::key::Address::parse(a).map(|x| Some(To::Address(x.0))).ok_or_else(|| Fault::known(Known::AddressShape, a.to_string())),
        ("", k) => key_of(k).map(|p| Some(To::Key(p))).ok_or_else(|| Fault::known(Known::AddressedKeyShape, k.to_string())),
        _ => Err(Fault::known(Known::AddressedBoth, String::new())),
    }
}

/// A refusal of the sealed layer as the desktop names it (its evidence: the segment, the head's flaw; never a
/// recipient).
pub fn fault_of(r: &Refused) -> Fault {
    match zikaron_seal::addressed::Why::of(r) {
        Some(why) => Fault::known(Known::of_addressed(why), format!("{} {}", r.code(), r.tail()).trim().to_string()),
        None => Fault::unknown(r.tail().to_string()),
    }
}

/// This machine's identities' keys: each seat of each identity in the register (an identity's seed slot holds no
/// key and is not read). A seat whose slot is empty is passed over; so is one whose slot is unreadable on its own
/// (`zikaron_seal::Code::one_slot`), the first such fault kept beside the keys; the vault's own trouble is the answer.
pub fn local_keys() -> Result<(Vec<crate::key::Secret>, Option<Fault>), Fault> {
    use zikaron_seal::Coded as _;
    let reg = crate::register::view()?;
    let (mut keys, mut passed) = (Vec::new(), None);
    for row in &reg.rows {
        for seat in row.keys.seats() {
            let Some(acct) = row.account(seat) else { continue };
            match crate::key::load_at(&acct) {
                Ok(Some(k)) => {
                    keys.push(k);
                }
                Ok(None) => {}
                Err(f) if f.sealed_code().is_some_and(|c| c.one_slot()) => {
                    passed.get_or_insert(f);
                }
                Err(f) => return Err(f),
            }
        }
    }
    Ok((keys, passed))
}

/// Open the addressed package at `p` with the first of this machine's identities that is its recipient, into a
/// folder of its own (removed when the result drops). None of them: refused by name, nothing left behind; and when a
/// seat was passed over because its slot did not read, that seat may be the recipient, so its fault is the answer
/// instead (never "none of them is the recipient").
pub fn open(p: &Path) -> Result<Opened, Fault> {
    let (keys, passed) = local_keys()?;
    let lent: Vec<&dyn zikaron_seal::addressed::Recipient> = keys.iter().map(|k| k as &dyn zikaron_seal::addressed::Recipient).collect();
    match (zikaron_seal::addressed::open(p, &lent), passed) {
        (Ok(o), _) => Ok(o),
        (Err(zikaron_seal::addressed::Refused::NotRecipient), Some(f)) => Err(f),
        (Err(r), _) => Err(fault_of(&r)),
    }
}

/// The main network as a key is looked for on it: its chain, registry and start block, and its nodes.
pub struct Main {
    pub chain: Option<u64>,
    pub registry: Option<[u8; 20]>,
    pub from_block: u64,
    pub nodes: Vec<crate::chainx::NodeAddr>,
}

/// The key behind `address`, looked for on the main network and every read-only network, by the sender's one
/// judgement for both ends (`chainx::key_for_sealing`): found, it proves itself (its address is this one);
/// every node of every network read through with no anchor this address signed is `ADDRESS_NOT_ANCHORED`; otherwise
/// `ADDRESS_CHAIN_UNREAD` with each node's words. A read-only network whose registry is not the pinned build
/// (`chainx::fingerprint_gate`) is a network not read: no anchor is taken from it, and it never makes "not
/// anchored". Nothing is written either way.
pub fn key_for(address: &[u8; 20], main: &Main, readonly: &[crate::readnets::Net]) -> Result<[u8; 33], Fault> {
    use crate::chainx::RecipientNet as Net;
    // A main network with no chain set is no network to read (the sender's judgement counts it for nothing).
    let mut nets: Vec<Net> = Vec::new();
    if let (Some(chain), Some(registry)) = (main.chain, main.registry) {
        nets.push(Net { chain, registry, from_block: main.from_block, nodes: main.nodes.clone() });
    }
    let readonly: Vec<Net> = readonly
        .iter()
        .map(|n| Net { chain: n.chain_id, registry: n.registry.0, from_block: n.from_block, nodes: n.nodes.iter().map(|u| crate::chainx::NodeAddr::new(u.as_str())).collect() })
        .collect();
    crate::chainx::key_for_sealing(address, nets, &readonly, Vec::new(), crate::pinned::CODE_HASH).map_err(|(why, words)| Fault::known(Known::of_addressed(why), words))
}

/// Seal the plain package that `lay` writes to `to` at `out`, the sealed layer's one way for both ends
/// (`zikaron_seal::addressed::seal_out`: `out` taken refused first, the plain package laid out in a folder of its own
/// in the system's temporary place and removed either way, nothing at `out` on failure). Returns what `lay` returned
/// and the package's one-time key, by which its local copy is told apart (`kitsindex`); it names no recipient.
pub fn seal_out<T>(to: &[u8; 33], out: &Path, lay: impl FnOnce(&Path) -> Result<T, Fault>) -> Result<(T, [u8; 33]), Fault> {
    use zikaron_seal::addressed::Sealing;
    zikaron_seal::addressed::seal_out(to, out, lay).map_err(|s| match s {
        Sealing::Lay(f) => f,
        Sealing::Refused(r) => fault_of(&r),
        Sealing::Land(t) => Fault::of_landing(t),
    })
}

/// The one-time key in the head of the addressed package at `p` (`None` when it is not one, or does not read).
pub fn one_time_of(p: &Path) -> Option<[u8; 33]> {
    use std::io::Read;
    let mut head = [0u8; zikaron_glue::addressed::HEAD];
    let mut f = std::fs::File::open(p).ok()?;
    f.read_exact(&mut head).ok()?;
    zikaron_glue::addressed::head(&head).ok().map(|h| h.point)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_fields_give_one_recipient_or_are_refused_by_name() {
        assert_eq!(to_of("", "").unwrap(), None);
        let a = format!("0x{}", "ab".repeat(20));
        assert_eq!(to_of(&a, "").unwrap(), Some(To::Address([0xab; 20])));
        let k = zikaron_seal::crypto::public_of(&[3u8; 32]).unwrap();
        let khex = zikaron::hexfmt::encode(&k);
        assert_eq!(to_of("", &khex).unwrap(), Some(To::Key(k)));
        let full = zikaron::hexfmt::encode(&zikaron_seal::crypto::uncompressed(&k).unwrap());
        assert_eq!(to_of("", &full).unwrap(), Some(To::Key(k)), "65 字节非压缩形同收");
        assert_eq!(to_of(&a, &khex).unwrap_err().which(), Some(Known::AddressedBoth));
        assert_eq!(to_of("0x12", "").unwrap_err().which(), Some(Known::AddressShape));
        let off = format!("0x02{}", "00".repeat(32));
        assert_eq!(to_of("", &off).unwrap_err().which(), Some(Known::AddressedKeyShape));
        assert_eq!(to_of("", &khex[2..]).unwrap_err().which(), Some(Known::AddressedKeyShape), "不带 0x 不认");
    }
}
