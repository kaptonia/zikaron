use zikaron::cryptox::keccak256;
use zikaron::json::Value;

pub const CODE_HASH: &str = "0xdb3293b42187b8b7270e62d5eaafee45af02c9caf3a0c8fe376cb4195f2f6aca";

pub const ROWS: [(u64, &str, u64); 4] = [
    (11_155_111, "0x3db7c505dd25361f1a66f6c5bd96b48df27f668c", 11_998_635),
    (1, "0x3db7c505dd25361f1a66f6c5bd96b48df27f668c", 26_387_027),
    (42_161, "0x3db7c505dd25361f1a66f6c5bd96b48df27f668c", 523_089_291),
    (10, "0x3db7c505dd25361f1a66f6c5bd96b48df27f668c", 159_372_716),
];

pub fn row(chain: u64) -> Option<String> {
    ROWS.iter().find(|(c, _, _)| *c == chain).map(|(c, a, _)| format!("{c}:{a}"))
}

pub fn at(chain: u64) -> Option<[u8; 20]> {
    let (_, a, _) = ROWS.iter().find(|(c, _, _)| *c == chain)?;
    zikaron::hexfmt::decode(a)?.try_into().ok()
}

pub fn from_block(chain: u64) -> Option<u64> {
    ROWS.iter().find(|(c, _, _)| *c == chain).map(|(_, _, b)| *b)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Declare,
    Accede,
    Current,
    Root,
    Pending,
    Entered,
}

impl Call {
    pub const ALL: [Call; 6] = [Call::Declare, Call::Accede, Call::Current, Call::Root, Call::Pending, Call::Entered];

    pub fn signature(self) -> &'static str {
        match self {
            Call::Declare => "declare(address)",
            Call::Accede => "accede(address)",
            Call::Current => "current(address)",
            Call::Root => "root(address)",
            Call::Pending => "pending(address)",
            Call::Entered => "entered(address)",
        }
    }

    pub fn selector(self) -> [u8; 4] {
        let h = keccak256(self.signature().as_bytes());
        [h[0], h[1], h[2], h[3]]
    }

    pub fn data(self, who: &[u8; 20]) -> Vec<u8> {
        let mut out = self.selector().to_vec();
        out.extend_from_slice(&[0u8; 12]);
        out.extend_from_slice(who);
        out
    }
}

pub fn call_params(book: &[u8; 20], data: &[u8], at: &str) -> Value {
    Value::Arr(vec![
        Value::Obj(vec![("data".into(), Value::Str(zikaron::hexfmt::encode(data))), ("to".into(), Value::Str(zikaron::hexfmt::encode(book)))]),
        Value::Str(at.to_string()),
    ])
}

pub fn address_word(answer: &str) -> Option<[u8; 20]> {
    let b = zikaron::hexfmt::decode(answer)?;
    if b.len() != 32 || b[..12].iter().any(|x| *x != 0) {
        return None;
    }
    b[12..].try_into().ok()
}

pub fn bool_word(answer: &str) -> Option<bool> {
    let b = zikaron::hexfmt::decode(answer)?;
    if b.len() != 32 || b[..31].iter().any(|x| *x != 0) {
        return None;
    }
    match b[31] {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

pub fn is_the_book(code: &[u8]) -> bool {
    !code.is_empty() && zikaron::hexfmt::encode(&keccak256(code)) == CODE_HASH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_books_calls_and_answers_read_in_its_shape() {
        let sel = |c: Call| zikaron::hexfmt::encode(&c.selector());
        let want = [(Call::Declare, "0xa8788022"), (Call::Accede, "0x2e3eadf8"), (Call::Current, "0xdc9d067b"), (Call::Root, "0x6e5ac882"), (Call::Pending, "0x5eebea20"), (Call::Entered, "0x17b299aa")];
        for (c, w) in want {
            assert_eq!(sel(c), w, "{}", c.signature());
        }
        let who = [0xabu8; 20];
        let d = Call::Accede.data(&who);
        assert_eq!(d.len(), 36);
        assert_eq!(&d[4..16], &[0u8; 12]);
        assert_eq!(&d[16..], &who);
        let word = format!("0x{}{}", "00".repeat(12), "ab".repeat(20));
        assert_eq!(address_word(&word), Some(who));
        assert_eq!(address_word(&format!("0x01{}{}", "00".repeat(11), "ab".repeat(20))), None, "high bytes set");
        assert_eq!(address_word("0x"), None, "no word");
        assert_eq!(bool_word(&format!("0x{}01", "00".repeat(31))), Some(true));
        assert_eq!(bool_word(&format!("0x{}00", "00".repeat(31))), Some(false));
        assert_eq!(bool_word(&format!("0x{}02", "00".repeat(31))), None);
        assert!(!is_the_book(&[]), "no code is no book");
        assert!(!is_the_book(b"other code"), "other code is no book");
        assert_eq!(row(11_155_111).as_deref(), Some("11155111:0x3db7c505dd25361f1a66f6c5bd96b48df27f668c"));
        assert_eq!(at(31_337), None, "a chain with no row has no book");
        assert_eq!(from_block(11_155_111), Some(11_998_635));
        assert_eq!(from_block(31_337), None, "a chain with no row has no deployment block");
        for (chain, block) in [(1, 26_387_027), (42_161, 523_089_291), (10, 159_372_716)] {
            assert_eq!(row(chain), Some(format!("{chain}:0x3db7c505dd25361f1a66f6c5bd96b48df27f668c")), "chain {chain}: the same address as Sepolia's");
            assert_eq!(from_block(chain), Some(block), "chain {chain}: the block read back");
        }
    }
}
