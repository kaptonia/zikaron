use crate::roles::Role;

pub const PURPOSE: u32 = 44;
pub const COIN: u32 = 60;
pub const PROJECT: u32 = 0;
pub const GROUP: u32 = 0;
pub const AUTHOR_INDEX: u32 = 0;
pub const GRANTEE_INDEX: u32 = 1;
pub const HARDENED: u32 = 0x8000_0000;
pub const WORDS: usize = 12;
pub const ENTROPY_BYTES: usize = 16;

pub fn index_of(role: Role) -> u32 {
    match role {
        Role::Author => AUTHOR_INDEX,
        Role::Grantee => GRANTEE_INDEX,
    }
}

pub fn path(role: Role) -> [u32; 5] {
    path_at(role, GROUP)
}

pub fn path_at(role: Role, group: u32) -> [u32; 5] {
    [PURPOSE | HARDENED, COIN | HARDENED, PROJECT | HARDENED, group, index_of(role)]
}

pub const GROUPS_SEARCHED: u32 = 64;

pub fn path_text(role: Role) -> String {
    path_text_at(role, GROUP)
}

pub fn path_text_at(role: Role, group: u32) -> String {
    format!("m/{PURPOSE}'/{COIN}'/{PROJECT}'/{group}/{}", index_of(role))
}
