//! Closed table of component codes for the diagnostic trace. A trace mark's content can only be one of these.
//!
//! The trace API is `mark(component code)`. The parameter is a closed type rather than a string so that
//! "stuff free text into a mark" cannot be written: such a mark would become a second log, and sooner or
//! later someone would start relying on that log.

/// The registered component codes. Grows with the component list, not with call sites.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Feature {
    /// Widget library and skin.
    H0,
    /// Shell skeleton.
    H1,
    /// Identity and keys.
    H2,
    /// Archive and single writer.
    H3,
    /// Mirror and restore.
    H4,
    /// First run and adoption into the home.
    H5,
    /// Bilingual base.
    H6,
    /// A restored identity is read-only until the full ledger is fetched.
    H8,
    /// Ledger view.
    W1,
    /// Self-audit clock.
    W2,
    /// Anchoring desk.
    W3,
    /// Batch anchoring queue.
    W4,
    /// Disclosure kit maker.
    W5,
    /// Depth page.
    W6,
    /// Grant drafter.
    W7,
    /// Grant register and double-sale gate.
    W8,
    /// First-window checklist page.
    W13,
    /// Revocation flow.
    W9,
    /// Adoption desk.
    W10,
    /// Succession desk.
    W11,
    /// Others' ledger reader.
    W14,
    /// Diligence desk (grantee seat).
    D1,
    /// Record verifier (grantee seat).
    D2,
    /// Check received records (grantee seat).
    D4,
    /// Grant vault.
    D6,
    /// Revocation sentinel.
    D7,
    /// Multi-upstream register.
    D8,
    /// Badge packer.
    D9,
    /// Relicense drafter.
    D10,
    /// Grant check page.
    P1,
    /// Watch and notifications.
    P2,
}

impl Feature {
    pub const ALL: [Feature; 31] = [
        Feature::H0,
        Feature::H1,
        Feature::H2,
        Feature::H3,
        Feature::H4,
        Feature::H5,
        Feature::H6,
        Feature::H8,
        Feature::W1,
        Feature::W2,
        Feature::W3,
        Feature::W4,
        Feature::W5,
        Feature::W6,
        Feature::W7,
        Feature::W8,
        Feature::W13,
        Feature::W9,
        Feature::W10,
        Feature::W11,
        Feature::W14,
        Feature::D1,
        Feature::D2,
        Feature::D4,
        Feature::D6,
        Feature::D7,
        Feature::D8,
        Feature::D9,
        Feature::D10,
        Feature::P1,
        Feature::P2,
    ];

    /// The entire content of a trace mark: one component code.
    pub fn id(self) -> &'static str {
        match self {
            Feature::H0 => "H0",
            Feature::H1 => "H1",
            Feature::H2 => "H2",
            Feature::H3 => "H3",
            Feature::H4 => "H4",
            Feature::H5 => "H5",
            Feature::H6 => "H6",
            Feature::H8 => "H8",
            Feature::W1 => "W1",
            Feature::W2 => "W2",
            Feature::W3 => "W3",
            Feature::W4 => "W4",
            Feature::W5 => "W5",
            Feature::W6 => "W6",
            Feature::W7 => "W7",
            Feature::W8 => "W8",
            Feature::W13 => "W13",
            Feature::W9 => "W9",
            Feature::W10 => "W10",
            Feature::W11 => "W11",
            Feature::W14 => "W14",
            Feature::D1 => "D1",
            Feature::D2 => "D2",
            Feature::D4 => "D4",
            Feature::D6 => "D6",
            Feature::D7 => "D7",
            Feature::D8 => "D8",
            Feature::D9 => "D9",
            Feature::D10 => "D10",
            Feature::P1 => "P1",
            Feature::P2 => "P2",
        }
    }
}
