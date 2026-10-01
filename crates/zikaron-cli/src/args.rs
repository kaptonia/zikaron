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
use std::ffi::OsString;

pub struct Args {
    verb: String,
    pairs: Vec<(String, String)>,
}

impl Args {
    /// Read a command line: the first token is the verb, then `--name value` pairs.
    pub fn read(argv: Vec<OsString>) -> Args {
        let mut it = argv.into_iter();
        let verb = match it.next() {
            Some(x) => text(&x),
            None => out::misuse(Reason::Args, USAGE),
        };
        let rest: Vec<String> = it.map(|x| text(&x)).collect();
        let mut pairs: Vec<(String, String)> = Vec::new();
        let mut i = 0usize;
        while i < rest.len() {
            let name = &rest[i];
            let stripped = match name.strip_prefix("--") {
                Some(x) if !x.is_empty() => x.to_string(),
                _ => out::misuse(Reason::Args, name),
            };
            let value = match rest.get(i + 1) {
                Some(v) if !v.starts_with("--") => v.clone(),
                _ => out::misuse(Reason::Args, &format!("--{stripped} 要一个值")),
            };
            pairs.push((stripped, value));
            i += 2;
        }
        Args { verb, pairs }
    }

    pub fn verb(&self) -> &str {
        &self.verb
    }

    /// The verb's own flag list: a flag outside it is misuse. Each verb calls this first.
    pub fn close(&self, allowed: &[&str]) {
        for (k, _) in &self.pairs {
            if !allowed.contains(&k.as_str()) {
                out::misuse(Reason::Args, &format!("--{k} 不在 {} 的旗单里", self.verb));
            }
        }
    }

    pub fn one(&self, name: &str) -> Option<String> {
        let mut found = self.pairs.iter().filter(|(k, _)| k == name);
        let first = found.next()?;
        if found.next().is_some() {
            out::misuse(Reason::Args, &format!("--{name} 给了不止一次"));
        }
        Some(first.1.clone())
    }

    pub fn need(&self, name: &str) -> String {
        match self.one(name) {
            Some(x) => x,
            None => out::misuse(Reason::Args, &format!("缺 --{name}")),
        }
    }

    pub fn many(&self, name: &str) -> Vec<String> {
        self.pairs
            .iter()
            .filter(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// Exactly one of the named flags is present; returns its name and value.
    pub fn exactly_one_of(&self, names: &[&str]) -> (String, String) {
        let mut hit: Vec<(String, String)> = Vec::new();
        for n in names {
            if let Some(v) = self.one(n) {
                hit.push(((*n).to_string(), v));
            }
        }
        match hit.len() {
            1 => hit.remove(0),
            _ => out::misuse(
                Reason::Args,
                &format!(
                    "{} 之中恰要一面",
                    names
                        .iter()
                        .map(|n| format!("--{n}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
            ),
        }
    }

    pub fn u64_of(&self, name: &str) -> Option<u64> {
        let raw = self.one(name)?;
        match raw.parse::<u64>() {
            Ok(x) => Some(x),
            Err(_) => out::misuse(Reason::Args, &format!("--{name} 不是一个整数:{raw}")),
        }
    }

    pub fn need_u64(&self, name: &str) -> u64 {
        match self.u64_of(name) {
            Some(x) => x,
            None => out::misuse(Reason::Args, &format!("缺 --{name}")),
        }
    }

    /// The injected now. Absent means nothing is injected (decisions use chain time only).
    pub fn now(&self) -> Option<u64> {
        self.u64_of("now")
    }

    /// Private key: 64 hex digits (optional `0x`), a scalar in [1, n−1]. Out of range is misuse (as in
    /// HARNESS).
    pub fn key(&self, name: &str) -> [u8; 32] {
        let raw = self.need(name);
        let k = match zikaron::hexfmt::scalar32(&raw) {
            Some(x) => x,
            None => out::misuse(Reason::Key, &format!("--{name} 不是六十四位十六进制")),
        };
        if !zikaron::cryptox::in_range(&k) {
            out::misuse(Reason::Key, &format!("--{name} 不在曲线的范围里"));
        }
        k
    }
}

/// Read a file's bytes; unreadable is misuse with the path as subject.
///
/// Not named `read`: `Args::read` holds that name, and two `fn read(` in one file would be ambiguous to tools
/// that locate functions by name.
pub fn slurp(path: &str) -> Vec<u8> {
    match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => out::misuse(Reason::Unreadable, path),
    }
}

/// Read a JSON file through the core's parser (law §3.5 tests 1 to 5); failure is misuse. This is the shell's
/// only JSON parsing.
pub fn slurp_json(path: &str) -> zikaron::json::Value {
    let bytes = slurp(path);
    match zikaron::json::parse_tests_1_5(&bytes) {
        Ok(v) => v,
        Err(_) => out::misuse(Reason::Unreadable, &format!("{path}(不是法 §3.5 收的 JSON)")),
    }
}

fn text(x: &OsString) -> String {
    match x.to_str() {
        Some(s) => s.to_string(),
        None => out::misuse(Reason::Args, "参数不是合法 UTF-8"),
    }
}

pub const USAGE: &str = "usage: zikaron <keygen|init|history|grant|revoke|adopt|attest|succeed|annotate|retract|anchor|scan|audit|check-grant|chain-check|depth|fpm-sign|ack-sign|badge|kit-export|show> [--flag value ...]";
