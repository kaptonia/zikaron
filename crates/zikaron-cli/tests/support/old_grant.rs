use zikaron_cli::codes::Field;

pub fn old_grant(ledger: &std::path::Path, key_hex: &str, grantee: &str, work: &str, terms: &str) -> String {
    let at = ledger.to_string_lossy().into_owned();
    let dir = zikaron_cli::ledger::open(&at).expect("a ledger");
    let items = zikaron_cli::ledger::pile(&dir).expect("its entries");
    let root = zikaron_cli::ledger::root_of(&items).expect("its root");
    let (seq, prev) = zikaron_cli::ledger::tip(&root, &items).expect("its tip");
    let key: [u8; 32] = zikaron::hexfmt::decode(key_hex).and_then(|b| b.try_into().ok()).expect("a key");
    let author = zikaron_cli::entry::address(&key).expect("an address");
    let s = |x: &str| zikaron::json::Value::Str(x.to_string());
    let body = zikaron_cli::entry::shape(vec![(Field::Grantee, s(grantee)), (Field::Work, s(work)), (Field::Terms, s(terms)), (Field::Ledger, s(&root))]);
    let sealed = zikaron_cli::entry::seal(&author, zikaron_glue::book::Kind::Grant.as_str(), seq, Some(&prev), body, &key).expect("sealed");
    zikaron_cli::ledger::append(&dir, &sealed.entry, &sealed.bytes).expect("landed");
    sealed.entry.id_hex()
}
