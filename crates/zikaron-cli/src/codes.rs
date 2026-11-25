#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exit {
    Affirmed,
    Denied,
    Misuse,
    Partial,
    Unanswered,
}

impl Exit {
    pub fn code(self) -> u8 {
        match self {
            Exit::Affirmed => 0,
            Exit::Denied => 1,
            Exit::Misuse => 2,
            Exit::Partial => 3,
            Exit::Unanswered => 4,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Exit::Affirmed => "affirmed",
            Exit::Denied => "denied",
            Exit::Misuse => "misuse",
            Exit::Partial => "partial",
            Exit::Unanswered => "unanswered",
        }
    }

    pub const ALL: [Exit; 5] = [
        Exit::Affirmed,
        Exit::Denied,
        Exit::Misuse,
        Exit::Partial,
        Exit::Unanswered,
    ];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Contract {
    Exits,
    Code,
    Name,
    Reasons,
    StandsFor,
    Flag,
    For,
    Verbs,
    Flags,
    HomeFlags,
    Required,
}

impl Contract {
    pub fn as_str(self) -> &'static str {
        match self {
            Contract::Exits => "exits",
            Contract::Code => "code",
            Contract::Name => "name",
            Contract::Reasons => "reasons",
            Contract::StandsFor => "standsFor",
            Contract::Flag => "flag",
            Contract::For => "for",
            Contract::Verbs => "verbs",
            Contract::Flags => "flags",
            Contract::HomeFlags => "homeFlags",
            Contract::Required => "required",
        }
    }

    pub const ALL: [Contract; 11] = [
        Contract::Exits,
        Contract::Code,
        Contract::Name,
        Contract::Reasons,
        Contract::StandsFor,
        Contract::Flag,
        Contract::For,
        Contract::Verbs,
        Contract::Flags,
        Contract::HomeFlags,
        Contract::Required,
    ];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Door {
    NotOpen,
    Closed,
    PathTooLong,
    NotYou,
    Broken,
    NoMachine,
    Alone,
}

impl Door {
    pub fn as_str(self) -> &'static str {
        match self {
            Door::NotOpen => "NOT_OPEN",
            Door::Closed => "CLOSED",
            Door::PathTooLong => "PATH_TOO_LONG",
            Door::NotYou => "NOT_YOU",
            Door::Broken => "BROKEN",
            Door::NoMachine => "NO_MACHINE",
            Door::Alone => "ALONE",
        }
    }

    pub const ALL: [Door; 7] = [Door::NotOpen, Door::Closed, Door::PathTooLong, Door::NotYou, Door::Broken, Door::NoMachine, Door::Alone];
}

pub use crate::codes_own::{Field, Key, Reason, Word};
