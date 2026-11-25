#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Author,
    Grantee,
}

impl Role {
    pub const ALL: [Role; 2] = [Role::Author, Role::Grantee];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Author => zikaron_seal::envelope::SEATS[0],
            Role::Grantee => zikaron_seal::envelope::SEATS[1],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Role::Author => crate::lang::t(crate::lang::Key::IdSeatAuthor),
            Role::Grantee => crate::lang::t(crate::lang::Key::IdSeatGrantee),
        }
    }

    pub fn other(self) -> Role {
        match self {
            Role::Author => Role::Grantee,
            Role::Grantee => Role::Author,
        }
    }
}

pub fn seat_word(r: crate::roles::Role) -> crate::lang::Key {
    match r {
        crate::roles::Role::Author => crate::lang::Key::IdSeatAuthor,
        crate::roles::Role::Grantee => crate::lang::Key::IdSeatGrantee,
    }
}
