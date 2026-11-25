use crate::task::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Back {
    Said,
    Vault,
    Gate,
    Path,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnOk {
    Toast,
    Quiet,
    Back(Back),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnErr {
    Said,
    Back(Back),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    Last,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Then {
    Nothing,
    ReadChain,
    CloseIssuerField,
}

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

pub const TABLE: [Landing; 47] = [
    row(Kind::SelfCheck, TOAST, SAID),
    row(Kind::Archive, TOAST, SAID),
    Landing { kept: Kept::Failed, ..row(Kind::Chain, TOAST, SAID) },
    row(Kind::Reconcile, TOAST, SAID),
    row(Kind::Audit, TOAST, SAID),
    row(Kind::Ledger, TOAST, SAID),
    Landing { then: Then::ReadChain, ..row(Kind::Anchor, TOAST, SAID) },
    row(Kind::Kit, TOAST, SAID),
    row(Kind::Grants, TOAST, SAID),
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
    row(Kind::Fetch, TOAST, SAID),
    row(Kind::Vet, QUIET, SAID),
    row(Kind::Peek, QUIET, SAID),
    row(Kind::Backup, TOAST, SAID),
    row(Kind::Gate, OnOk::Back(Back::Gate), OnErr::Back(Back::Gate)),
    row(Kind::ReadNet, QUIET, SAID),
    row(Kind::Basis, TOAST, SAID),
    row(Kind::Gas, OnOk::Back(Back::Said), OnErr::Back(Back::Said)),
    row(Kind::Take, OnOk::Back(Back::Said), OnErr::Back(Back::Said)),
    row(Kind::Record, OnOk::Back(Back::Said), OnErr::Back(Back::Said)),
    row(Kind::Migrate, OnOk::Back(Back::Said), OnErr::Back(Back::Said)),
    row(Kind::Path, OnOk::Back(Back::Path), SAID),
    row(Kind::CliPath, TOAST, SAID),
    row(Kind::Seat, OnOk::Back(Back::Said), OnErr::Back(Back::Said)),
    row(Kind::Extract, TOAST, SAID),
    row(Kind::ChainTime, QUIET, SAID),
    row(Kind::Rotate, QUIET, SAID),
    row(Kind::CleanList, QUIET, SAID),
    row(Kind::Clean, TOAST, SAID),
    row(Kind::Persona, QUIET, SAID),
    row(Kind::FaceBooks, QUIET, SAID),
    row(Kind::RerootFee, QUIET, SAID),
    row(Kind::FaceJoined, QUIET, SAID),
    row(Kind::Behind, QUIET, SAID),
    row(Kind::Look, QUIET, SAID),
    row(Kind::Regain, QUIET, SAID),
    row(Kind::Keep, QUIET, SAID),
];

pub fn of(k: Kind) -> &'static Landing {
    TABLE.iter().find(|r| r.kind == k).unwrap_or(&TABLE[0])
}

pub fn goes_back(k: Kind, back: Back) -> bool {
    of(k).ok == OnOk::Back(back)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_one_row() {
        assert_eq!(TABLE.len(), Kind::ALL.len());
        for (r, k) in TABLE.iter().zip(Kind::ALL) {
            assert_eq!(r.kind, k, "row out of order or missing");
        }
    }

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
