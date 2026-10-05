//! Others' ledger reader. "Anchors seen, bytes not seen" is an honest state, not a failure.
//!
//! ─── No indexer ───
//!
//! Type an address and scan its anchors by the endpoint rule. The address book is purely local and never
//! becomes a directory: this layer neither discovers nor enumerates addresses and asks no one "who is there";
//! the book holds only what the person pasted in.
//!
//! ─── This desk does not guess the bytes ───
//!
//! The chain has only anchor hashes, no bytes. Bytes come from two places: the counterpart's public storage,
//! or a disclosure kit they give. With neither, the face says "anchors seen, bytes not seen": how many
//! anchors were found is stated plainly, while audit label, timeline and grant history have no reading (not
//! empty, not green).
//!
//! ─── With bytes, judging is still not in this layer ───
//!
//! The label comes from the core's `audit`, entries are accepted by the core's thirteen steps, and grant
//! history comes from the grant ledger's table reading. This layer lays the root, the fragment and the bytes out as an
//! audit input and hands it over.

use crate::fault::{Fault, Known};
use zikaron::json::Value;

/// One read's result.
pub struct Book {
    /// The typed address (lowercase).
    pub who: String,
    /// How many anchors were found.
    pub anchors: usize,
    /// How many endpoints were asked.
    pub asked: usize,
    /// How many entries' bytes were obtained. Zero means "anchors seen, bytes not seen".
    pub entries: usize,
    /// With bytes, the core's label; without, an empty string (no reading then).
    pub label: String,
    /// Timeline: rows in descending seq (only with bytes).
    pub timeline: Vec<crate::ledgerx::Row>,
    /// Grant history (only with bytes).
    pub grants: Vec<crate::grantx::Row>,
    /// Block time of the latest anchor (the largest in the fragment this pass scanned; none without anchors).
    /// "Last anchored" reads it.
    pub latest: Option<u64>,
}

impl Book {
    /// Anchors seen, bytes not seen. A state, not a failure.
    pub fn only_anchors(&self) -> bool {
        self.entries == 0
    }
}

/// Recognize an address. Unrecognized is refused by name, never scanning with an arbitrary string.
pub fn who(typed: &str) -> Result<crate::key::Address, Fault> {
    crate::key::Address::parse(typed)
        .ok_or_else(|| Fault::known(Known::AddressShape, typed.trim().to_string()))
}

/// Read bytes from a disclosure kit or a directory. As many entries as can be read are reported; what cannot
/// be read as an entry does not count.
///
/// Uses the store crate's names (the `.entry` form comes from `layout`), so "which files are entries" is not
/// decided here.
pub fn bytes_from(dir: &std::path::Path) -> Result<Vec<Vec<u8>>, Fault> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(listing) = std::fs::read_dir(&at) else { continue };
        let mut names: Vec<std::path::PathBuf> =
            listing.filter_map(|e| e.ok().map(|e| e.path())).collect();
        names.sort();
        for p in names {
            let Ok(md) = std::fs::symlink_metadata(&p) else { continue };
            if md.is_dir() {
                stack.push(p);
                continue;
            }
            if !md.is_file() {
                continue;
            }
            let name = p.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            if zikaron_store::layout::parse_entry_file(&name).is_none() {
                continue;
            }
            if let Ok(b) = std::fs::read(&p) {
                if zikaron::entry::check(&b).is_ok() {
                    out.push(b);
                }
            }
        }
    }
    if out.is_empty() {
        return Err(Fault::known(Known::NoBytes, dir.display().to_string()));
    }
    Ok(out)
}

/// The basis this pass scans. Senders come from that ledger's own lineage (the genesis key, plus every key
/// handed over by its successions); with not one byte in hand only the entered address can be scanned, and
/// the page then says exactly "anchors seen, bytes not seen".
///
/// With senders fixed as `[entered address]`, a ledger that went through succession would have none of the
/// new key's anchors scanned, while the page top would still print that number as "all of this person's
/// anchors on chain".
pub fn basis_for(
    ground: &crate::auditx::Ground,
    who: &crate::key::Address,
    bytes: &[Vec<u8>],
) -> crate::auditx::Ground {
    let mut g = ground.clone();
    let line = crate::auditx::senders_of(bytes);
    g.senders = if line.is_empty() { vec![who.hex()] } else { line };
    g
}

pub fn read(
    eps: &[crate::chainx::Endpoint],
    ground: &crate::auditx::Ground,
    who: &crate::key::Address,
    bytes: &[Vec<u8>],
) -> Result<Book, Fault> {
    let g = basis_for(ground, who, bytes);
    let scanned = crate::auditx::scan_once(eps, &g)?;
    read_scanned(&scanned, who, bytes)
}

/// [`read`] across networks: the main network and every read-only network (`widex`), each chain on its own.
/// Returns the networks not read beside the ledger.
pub fn read_wide(
    eps: &[crate::chainx::Endpoint],
    ground: &crate::auditx::Ground,
    who: &crate::key::Address,
    bytes: &[Vec<u8>],
    nets: &[crate::readnets::Net],
) -> Result<(Book, Vec<crate::widex::Missed>), Fault> {
    let (scanned, missed) = scan_wide(eps, ground, who, bytes, nets)?;
    let mut book = read_scanned(&scanned, who, bytes)?;
    unread_where_missed(&mut book, &missed);
    Ok((book, missed))
}

/// A pass that left a network out cannot say an entry no read chain reaches is not on chain: its anchor may be
/// on the network not read. So with any network missed, every timeline row lit "not on chain" (`Landed`) reads
/// "chain not read" (`ChainUnread`). A pass that read every network is left as it is.
pub fn unread_where_missed(book: &mut Book, missed: &[crate::widex::Missed]) {
    if missed.is_empty() {
        return;
    }
    for row in book.timeline.iter_mut().filter(|r| r.lamp == crate::ledgerx::Lamp::Landed) {
        row.lamp = crate::ledgerx::Lamp::ChainUnread;
    }
}

/// One scan across networks, as one scan's reading (the diligence desk reads its fragment too).
pub fn scan_wide(
    eps: &[crate::chainx::Endpoint],
    ground: &crate::auditx::Ground,
    who: &crate::key::Address,
    bytes: &[Vec<u8>],
    nets: &[crate::readnets::Net],
) -> Result<(crate::auditx::Scanned1, Vec<crate::widex::Missed>), Fault> {
    let g = basis_for(ground, who, bytes);
    let w = crate::widex::scan(Some((eps, &g)), nets, &g.senders, crate::widex::Ask::First)?;
    Ok((crate::auditx::Scanned1 { anchors: w.anchors, asked: w.asked, fragment: w.fragment }, w.missed))
}

/// The after-scan half: read one scan's reading plus a stack of bytes as a ledger.
///
/// Kept apart from [`read`] so the diligence desk can ask for depth with the same scan's fragment: the
/// basis the quantities refer to and the anchor count at the page top must come from the same pass (the
/// fragment is scanned once and read by both).
pub fn read_scanned(
    scanned: &crate::auditx::Scanned1,
    who: &crate::key::Address,
    bytes: &[Vec<u8>],
) -> Result<Book, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::D1);
    let (anchors, asked) = (scanned.anchors, scanned.asked);
    let latest = crate::vaultx::latest_anchor_of(&scanned.fragment);
    if bytes.is_empty() {
        return Ok(Book {
            who: who.hex(),
            anchors,
            asked,
            entries: 0,
            label: String::new(),
            timeline: Vec::new(),
            grants: Vec::new(),
            latest,
        });
    }
    // With bytes: the core produces the report; timeline and grant history are read like this seat's two
    // tables.
    //
    // The fragment is the one just scanned, not an empty one. Under an empty fragment the report's anchor set
    // is always empty, so every row below would print "recorded, not anchored", a claim never verified (while
    // the page top honestly says how many anchors were found).
    let v = crate::auditx::ask_from(bytes, &scanned.fragment, Vec::new(), asked, true)?;
    // These bytes must really be their ledger. The directory cell is chosen by the person; pointing it at a
    // third person's disclosure kit would show that ledger under the entered address (timeline, label and
    // grant history all someone else's). This closes the "identity and bytes parted" form: the report carries
    // whom it is about, and this compares once. The check is against the ledger's lineage (the genesis key
    // plus every key handed over by successions, the same algorithm as self-audit), not only the genesis key:
    // a ledger that went through succession must be readable when the new key reads its own ledger.
    let line = crate::auditx::senders_of(bytes);
    if !line.iter().any(|a| a.eq_ignore_ascii_case(&who.hex())) {
        return Err(Fault::known(
            Known::NotHisBook,
            crate::lang::filln(crate::lang::Key::Tail202, &[&(who.hex()).to_string(), &(line.join(" ")).to_string()]),
        ));
    }
    let anchored = crate::ledgerx::anchored_of(&v.report);
    let mut timeline: Vec<crate::ledgerx::Row> = Vec::new();
    let mut grants: Vec<crate::grantx::Row> = Vec::new();
    let revoked: Vec<String> = bytes
        .iter()
        .filter_map(|b| zikaron::entry::check(b).ok())
        .filter(|e| e.kind == zikaron::tokens::EntryType::Revocation)
        .filter_map(|e| match &e.body {
            Value::Obj(m) => m.iter().find(|(k, _)| k == "grant").and_then(|(_, x)| match x {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            }),
            _ => None,
        })
        .collect();
    for b in bytes {
        let Ok(e) = zikaron::entry::check(b) else { continue };
        let id = zikaron::hexfmt::encode(&e.id);
        let tx = anchored.iter().find(|(h, _)| *h == id).map(|(_, t)| t.clone());
        timeline.push(crate::ledgerx::Row {
            seq: e.seq,
            kind: e.kind,
            summary: crate::ledgerx::summarize(e.kind, &e.body),
            work: crate::ledgerx::work_of(e.kind, &e.body),
            facts: crate::ledgerx::facts_of(&e),
            prev: e.prev.clone(),
            author: e.author.clone(),
            lamp: if tx.is_some() {
                crate::ledgerx::Lamp::Anchored
            } else {
                crate::ledgerx::Lamp::Landed
            },
            tx,
            bytes: b.len(),
            id: id.clone(),
            anchored_at: None,
        });
        if e.kind == zikaron::tokens::EntryType::Grant {
            let g = |k: &str| -> String {
                match &e.body {
                    Value::Obj(m) => m
                        .iter()
                        .find(|(n, _)| n == k)
                        .and_then(|(_, x)| match x {
                            Value::Str(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_default(),
                    _ => String::new(),
                }
            };
            grants.push(crate::grantx::Row {
                grantee: g("grantee"),
                work: g("work"),
                terms: g("terms"),
                // The window is read from the entry (the same reading as this seat's register): a fixed
                // `None` would make every grant on others' ledgers show "no reading", and the diligence
                // desk's double-sale check would treat "no window" as "forever", turning every live grant
                // red.
                window: crate::grantx::window_of(&e.body),
                // On someone else's ledger this machine's exclusive flag never counts: it is this machine's
                // bookkeeping, not theirs.
                exclusive: false,
                exclusive_from: crate::termsx::Exclusive::No,
                doc: None,
                doc_name: None,
                revoked: revoked.iter().any(|r| *r == id),
                seq: e.seq,
                id,
            });
        }
    }
    timeline.sort_by(|a, b| (b.seq, &a.id).cmp(&(a.seq, &b.id)));
    crate::ledgerx::stamp(&mut timeline, bytes, &scanned.fragment);
    grants.sort_by(|a, b| (b.seq, &a.id).cmp(&(a.seq, &b.id)));
    Ok(Book {
        latest,
        who: who.hex(),
        anchors,
        asked,
        entries: bytes.len(),
        label: v.label,
        timeline,
        grants,
    })
}
