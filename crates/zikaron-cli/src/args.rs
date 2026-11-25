use crate::codes::Reason;
use crate::out;
use crate::out::Subject;
use std::ffi::OsString;

pub struct Args {
    verb: String,
    pairs: Vec<(String, String, usize)>,
}

impl Args {
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
                _ => out::misuse(Reason::Args, Subject::Placed { at: i + 2, bytes: name.len() }, out::Said::NotAFlag),
            };
            let value = match rest.get(i + 1) {
                Some(v) if !v.starts_with("--") => v.clone(),
                _ => out::misuse(Reason::Args, flag_or_place(&stripped, i + 2), out::Said::NeedsValue),
            };
            let value = if stripped == "ledger" { crate::sealed::ledger_given(&value) } else { value };
            pairs.push((stripped, value, i + 3));
            i += 2;
        }
        Args { verb, pairs }
    }

    pub fn verb(&self) -> &str {
        &self.verb
    }

    pub fn close(&self, allowed: &[&str], needs: &[crate::verbs::Need]) {
        for (k, _, at) in &self.pairs {
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
        let given = |f: &str| self.pairs.iter().any(|(k, _, _)| k == f || STANDS_FOR.iter().any(|(flag, of)| k == flag && *of == f));
        for need in needs {
            use crate::verbs::Need;
            match *need {
                Need::Flag(f) if !given(f) => out::misuse(Reason::Args, Subject::flag(f), out::Said::Missing),
                Need::OneOf(fs) if fs.iter().filter(|f| given(f)).count() != 1 => out::misuse(Reason::Args, Subject::Flags(fs.to_vec()), out::Said::OneOf),
                Need::AnyOf(fs) if !fs.iter().any(|f| given(f)) => out::misuse(Reason::Args, Subject::Flags(fs.to_vec()), out::Said::AnyOf),
                _ => {}
            }
        }
    }

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

    pub fn many_at(&self, name: &'static str) -> Vec<(usize, String)> {
        self.pairs.iter().filter(|(k, _, _)| k == name).map(|(_, v, at)| (*at, v.clone())).collect()
    }

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

    pub fn now(&self) -> Option<u64> {
        self.u64_of("now")
    }

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

pub const KEY_FILE: &str = "key-file";

pub const STANDS_FOR: [(&str, &str); 1] = [(KEY_FILE, "key")];

pub const KEY_FILE_CAP: u64 = 4096;

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

pub fn slurp(path: &str) -> Vec<u8> {
    match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => out::misuse(Reason::Unreadable, out::typed(path), out::Said::Unreadable),
    }
}

pub fn slurp_json(path: &str) -> zikaron::json::Value {
    let bytes = slurp(path);
    match zikaron::json::parse_tests_1_5(&bytes) {
        Ok(v) => v,
        Err(_) => out::misuse(Reason::Unreadable, out::typed(path), out::Said::NotLawJson),
    }
}

fn flag_or_place(word: &str, at: usize) -> Subject {
    match crate::verbs::flag_named(word) {
        Some(name) => Subject::flag(name),
        None => Subject::Placed { at, bytes: word.len() + 2 },
    }
}

fn text(x: &OsString, at: usize) -> String {
    match x.to_str() {
        Some(s) => s.to_string(),
        None => out::misuse(Reason::Args, Subject::Placed { at, bytes: x.len() }, out::Said::NotUtf8),
    }
}

pub fn usage() -> &'static str {
    static LINE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    LINE.get_or_init(|| format!("usage: {} <{}> [--flag value ...]", crate::verbs_own::PROGRAM, crate::verbs::VERBS.join("|")))
}
