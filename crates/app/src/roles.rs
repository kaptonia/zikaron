//! Role labels and the two-seat view.
//!
//! A seat is a view, never a process (one app, several seats, identity switching). So this is only a label:
//! no second chain connection, notification channel or process. One anchor key opens both seats.

/// The two seats.
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

    /// The other seat. Switching changes only the view; key, home and notifications stay.
    pub fn other(self) -> Role {
        match self {
            Role::Author => Role::Grantee,
            Role::Grantee => Role::Author,
        }
    }
}
