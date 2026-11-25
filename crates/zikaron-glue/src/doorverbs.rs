#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    Value,
    File,
    Out,
    Place,
}

impl Flag {
    pub fn is_path(self) -> bool {
        !matches!(self, Flag::Value)
    }
}

use Flag::{File, Out, Place, Value};

pub const HOME: [(&str, &[(&str, Flag)]); 20] = [
    ("init", &[("statement", Value)]),
    ("history", &[("file", File), ("note", Value)]),
    ("grant", &[("grantee", Value), ("work", Value), ("terms", Value), ("history", Value), ("window-from", Value), ("window-to", Value), ("scope", Value), ("upstream", Value), ("supersedes", Value)]),
    ("revoke", &[("grant", Value), ("case", Value)]),
    ("annotate", &[("subject", Value), ("note", Value)]),
    ("retract", &[("subject", Value), ("note", Value)]),
    ("anchor", &[]),
    ("kit-export", &[("entry", Value), ("note", Value), ("out", Out), ("as", Value), ("to", Value), ("to-key", Value), ("originals", Value)]),
    ("kit-verify", &[("kit", Place), ("originals-to", Place)]),
    ("grant-package", &[("grant", Value), ("originals", Value), ("to", Value), ("out", Out)]),
    ("clean", &[("yes", Value)]),
    ("face-table", &[]),
    ("face-home", &[("chain", Value)]),
    ("face-account", &[("platform", Value), ("handle", Value), ("note", Value)]),
    ("face-key", &[("address", Value), ("role", Value), ("sig", Value), ("note", Value)]),
    ("face-name", &[("name", Value)]),
    ("face-release", &[("subject", Value)]),
    ("face-reroot", &[("to", Value)]),
    ("face-origin", &[("from", Value)]),
    ("face-joined", &[("root", Value), ("to", Value)]),
];

pub fn names(row: &[(&'static str, Flag)]) -> Vec<&'static str> {
    row.iter().map(|(f, _)| *f).collect()
}

pub fn kind(verb: &str, flag: &str) -> Option<Flag> {
    HOME.iter().find(|(v, _)| *v == verb).and_then(|(_, fl)| fl.iter().find(|(f, _)| *f == flag)).map(|(_, k)| *k)
}
