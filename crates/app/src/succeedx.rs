//! Succession desk. The semantics of transferring rights are in the core; this layer only lays out
//! parameters and raises alarms.
//!
//! One key, one ledger (law §7.5): succession only goes to a new key, so `to` should never be a key that has
//! opened its own ledger. This desk checks the part it can see: it scans that address on the declared basis,
//! and any anchor it has sent turns red.
//!
//! Whether `to` has a genesis entry is invisible here: genesis is bytes, and the chain holds only anchor
//! hashes. So this layer guards the half of the rule visible on chain and says so in the UI. Finding nothing
//! does not prove the key is new; the UI then says "zero found", not "it is a new key".
//!
//! Once the succession entry is anchored, this desk has handed over: [`handed_over`] reads the ledger and
//! answers whose ledger this is from now on, and `Shell::writable` asks it. So the old machine becoming
//! read-only is not a separate hookup but part of the same writer predicate.

use crate::fault::{Fault, Known};
use crate::key::Address;
use zikaron::json::Value;
use zikaron::tokens::EntryType;

/// The two preset kinds (law §6.7: any token is a legal value; these two are ready-made choices for the UI).
pub const KIND_HANDOVER: &str = "handover";
pub const KIND_ROTATION: &str = "keyrotation";

/// The kinds offered in the UI: the screen shows plain words, and the entry holds the token. Ordered as in
/// the dropdown.
pub const KINDS: [&str; 2] = [KIND_ROTATION, KIND_HANDOVER];

/// Fill the two fields this desk completes for the person: an empty effective time means the moment of
/// signing (`now`, from the shell's clock); an empty note means the kind's plain words (other tokens have
/// none, so the note stays empty and [`succession_body`] refuses the missing field). Values the person gave
/// are used as given. Returns (effective time, note).
pub fn fill(kind: &str, effective: &str, statement_md: &str, now: u64) -> (String, String) {
    let effective = if effective.trim().is_empty() { now.to_string() } else { effective.to_string() };
    let statement = if statement_md.trim().is_empty() { kind_words(kind).unwrap_or("").to_string() } else { statement_md.to_string() };
    (effective, statement)
}

/// A kind's plain words (shown in the dropdown, and written into the entry when "note" is empty). Tokens
/// beyond the two have none.
pub fn kind_words(kind: &str) -> Option<&'static str> {
    use crate::lang::{t, Key};
    match kind.trim() {
        KIND_ROTATION => Some(t(Key::SucceedKindRotation)),
        KIND_HANDOVER => Some(t(Key::SucceedKindHandover)),
        _ => None,
    }
}

/// The body of `succession` (law §6.7: `to` hex20, `kind` token, `effective` int, `statement_md` prose, all
/// four required).
pub fn succession_body(
    to: &str,
    kind: &str,
    effective: &str,
    statement_md: &str,
) -> Result<Value, Fault> {
    // Public functions emit their trace mark, so direct calls that bypass `apply` (tests, the CLI) are traced
    // too.
    crate::trace::mark(crate::feature::Feature::W11);
    let need = |s: &str, what: &str| -> Result<String, Fault> {
        let t = s.trim();
        if t.is_empty() {
            return Err(Fault::known(Known::FieldMissing, what.to_string()));
        }
        Ok(t.to_string())
    };
    let eff: u64 = crate::fault::whole_within_ceiling(effective, crate::lang::Key::Tail217)?;
    Ok(Value::Obj(vec![
        ("effective".to_string(), Value::Int(eff)),
        ("kind".to_string(), Value::Str(need(kind, "kind")?)),
        ("statement_md".to_string(), Value::Str(need(statement_md, "statement_md")?)),
        ("to".to_string(), Value::Str(need(to, "to")?)),
    ]))
}

/// The reading of one scan for `to`.
pub struct Sighting {
    pub to: Address,
    /// How many anchors this address sent on the declared basis. Zero does not prove the key is new.
    pub anchors: usize,
    /// How many endpoints were asked.
    pub asked: usize,
}

impl Sighting {
    /// Red when the address has sent any anchor.
    pub fn red(&self) -> bool {
        self.anchors > 0
    }
}

/// Run one basis scan for `to`. The basis is built here from that address (the sender this pass scans); the
/// scan is still the anchoring crate's, and judging "is this a basis" is still the core's.
pub fn look_at(
    eps: &[crate::chainx::Endpoint],
    ground: &crate::auditx::Ground,
    to: &Address,
) -> Result<Sighting, Fault> {
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::t(crate::lang::Key::Tail218).to_string()));
    }
    let mut g = ground.clone();
    g.senders = vec![to.hex()];
    let scanned = crate::auditx::scan_once(eps, &g)?;
    Ok(Sighting { to: *to, anchors: scanned.anchors, asked: scanned.asked })
}

/// Who writes this ledger from now on, read from the ledger itself with lineage computed by the core: this
/// key wrote a succession in the lineage, and the latest succession in the lineage hands to a key other than
/// this one.
///
/// `Some(new key address)` means this desk has handed over; `None` means the ledger is still its own. Without
/// a local key it returns `None`: ownership cannot be asked yet, and writing has two other gates, the lock and
/// the pen.
///
/// Only successions in the lineage written by this key count. Anyone can copy entries into a ledger
/// directory: taking the latest succession in the directory and asking only whether its `to` is me would let
/// a succession signed by another key make an author without a backup read as "handed over"; the delete check
/// would then let the identity be deleted and the key would be lost forever. So successions first pass the
/// core's lineage (law §7.4: only successions from the root, signed by keys in the set, bring `to` into the
/// set), which drops those outside it, and then this key must have written one of them. Successions written by
/// others, outside the lineage, or handing back to itself never count as a handover. The ledger page banner
/// and the write gates (`identity::delete`'s first check, the entry-writing gate) all ask here.
pub fn handed_over(items: &[Vec<u8>], mine: Option<Address>) -> Option<String> {
    let me = mine?.hex();
    // An unrecognizable root (no genesis, forked root) means the lineage cannot be computed, and this ledger
    // was handed to no one.
    let lineage = crate::auditx::outcome_of(items, &crate::auditx::empty_fragment()).ok()?.ledger;
    // Pick the latest succession first, then ask whom it hands to.
    //
    // Skipping successions that hand back to me while picking would, after A hands to B and B hands back to
    // A, still let the old "hand to B" win on A's machine, leaving the ledger read-only forever although the
    // ledger says it came back. Picking and judging must stay separate.
    //
    // The compared values must be of the same kind: `best` stores and compares `(seq, id)`. Storing
    // `(seq, to)` while comparing `(seq, id)` would order 66-character ids against 42-character addresses, and
    // which of two same-seq successions wins would depend on directory read order.
    let mut best: Option<(u64, String, String)> = None;
    let mut wrote = false;
    for e in &lineage {
        if e.kind != EntryType::Succession {
            continue;
        }
        let Value::Obj(m) = &e.body else { continue };
        let Some((_, Value::Str(to))) = m.iter().find(|(k, _)| k == "to") else { continue };
        if e.author.eq_ignore_ascii_case(&me) {
            wrote = true;
        }
        // Same seq is ordered by id, so two passes over one ledger answer the same entry.
        let id = zikaron::hexfmt::encode(&e.id);
        let better = match &best {
            None => true,
            Some((s, bid, _)) => (e.seq, &id) > (*s, bid),
        };
        if better {
            best = Some((e.seq, id, to.clone()));
        }
    }
    // If this key wrote no succession in the lineage, it never handed over; if the latest hands back to
    // itself, the ledger is still its own.
    if !wrote {
        return None;
    }
    best.filter(|(_, _, to)| !to.eq_ignore_ascii_case(&me)).map(|(_, _, to)| to)
}
