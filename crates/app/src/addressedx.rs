use crate::fault::{Fault, Known};
use crate::key::Address;
use std::path::Path;
use zikaron_seal::addressed::{Opened, Refused};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum To {
    Address([u8; 20]),
    Key([u8; 33]),
}

pub fn address_ok(text: &str) -> bool {
    crate::key::Address::parse(text).is_some()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Takes {
    Address,
    OrMarker,
    OrConsent,
}

pub fn not_address(text: &str, takes: Takes) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    let bare = match takes {
        Takes::Address => t,
        Takes::OrMarker => t.strip_prefix(zikaron_glue::face::MARKER_PREFIX).unwrap_or(t),
        Takes::OrConsent if t.starts_with(zikaron_glue::face::CONSENT_PREFIX) => return false,
        Takes::OrConsent => t,
    };
    !address_ok(bare)
}

pub fn key_of(text: &str) -> Option<[u8; 33]> {
    zikaron_seal::addressed::key_text(text)
}

pub fn to_of(address: &str, key: &str) -> Result<Option<To>, Fault> {
    match (address.trim(), key.trim()) {
        ("", "") => Ok(None),
        (a, "") => crate::key::Address::parse(a).map(|x| Some(To::Address(x.0))).ok_or_else(|| Fault::known(Known::AddressShape, a.to_string())),
        ("", k) => key_of(k).map(|p| Some(To::Key(p))).ok_or_else(|| Fault::known(Known::AddressedKeyShape, k.to_string())),
        _ => Err(Fault::known(Known::AddressedBoth, String::new())),
    }
}

pub fn fault_of(r: &Refused) -> Fault {
    match zikaron_seal::addressed::Why::of(r) {
        Some(why) => Fault::known(Known::of_addressed(why), format!("{} {}", r.code(), r.tail()).trim().to_string()),
        None => Fault::unknown(r.tail().to_string()),
    }
}

pub fn local_keys() -> Result<(Vec<crate::key::Secret>, Option<Fault>), Fault> {
    use zikaron_seal::Coded as _;
    let reg = crate::register::view()?;
    let (mut keys, mut passed) = (Vec::new(), None);
    for row in &reg.rows {
        for seat in row.keys.seats() {
            let Some(office) = row.office(seat) else { continue };
            let mut held: Vec<Address> = row.address(seat).into_iter().collect();
            for a in std::iter::once(office.current).chain(office.former.iter().copied()).map(Address) {
                if !held.contains(&a) {
                    held.push(a);
                }
            }
            for acct in held.iter().map(crate::places::key_slot) {
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
    }
    Ok((keys, passed))
}

pub fn open(p: &Path) -> Result<Opened, Fault> {
    let (keys, passed) = local_keys()?;
    let lent: Vec<&dyn zikaron_seal::addressed::Recipient> = keys.iter().map(|k| k as &dyn zikaron_seal::addressed::Recipient).collect();
    match (zikaron_seal::addressed::open(p, &lent), passed) {
        (Ok(o), _) => Ok(o),
        (Err(zikaron_seal::addressed::Refused::NotRecipient), Some(f)) => Err(f),
        (Err(r), _) => Err(fault_of(&r)),
    }
}

pub struct Main {
    pub chain: Option<u64>,
    pub registry: Option<[u8; 20]>,
    pub from_block: u64,
    pub nodes: Vec<crate::chainx::NodeAddr>,
}

pub fn key_for(address: &[u8; 20], main: &Main, readonly: &[crate::readnets::Net]) -> Result<[u8; 33], Fault> {
    use crate::chainx::RecipientNet as Net;
    let mut nets: Vec<Net> = Vec::new();
    if let (Some(chain), Some(registry)) = (main.chain, main.registry) {
        nets.push(Net { chain, registry, from_block: main.from_block, nodes: main.nodes.clone() });
    }
    let readonly: Vec<Net> = readonly
        .iter()
        .map(|n| Net { chain: n.chain_id, registry: n.registry.0, from_block: n.from_block, nodes: n.nodes.iter().map(|u| crate::chainx::NodeAddr::new(u.as_str())).collect() })
        .collect();
    crate::chainx::key_for_root(address, held_office(address), nets, &readonly, Vec::new(), crate::pinned::CODE_HASH)
        .map(|(k, _)| k)
        .map_err(|(why, words)| Fault::known(Known::of_addressed(why), words))
}

pub fn held_office(root: &[u8; 20]) -> Option<[u8; 20]> {
    let reg = crate::register::view().ok();
    let seats: Vec<([u8; 20], [u8; 20])> = reg.iter().flat_map(|r| r.rows.iter()).flat_map(|row| row.seats().into_iter().filter_map(move |seat| Some((row.address(seat)?.0, row.current(seat)?.0)))).collect();
    let kept: Vec<Vec<Vec<u8>>> = crate::identity::home_now(reg.as_ref())
        .ok()
        .flatten()
        .and_then(|p| crate::home::Home::open(&p).ok())
        .and_then(|home| crate::grantfilex::kept(&home).ok())
        .map(|k| k.into_iter().map(|(_, o)| o.ledger).collect())
        .unwrap_or_default();
    crate::chainx::held_in(root, &seats, &kept)
}

pub fn seal_out<T>(to: &[u8; 33], out: &Path, lay: impl FnOnce(&Path) -> Result<T, Fault>) -> Result<(T, [u8; 33]), Fault> {
    use zikaron_seal::addressed::Sealing;
    zikaron_seal::addressed::seal_out(to, out, lay).map_err(|s| match s {
        Sealing::Lay(f) => f,
        Sealing::Refused(r) => fault_of(&r),
        Sealing::Land(t) => Fault::of_landing(t),
    })
}

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

#[cfg(test)]
mod shape_tests {
    use super::{not_address, Takes};

    const A: &str = "0x2da333b436f845d65c9a9d7bea9a819062e3c2ff";

    #[test]
    fn every_address_cell_judges_its_form_in_one_place() {
        for bad in ["0x12zz", "0x2da333b436f845d65c9a9d7bea9a819062e3c2", "2da333b436f845d65c9a9d7bea9a819062e3c2ff", "0x2da333b436f845d65c9a9d7bea9a819062e3c2ff00"] {
            for takes in [Takes::Address, Takes::OrMarker, Takes::OrConsent] {
                assert!(not_address(bad, takes), "{bad} {takes:?}");
            }
        }
        for takes in [Takes::Address, Takes::OrMarker, Takes::OrConsent] {
            assert!(!not_address(A, takes) && !not_address("", takes), "{takes:?}");
        }
        let marker = format!("{}{A}", zikaron_glue::face::MARKER_PREFIX);
        assert!(!not_address(&marker, Takes::OrMarker));
        assert!(not_address(&marker, Takes::Address));
        let consent = format!("{}x", zikaron_glue::face::CONSENT_PREFIX);
        assert!(!not_address(&consent, Takes::OrConsent));
        assert!(not_address(&consent, Takes::OrMarker));
    }
}
