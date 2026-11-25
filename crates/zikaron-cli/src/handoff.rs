use crate::args::Args;
use crate::codes::{Key, Reason};
use crate::ledger;
use crate::out::{self, s, Answer, Said};
use zikaron::json::Value;
use zikaron::tokens::FindingName;
use zikaron_seal::handover::{self, HandId, Note, Refusal};

pub const NOTE_FILE: &str = zikaron_glue::names::HANDOFF_NOTE;

const KIT_NOTE: &str = "ZIKARON ledger handoff";

pub struct Zk;

impl handover::Form for Zk {
    type Entry = Vec<u8>;

    fn id(&self, e: &Vec<u8>) -> [u8; 32] {
        zikaron_glue::book::entry_id(e)
    }

    fn one_line(&self, all: &[Vec<u8>]) -> bool {
        if all.iter().any(|b| zikaron_glue::book::check(b).is_err()) {
            return false;
        }
        let Ok(root) = ledger::root_of(all) else { return false };
        let Ok(read) = ledger::read(&root, all) else { return false };
        let names = [FindingName::Equivocation.as_str(), FindingName::PrevMismatch.as_str()];
        read.joined() && !read.rows("findings").iter().any(|f| f.member("name").and_then(Value::as_str).is_some_and(|n| names.contains(&n)))
    }
}

fn root_id(items: &[Vec<u8>]) -> Option<[u8; 32]> {
    ledger::root_of(items).ok()?;
    zikaron_glue::book::root_entry(items).map(|e| e.id)
}

fn by_seq(mut items: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    items.sort_by_key(|b| zikaron_glue::book::check(b).map(|e| (e.law, e.seq)).unwrap_or((zikaron_glue::book::Law::Two, u64::MAX)));
    items
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

enum Place {
    Dir(zikaron_store::ledger::LedgerDir),
    Home(crate::sealed::Opened),
}

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

pub fn hand_over(a: &Args) -> Answer {
    let at = a.need("ledger");
    let out_path = a.need("out");
    let (place, items) = match open_place(&at, false) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
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
            pens.set(&key, before);
            let _ = zikaron_seal::pens::write(&machine, &pens);
            out::denied(Reason::Ledger, vec![(Key::Detail, s(t.code())), (Key::Path, s(&t.subject()))])
        }
    }
}

fn refused(token: &str, said: Said, more: Vec<(Key, Value)>) -> Answer {
    let mut ms = vec![(Key::Token, s(token))];
    ms.extend(more);
    out::denied(Reason::Handoff, ms).telling(said.text().to_string())
}

fn read_handoff(file: &str) -> Result<(Vec<Vec<u8>>, Note), Answer> {
    let p = std::path::Path::new(file);
    if !zikaron_glue::container::is_container_at(p) {
        return Err(refused("E_FILE_MAGIC", Said::HandoffFile, vec![]));
    }
    let unpacked = zikaron_glue::read::unpack(p).map_err(|b| refused(b.code(), Said::HandoffFile, vec![(Key::Detail, s(&b.subject()))]))?;
    let read = (|| {
        if let Err((verdict, _)) = zikaron_glue::book::kit_verdict(&unpacked.dir) {
            return Err(refused(&verdict, Said::HandoffFile, vec![]));
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
        let id = zikaron::hexfmt::encode(&zikaron_glue::book::entry_id(b));
        let landed = match &place {
            Place::Dir(dir) => match zikaron_glue::book::check(b) {
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
