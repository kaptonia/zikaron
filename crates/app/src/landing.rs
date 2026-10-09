//! What happens when a background task finishes, one row per task kind: what the window does on success and
//! on failure, what stays shown after a failure, and what follows. The shell, the action layer and the window
//! (`window::landing`) all route results through this table; nothing else decides it.
//!
//! The toast text and tone depend on the result, not the kind, and are produced by the window's `said_of`;
//! this table only says whether there is a toast.

use crate::task::Kind;

/// Which kind of requester a result is returned to; the requester shows the outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Back {
    /// The action's UI-thread half runs when the result arrives; its answer (`Shell::said`) goes back to the
    /// view that triggered it, reported as if it had run synchronously.
    Said,
    /// Passcode tasks: the shell finishes them and the answer (`Shell::vault_said`) goes to the passcode view
    /// that asked.
    Vault,
    /// An export's exit gate: on a pass the export runs when the result arrives (`Shell::gate_said`); the
    /// answer, or the gate's refusal, goes to the view that started the export.
    Gate,
    /// The system file dialog: the chosen path is delivered to the view that asked.
    Path,
}

/// What the window does when a task of the kind went well.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnOk {
    /// A toast with the outcome (with a "view" link when the user has left the page it started on); the task's
    /// button shows a check or red. Results already shown in place get no toast (`said_of` lists them).
    Toast,
    /// No toast: the result shows in place (its page, row or card); the task's button still shows the outcome.
    Quiet,
    /// The answer goes back to the requester.
    Back(Back),
}

/// What the window does when a task of the kind failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnErr {
    /// The shell records the error; a network error from a user-started task is shown in a toast, any other in
    /// the trouble list with details (`tell_faults`). The task's button turns red.
    Said,
    /// The error goes back to the requester, which shows the reason (its form stays open).
    Back(Back),
}

/// What stays shown after a failed task of the kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    /// The last good result stays shown; the error is reported as above.
    Last,
    /// The result area says the read failed (instead of showing the previous result) until one succeeds.
    Failed,
}

/// Follow-up work after a successful task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Then {
    Nothing,
    /// After anchoring talked to the node, read the chain again (recording the time).
    ReadChain,
    /// After a new check, close the issuer-ledger field opened for the previous check's gap.
    CloseIssuerField,
}

/// One kind's row.
#[derive(Clone, Copy, Debug)]
pub struct Landing {
    pub kind: Kind,
    pub ok: OnOk,
    pub err: OnErr,
    pub kept: Kept,
    pub then: Then,
}

const fn row(kind: Kind, ok: OnOk, err: OnErr) -> Landing {
    Landing { kind, ok, err, kept: Kept::Last, then: Then::Nothing }
}

const TOAST: OnOk = OnOk::Toast;
const QUIET: OnOk = OnOk::Quiet;
const SAID: OnErr = OnErr::Said;

/// The table: one row per kind, in the order of [`Kind::ALL`].
pub const TABLE: [Landing; 35] = [
    row(Kind::SelfCheck, TOAST, SAID),
    row(Kind::Archive, TOAST, SAID),
    // The settings page's chain row shows a failed read instead of the previous result.
    Landing { kept: Kept::Failed, ..row(Kind::Chain, TOAST, SAID) },
    row(Kind::Reconcile, TOAST, SAID),
    row(Kind::Audit, TOAST, SAID),
    row(Kind::Ledger, TOAST, SAID),
    // The toast comes with the receipt; a receipt with a status other than 1 is reported as a failed send.
    Landing { then: Then::ReadChain, ..row(Kind::Anchor, TOAST, SAID) },
    row(Kind::Depth, TOAST, SAID),
    row(Kind::Kit, TOAST, SAID),
    row(Kind::Grants, TOAST, SAID),
    row(Kind::Adopt, TOAST, SAID),
    row(Kind::Sighting, QUIET, SAID),
    row(Kind::Book, TOAST, SAID),
    row(Kind::Diligence, TOAST, SAID),
    row(Kind::Verify, TOAST, SAID),
    row(Kind::Delivery, TOAST, SAID),
    row(Kind::Review, TOAST, SAID),
    row(Kind::Held, QUIET, SAID),
    row(Kind::Badge, TOAST, SAID),
    Landing { then: Then::CloseIssuerField, ..row(Kind::Check, TOAST, SAID) },
    row(Kind::Keystore, TOAST, SAID),
    row(Kind::Vault, OnOk::Back(Back::Vault), OnErr::Back(Back::Vault)),
    row(Kind::Publish, TOAST, SAID),
    // A conflict shows as a card on the home page and a tail check in the read-only bar; a fetch that set the
    // old data aside says so, with a button to view it.
    row(Kind::Fetch, TOAST, SAID),
    row(Kind::Vet, QUIET, SAID),
    // A backup's contents (a peek) go on its confirmation card; a completed backup gets a toast.
    row(Kind::Backup, TOAST, SAID),
    row(Kind::Gate, OnOk::Back(Back::Gate), OnErr::Back(Back::Gate)),
    row(Kind::ReadNet, QUIET, SAID),
    // Checked when saving: a toast says saved, or reports the fingerprint. Checked after nodes were saved: the
    // result shows beside the field (including a fingerprint that is not the pinned build).
    row(Kind::Basis, TOAST, SAID),
    row(Kind::Gas, OnOk::Back(Back::Said), OnErr::Back(Back::Said)),
    row(Kind::Take, OnOk::Back(Back::Said), OnErr::Back(Back::Said)),
    row(Kind::Record, OnOk::Back(Back::Said), OnErr::Back(Back::Said)),
    row(Kind::Migrate, OnOk::Back(Back::Said), OnErr::Back(Back::Said)),
    // A cancel delivers nothing; a dialog that failed to open or to return is recorded and reported.
    row(Kind::Path, OnOk::Back(Back::Path), SAID),
    // Installing or removing the command line on PATH gets a toast; a refusal (another install there, the
    // system dialog cancelled) is reported with its reason.
    row(Kind::CliPath, TOAST, SAID),
];

/// The row of a kind.
pub fn of(k: Kind) -> &'static Landing {
    TABLE.iter().find(|r| r.kind == k).unwrap_or(&TABLE[0])
}

/// Whether a kind's result is returned to its requester through `back`.
pub fn goes_back(k: Kind, back: Back) -> bool {
    of(k).ok == OnOk::Back(back)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind has exactly one row, in the order of `Kind::ALL`.
    #[test]
    fn every_kind_has_one_row() {
        assert_eq!(TABLE.len(), Kind::ALL.len());
        for (r, k) in TABLE.iter().zip(Kind::ALL) {
            assert_eq!(r.kind, k, "row out of order or missing");
        }
    }

    /// A kind returned to its requester on success is returned the same way on failure (except file dialogs,
    /// whose failures are reported like any other error).
    #[test]
    fn what_goes_back_goes_back_both_ways() {
        for r in TABLE {
            if let OnOk::Back(b) = r.ok {
                if b != Back::Path {
                    assert_eq!(r.err, OnErr::Back(b), "{:?}", r.kind);
                }
            }
            if let OnErr::Back(b) = r.err {
                assert_eq!(r.ok, OnOk::Back(b), "{:?}", r.kind);
            }
        }
    }
}
