//! Relicense drafter. The same component as the author seat's grant drafter, with upstream prefilled from
//! the held grant.
//!
//! This layer does two parameter jobs: fill `upstream` and `work` from a vault item into the grant draft; and
//! check whether this desk can sign (anchor key present, own ledger present), otherwise guide the person to
//! the existing setup paths (identity setup generates the anchor key, the ledger's first run writes genesis),
//! without a separate path. Issuing, the double-sale gate and queueing all follow the grant drafter's path:
//! the relicensor is the author in their own ledger. Weighing scope belongs to courts; the tool does no
//! nesting check (as kit law §10).

use crate::grantx::Draft;
use crate::vaultx::Held;

/// Prefill. upstream is the held grant's id, work follows it; other cells are left empty for the person.
pub fn prefill(h: &Held) -> Draft {
    Draft { upstream: h.id.clone(), work: h.work.clone(), ..Default::default() }
}

/// What this desk still lacks to sign a relicense. Closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guide {
    /// Generate the anchor key first.
    NeedKey,
    /// Open your own ledger first (its first run: "you will open your own ledger").
    NeedGenesis,
    /// Ready to draft.
    Ready,
}

impl Guide {
    pub fn as_str(self) -> &'static str {
        match self {
            Guide::NeedKey => "need_key",
            Guide::NeedGenesis => "need_genesis",
            Guide::Ready => "ready",
        }
    }
}

/// Guide. Two questions in order: is the key present, does the ledger have a genesis; Ready only when both
/// hold.
pub fn guide(has_key: bool, rooted: bool) -> Guide {
    if !has_key {
        Guide::NeedKey
    } else if !rooted {
        Guide::NeedGenesis
    } else {
        Guide::Ready
    }
}
