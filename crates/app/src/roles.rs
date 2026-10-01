//! Role labels and the two-seat view.
//!
//! A seat is a view, never a process (one app, several seats, identity switching). So there is only a label
//! here: no second chain access, no second notification channel, no second process. One anchor key opens both
//! seats.

/// Two seats. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Author seat.
    Author,
    /// Grantee seat.
    Grantee,
}

impl Role {
    pub const ALL: [Role; 2] = [Role::Author, Role::Grantee];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Author => "author",
            Role::Grantee => "grantee",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Role::Author => crate::lang::t(crate::lang::Key::IdSeatAuthor),
            Role::Grantee => crate::lang::t(crate::lang::Key::IdSeatGrantee),
        }
    }

    /// Switch seat. What changes is the view; key, home and notifications stay.
    pub fn other(self) -> Role {
        match self {
            Role::Author => Role::Grantee,
            Role::Grantee => Role::Author,
        }
    }
}
