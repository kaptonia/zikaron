//! Remote fetch: download a record bundle from an `https://` address and run it through the same kit
//! verification as a local bundle.
//!
//! The spec links chain to entries (law §2.1, §9), entries to files (law §6.2, kit law §7.6) and a bundle
//! to its manifest (kit law §7.3, §7.4), so storage can be entirely untrusted: bytes from anyone are
//! recomputed and checked. This module only transports: fetch the manifest, fetch each file it lists, hand
//! the whole enumeration to the kit crate's `verify_enumeration`, and return it only if it passes. A fetched
//! bundle cannot have extra files, since only listed files are fetched; the page labels it apart from local
//! bundles ("from an address · verified").
//!
//! Bounds:
//!
//! - `https://` only; certificate chain and host name always verified (`chainx::Https`, the same TLS client
//!   as node queries).
//! - At most [`MAX_FILE`] per file and [`MAX_TOTAL`] per bundle (the same cap as single-file bundles), and
//!   at most [`TIMEOUT_SECS`] seconds per fetch.
//! - Redirects are followed only to the same `https` origin (same host and port), at most [`MAX_HOPS`]
//!   times; other redirects are refused by name.
//! - Each failure mode has its own code (`REMOTE_*`), with the address in the evidence tail.

use crate::fault::{Fault, Known};
use zikaron::json::Value;

/// Total deadline of one fetch (seconds).
pub const TIMEOUT_SECS: u64 = 20;
/// Per-file cap (bytes).
pub const MAX_FILE: usize = 32 << 20;
/// Total cap per bundle (bytes). The same number as single-file bundles.
pub const MAX_TOTAL: u64 = zikaron_glue::container::MAX_TOTAL;
/// At most this many same-origin redirects.
pub const MAX_HOPS: usize = 3;

/// A publish address: a recognized `https://…/` (ending in `/`, with paths within the bundle appended
/// directly).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
    url: String,
}

impl Base {
    pub fn as_str(&self) -> &str {
        &self.url
    }

    /// The address of a path in the bundle. Addresses are assembled only here.
    pub fn at(&self, rel: &str) -> String {
        format!("{}{}", self.url, rel)
    }
}

/// Parses a publish address. `https` only, scheme and host in any case (read by the shared transport's
/// parser); anything else is `REMOTE_NOT_HTTPS` with the input unchanged as subject. The address is kept as
/// the parser normalizes it (lowercase scheme and host), so `HTTPS://Host/k` and `https://host/k` are one
/// address. An address ending in `…/manifest.json` steps back to its directory.
pub fn base_of(typed: &str) -> Result<Base, Fault> {
    let typed = typed.trim();
    let Some(h) = crate::chainx::Https::new(typed) else {
        // Say why when the port or shape is wrong. A publish address is where a kit is published, not a node,
        // so it is echoed as typed (node addresses are echoed only via `zikaron_net::sayable`).
        use crate::lang::{filln, Key};
        let why = match zikaron_net::read_address(typed) {
            Err(zikaron_net::NotAnAddress::Port) => filln(Key::TailNodePort, &[typed]),
            Err(zikaron_net::NotAnAddress::Host | zikaron_net::NotAnAddress::Shape) => filln(Key::TailNodeAddress, &[typed]),
            _ => typed.to_string(),
        };
        return Err(Fault::known(Known::RemoteNotHttps, why));
    };
    let normal = h.address();
    let t = normal.as_str();
    // Step back to the manifest's directory by whole path segment only: a suffix match would cut
    // `…/oldmanifest.json` to `…/old/`, and every later fetch would get a 404.
    let t = t.strip_suffix(&format!("/{}", zikaron_glue::names::MANIFEST)).map(|x| format!("{x}/")).unwrap_or_else(|| t.to_string());
    let t = t.as_str();
    let url = if t.ends_with('/') { t.to_string() } else { format!("{t}/") };
    Ok(Base { url })
}

/// Whether this text is written as an address: a scheme (a letter, then letters, digits, `+`, `-`, `.`)
/// followed by `://`. The bytes cell branches on it: addresses go to remote fetch, everything else to local
/// paths. Text that is not a valid https address (a port past 16 bits, an empty port, another scheme) is
/// still refused by name as a remote address, never looked up as a local path.
pub fn is_address(typed: &str) -> bool {
    match typed.trim().split_once("://") {
        Some((scheme, _)) => {
            let mut c = scheme.chars();
            c.next().is_some_and(|f| f.is_ascii_alphabetic()) && c.all(|x| x.is_ascii_alphanumeric() || matches!(x, '+' | '-' | '.'))
        }
        None => false,
    }
}

fn limits() -> zikaron_anchor::rpc::Limits {
    // Headers and chunk overhead count toward the answer; `get_one` checks the body cap separately. The
    // deadline can only be shortened, never past [`TIMEOUT_SECS`]: by `ZKA_TIMEOUT_SECS` (shared with node
    // queries), or by `ZKA_TIMEOUT_MS` when present (for deadlines under a second).
    let secs = std::env::var(zikaron_anchor::rpc::env::TIMEOUT_SECS).ok().and_then(|x| x.parse::<u64>().ok()).filter(|n| *n > 0).map(|n| n.min(TIMEOUT_SECS)).unwrap_or(TIMEOUT_SECS);
    let cap = std::time::Duration::from_secs(TIMEOUT_SECS);
    let deadline = match std::env::var(zikaron_anchor::rpc::env::TIMEOUT_MS).ok().and_then(|x| x.parse::<u64>().ok()).filter(|n| *n > 0) {
        Some(ms) => std::time::Duration::from_millis(ms).min(cap),
        None => std::time::Duration::from_secs(secs),
    };
    zikaron_anchor::rpc::Limits { deadline, max_answer: MAX_FILE + (64 << 10) }
}

/// Transport troubles to codes: certificate, deadline and overflow each have a name; anything else is
/// unreachable.
fn trouble(url: &str, t: &zikaron_anchor::rpc::Trouble) -> Fault {
    let said = crate::chainx::trouble_said(url, t);
    let k = match crate::chainx::said(t) {
        crate::chainx::Said::Wire(crate::chainx::Wire::Tls(crate::chainx::Layer::Certificate)) => Known::RemoteCert,
        crate::chainx::Said::Wire(crate::chainx::Wire::Timeout) => Known::RemoteTimeout,
        crate::chainx::Said::Wire(crate::chainx::Wire::Oversize) => Known::RemoteTooLarge,
        _ => Known::RemoteUnreachable,
    };
    Fault::known(k, said)
}

/// An address's origin (for same-origin comparison).
fn origin(url: &str) -> Option<(String, u16)> {
    let h = crate::chainx::Https::new(url)?;
    let (host, port, _) = h.parts();
    Some((host.to_ascii_lowercase(), port))
}

/// A fetch result: the bytes, or the server says it is absent (404 or 410; "missing" when checking
/// publication).
pub enum One {
    Bytes(Vec<u8>),
    Absent(u16),
}

/// Fetch one. Only same-origin https redirects are followed; a status other than 200 is named (404/410 return
/// `Absent`, others are refused).
pub fn get_one(url: &str) -> Result<One, Fault> {
    let home = origin(url).ok_or_else(|| Fault::known(Known::RemoteNotHttps, url.to_string()))?;
    let mut at = url.to_string();
    for _ in 0..=MAX_HOPS {
        let h = crate::chainx::Https::new(&at).ok_or_else(|| Fault::known(Known::RemoteNotHttps, at.clone()))?;
        let got = h.get(&limits()).map_err(|t| trouble(&at, &t))?;
        match got.status {
            200 => {
                if got.body.len() > MAX_FILE {
                    return Err(Fault::known(Known::RemoteTooLarge, format!("{at} · {MAX_FILE}")));
                }
                return Ok(One::Bytes(got.body));
            }
            301 | 302 | 303 | 307 | 308 => {
                let to = got.location.unwrap_or_default();
                let next = if to.starts_with('/') {
                    let (host, port) = &home;
                    if *port == 443 { format!("https://{host}{to}") } else { format!("https://{host}:{port}{to}") }
                } else {
                    to.clone()
                };
                if origin(&next).as_ref() != Some(&home) {
                    return Err(Fault::known(Known::RemoteRedirect, format!("{at} → {to}")));
                }
                at = next;
            }
            404 | 410 => return Ok(One::Absent(got.status)),
            n => return Err(Fault::known(Known::RemoteStatus, format!("{n} {at}")).with_status(n)),
        }
    }
    Err(Fault::known(Known::RemoteRedirect, format!("{url} · {MAX_HOPS}")))
}

/// Fetch one; the peer saying absent is also a refusal (fetching a bundle needs every item).
fn must(url: &str) -> Result<Vec<u8>, Fault> {
    match get_one(url)? {
        One::Bytes(b) => Ok(b),
        One::Absent(n) => Err(Fault::known(Known::RemoteStatus, format!("{n} {url}")).with_status(n)),
    }
}

/// The in-bundle paths the manifest lists (entries, files, proofs), read from kit law §7.3's three tables.
/// The manifest is not judged here; `verify_enumeration` judges it afterwards. If the three tables cannot be
/// read, this reports what kit verification would (`E_KIT_MANIFEST`).
fn named_in(manifest: &[u8]) -> Result<Vec<String>, Fault> {
    let v = zikaron::json::parse(manifest).map_err(|_| Fault::known(Known::RemoteKit, zikaron_kit::tokens::KitFailToken::Manifest.as_str().to_string()))?;
    let mut out: Vec<String> = Vec::new();
    let arr = |k: &str| -> Vec<Value> {
        match v.member(k) {
            Some(Value::Arr(a)) => a.clone(),
            _ => Vec::new(),
        }
    };
    for id in arr(zikaron_glue::names::ENTRIES_DIR) {
        if let Value::Str(s) = id {
            out.push(format!("{}/{}{}", zikaron_glue::names::ENTRIES_DIR, s.trim_start_matches("0x"), zikaron_glue::names::ENTRY_SUFFIX));
        }
    }
    for dir in [zikaron_glue::names::FILES_DIR, zikaron_glue::names::PROOFS_DIR] {
        for row in arr(dir) {
            if let Some(Value::Str(p)) = row.member("path") {
                out.push(format!("{dir}/{p}"));
            }
        }
    }
    // A path that fails kit law §7.2 is not fetched: put into an address, it could point elsewhere.
    if let Some(bad) = out.iter().find(|p| !zikaron_kit::kitdir::is_kit_path(p)) {
        return Err(Fault::known(Known::RemoteKit, format!("E_KIT_MANIFEST:{bad}")));
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// A fetched bundle.
#[derive(Clone, Debug)]
pub struct Remote {
    pub url: String,
    /// The enumeration (in-bundle path to bytes), verified.
    pub pairs: Vec<(String, Vec<u8>)>,
    /// How many items the manifest lists (including the manifest).
    pub files: usize,
}

/// Fetch a bundle per its manifest and hand it to kit verification. Returned only when it passes; otherwise
/// `REMOTE_KIT`, with the kit crate's verdict and subject in the tail.
pub fn fetch_kit(base: &Base) -> Result<Remote, Fault> {
    crate::trace::mark(crate::feature::Feature::W14);
    let manifest = must(&base.at(zikaron_glue::names::MANIFEST))?;
    let mut total = manifest.len() as u64;
    let mut pairs: Vec<(String, Vec<u8>)> = vec![(zikaron_glue::names::MANIFEST.to_string(), manifest.clone())];
    let named = named_in(&manifest)?;
    // Cap the item count too: the byte cap counts only fetched bodies, so a manifest listing a hundred
    // thousand empty files would pass it while each needs its own https round trip, and the pass would never
    // finish. Uses the single-file bundle's item cap.
    if named.len() + 1 > zikaron_glue::container::MAX_ITEMS {
        return Err(Fault::known(Known::RemoteKit, format!("E_KIT_ITEMS:{} · {}", named.len() + 1, base.as_str())));
    }
    for rel in named {
        let bytes = must(&base.at(&rel))?;
        total += bytes.len() as u64;
        if total > MAX_TOTAL {
            return Err(Fault::known(Known::RemoteTooLarge, format!("{} · {MAX_TOTAL}", base.as_str())));
        }
        pairs.push((rel, bytes));
    }
    match zikaron_kit::kitdir::verify_enumeration(&pairs) {
        zikaron_kit::kitdir::KitVerdict::Ok { .. } => Ok(Remote { url: base.as_str().to_string(), files: pairs.len(), pairs }),
        zikaron_kit::kitdir::KitVerdict::Fail { verdict, subject } => Err(Fault::known(
            Known::RemoteKit,
            format!("{}{} · {}", verdict.as_str(), subject.map(|s| format!(":{s}")).unwrap_or_default(), base.as_str()),
        )),
    }
}

/// One "check publication" reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
    /// How many items in the local bundle (including the manifest).
    pub total: usize,
    /// Items absent at the publish address (in-bundle paths).
    pub missing: Vec<String>,
    /// Items fetched whose bytes differ.
    pub differ: Vec<String>,
}

impl Published {
    pub fn complete(&self) -> bool {
        self.missing.is_empty() && self.differ.is_empty()
    }
}

/// Check publication. Fetch each file of the local bundle and compare bytes; an unreachable peer, failed
/// certificate, timeout or overflow refuses the whole pass by name ("unreachable"). The product does not
/// upload for the person: this step only reads.
pub fn compare(base: &Base, local: &[(String, Vec<u8>)]) -> Result<Published, Fault> {
    let mut missing = Vec::new();
    let mut differ = Vec::new();
    for (rel, bytes) in local {
        match get_one(&base.at(rel))? {
            One::Bytes(b) if b == *bytes => {}
            One::Bytes(_) => differ.push(rel.clone()),
            One::Absent(_) => missing.push(rel.clone()),
        }
    }
    Ok(Published { total: local.len(), missing, differ })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_addresses_are_taken() {
        assert_eq!(base_of("http://example.org/x").err().and_then(|f| f.which()), Some(Known::RemoteNotHttps));
        assert_eq!(base_of("ftp://example.org/").err().and_then(|f| f.which()), Some(Known::RemoteNotHttps));
        assert_eq!(base_of("https://").err().and_then(|f| f.which()), Some(Known::RemoteNotHttps));
        let b = base_of(" https://records.example.org/zikaron ").ok().expect("认得");
        assert_eq!(b.as_str(), "https://records.example.org/zikaron/");
        assert_eq!(base_of("https://h.example/k/manifest.json").ok().map(|b| b.at("x")), Some("https://h.example/k/x".to_string()));
    }

    #[test]
    fn a_manifest_path_outside_the_kit_law_is_not_fetched() {
        let m = br#"{"entries":[],"files":[{"path":"../../etc/passwd","sha256":"0x00","size":1}],"proofs":[]}"#;
        assert_eq!(named_in(m).err().and_then(|f| f.which()), Some(Known::RemoteKit));
    }
}
