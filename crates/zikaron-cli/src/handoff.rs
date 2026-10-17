//! Handing a ledger between two places, the same two verbs at both ends: `hand-over` makes a handoff file of the
//! whole ledger and stops this place writing it; `take-over` takes a handoff into this place's ledger. This
//! product's wiring of the family's handoff rules (`zikaron_seal::handover`: the pen goes with the ledger, only a
//! follow-on is taken); here is the ledger's form and its file.
//!
//! - **The form** ([`Zk`]): an entry is its canonical bytes, identified by the core's entry id; entries make one
//!   line when they have one root and the core's offline audit of them finds no two entries at the same position and
//!   no link to the wrong entry (no `EQUIVOCATION`, no `PREV_MISMATCH`).
//! - **The file**: a record package in one file (`zikaron_glue::container`, `.zkhand`): every entry of the ledger,
//!   and the handoff's note ([`NOTE_FILE`] under the files room). Its kit is checked by the kit law before
//!   anything in it is read.
//! - **A place**: a plain ledger folder, or the ledger of a home the desktop keeps (written with the credential,
//!   `sealed::open`, the desktop locked or not). The pen of each ledger is kept in this machine's pen list
//!   (`zikaron_seal::pens`), by the ledger's root id. Every entry-writing verb asks it first (`verbs::handed_here`),
//!   and the desktop asks it before each write.
//!
//! Whether an entry is on chain is not carried: the desktop reads it from the chain at its next self-audit, and
//! takes into its anchor queue only what that reading finds not anchored, so what one place anchored the other never
//! sends again. Entries taken into a home are not queued here.

use crate::args::Args;
use crate::codes::{Key, Reason};
use crate::ledger;
use crate::out::{self, s, Answer, Said};
use zikaron::json::Value;
use zikaron::tokens::FindingName;
use zikaron_seal::handover::{self, HandId, Note, Refusal};

/// The handoff's note in the package's files room (named where the kit layout's names are).
pub const NOTE_FILE: &str = zikaron_glue::names::HANDOFF_NOTE;

/// The kit note a handoff file carries (its manifest's `note_md`).
const KIT_NOTE: &str = "ZIKARON ledger handoff";

/// This product's ledger form.
pub struct Zk;

impl handover::Form for Zk {
    type Entry = Vec<u8>;

    fn id(&self, e: &Vec<u8>) -> [u8; 32] {
        zikaron::entry::entry_id(e)
    }

    fn one_line(&self, all: &[Vec<u8>]) -> bool {
        if all.iter().any(|b| zikaron::entry::check(b).is_err()) {
            return false;
        }
        let Ok(root) = ledger::root_of(all) else { return false };
        let Some(outcome) = ledger::offline_input(&root, all).and_then(|i| zikaron::audit::audit_full(&i)) else { return false };
        !outcome.findings.iter().any(|f| matches!(f.name, FindingName::Equivocation | FindingName::PrevMismatch))
    }
}

/// The root (genesis entry) id of these entries, when exactly one of them is at seq 0.
fn root_id(items: &[Vec<u8>]) -> Option<[u8; 32]> {
    let roots: Vec<[u8; 32]> = items.iter().filter_map(|b| zikaron::entry::check(b).ok()).filter(|e| e.seq == 0).map(|e| e.id).collect();
    match roots.as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}

/// Entries in the order they are written: by seq.
fn by_seq(mut items: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    items.sort_by_key(|b| zikaron::entry::check(b).map(|e| e.seq).unwrap_or(u64::MAX));
    items
}

/// Seconds since the epoch now (a home's credential is checked at it).
fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Where a place's ledger is: a plain folder (opened, made when `create` and missing) or a home the desktop keeps
/// (opened with the credential, its writer lock held while this runs).
enum Place {
    Dir(zikaron_store::ledger::LedgerDir),
    Home(crate::sealed::Opened),
}

/// Open the place `at` names, and its entries. A plain folder the desktop adopted is written through the desktop's
/// sealed copy, never here (`E_ADOPTED`).
fn open_place(at: &str, create: bool) -> Result<(Place, Vec<Vec<u8>>), Answer> {
    if crate::sealed::is_home(at) {
        let home = crate::sealed::open(at, now_secs()).map_err(|t| crate::sealed::refused(&t))?;
        let items = home.items.clone();
        return Ok((Place::Home(home), items));
    }
    let dir = if create { ledger::open_or_create(at) } else { ledger::open(at) }.map_err(|t| crate::verbs::ledger_trouble(&t))?;
    let items = ledger::pile(&dir).map_err(|t| crate::verbs::ledger_trouble(&t))?;
    if let Some(root) = root_id(&items) {
        let machine = crate::sealed::machine_here().map_err(|t| crate::sealed::refused(&t))?;
        match zikaron_seal::adopted::contains(&machine, &root) {
            Ok(false) => {}
            Ok(true) => return Err(out::denied(Reason::Adopted, vec![]).telling(Said::Adopted.text().to_string())),
            Err(t) => return Err(crate::sealed::refused(&t)),
        }
    }
    Ok((Place::Dir(dir), items))
}

/// `hand-over --ledger <place> --out <file>`: the whole ledger, one line under one root, into one handoff file at
/// `--out` (never over anything there); this place then stops writing it until a handoff after this one comes back
/// (`take-over`). A place already handed makes the same handoff again (same id, same entries). A home the desktop
/// keeps is handed only past its write gates (`sealedsend::hand_gates`: not under its read-only mark, not broken).
pub fn hand_over(a: &Args) -> Answer {
    let at = a.need("ledger");
    let out_path = a.need("out");
    let (place, items) = match open_place(&at, false) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
    // A home the desktop keeps is handed only past its write gates, judged as for a send or a write.
    if let Place::Home(home) = &place {
        if let Some(refused) = crate::sealedsend::hand_gates(home) {
            return refused;
        }
    }
    let Some(root) = root_id(&items) else {
        return out::denied(if items.is_empty() { Reason::TipAbsent } else { Reason::TipForked }, vec![]);
    };
    if !handover::Form::one_line(&Zk, &items) {
        return out::denied(Reason::TipForked, vec![]);
    }
    let machine = match crate::sealed::machine_here() {
        Ok(m) => m,
        Err(t) => return crate::sealed::refused(&t),
    };
    let mut pens = match zikaron_seal::pens::read(&machine) {
        Ok(p) => p,
        Err(t) => return crate::sealed::refused(&t),
    };
    let key = match zikaron_seal::pens::key_of(&machine, &root) {
        Ok(k) => k,
        Err(t) => return crate::sealed::refused(&t),
    };
    let before = pens.pen(&key);
    let fresh = match zikaron_seal::trouble::random(16) {
        Ok(b) => HandId(b.try_into().unwrap_or([0u8; 16])),
        Err(_) => return out::unanswered(Reason::Random, vec![]),
    };
    let (note, after) = handover::give(&before, fresh);
    // The pen leaves first, so two places never hold it: if the file then fails to land the pen is put back below,
    // and if even that fails no place holds it until the next `hand-over` makes the same handoff again.
    pens.set(&key, after);
    if let Err(t) = zikaron_seal::pens::write(&machine, &pens) {
        return crate::sealed::refused(&t);
    }
    let bundle = crate::kitout::Bundle {
        entries: by_seq(items.clone()),
        files: vec![(NOTE_FILE.to_string(), note.bytes())],
        note: KIT_NOTE.to_string(),
        ..Default::default()
    };
    match crate::kitout::export(std::path::Path::new(&out_path), bundle, crate::kitout::Form::File) {
        Ok(_) => out::affirmed(vec![
            (Key::Entries, Value::Int(items.len() as u64)),
            (Key::Handoff, s(&note.id.hex())),
            (Key::Path, s(&out_path)),
        ]),
        Err(t) => {
            // Not handed after all: the pen as it was.
            pens.set(&key, before);
            let _ = zikaron_seal::pens::write(&machine, &pens);
            out::denied(Reason::Ledger, vec![(Key::Detail, s(t.code())), (Key::Path, s(&t.subject()))])
        }
    }
}

/// The handoff's refusal as this command line's answer: `E_HANDOFF` with its token, one line for people.
fn refused(token: &str, said: Said, more: Vec<(Key, Value)>) -> Answer {
    let mut ms = vec![(Key::Token, s(token))];
    ms.extend(more);
    out::denied(Reason::Handoff, ms).telling(said.text().to_string())
}

/// A handoff file's entries and note: read out (`zikaron_glue::read::unpack`), the kit law over it, its entries, its
/// note. What does not read is the token of the layer that refused it.
fn read_handoff(file: &str) -> Result<(Vec<Vec<u8>>, Note), Answer> {
    let p = std::path::Path::new(file);
    if !zikaron_glue::container::is_container_at(p) {
        return Err(refused("E_FILE_MAGIC", Said::HandoffFile, vec![]));
    }
    let unpacked = zikaron_glue::read::unpack(p).map_err(|b| refused(b.code(), Said::HandoffFile, vec![(Key::Detail, s(&b.subject()))]))?;
    let read = (|| {
        if let zikaron_kit::kitdir::KitVerdict::Fail { verdict, .. } = zikaron_kit::kitdir::verify_kit(&unpacked.dir) {
            return Err(refused(verdict.as_str(), Said::HandoffFile, vec![]));
        }
        let items = zikaron_glue::read::kit_entries(&unpacked.dir).map_err(|_| refused(zikaron_kit::tokens::KitFailToken::Unreadable.as_str(), Said::HandoffFile, vec![]))?;
        let note = std::fs::read(unpacked.dir.join(zikaron_glue::names::FILES_DIR).join(NOTE_FILE))
            .ok()
            .and_then(|b| Note::of(&b))
            .ok_or_else(|| refused("E_HANDOFF_NOTE", Said::HandoffFile, vec![]))?;
        Ok((items, note))
    })();
    drop(unpacked);
    read
}

/// `take-over --ledger <place> --handoff <file>`: take a handoff into this place, by the family's rules: a place
/// that handed its ledger takes only the handoff after its own; every entry here must be in the handoff; the
/// handoff must be one line. Then the entries this place lacks are written, by seq, and this place holds the pen.
/// A plain folder not there yet is made. Refused by name (`E_HANDOFF` and its token), nothing written.
pub fn take_over(a: &Args) -> Answer {
    let at = a.need("ledger");
    let file = a.need("handoff");
    let (theirs, note) = match read_handoff(&file) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
    let theirs = by_seq(theirs);
    let Some(root) = root_id(&theirs) else {
        return refused("NOT_ONE_LINE", Said::HandoffNotOneLine, vec![]);
    };
    let (place, mine) = match open_place(&at, true) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
    let machine = match crate::sealed::machine_here() {
        Ok(m) => m,
        Err(t) => return crate::sealed::refused(&t),
    };
    let (mut pens, key) = match zikaron_seal::pens::read(&machine).and_then(|p| zikaron_seal::pens::key_of(&machine, &root).map(|k| (p, k))) {
        Ok(x) => x,
        Err(t) => return crate::sealed::refused(&t),
    };
    let (new, pen) = match handover::take(&Zk, &pens.pen(&key), &mine, &note, &theirs) {
        Ok(x) => x,
        Err(Refusal::NotBack { handed }) => return refused("NOT_BACK", Said::HandoffNotBack, vec![(Key::Handoff, s(&handed.hex()))]),
        Err(Refusal::Ahead(id)) => return refused("AHEAD", Said::HandoffAhead, vec![(Key::EntryId, s(&zikaron::hexfmt::encode(&id)))]),
        Err(Refusal::NotOneLine) | Err(Refusal::Handed(_)) => return refused("NOT_ONE_LINE", Said::HandoffNotOneLine, vec![]),
    };
    let mut written = 0u64;
    for b in new {
        let id = zikaron::hexfmt::encode(&zikaron::entry::entry_id(b));
        let landed = match &place {
            Place::Dir(dir) => match zikaron::entry::check(b) {
                Ok(e) => ledger::append(dir, &e, b).map(|st| matches!(st, zikaron_store::ledger::Stored::Written)).map_err(|t| crate::verbs::ledger_trouble(&t)),
                Err(t) => Err(out::denied(Reason::Entry, vec![(Key::Token, s(t.as_str()))])),
            },
            Place::Home(home) => home.take(&id, b).map_err(|t| crate::sealed::refused(&t)),
        };
        match landed {
            Ok(true) => written += 1,
            Ok(false) => {}
            Err(answer) => return answer,
        }
    }
    // The pen is taken last: if writing stops partway, the pen stays where it was and the same handoff can be taken
    // again.
    pens.set(&key, pen);
    if let Err(t) = zikaron_seal::pens::write(&machine, &pens) {
        return crate::sealed::refused(&t);
    }
    let ledger_at = match &place {
        Place::Dir(dir) => dir.root().to_string_lossy().into_owned(),
        Place::Home(home) => home.ledger_root(),
    };
    out::affirmed(vec![
        (Key::Count, Value::Int(written)),
        (Key::Entries, Value::Int(theirs.len() as u64)),
        (Key::Handoff, s(&note.id.hex())),
        (Key::Ledger, s(&ledger_at)),
    ])
}
