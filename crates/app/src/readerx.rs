//! Reader for other people's ledgers. "Anchors seen, bytes not seen" is a valid state, not a failure.
//!
//! ─── No indexer ───
//!
//! The user types an address and its anchors are scanned via the configured endpoints. The address book is
//! purely local: this module never discovers or enumerates addresses; the book holds only what the user
//! pasted in.
//!
//! ─── No guessing the bytes ───
//!
//! The chain holds only anchor hashes. Entry bytes come from the other party's public storage or from a
//! disclosure kit they provide. Without them the UI says "anchors seen, bytes not seen": the anchor count is
//! shown, while audit label, timeline and grant history show no result (not empty, not green).
//!
//! ─── Validation stays in the core ───
//!
//! The label comes from the core's `audit`, entries are validated by the core's `entry::check`, and grant
//! history comes from the grant table reader. This module only assembles the root, chain data and bytes into
//! an audit input.

use crate::fault::{Fault, Known};
use zikaron::json::Value;

/// The result of reading one ledger.
pub struct Book {
    /// The typed address (lowercase).
    pub who: String,
    /// How many anchors were found.
    pub anchors: usize,
    /// How many endpoints were asked.
    pub asked: usize,
    /// How many entries' bytes were obtained. Zero means "anchors seen, bytes not seen".
    pub entries: usize,
    /// The core's audit label with bytes; empty without (no result).
    pub label: String,
    /// Timeline: rows in descending seq (only with bytes).
    pub timeline: Vec<crate::ledgerx::Row>,
    /// Grant history (only with bytes).
    pub grants: Vec<crate::grantx::Row>,
    /// Block time of the latest anchor in the scanned chain data (`None` without anchors), shown as "last
    /// anchored".
    pub latest: Option<u64>,
}

impl Book {
    /// Anchors seen, bytes not seen: a valid state, not a failure.
    pub fn only_anchors(&self) -> bool {
        self.entries == 0
    }
}

/// Parse an address. Invalid input is an error; nothing is ever scanned with an arbitrary string.
pub fn who(typed: &str) -> Result<crate::key::Address, Fault> {
    crate::key::Address::parse(typed)
        .ok_or_else(|| Fault::known(Known::AddressShape, typed.trim().to_string()))
}

/// Read entry bytes from a disclosure kit or directory. Every valid entry found is returned; anything else is
/// skipped. Errors with `NO_BYTES` when none is found.
///
/// Which files are entries is decided by the store crate's naming (`layout`), not here.
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

/// The scan basis. Senders come from the ledger's own lineage (the genesis key plus every key handed over by
/// its successions); with no bytes only the entered address can be scanned, and the page says "anchors seen,
/// bytes not seen".
///
/// Scanning only the entered address would miss the new key's anchors after a succession, while the page
/// would still present the count as all of this person's anchors.
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

/// Scan and read one ledger on the main network.
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

/// [`read`] across networks: the main network and every read-only network (`widex`), each chain separately.
/// Also returns the networks that could not be read.
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

/// If any network could not be read, an entry with no anchor on the networks read may still be anchored on
/// the missing one, so every timeline row marked "not on chain" (`Landed`) becomes "chain not read"
/// (`ChainUnread`). Unchanged when every network was read.
pub fn unread_where_missed(book: &mut Book, missed: &[crate::widex::Missed]) {
    if missed.is_empty() {
        return;
    }
    for row in book.timeline.iter_mut().filter(|r| r.lamp == crate::ledgerx::Lamp::Landed) {
        row.lamp = crate::ledgerx::Lamp::ChainUnread;
    }
}

/// One scan across networks, returned as a single scan result (the diligence view also uses its chain data).
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

/// The post-scan half: read a scan result plus entry bytes as a ledger.
///
/// Separate from [`read`] so the diligence view can compute depth from the same scan's chain data: its
/// figures and the anchor count at the top of the page must come from the same scan.
pub fn read_scanned(
    scanned: &crate::auditx::Scanned1,
    who: &crate::key::Address,
    bytes: &[Vec<u8>],
) -> Result<Book, Fault> {
    // Traced here so direct calls that bypass `apply` (tests, the CLI) are traced too.
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
    // With bytes: the core produces the report; timeline and grant history are built as for one's own ledger.
    //
    // Audit against the chain data just scanned, not an empty one: with empty chain data the anchor set is
    // always empty and every row would wrongly show "recorded, not anchored".
    let v = crate::auditx::ask_from(bytes, &scanned.fragment, Vec::new(), asked, true)?;
    // The bytes must belong to the entered address. The user picks the directory, and pointing it at a third
    // party's kit would show someone else's ledger under this address. The check uses the ledger's lineage
    // (genesis key plus every key handed over by successions, as in self-audit), so a ledger is still
    // recognized after a succession.
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
                // Read the window from the entry, as for one's own grants: `None` would show "no result" for
                // every grant, and the diligence double-sale check would treat "no window" as "forever",
                // flagging every live grant red.
                window: crate::grantx::window_of(&e.body),
                // This machine's exclusivity flag is local bookkeeping and never applies to others' ledgers.
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
