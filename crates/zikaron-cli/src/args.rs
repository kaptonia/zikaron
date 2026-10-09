//! Arguments: every flag carries a value, `--name value` in pairs, long flags only.
//!
//! There are no boolean flags, so "is the next token a value or the next flag" cannot be ambiguous: a token
//! starting with `--` in a value position is misuse. Flags may repeat (`--hash` one per hash); each verb's
//! flag list is closed, and a flag outside it is misuse (HARNESS: an argument beyond a command's own list).
//!
//! This layer only checks that a value can be built: a private key must be 32 bytes within the curve range
//! (following HARNESS `zk1 sign`, out of range is misuse), an integer must parse. Field shapes are not judged
//! here: whether `--grantee` is hex20 or `--work` is hex32 goes into the body as given and is judged by the
//! core's `entry::check`. The law is the one place shapes are judged, so every verb's refusal is the law's
//! own token.

use crate::codes::Reason;
use crate::out;
use crate::out::Subject;
use std::ffi::OsString;

pub struct Args {
    verb: String,
    /// Each flag's name, its value, and the value's place among the arguments (the verb is #1).
    pairs: Vec<(String, String, usize)>,
}

impl Args {
    /// Read a command line: the first token is the verb, then `--name value` pairs.
    pub fn read(argv: Vec<OsString>) -> Args {
        let mut it = argv.into_iter();
        let verb = match it.next() {
            Some(x) => text(&x, 1),
            None => out::misuse(Reason::Args, Subject::Program, out::Said::Usage),
        };
        let rest: Vec<String> = it.enumerate().map(|(i, x)| text(&x, i + 2)).collect();
        out::keep_arguments(std::iter::once(verb.clone()).chain(rest.iter().cloned()).collect());
        let mut pairs: Vec<(String, String, usize)> = Vec::new();
        let mut i = 0usize;
        while i < rest.len() {
            let name = &rest[i];
            let stripped = match name.strip_prefix("--") {
                Some(x) if !x.is_empty() => x.to_string(),
                // A word standing alone where a flag belongs is named by its place and length, never echoed.
                _ => out::misuse(Reason::Args, Subject::Placed { at: i + 2, bytes: name.len() }, out::Said::NotAFlag),
            };
            let value = match rest.get(i + 1) {
                Some(v) if !v.starts_with("--") => v.clone(),
                _ => out::misuse(Reason::Args, flag_or_place(&stripped, i + 2), out::Said::NeedsValue),
            };
            pairs.push((stripped, value, i + 3));
            i += 2;
        }
        Args { verb, pairs }
    }

    pub fn verb(&self) -> &str {
        &self.verb
    }

    /// The verb's own flag list: a flag outside it is misuse. Each verb calls this first. A flag the verb takes
    /// on its own but not together with `--home` is said so.
    pub fn close(&self, allowed: &[&str]) {
        for (k, _, at) in &self.pairs {
            // `--key-file` is the other way to give `--key` (read from a file): a verb that takes one takes both
            // ([`STANDS_FOR`]).
            let taken = allowed.contains(&k.as_str()) || STANDS_FOR.iter().any(|(flag, of)| k == flag && allowed.contains(of));
            if !taken {
                let own = |a: &[&str]| a.contains(&k.as_str()) || STANDS_FOR.iter().any(|(flag, of)| k == flag && a.contains(of));
                let said = match (self.homed(), crate::verbs::accepts(&self.verb, false)) {
                    (true, Some(row)) if own(&row) => out::Said::NotWithHome,
                    _ => out::Said::NotThisVerbsFlag,
                };
                out::misuse(Reason::Args, flag_or_place(k, *at - 1), said);
            }
        }
    }

    /// Whether `--home` was given: the verb is handed to the desktop.
    pub fn homed(&self) -> bool {
        self.pairs.iter().any(|(k, _, _)| k == crate::verbs::HOME_FLAG)
    }

    pub fn one(&self, name: &'static str) -> Option<String> {
        let mut found = self.pairs.iter().filter(|(k, _, _)| k == name);
        let first = found.next()?;
        if found.next().is_some() {
            out::misuse(Reason::Args, Subject::flag(name), out::Said::Repeated);
        }
        Some(first.1.clone())
    }

    pub fn need(&self, name: &'static str) -> String {
        match self.one(name) {
            Some(x) => x,
            None => out::misuse(Reason::Args, Subject::flag(name), out::Said::Missing),
        }
    }

    pub fn many(&self, name: &'static str) -> Vec<String> {
        self.many_at(name).into_iter().map(|(_, v)| v).collect()
    }

    /// [`Args::many`] with each value's place among the arguments (the verb is #1), for a value that is named
    /// without being echoed ([`out::unechoed`]).
    pub fn many_at(&self, name: &'static str) -> Vec<(usize, String)> {
        self.pairs.iter().filter(|(k, _, _)| k == name).map(|(_, v, at)| (*at, v.clone())).collect()
    }

    /// Exactly one of the named flags is present; returns its name and value.
    pub fn exactly_one_of(&self, names: &[&'static str]) -> (String, String) {
        let mut hit: Vec<(String, String)> = Vec::new();
        for n in names {
            if let Some(v) = self.one(n) {
                hit.push(((*n).to_string(), v));
            }
        }
        match hit.len() {
            1 => hit.remove(0),
            _ => out::misuse(Reason::Args, Subject::Flags(names.to_vec()), out::Said::OneOf),
        }
    }

    pub fn u64_of(&self, name: &'static str) -> Option<u64> {
        let raw = self.one(name)?;
        match raw.parse::<u64>() {
            Ok(x) => Some(x),
            Err(_) => out::misuse(Reason::Args, Subject::flag(name), out::Said::NotInt),
        }
    }

    /// A whole number that goes into an entry: within the law's ceiling (`2^53 − 1`). Digits past it (within 64
    /// bits or not) are misuse naming the ceiling; anything else that is not a whole number, as `u64_of`.
    pub fn within_ceiling(&self, name: &'static str) -> Option<u64> {
        let raw = self.one(name)?;
        let digits = raw.strip_prefix('+').unwrap_or(&raw);
        match raw.parse::<u64>() {
            Ok(x) if x <= zikaron::json::MAX_INT => Some(x),
            Ok(_) => out::misuse(Reason::Args, Subject::flag(name), out::Said::PastIntCeiling),
            Err(_) if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => out::misuse(Reason::Args, Subject::flag(name), out::Said::PastIntCeiling),
            Err(_) => out::misuse(Reason::Args, Subject::flag(name), out::Said::NotInt),
        }
    }

    pub fn need_u64(&self, name: &'static str) -> u64 {
        match self.u64_of(name) {
            Some(x) => x,
            None => out::misuse(Reason::Args, Subject::flag(name), out::Said::Missing),
        }
    }

    /// The injected now. Absent means nothing is injected (decisions use chain time only).
    pub fn now(&self) -> Option<u64> {
        self.u64_of("now")
    }

    /// Private key: 64 hex digits (optional `0x`), a scalar in [1, n−1], given on the command line (`--key`) or
    /// read from a file (`--key-file`, [`key_file_text`]), exactly one of the two. Neither is misuse naming `--key`;
    /// both is misuse naming both. Not hex or out of range is misuse naming the flag it came by (as in HARNESS).
    pub fn key(&self, name: &'static str) -> [u8; 32] {
        let (raw, by) = match (self.one(name), self.one(KEY_FILE)) {
            (Some(_), Some(_)) => out::misuse(Reason::Args, Subject::Flags(vec![name, KEY_FILE]), out::Said::OneOf),
            (Some(v), None) => (v, Subject::flag(name)),
            (None, Some(path)) => (key_file_text(&path), Subject::flag(KEY_FILE)),
            (None, None) => out::misuse(Reason::Args, Subject::flag(name), out::Said::Missing),
        };
        let k = match zikaron::hexfmt::scalar32(&raw) {
            Some(x) => x,
            None => out::misuse(Reason::Key, by.clone(), out::Said::KeyNotHex),
        };
        if !zikaron::cryptox::in_range(&k) {
            out::misuse(Reason::Key, by, out::Said::KeyOutOfRange);
        }
        k
    }
}

/// The flag that gives the private key from a file.
pub const KEY_FILE: &str = "key-file";

/// Flags that give another flag's value another way, as `(flag, the flag it stands for)`: a verb whose list
/// takes the second takes the first too. One row, `--key-file` for `--key`. [`Args::close`] reads this table and
/// the `contract` verb prints it.
pub const STANDS_FOR: [(&str, &str); 1] = [(KEY_FILE, "key")];

/// The most bytes a key file may hold: a key is 64 hex digits, a prefix and a line end; anything this long is
/// not a key.
pub const KEY_FILE_CAP: u64 = 4096;

/// The text of a key file: the file must be there, a file, readable by its owner only (`zikaron_os`: a key
/// another account can read is no longer this account's); its bytes UTF-8 and at most [`KEY_FILE_CAP`], with
/// white space and line ends at either end dropped. Not there, not a file or not readable is misuse naming the
/// path (as every file the command line reads); readable by others is misuse naming the path
/// (`KeyFileOpen`); too long or not text reads as a key that is not hex.
fn key_file_text(path: &str) -> String {
    use std::io::Read;
    let p = std::path::Path::new(path);
    if !std::fs::metadata(p).map(|m| m.is_file()).unwrap_or(false) {
        out::misuse(Reason::Unreadable, out::typed(path), out::Said::Unreadable);
    }
    match zikaron_os::is_owner_only(p) {
        Ok(true) => {}
        Ok(false) => out::misuse(Reason::Key, out::typed(path), out::Said::KeyFileOpen),
        Err(_) => out::misuse(Reason::Unreadable, out::typed(path), out::Said::Unreadable),
    }
    let mut bytes = Vec::new();
    if std::fs::File::open(p).and_then(|f| f.take(KEY_FILE_CAP + 1).read_to_end(&mut bytes)).is_err() {
        out::misuse(Reason::Unreadable, out::typed(path), out::Said::Unreadable);
    }
    if bytes.len() as u64 > KEY_FILE_CAP {
        return String::new();
    }
    String::from_utf8(bytes).map(|t| t.trim_matches(|c: char| c.is_ascii_whitespace()).to_string()).unwrap_or_default()
}

/// Read a file's bytes; unreadable is misuse with the path as subject.
///
/// Not named `read`: `Args::read` holds that name, and two `fn read(` in one file would be ambiguous to tools
/// that locate functions by name.
pub fn slurp(path: &str) -> Vec<u8> {
    match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => out::misuse(Reason::Unreadable, out::typed(path), out::Said::Unreadable),
    }
}

/// Read a JSON file through the core's parser (law §3.5 tests 1 to 5); failure is misuse. This is the shell's
/// only JSON parsing.
pub fn slurp_json(path: &str) -> zikaron::json::Value {
    let bytes = slurp(path);
    match zikaron::json::parse_tests_1_5(&bytes) {
        Ok(v) => v,
        Err(_) => out::misuse(Reason::Unreadable, out::typed(path), out::Said::NotLawJson),
    }
}

/// A flag word the person typed, as a misuse names it: the closed table's own name when it is one (`--ledger`),
/// else its place and length (`--key=0x…`, `--nosuch`: the word may carry a value or a secret).
fn flag_or_place(word: &str, at: usize) -> Subject {
    match crate::verbs::flag_named(word) {
        Some(name) => Subject::flag(name),
        None => Subject::Placed { at, bytes: word.len() + 2 },
    }
}

/// An argument as text; one that is not is named by its place (`at`, the verb being #1) and length, never
/// echoed.
fn text(x: &OsString, at: usize) -> String {
    match x.to_str() {
        Some(s) => s.to_string(),
        None => out::misuse(Reason::Args, Subject::Placed { at, bytes: x.len() }, out::Said::NotUtf8),
    }
}

pub const USAGE: &str = "usage: zikaron <keygen|init|history|grant|revoke|adopt|attest|succeed|annotate|retract|anchor|scan|audit|check-grant|chain-check|depth|fpm-sign|ack-sign|badge|kit-export|show|contract> [--flag value ...]";
