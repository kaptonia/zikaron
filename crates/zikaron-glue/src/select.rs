//! Choosing entries: by seq range or by work digest, plus one closure rule.
//!
//! A revocation always travels with the grant it revokes. If a chosen entry is a grant and the ledger holds a
//! revocation pointing at it, that revocation enters the kit whatever range the caller gave.
//!
//! This is fixed in the selector because the sixth grant check (REVOKED) looks for that revocation in the
//! pile at hand. Left out, the recipient's checks would pass a revoked grant. Of all omissions this is the
//! only one that flips a verdict: a missing lineage link makes the audit report SEQ_GAP, the label drop to
//! GAPS and the checks fall to PARTIAL, which says "not known yet", not "fine".
//!
//! The ledger's spine travels with the kit: a partial kit also carries the genesis entry and every
//! succession, so the verifier can tell whose ledger it is and which keys' anchors to look for on chain. The
//! spine is not a record and is not listed among the recipient's records; it only answers "whose entries are
//! these".

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
    /// Named entry ids (`0x` plus 64 lowercase hex): when non-empty, keep only these. Named choices pass the
    /// closure rule too.
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

/// About this work: a history whose `content` is it (the kit law §9.2 reading, the same predicate as the kit
/// core's depth), or a grant whose `work` is it.
///
/// The second half is a product choice, not a law reading: the law does not say which entries belong to a
/// work, and a kit with history but no grants is of no use to the other party.
fn about(e: &Entry, work: &str) -> bool {
    let member = match e.kind {
        EntryType::History => Field::Content,
        EntryType::Grant => Field::Work,
        _ => return false,
    };
    e.body.member(member.as_str()).and_then(|x| x.as_str()) == Some(work)
}

/// The closure rule: for each chosen grant, pull in the revocations pointing at it, and name them.
///
/// It stands alone because this is the only omission that flips a verdict.
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

/// Choose. Bytes that fail §4.3 are never chosen (in a kit they would only be an `invalid` row in the
/// manifest).
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

/// The spine rule: whatever is chosen brings the ledger's spine (genesis and every succession) along, named.
/// Nothing chosen pulls nothing (an empty named choice is refused above).
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

/// Read the pile of a ledger directory through the storage crate's strict read (its bytes are the audit
/// pile).
///
/// Kit output neither writes nor reads the directory directly: what is there, what cannot be accounted for
/// and what is over the cap are all judged and named by the storage crate.
pub fn read_ledger(root: &str) -> Result<Vec<Vec<u8>>, zikaron_store::codes::Trouble> {
    crate::seam_v2();
    let dir = zikaron_store::ledger::LedgerDir::open(root)?;
    Ok(dir.pile()?.items)
}
