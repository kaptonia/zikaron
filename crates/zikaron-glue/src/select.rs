//! Choosing entries for a kit: by seq range, work digest or named ids, plus closure rules.
//!
//! A revocation always travels with the grant it revokes: if a chosen grant has a revocation in the ledger,
//! that revocation enters the kit whatever range the caller gave. The sixth grant check (REVOKED) looks for it
//! in the pile at hand, so leaving it out would make the recipient pass a revoked grant. It is the only
//! omission that flips a verdict; a missing lineage link instead gives SEQ_GAP, label GAPS and checks PARTIAL
//! ("not known yet", not "fine").
//!
//! The ledger's spine also travels with the kit: a partial kit carries the genesis entry and every succession,
//! so the verifier can tell whose ledger it is and which keys' anchors to look for on chain. The spine is not
//! listed among the recipient's records.

use crate::names::Field;
use zikaron::entry::{self, Entry};
use zikaron::tokens::EntryType;

/// The selector. With neither field set, everything is chosen.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    /// Lower seq bound (inclusive).
    pub from: Option<u64>,
    /// Upper seq bound (inclusive).
    pub to: Option<u64>,
    /// Work digest: keep only entries about this work.
    pub work: Option<String>,
    /// Named entry ids (`0x` plus 64 lowercase hex): when non-empty, keep only these. The closure rules still
    /// apply.
    pub ids: Vec<String>,
}

/// The result of a choice: the chosen bytes, what the closure rule pulled in, and the ledger spine.
pub struct Chosen {
    pub items: Vec<Vec<u8>>,
    /// Ids pulled in by the revocation closure (reported, never added silently).
    pub pulled: Vec<String>,
    /// Ids of the spine pulled in (genesis and successions); not listed when already chosen.
    pub spine: Vec<String>,
}

/// About this work: a history whose `content` is the digest (the kit law §9.2 reading, the same
/// predicate as the kit core's depth), or a grant whose `work` is it.
///
/// Including grants is a product choice, not a spec rule: the spec does not say which entries belong to a
/// work, and a kit with history but no grants is of no use to the other party.
fn about(e: &Entry, work: &str) -> bool {
    let member = match e.kind {
        EntryType::History => Field::Content,
        EntryType::Grant => Field::Work,
        _ => return false,
    };
    e.body.member(member.as_str()).and_then(|x| x.as_str()) == Some(work)
}

/// The revocation closure: for each chosen grant, pull in and name the revocations pointing at it (the only
/// omission that would flip a verdict).
pub fn pull_revocations(read: &[(usize, Entry)], take: &mut Vec<usize>) -> Vec<String> {
    crate::seam_v2();
    let picked: Vec<String> = read
        .iter()
        .filter(|(i, e)| take.contains(i) && e.kind == EntryType::Grant)
        .map(|(_, e)| e.id_hex())
        .collect();
    let mut pulled: Vec<String> = Vec::new();
    for (i, e) in read {
        if take.contains(i) || e.kind != EntryType::Revocation {
            continue;
        }
        let Some(target) = e.body.member(Field::Grant.as_str()).and_then(|x| x.as_str()) else {
            continue;
        };
        if picked.iter().any(|p| p == target) {
            take.push(*i);
            pulled.push(e.id_hex());
        }
    }
    pulled
}

/// Choose entries. Bytes that fail law §4.3 are never chosen (in a kit they would only be an `invalid`
/// manifest row).
pub fn choose(pile: &[Vec<u8>], sel: &Selection) -> Chosen {
    crate::seam_v2();
    let read: Vec<(usize, Entry)> = pile
        .iter()
        .enumerate()
        .filter_map(|(i, b)| entry::check(b).ok().map(|e| (i, e)))
        .collect();

    let mut take: Vec<usize> = Vec::new();
    for (i, e) in &read {
        if let Some(lo) = sel.from {
            if e.seq < lo {
                continue;
            }
        }
        if let Some(hi) = sel.to {
            if e.seq > hi {
                continue;
            }
        }
        if let Some(w) = &sel.work {
            if !about(e, w) {
                continue;
            }
        }
        if !sel.ids.is_empty() && !sel.ids.contains(&e.id_hex()) {
            continue;
        }
        take.push(*i);
    }

    let mut pulled = pull_revocations(&read, &mut take);
    let mut spine = pull_lineage(&read, &mut take);

    take.sort_unstable();
    pulled.sort();
    spine.sort();
    Chosen {
        items: take.iter().map(|i| pile[*i].clone()).collect(),
        pulled,
        spine,
    }
}

/// The spine rule: any non-empty choice brings the ledger's spine (genesis and every succession), named. An
/// empty choice pulls nothing (an empty named choice is refused above).
pub fn pull_lineage(read: &[(usize, Entry)], take: &mut Vec<usize>) -> Vec<String> {
    crate::seam_v2();
    if take.is_empty() {
        return Vec::new();
    }
    let mut spine: Vec<String> = Vec::new();
    for (i, e) in read {
        if take.contains(i) || !matches!(e.kind, EntryType::Genesis | EntryType::Succession) {
            continue;
        }
        take.push(*i);
        spine.push(e.id_hex());
    }
    spine
}

/// Read a ledger directory's pile through the storage crate's strict read (its bytes are the audit pile). Kit
/// output never touches the directory directly: unknown files and size caps are handled by the storage crate.
pub fn read_ledger(root: &str) -> Result<Vec<Vec<u8>>, zikaron_store::codes::Trouble> {
    crate::seam_v2();
    let dir = zikaron_store::ledger::LedgerDir::open(root)?;
    Ok(dir.pile()?.items)
}
