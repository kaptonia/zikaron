pub const CODE_FILE: &str = "zikaron-grant.txt";
pub const PUBLISH_FILE: &str = "publish.txt";
pub const TERMS_ROOM: &str = "terms";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Package,
    File,
}

#[derive(Clone, Debug)]
pub struct Opened {
    pub form: Form,
    pub hops: Vec<Vec<u8>>,
    pub ledger: Vec<Vec<u8>>,
    pub terms: Vec<(String, Vec<u8>)>,
    pub publish: Option<String>,
    pub root: Option<String>,
    pub contents: Vec<(String, String)>,
    pub kit_id: String,
    pub files: usize,
}

#[derive(Debug)]
pub enum Refused {
    Shape(crate::container::Bad),
    Kit(String, Option<String>),
    NoCode(String),
    Code(zikaron_kit::v2::doc::DecodeReject),
    Uncarried(String),
    Package(String, Option<String>),
}

fn under(rel: &str) -> String {
    format!("{}/{rel}", crate::names::FILES_DIR)
}

pub fn open(bytes: &[u8]) -> Result<Opened, Refused> {
    crate::seam_v2();
    let pairs = crate::container::decode(bytes).map_err(Refused::Shape)?;
    open_pairs(&pairs)
}

pub fn left_out(work: &str, delivered: bool, places: Vec<String>) -> Vec<String> {
    if delivered || !places.is_empty() {
        places
    } else {
        vec![work.to_string()]
    }
}

pub fn is_package(pairs: &[(String, Vec<u8>)]) -> bool {
    pairs.iter().any(|(p, _)| *p == under(CODE_FILE))
}

pub fn open_pairs(pairs: &[(String, Vec<u8>)]) -> Result<Opened, Refused> {
    crate::seam_v2();
    let pairs = pairs.to_vec();
    let manifest = pairs.iter().find(|(p, _)| p == crate::names::MANIFEST).map(|(_, b)| b.as_slice());
    let (form, kit_id, root, contents) = match zikaron_kit::v2::dispatch::manifest_law(manifest) {
        zikaron_kit::v2::dispatch::Law::Kit1 => match zikaron_kit::kitdir::verify_enumeration(&pairs) {
            zikaron_kit::kitdir::KitVerdict::Ok { kit_id, .. } => (Form::File, zikaron::hexfmt::encode(&kit_id), None, Vec::new()),
            zikaron_kit::kitdir::KitVerdict::Fail { verdict, subject } => return Err(Refused::Kit(verdict.as_str().to_string(), subject)),
        },
        zikaron_kit::v2::dispatch::Law::Kit2 => {
            let en: zikaron_kit::v2::kit::Enumeration = pairs.iter().cloned().collect();
            package_verdict(&en)?
        }
    };
    let code = pairs
        .iter()
        .find(|(p, _)| *p == under(CODE_FILE))
        .map(|(_, b)| b.clone())
        .ok_or_else(|| Refused::NoCode(under(CODE_FILE)))?;
    let trimmed: Vec<u8> = code.iter().copied().filter(|c| !c.is_ascii_whitespace()).collect();
    let hops: Vec<Vec<u8>> = zikaron_kit::v2::doc::badge_decode(&trimmed).map_err(Refused::Code)?.iter().map(|e| e.bytes().to_vec()).collect();
    let entries_dir = format!("{}/", crate::names::ENTRIES_DIR);
    let ledger: Vec<Vec<u8>> = pairs
        .iter()
        .filter(|(p, _)| p.starts_with(&entries_dir))
        .filter(|(_, b)| crate::book::check(b).is_ok())
        .map(|(_, b)| b.clone())
        .collect();
    if let Some(h) = zikaron_kit::badge::uncarried(&hops, &ledger) {
        return Err(Refused::Uncarried(zikaron::hexfmt::encode(&zikaron::entry::entry_id(h))));
    }
    let files_dir = crate::names::FILES_DIR;
    let terms_dir = format!("{files_dir}/{TERMS_ROOM}/");
    let terms: Vec<(String, Vec<u8>)> = pairs
        .iter()
        .filter(|(p, _)| p.starts_with(&terms_dir))
        .map(|(p, b)| (p[files_dir.len() + 1..].to_string(), b.clone()))
        .collect();
    let publish = pairs
        .iter()
        .find(|(p, _)| *p == under(PUBLISH_FILE))
        .map(|(_, b)| String::from_utf8_lossy(b).trim().to_string())
        .filter(|s| !s.is_empty());
    Ok(Opened { form, hops, ledger, terms, publish, root, contents, kit_id, files: pairs.len() })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Granted {
    pub id: String,
    pub work: String,
}

pub fn granted(pairs: &[(String, Vec<u8>)]) -> Option<Granted> {
    let o = open_pairs(pairs).ok()?;
    let read: Vec<crate::book::Entry> = o.hops.iter().map(|h| crate::book::check(h).ok()).collect::<Option<_>>()?;
    let work_of = |e: &crate::book::Entry| e.body.member(crate::names::Field::Work.as_str()).and_then(zikaron::json::Value::as_str).map(str::to_ascii_lowercase);
    let work = work_of(read.first()?)?;
    if read.iter().any(|e| work_of(e).as_deref() != Some(work.as_str())) {
        return None;
    }
    Some(Granted { id: read.last()?.id_hex(), work })
}

fn package_verdict(en: &zikaron_kit::v2::kit::Enumeration) -> Result<(Form, String, Option<String>, Vec<(String, String)>), Refused> {
    use zikaron::json::Value;
    use zikaron_kit::v2::tokens::GpToken;
    let v = zikaron_kit::v2::kit::verify_grant_package(en);
    let text = |k: &str| v.member(k).and_then(Value::as_str).map(str::to_string);
    let verdict = text("verdict").unwrap_or_default();
    if verdict == zikaron_kit::v2::tokens::GP_OK {
        let contents = match v.member("contents") {
            Some(Value::Arr(rows)) => rows
                .iter()
                .filter_map(|r| Some((r.member("content")?.as_str()?.to_string(), r.member("path")?.as_str()?.to_string())))
                .collect(),
            _ => Vec::new(),
        };
        return Ok((Form::Package, text("kit_id").unwrap_or_default(), text("root"), contents));
    }
    let subject = match v.member("subject") {
        Some(Value::Str(s)) => Some(s.clone()),
        Some(Value::Int(n)) => Some(n.to_string()),
        _ => None,
    };
    if [GpToken::NoCode, GpToken::Root, GpToken::Uncarried, GpToken::Content, GpToken::Terms].iter().any(|t| t.as_str() == verdict) {
        return Err(Refused::Package(verdict, subject));
    }
    if verdict.starts_with("E_KIT") {
        return Err(Refused::Kit(verdict, subject));
    }
    let code = en.get(&under(CODE_FILE)).cloned().unwrap_or_default();
    let trimmed: Vec<u8> = code.iter().copied().filter(|c| !c.is_ascii_whitespace()).collect();
    match zikaron_kit::v2::doc::badge_decode(&trimmed) {
        Err(r) => Err(Refused::Code(r)),
        Ok(_) => Err(Refused::Package(verdict, subject)),
    }
}

#[derive(Default)]
pub struct Parts {
    pub hops: Vec<Vec<u8>>,
    pub ledger: Vec<Vec<u8>>,
    pub root: Option<String>,
    pub code: String,
    pub terms: Vec<(String, Vec<u8>)>,
    pub publish: Option<String>,
    pub original: Option<Original>,
    pub anchored_on: Option<crate::anchoredon::AnchoredOn>,
    pub anchors: Vec<crate::pack::Anchor>,
    pub upstream: Vec<Vec<Vec<u8>>>,
}

pub enum Original {
    File(String, Vec<u8>),
    Dir(String, Vec<(String, Vec<u8>)>),
}

pub fn build(p: Parts) -> Result<crate::pack::Bundle, String> {
    crate::seam_v2();
    let mut entries: Vec<Vec<u8>> = p.hops.clone();
    for b in carried(&p.hops, &p.ledger, &p.upstream) {
        if !entries.contains(&b) {
            entries.push(b);
        }
    }
    let mut b = crate::pack::Bundle { entries, root: p.root, anchored_on: p.anchored_on, anchors: p.anchors, one_work: true, ..Default::default() };
    b.files.push((CODE_FILE.to_string(), p.code.into_bytes()));
    for (rel, bytes) in p.terms {
        if !b.files.iter().any(|(x, _)| *x == rel) {
            b.files.push((rel, bytes));
        }
    }
    if let Some(u) = p.publish.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        b.files.push((PUBLISH_FILE.to_string(), u.as_bytes().to_vec()));
    }
    match p.original {
        None => {}
        Some(Original::File(name, bytes)) => {
            crate::kitnames::put_original(&name, bytes, &mut b)?;
        }
        Some(Original::Dir(name, files)) => {
            crate::kitnames::put_directory(&name, files, &mut b)?;
        }
    }
    Ok(b)
}

pub fn carried(hops: &[Vec<u8>], ledger: &[Vec<u8>], upstream: &[Vec<Vec<u8>>]) -> Vec<Vec<u8>> {
    let mine: Vec<&Vec<u8>> = hops.iter().filter(|h| ledger.contains(h) && crate::notices::carries(h)).collect();
    let mut out = if mine.is_empty() { ledger.to_vec() } else { spine_beside(&mine, ledger) };
    for h in hops.iter().filter(|h| !ledger.contains(h) && crate::notices::carries(h)) {
        for theirs in upstream.iter().filter(|p| p.contains(h)) {
            for b in spine_beside(&[h], theirs) {
                if !out.contains(&b) {
                    out.push(b);
                }
            }
        }
    }
    out
}

fn spine_beside(mine: &[&Vec<u8>], ledger: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let read: Vec<(usize, crate::book::Entry)> = ledger.iter().enumerate().filter_map(|(i, b)| crate::book::check(b).ok().map(|e| (i, e))).collect();
    let mut take: Vec<usize> = read.iter().filter(|(_, e)| mine.iter().any(|h| **h == e.bytes)).map(|(i, _)| *i).collect();
    let named: Vec<String> = mine.iter().filter_map(|h| crate::book::check(h).ok()).filter_map(|e| e.body.member("history").and_then(|v| v.as_str()).map(str::to_ascii_lowercase)).collect();
    for (i, e) in &read {
        if named.contains(&e.id_hex()) && !take.contains(i) {
            take.push(*i);
        }
    }
    crate::select::pull_lineage(&read, &mut take);
    take.sort_unstable();
    take.into_iter().map(|i| ledger[i].clone()).collect()
}

pub fn first_root(chain: &[Vec<u8>], ledger: &[Vec<u8>]) -> Option<String> {
    let first = chain.first()?;
    if ledger.contains(first) {
        if let Ok(r) = crate::book::root_of(ledger) {
            return Some(r);
        }
    }
    let e = crate::book::check(first).ok()?;
    Some(e.body.member("ledger").and_then(|v| v.as_str()).map(str::to_ascii_lowercase).unwrap_or(e.author))
}

pub fn hop_inputs(o: &Opened, fragment: &zikaron::json::Value) -> Vec<(Option<String>, Option<zikaron::json::Value>)> {
    let mut out = Vec::with_capacity(o.hops.len());
    for k in 0..o.hops.len() {
        let root = hop_root(&o.hops, &o.ledger, o.root.as_deref(), k);
        let input = root.as_deref().and_then(|r| crate::book::input_of(r, &o.ledger, fragment).ok());
        out.push((root, input));
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct Judged {
    pub root: Option<String>,
    pub input: Option<zikaron::json::Value>,
    pub late: bool,
}

pub fn hops_judged(o: &Opened, fragment: &zikaron::json::Value, book: &dyn Fn(&str) -> Option<String>) -> Vec<Judged> {
    o.hops
        .iter()
        .zip(hop_inputs(o, fragment))
        .map(|(h, (root, input))| {
            let copy: Vec<Vec<u8>> = crate::book::check(h).ok().and_then(|e| crate::book::ledger_of(&o.ledger, &e.id_hex(), &e.author)).unwrap_or_default();
            let late = root.as_deref().is_some_and(|r| !crate::notices::copy_stands(h, &copy, book(r).as_deref()));
            Judged { root, input: input.filter(|_| !late), late }
        })
        .collect()
}

pub fn hop_root(hops: &[Vec<u8>], ledger: &[Vec<u8>], root: Option<&str>, k: usize) -> Option<String> {
    if k == 0 {
        return root.map(str::to_string).or_else(|| first_root(hops, ledger));
    }
    crate::book::check(hops.get(k - 1)?).ok().and_then(|u| u.body.member("grantee").and_then(zikaron::json::Value::as_str).map(str::to_ascii_lowercase))
}

#[derive(Debug)]
pub enum Unbuilt {
    Name(String),
    Kit(crate::pack::Trouble),
    Package(String, Option<String>),
}

pub fn lay(parts: Parts) -> Result<(Vec<u8>, crate::pack::Landed), Unbuilt> {
    let b = build(parts).map_err(Unbuilt::Name)?;
    let (pairs, landed) = crate::pack::enumerate(b).map_err(Unbuilt::Kit)?;
    check_built(&pairs).map_err(|(t, at)| Unbuilt::Package(t, at))?;
    Ok((crate::container::encode(&pairs), landed))
}

pub fn check_built(pairs: &[(String, Vec<u8>)]) -> Result<(), (String, Option<String>)> {
    let en: zikaron_kit::v2::kit::Enumeration = pairs.iter().cloned().collect();
    match package_verdict(&en) {
        Ok(_) => Ok(()),
        Err(Refused::Kit(t, s)) | Err(Refused::Package(t, s)) => Err((t, s)),
        Err(Refused::NoCode(at)) => Err(("E_GRANT_FILE_CODE".to_string(), Some(at))),
        Err(Refused::Uncarried(id)) => Err(("E_GRANT_FILE_CHAIN".to_string(), Some(id))),
        Err(Refused::Code(r)) => Err((r.token.as_str().to_string(), r.index.map(|i| i.to_string()))),
        Err(Refused::Shape(b)) => Err((b.code().to_string(), Some(b.subject()))),
    }
}

pub fn chain_pool(held: impl IntoIterator<Item = Vec<u8>>, ledger: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = held.into_iter().collect();
    out.extend(ledger.iter().filter(|b| crate::book::check(b).is_ok_and(|e| e.kind == crate::book::Kind::Grant)).cloned());
    out
}
