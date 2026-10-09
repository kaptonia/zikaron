//! Third-party notices: every registry crate linked into the given roots' dependency trees on one target, with
//! its licence expression and the licence files it ships, generated from the workspace's `Cargo.lock` (via
//! `cargo metadata --filter-platform <target> --locked --offline`). The app's build script embeds the list in
//! the app; the packages ship it for everything they install, followed by the licences of embedded fonts. One
//! generator serves both, so they never diverge.
//!
//! Workspace crates are this project's own and are not listed. A crate that ships no licence file is listed
//! with its expression and a note saying so. A crate that names its licence by file (`license-file`) rather
//! than by expression is listed under that file, whose text is included whatever its name; a named file
//! missing from the crate is reported.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The notices text for `roots` (workspace package names) on `target`, followed by each file in `extra` (a
/// licence beyond the crates', such as a font's) under its own name. `cargo` is the cargo binary to run;
/// `manifest` the workspace's `Cargo.toml`.
pub fn make(cargo: &str, manifest: &Path, target: &str, roots: &[&str], extra: &[PathBuf]) -> Result<String, String> {
    let out = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--locked", "--offline", "--filter-platform", target, "--manifest-path"])
        .arg(manifest)
        .output()
        .map_err(|e| format!("cargo metadata did not start: {e}"))?;
    if !out.status.success() {
        return Err(format!("cargo metadata refused: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let meta = json::parse(&out.stdout).map_err(|e| format!("cargo metadata answer unreadable: {e}"))?;
    let mut text = notices_of(&meta, roots)?;
    for f in extra {
        let t = std::fs::read_to_string(f).map_err(|e| format!("cannot read {}: {e}", f.display()))?;
        text.push_str("\n\u{2500}\u{2500}\u{2500}\u{2500}\n");
        text.push_str(&f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
        text.push_str("\n\n");
        text.push_str(t.trim_end());
        text.push('\n');
    }
    Ok(text)
}

struct Pkg {
    name: String,
    version: String,
    license: String,
    /// The `license-file` the crate's manifest names (relative to its directory), if any.
    license_file: Option<String>,
    dir: PathBuf,
    registry: bool,
}

/// The notices text: one line per crate (name, version, licence expression), then each distinct licence
/// text once under the crates that ship it.
fn notices_of(meta: &json::Value, roots: &[&str]) -> Result<String, String> {
    let packages: BTreeMap<String, Pkg> = meta
        .get("packages")
        .and_then(|p| p.as_array())
        .ok_or("cargo metadata answer has no packages")?
        .iter()
        .map(|p| {
            let s = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let dir = Path::new(&s("manifest_path")).parent().map(Path::to_path_buf).unwrap_or_default();
            let license_file = p.get("license_file").and_then(|v| v.as_str()).filter(|f| !f.is_empty()).map(str::to_string);
            (s("id"), Pkg { name: s("name"), version: s("version"), license: s("license"), license_file, dir, registry: p.get("source").and_then(|v| v.as_str()).is_some() })
        })
        .collect();
    let nodes: BTreeMap<String, Vec<String>> = meta
        .get("resolve")
        .and_then(|r| r.get("nodes"))
        .and_then(|n| n.as_array())
        .ok_or("cargo metadata answer has no resolve.nodes")?
        .iter()
        .map(|n| {
            let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            // Normal dependencies only: build scripts and test helpers are not in the shipped binary.
            let deps = n
                .get("deps")
                .and_then(|d| d.as_array())
                .map(|ds| {
                    ds.iter()
                        .filter(|d| d.get("dep_kinds").and_then(|k| k.as_array()).map(|ks| ks.iter().any(|k| k.get("kind").map(|x| x.is_null()).unwrap_or(false))).unwrap_or(false))
                        .filter_map(|d| d.get("pkg").and_then(|v| v.as_str()).map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            (id, deps)
        })
        .collect();
    let mut stack = Vec::new();
    for r in roots {
        let id = packages.iter().find(|(_, p)| p.name == *r && !p.registry).map(|(id, _)| id.clone()).ok_or_else(|| format!("no workspace package named {r}"))?;
        stack.push(id);
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    while let Some(id) = stack.pop() {
        if seen.insert(id.clone()) {
            stack.extend(nodes.get(&id).cloned().unwrap_or_default());
        }
    }
    let mut third: Vec<&Pkg> = seen.iter().filter_map(|id| packages.get(id)).filter(|p| p.registry).collect();
    third.sort_by(|a, b| (a.name.as_str(), a.version.as_str()).cmp(&(b.name.as_str(), b.version.as_str())));
    let mut texts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut out = String::new();
    for p in &third {
        let mut files = licence_files(&p.dir);
        let tag = format!("{} {}", p.name, p.version);
        // The file the manifest names: included whatever its name (once, even if also found at the root).
        let named = p.license_file.as_ref().map(|f| (f, p.dir.join(f)));
        let named_there = named.as_ref().map(|(_, at)| at.is_file()).unwrap_or(false);
        if let Some((_, at)) = named.as_ref().filter(|_| named_there) {
            if !files.contains(at) {
                files.push(at.clone());
            }
        }
        let expression = match (&named, p.license.is_empty()) {
            (Some((f, _)), true) => format!("see {f}"),
            _ => p.license.clone(),
        };
        if let Some((f, _)) = named.as_ref().filter(|_| !named_there) {
            out.push_str(&format!("{tag} \u{b7} {expression} \u{b7} (licence file {f} not in the crate)\n"));
        } else if files.is_empty() {
            out.push_str(&format!("{tag} \u{b7} {expression} \u{b7} (no licence file in the crate)\n"));
        } else {
            out.push_str(&format!("{tag} \u{b7} {expression}\n"));
        }
        for f in files {
            if let Ok(t) = std::fs::read_to_string(&f) {
                texts.entry(t.trim_end().to_string()).or_default().push(format!("{tag} ({})", f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
            }
        }
    }
    for (text, who) in texts {
        out.push_str("\n\u{2500}\u{2500}\u{2500}\u{2500}\n");
        for w in who {
            out.push_str(&w);
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&text);
        out.push('\n');
    }
    Ok(out)
}

/// The licence files a crate ships at its root (LICENSE*, LICENCE*, COPYING*, NOTICE*, UNLICENSE), by name.
fn licence_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .filter(|p| {
                    let n = p.file_name().map(|n| n.to_string_lossy().to_ascii_uppercase()).unwrap_or_default();
                    ["LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE"].iter().any(|k| n.starts_with(k))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// A small JSON reader for the metadata output (the build script uses no external crates).
mod json {
    #[derive(Debug)]
    pub enum Value {
        Null,
        Bool,
        Num,
        Str(String),
        Arr(Vec<Value>),
        Obj(Vec<(String, Value)>),
    }

    impl Value {
        pub fn get(&self, k: &str) -> Option<&Value> {
            match self {
                Value::Obj(m) => m.iter().find(|(x, _)| x == k).map(|(_, v)| v),
                _ => None,
            }
        }
        pub fn as_str(&self) -> Option<&str> {
            match self {
                Value::Str(s) => Some(s),
                _ => None,
            }
        }
        pub fn as_array(&self) -> Option<&Vec<Value>> {
            match self {
                Value::Arr(a) => Some(a),
                _ => None,
            }
        }
        pub fn is_null(&self) -> bool {
            matches!(self, Value::Null)
        }
    }

    pub fn parse(b: &[u8]) -> Result<Value, String> {
        let mut p = P { b, i: 0 };
        let v = p.value()?;
        p.ws();
        if p.i != b.len() {
            return Err(format!("trailing bytes at {}", p.i));
        }
        Ok(v)
    }

    struct P<'a> {
        b: &'a [u8],
        i: usize,
    }

    impl P<'_> {
        fn ws(&mut self) {
            while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\n' | b'\r' | b'\t') {
                self.i += 1;
            }
        }
        fn eat(&mut self, c: u8) -> Result<(), String> {
            self.ws();
            if self.b.get(self.i) == Some(&c) {
                self.i += 1;
                Ok(())
            } else {
                Err(format!("expected {} at {}", c as char, self.i))
            }
        }
        fn value(&mut self) -> Result<Value, String> {
            self.ws();
            match self.b.get(self.i) {
                Some(b'{') => {
                    self.i += 1;
                    let mut m = Vec::new();
                    self.ws();
                    if self.b.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Ok(Value::Obj(m));
                    }
                    loop {
                        self.ws();
                        let k = self.string()?;
                        self.eat(b':')?;
                        let v = self.value()?;
                        m.push((k, v));
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b'}') => {
                                self.i += 1;
                                return Ok(Value::Obj(m));
                            }
                            _ => return Err(format!("object at {}", self.i)),
                        }
                    }
                }
                Some(b'[') => {
                    self.i += 1;
                    let mut a = Vec::new();
                    self.ws();
                    if self.b.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(Value::Arr(a));
                    }
                    loop {
                        a.push(self.value()?);
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b']') => {
                                self.i += 1;
                                return Ok(Value::Arr(a));
                            }
                            _ => return Err(format!("array at {}", self.i)),
                        }
                    }
                }
                Some(b'"') => Ok(Value::Str(self.string()?)),
                Some(b't') => self.word("true", Value::Bool),
                Some(b'f') => self.word("false", Value::Bool),
                Some(b'n') => self.word("null", Value::Null),
                Some(c) if *c == b'-' || c.is_ascii_digit() => {
                    while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                        self.i += 1;
                    }
                    Ok(Value::Num)
                }
                _ => Err(format!("value at {}", self.i)),
            }
        }
        fn word(&mut self, w: &str, v: Value) -> Result<Value, String> {
            if self.b[self.i..].starts_with(w.as_bytes()) {
                self.i += w.len();
                Ok(v)
            } else {
                Err(format!("word at {}", self.i))
            }
        }
        fn string(&mut self) -> Result<String, String> {
            if self.b.get(self.i) != Some(&b'"') {
                return Err(format!("string at {}", self.i));
            }
            self.i += 1;
            let mut out: Vec<u8> = Vec::new();
            loop {
                match self.b.get(self.i) {
                    None => return Err("unterminated string".into()),
                    Some(b'"') => {
                        self.i += 1;
                        return String::from_utf8(out).map_err(|e| e.to_string());
                    }
                    Some(b'\\') => {
                        let c = *self.b.get(self.i + 1).ok_or("escape")?;
                        self.i += 2;
                        match c {
                            b'n' => out.push(b'\n'),
                            b't' => out.push(b'\t'),
                            b'r' => out.push(b'\r'),
                            b'b' => out.push(8),
                            b'f' => out.push(12),
                            b'u' => {
                                let hex = std::str::from_utf8(self.b.get(self.i..self.i + 4).ok_or("\\u")?).map_err(|e| e.to_string())?;
                                let mut cp = u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
                                self.i += 4;
                                if (0xD800..0xDC00).contains(&cp) && self.b.get(self.i..self.i + 2) == Some(b"\\u") {
                                    let lo = std::str::from_utf8(self.b.get(self.i + 2..self.i + 6).ok_or("\\u")?).map_err(|e| e.to_string())?;
                                    let lo = u32::from_str_radix(lo, 16).map_err(|e| e.to_string())?;
                                    cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                    self.i += 6;
                                }
                                let ch = char::from_u32(cp).unwrap_or('\u{fffd}');
                                let mut buf = [0u8; 4];
                                out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                            }
                            other => out.push(other),
                        }
                    }
                    Some(c) => {
                        out.push(*c);
                        self.i += 1;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One registry crate per case, each in its own directory: an expression with a root licence file; only
    /// `license-file` (a name no root pattern matches); both, the named file also a root one (included once);
    /// `license-file` naming a file in a subdirectory; `license-file` naming a missing file; neither.
    #[test]
    fn a_licence_named_by_file_is_listed_and_carried() {
        let root = std::env::temp_dir().join(format!("zk-pack-notices-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let krate = |name: &str, files: &[(&str, &str)]| {
            let d = root.join(name);
            std::fs::create_dir_all(&d).expect("the crate's directory");
            for (f, text) in files {
                let at = d.join(f);
                std::fs::create_dir_all(at.parent().expect("a parent")).expect("dir");
                std::fs::write(at, text).expect("file");
            }
            std::fs::write(d.join("Cargo.toml"), "").expect("manifest");
            d.join("Cargo.toml").display().to_string().replace('\\', "/")
        };
        let rows = [
            ("expr", "MIT", None, krate("expr", &[("LICENSE", "expression text")])),
            ("fileonly", "", Some("COPYRIGHT.txt"), krate("fileonly", &[("COPYRIGHT.txt", "named text")])),
            ("both", "Apache-2.0", Some("LICENSE-APACHE"), krate("both", &[("LICENSE-APACHE", "apache text")])),
            ("nested", "", Some("legal/terms.txt"), krate("nested", &[("legal/terms.txt", "nested text")])),
            ("missing", "", Some("GONE.txt"), krate("missing", &[])),
            ("neither", "", None, krate("neither", &[])),
        ];
        let mut packages = vec!["{\"id\":\"root\",\"name\":\"root\",\"version\":\"0.1.0\",\"license\":\"MIT\",\"manifest_path\":\"/w/Cargo.toml\",\"source\":null}".to_string()];
        let mut deps = Vec::new();
        for (name, license, file, manifest) in &rows {
            let lf = file.map(|f| format!("\"{f}\"")).unwrap_or_else(|| "null".into());
            packages.push(format!("{{\"id\":\"{name}\",\"name\":\"{name}\",\"version\":\"1.0.0\",\"license\":\"{license}\",\"license_file\":{lf},\"manifest_path\":\"{manifest}\",\"source\":\"registry\"}}"));
            deps.push(format!("{{\"pkg\":\"{name}\",\"dep_kinds\":[{{\"kind\":null}}]}}"));
        }
        let meta = format!("{{\"packages\":[{}],\"resolve\":{{\"nodes\":[{{\"id\":\"root\",\"deps\":[{}]}}]}}}}", packages.join(","), deps.join(","));
        let text = notices_of(&json::parse(meta.as_bytes()).expect("metadata"), &["root"]).expect("notices");
        for line in [
            "expr 1.0.0 \u{b7} MIT\n",
            "fileonly 1.0.0 \u{b7} see COPYRIGHT.txt\n",
            "both 1.0.0 \u{b7} Apache-2.0\n",
            "nested 1.0.0 \u{b7} see legal/terms.txt\n",
            "missing 1.0.0 \u{b7} see GONE.txt \u{b7} (licence file GONE.txt not in the crate)\n",
            "neither 1.0.0 \u{b7}  \u{b7} (no licence file in the crate)\n",
        ] {
            assert!(text.contains(line), "{line:?} in:\n{text}");
        }
        for body in ["expression text", "named text", "apache text", "nested text"] {
            assert_eq!(text.matches(body).count(), 1, "{body}: carried once:\n{text}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
