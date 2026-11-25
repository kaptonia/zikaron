pub const CODE_HASH: &str = "0x07a2fef228662aa2adc0a273a304ac4372898138ee1a8a8531760207988ddec2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub chain_id: u64,
    pub at: &'static str,
    pub from_block: u64,
}

pub const ROWS: [Row; 4] = [
    Row { chain_id: 11_155_111, at: "0x506E536d4b463f48BB053Fe7C3DF265BC21B02Fe", from_block: 12_101_379 },
    Row { chain_id: 1, at: "0x0Af771f809358FcC2afd13Bdff98fb7cd6c9Fe42", from_block: 26_387_028 },
    Row { chain_id: 42_161, at: "0x0Af771f809358FcC2afd13Bdff98fb7cd6c9Fe42", from_block: 523_089_554 },
    Row { chain_id: 10, at: "0x0Af771f809358FcC2afd13Bdff98fb7cd6c9Fe42", from_block: 159_372_720 },
];

pub const fn row(chain: u64) -> Option<Row> {
    let mut i = 0;
    while i < ROWS.len() {
        if ROWS[i].chain_id == chain {
            return Some(ROWS[i]);
        }
        i += 1;
    }
    None
}

pub fn at(chain: u64) -> Option<[u8; 20]> {
    zikaron::hexfmt::decode(row(chain)?.at)?.try_into().ok()
}

pub fn registries(chain: u64, registries: &[[u8; 20]]) -> Vec<[u8; 20]> {
    let mut out = registries.to_vec();
    out.extend(at(chain));
    out.sort();
    out.dedup();
    out
}

pub const REVOKE_SIGNATURE: &str = "revoke(bytes32,bytes32)";

pub fn calldata(entry: &[u8; 32], grant: &[u8; 32]) -> Vec<u8> {
    let h = zikaron::cryptox::keccak256(REVOKE_SIGNATURE.as_bytes());
    let mut out = h[..4].to_vec();
    out.extend_from_slice(entry);
    out.extend_from_slice(grant);
    out
}

pub const REVOKED_SIGNATURE: &str = "Revoked(address,bytes32)";

pub const ACCEDED_SIGNATURE: &str = "Acceded(address,address,address)";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Venue {
    pub chain: u64,
    pub contract: [u8; 20],
    pub from_block: u64,
    pub book: [u8; 20],
    pub book_from_block: u64,
}

pub fn venue(chain: u64) -> Option<Venue> {
    Some(Venue { chain, contract: at(chain)?, from_block: row(chain)?.from_block, book: crate::book::at(chain)?, book_from_block: crate::book::from_block(chain)? })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_names_one_contract_per_chain_in_form() {
        for r in ROWS {
            assert!(at(r.chain_id).is_some(), "{} reads as an address", r.at);
            assert_eq!(ROWS.iter().filter(|x| x.chain_id == r.chain_id).count(), 1, "chain {} listed once", r.chain_id);
        }
        assert!(CODE_HASH.len() == 66 && CODE_HASH.starts_with("0x") && CODE_HASH[2..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_eq!(at(31_337), None, "a chain with no row has no contract");
    }

    #[test]
    fn a_venue_is_the_two_rows_of_its_chain() {
        let v = venue(11_155_111).expect("Sepolia has both rows");
        assert_eq!((v.contract, v.from_block), (at(11_155_111).unwrap(), 12_101_379));
        assert_eq!((v.book, v.book_from_block), (crate::book::at(11_155_111).unwrap(), 11_998_635));
        assert_eq!(venue(31_337), None);
        let t0 = |sig: &str| zikaron::hexfmt::encode(&zikaron::cryptox::keccak256(sig.as_bytes()));
        assert_eq!(t0(REVOKED_SIGNATURE), "0x6e70be4be1a4aebd688b5523bd8b6278acac3963d71ebf2bd5ea50757047664b", "CODEHASH.md, second table");
        assert_eq!(t0(ACCEDED_SIGNATURE), "0x4486eef2c3fe5c17b0589612262282787f2bacad99bd69d32652ef1d95c0b6b4");
    }

    #[test]
    fn each_row_is_the_deployment_read_back_from_its_chain() {
        let want = [
            (11_155_111, "0x506E536d4b463f48BB053Fe7C3DF265BC21B02Fe", 12_101_379),
            (1, "0x0Af771f809358FcC2afd13Bdff98fb7cd6c9Fe42", 26_387_028),
            (42_161, "0x0Af771f809358FcC2afd13Bdff98fb7cd6c9Fe42", 523_089_554),
            (10, "0x0Af771f809358FcC2afd13Bdff98fb7cd6c9Fe42", 159_372_720),
        ];
        assert_eq!(ROWS.len(), want.len(), "the table holds these rows and no other");
        for (chain, at_hex, block) in want {
            assert_eq!(row(chain), Some(Row { chain_id: chain, at: at_hex, from_block: block }), "chain {chain}");
            let v = venue(chain).expect("a chain with a revocation contract has a venue");
            assert_eq!((v.contract, v.from_block), (at(chain).unwrap(), block));
        }
    }

    #[test]
    fn the_call_and_the_registries_read_as_the_contract_and_the_law_read_them() {
        let d = calldata(&[0xaa; 32], &[0xbb; 32]);
        assert_eq!(zikaron::hexfmt::encode(&d[..4]), "0xc2664610");
        assert_eq!((&d[4..36], &d[36..], d.len()), (&[0xaa; 32][..], &[0xbb; 32][..], 68));
        let reg = [0xff; 20];
        let here = at(11_155_111).unwrap();
        assert_eq!(registries(11_155_111, &[reg]), vec![here, reg]);
        assert_eq!(registries(11_155_111, &[here, reg, reg]), vec![here, reg]);
        assert_eq!(registries(31_337, &[reg]), vec![reg], "a chain with no row adds nothing");
    }
}
