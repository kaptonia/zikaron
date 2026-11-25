use crate::codes::Reason;
use zikaron::hexfmt;
use zikaron::json::Value;
use zikaron_glue::book::{self, Entry};
use zikaron_store::codes::Trouble;
use zikaron_store::layout::EntryName;
use zikaron_store::ledger::{LedgerDir, Stored};

pub fn open(root: &str) -> Result<LedgerDir, Trouble> {
    crate::seam();
    LedgerDir::open(root)
}

pub fn open_or_create(root: &str) -> Result<LedgerDir, Trouble> {
    crate::seam();
    LedgerDir::open_or_create(root)
}

pub fn append(dir: &LedgerDir, entry: &Entry, bytes: &[u8]) -> Result<Stored, Trouble> {
    crate::seam();
    let hex = entry.id_hex();
    let name = match EntryName::parse(hex.trim_start_matches("0x")) {
        Some(n) => n,
        None => return Err(Trouble::named(zikaron_store::codes::Code::BadName, hex)),
    };
    dir.append(&name, bytes)
}

pub fn pile(dir: &LedgerDir) -> Result<Vec<Vec<u8>>, Trouble> {
    crate::seam();
    let items = dir.pile()?.items;
    refuse_sealed(&dir.root().to_string_lossy(), &items);
    Ok(items)
}

pub fn sealed_said() -> &'static str {
    crate::out::Said::Sealed.text()
}

pub fn refuse_sealed(subject: &str, items: &[Vec<u8>]) {
    if items.iter().any(|b| zikaron_glue::sealed::is_sealed(b)) {
        crate::out::misuse(crate::codes::Reason::Unreadable, crate::out::typed(subject), crate::out::Said::Sealed);
    }
}

pub fn pile_hex(items: &[Vec<u8>]) -> Vec<String> {
    items.iter().map(|b| hexfmt::encode(b)).collect()
}

pub fn empty_basis() -> Value {
    book::empty_fragment().member("basis").cloned().unwrap_or(Value::Null)
}

pub fn root_of(items: &[Vec<u8>]) -> Result<String, Reason> {
    crate::seam();
    book::root_of(items).map_err(|n| match n {
        book::NoRoot::Empty => Reason::TipAbsent,
        book::NoRoot::Forked(_) => Reason::TipForked,
    })
}

pub fn read(root: &str, items: &[Vec<u8>]) -> Result<book::Ledger, Reason> {
    crate::seam();
    book::read_under(root, items, &book::empty_fragment()).map_err(|_| Reason::TipAbsent)
}

pub fn entries(root: &str, items: &[Vec<u8>]) -> Result<Vec<Entry>, Reason> {
    Ok(read(root, items)?.entries)
}

pub fn tip(root: &str, items: &[Vec<u8>]) -> Result<(u64, String), Reason> {
    crate::seam();
    book::tip(root, items).map_err(reason_of)
}

pub fn new_findings(items: &[Vec<u8>], candidate: &[u8], named: Option<&str>) -> Result<Vec<String>, book::Barred> {
    crate::seam();
    book::new_findings(items, candidate, named)
}

pub(crate) fn reason_of(n: book::NoTip) -> Reason {
    match n {
        book::NoTip::Absent => Reason::TipAbsent,
        book::NoTip::Forked => Reason::TipForked,
        book::NoTip::NotMoved => Reason::NotMoved,
    }
}

pub enum ReadRefused {
    Ledger(Trouble),
    Kit(String, Option<String>),
    Mirror(String),
    Unreadable(String),
    File(zikaron_glue::container::Bad),
    Sealed(zikaron_seal::Trouble),
    Addressed,
}

pub fn read_any(path: &str, now: u64) -> Result<Vec<Vec<u8>>, ReadRefused> {
    crate::seam();
    let at = std::path::Path::new(path);
    if crate::sealed::is_home(path) {
        return crate::sealed::read(path, now).map(|r| r.items).map_err(ReadRefused::Sealed);
    }
    if zikaron_glue::addressed::is_addressed_at(at) {
        return Err(ReadRefused::Addressed);
    }
    let unpacked = if zikaron_glue::container::is_container_at(at) { Some(zikaron_glue::read::unpack(at).map_err(ReadRefused::File)?) } else { None };
    let dir = unpacked.as_ref().map(|u| u.dir.as_path()).unwrap_or(at);
    if zikaron_glue::mirror::is_bundle(dir) {
        return zikaron_glue::mirror::entries(dir).map_err(|t| match t {
            zikaron_glue::mirror::ReadTrouble::Unreadable(p) => ReadRefused::Unreadable(p),
            zikaron_glue::mirror::ReadTrouble::NotJson(x) | zikaron_glue::mirror::ReadTrouble::NotThisKind(x) | zikaron_glue::mirror::ReadTrouble::NotAnEntryName(x) => ReadRefused::Mirror(x),
        });
    }
    if zikaron_glue::read::is_kit(dir) {
        if let Err((verdict, subject)) = book::kit_verdict(dir) {
            return Err(ReadRefused::Kit(verdict, subject));
        }
        zikaron_glue::anchorproofs::offer_beside(at);
        return zikaron_glue::read::kit_entries(dir).map_err(ReadRefused::Unreadable);
    }
    let d = open(path).map_err(ReadRefused::Ledger)?;
    pile(&d).map_err(ReadRefused::Ledger)
}

