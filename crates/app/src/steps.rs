use crate::lang::Key;
use crate::roles::Role;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Pin,
    Key,
    Own(crate::own::OwnStep),
    Backup,
}

impl std::fmt::Debug for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Step::Pin => f.write_str("Pin"),
            Step::Key => f.write_str("Key"),
            Step::Own(o) => write!(f, "{o:?}"),
            Step::Backup => f.write_str("Backup"),
        }
    }
}

impl Step {
    pub const ALL: [Step; crate::own::WALK.len()] = crate::own::WALK;

    pub fn title(self) -> Key {
        match self {
            Step::Key => Key::WizKeyTitle,
            Step::Pin => Key::WizPinTitle,
            Step::Own(o) => o.title(),
            Step::Backup => Key::WizBackupTitle,
        }
    }

    pub fn deferrable(self, role: Role) -> bool {
        match self {
            Step::Pin | Step::Key => false,
            Step::Own(o) => o.deferrable(role),
            Step::Backup => true,
        }
    }

    pub fn opens(self, role: Role) -> bool {
        match self {
            Step::Pin | Step::Key => true,
            Step::Own(o) => o.opens(role),
            Step::Backup => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sub {
    First,
    Check,
}

pub fn prev(step: Step, sub: Sub) -> Option<(Step, Sub)> {
    if sub != Sub::First {
        return Some((step, Sub::First));
    }
    let at = Step::ALL.iter().position(|s| *s == step)?;
    at.checked_sub(1).map(|i| (Step::ALL[i], Sub::First))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Progress {
    pub key: bool,
    pub pin: bool,
    pub backup: bool,
    pub own: crate::own::OwnProgress,
}

impl Progress {
    pub fn of(shell: &crate::shell::Shell) -> Progress {
        Progress { key: shell.anchor.is_some(), pin: !shell.vault.absent(), backup: shell.machine.backup.is_some(), own: crate::own::OwnProgress::of(shell) }
    }

    pub fn done(&self, s: Step) -> bool {
        match s {
            Step::Key => self.key,
            Step::Pin => self.pin,
            Step::Own(o) => self.own.done(o),
            Step::Backup => self.backup,
        }
    }

    pub fn gate(&self, want: Step) -> Step {
        if want != Step::Pin && !self.pin {
            Step::Pin
        } else if !matches!(want, Step::Pin | Step::Key) && !self.key {
            Step::Key
        } else {
            want
        }
    }

    pub fn first_open(&self) -> Option<Step> {
        Step::ALL.iter().copied().find(|s| !self.done(*s))
    }

    pub fn wants_wizard(&self, role: Role) -> bool {
        Step::ALL.iter().any(|s| !self.done(*s) && !s.deferrable(role) && s.opens(role))
    }
}
