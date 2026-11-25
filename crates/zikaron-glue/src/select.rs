use crate::names::Field;
use crate::book::{self as entry, Entry, Kind as EntryType};

#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub from: Option<u64>,
    pub to: Option<u64>,
    pub work: Option<String>,
    pub ids: Vec<String>,
}

pub struct Chosen {
    pub items: Vec<Vec<u8>>,
    pub pulled: Vec<String>,
    pub spine: Vec<String>,
}

fn about(e: &Entry, work: &str) -> bool {
    let member = match e.kind {
        EntryType::History => Field::Content,
        EntryType::Grant => Field::Work,
        _ => return false,
    };
    e.body.member(member.as_str()).and_then(|x| x.as_str()) == Some(work)
}

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

pub fn pull_lineage(read: &[(usize, Entry)], take: &mut Vec<usize>) -> Vec<String> {
    crate::seam_v2();
    if take.is_empty() {
        return Vec::new();
    }
    let mut spine: Vec<String> = Vec::new();
    for (i, e) in read {
        if take.contains(i) || !matches!(e.kind, EntryType::Genesis | EntryType::Succession | EntryType::Migration) {
            continue;
        }
        take.push(*i);
        spine.push(e.id_hex());
    }
    spine
}

pub fn read_ledger(root: &str) -> Result<Vec<Vec<u8>>, zikaron_store::codes::Trouble> {
    crate::seam_v2();
    let dir = zikaron_store::ledger::LedgerDir::open(root)?;
    Ok(dir.pile()?.items)
}
