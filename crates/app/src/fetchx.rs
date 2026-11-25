use crate::fault::{Fault, Known};
use zikaron::json::Value;

pub const TIMEOUT_SECS: u64 = 20;
pub const MAX_FILE: usize = 32 << 20;
pub const MAX_TOTAL: u64 = zikaron_glue::container::MAX_TOTAL;
pub const MAX_HOPS: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
    url: String,
}

impl Base {
    pub fn as_str(&self) -> &str {
        &self.url
    }

    pub fn at(&self, rel: &str) -> String {
        format!("{}{}", self.url, rel)
    }
}

pub fn base_of(typed: &str) -> Result<Base, Fault> {
    let typed = typed.trim();
    let Some(h) = crate::chainx::https(typed) else {
        use crate::lang::{filln, Key};
        let why = crate::lang::both_of(|| match zikaron_net::read_address(typed) {
            Err(zikaron_net::NotAnAddress::Port) => filln(Key::TailNodePort, &[typed]),
            Err(zikaron_net::NotAnAddress::Host | zikaron_net::NotAnAddress::Shape) => filln(Key::TailNodeAddress, &[typed]),
            _ => typed.to_string(),
        });
        return Err(Fault::known(Known::RemoteNotHttps, why));
    };
    let normal = h.address();
    let t = normal.as_str();
    let t = t.strip_suffix(&format!("/{}", zikaron_glue::names::MANIFEST)).map(|x| format!("{x}/")).unwrap_or_else(|| t.to_string());
    let t = t.as_str();
    let url = if t.ends_with('/') { t.to_string() } else { format!("{t}/") };
    Ok(Base { url })
}

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
    let secs = std::env::var(zikaron_anchor::rpc::env::TIMEOUT_SECS).ok().and_then(|x| x.parse::<u64>().ok()).filter(|n| *n > 0).map(|n| n.min(TIMEOUT_SECS)).unwrap_or(TIMEOUT_SECS);
    let cap = std::time::Duration::from_secs(TIMEOUT_SECS);
    let deadline = match std::env::var(zikaron_anchor::rpc::env::TIMEOUT_MS).ok().and_then(|x| x.parse::<u64>().ok()).filter(|n| *n > 0) {
        Some(ms) => std::time::Duration::from_millis(ms).min(cap),
        None => std::time::Duration::from_secs(secs),
    };
    zikaron_anchor::rpc::Limits { deadline, max_answer: MAX_FILE + (64 << 10) }
}

fn trouble(url: &str, t: &zikaron_anchor::rpc::Trouble) -> Fault {
    let said = crate::lang::both_of(|| crate::chainx::trouble_said(url, t));
    let k = match crate::chainx::said(t) {
        crate::chainx::Said::Wire(crate::chainx::Wire::Tls(crate::chainx::Layer::Certificate)) => Known::RemoteCert,
        crate::chainx::Said::Wire(crate::chainx::Wire::Timeout) => Known::RemoteTimeout,
        crate::chainx::Said::Wire(crate::chainx::Wire::Oversize) => Known::RemoteTooLarge,
        _ => Known::RemoteUnreachable,
    };
    Fault::known(k, said)
}

fn origin(url: &str) -> Option<(String, u16)> {
    let h = crate::chainx::https(url)?;
    let (host, port, _) = h.parts();
    Some((host.to_ascii_lowercase(), port))
}

pub enum One {
    Bytes(Vec<u8>),
    Absent(u16),
}

pub fn get_one(url: &str) -> Result<One, Fault> {
    let home = origin(url).ok_or_else(|| Fault::known(Known::RemoteNotHttps, url.to_string()))?;
    let mut at = url.to_string();
    for _ in 0..=MAX_HOPS {
        let h = crate::chainx::https(&at).ok_or_else(|| Fault::known(Known::RemoteNotHttps, at.clone()))?;
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

fn must(url: &str) -> Result<Vec<u8>, Fault> {
    match get_one(url)? {
        One::Bytes(b) => Ok(b),
        One::Absent(n) => Err(Fault::known(Known::RemoteStatus, format!("{n} {url}")).with_status(n)),
    }
}

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
    if let Some(bad) = out.iter().find(|p| !zikaron_kit::kitdir::is_kit_path(p)) {
        return Err(Fault::known(Known::RemoteKit, format!("E_KIT_MANIFEST:{bad}")));
    }
    out.sort();
    out.dedup();
    Ok(out)
}

#[derive(Clone, Debug)]
pub struct Remote {
    pub url: String,
    pub pairs: Vec<(String, Vec<u8>)>,
    pub files: usize,
}

pub fn fetch_kit(base: &Base) -> Result<Remote, Fault> {
    crate::trace::mark(crate::feature::Feature::W14);
    let manifest = must(&base.at(zikaron_glue::names::MANIFEST))?;
    let mut total = manifest.len() as u64;
    let mut pairs: Vec<(String, Vec<u8>)> = vec![(zikaron_glue::names::MANIFEST.to_string(), manifest.clone())];
    let named = named_in(&manifest)?;
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
    let facts = crate::verifyx::kit_facts_of(&pairs);
    if facts.ok {
        return Ok(Remote { url: base.as_str().to_string(), files: pairs.len(), pairs });
    }
    let subject = if facts.subject.is_empty() { String::new() } else { format!(":{}", facts.subject) };
    Err(Fault::known(Known::RemoteKit, format!("{}{subject} · {}", facts.verdict, base.as_str())))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
    pub total: usize,
    pub missing: Vec<String>,
    pub differ: Vec<String>,
}

impl Published {
    pub fn complete(&self) -> bool {
        self.missing.is_empty() && self.differ.is_empty()
    }
}

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
