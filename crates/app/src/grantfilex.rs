use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use std::path::{Path, PathBuf};

pub use zikaron_glue::grantfile::{CODE_FILE, PUBLISH_FILE};
pub const KEPT: &str = zikaron_seal::envelope::KEPT_DIR;

pub fn stem(id: &str) -> String {
    let head: String = id.trim().trim_start_matches("0x").chars().take(10).collect();
    format!("grant-{head}")
}

pub fn stem_of(id: &str, original: bool, sealed: bool) -> String {
    let mut s = stem(id);
    if original {
        s.push_str(FORM_ORIGINAL);
    }
    if sealed {
        s.push_str(FORM_SEALED);
    }
    s
}

pub const FORM_ORIGINAL: &str = "-original";
pub const FORM_SEALED: &str = "-sealed";

#[derive(Clone, Debug)]
pub struct Opened {
    pub hops: Vec<Vec<u8>>,
    pub ledger: Vec<Vec<u8>>,
    pub terms: Vec<(String, Vec<u8>)>,
    pub publish: Option<String>,
    pub root: Option<String>,
    pub contents: Vec<(String, String)>,
    pub kit_id: String,
    pub files: usize,
}

impl Opened {
    pub fn carries_ledger(&self) -> bool {
        self.ledger.len() > self.hops.len()
    }
}

pub fn open_bytes(bytes: &[u8]) -> Result<Opened, Fault> {
    use zikaron_glue::grantfile::Refused;
    crate::trace::mark(crate::feature::Feature::D6);
    let o = zikaron_glue::grantfile::open(bytes).map_err(|r| match r {
        Refused::Shape(b) => Fault::known(Known::GrantFileBad, format!("{}:{}", b.code(), b.subject())),
        Refused::Kit(verdict, subject) | Refused::Package(verdict, subject) => {
            Fault::known(Known::GrantFileKit, format!("{verdict}{}", subject.map(|s| format!(":{s}")).unwrap_or_default()))
        }
        Refused::NoCode(at) => Fault::known(Known::GrantFileKit, format!("E_GRANT_FILE_CODE:{at}")),
        Refused::Code(r) => crate::payloadx::refused(r),
        Refused::Uncarried(id) => Fault::known(Known::GrantFileKit, format!("E_GRANT_FILE_CHAIN:{id}")),
    })?;
    Ok(Opened { hops: o.hops, ledger: o.ledger, terms: o.terms, publish: o.publish, root: o.root, contents: o.contents, kit_id: o.kit_id, files: o.files })
}

pub fn open(path: &Path) -> Result<Opened, Fault> {
    let md = std::fs::metadata(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    if md.len() > zikaron_glue::container::MAX_TOTAL {
        return Err(Fault::known(Known::GrantFileBad, format!("E_FILE_OVERSIZE:{}", path.display())));
    }
    let bytes = std::fs::read(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    open_bytes(&bytes)
}

pub fn is_grant_file(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 32];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let n = f.read(&mut head).unwrap_or(0);
    zikaron_glue::container::is_container(&head[..n])
}

pub fn build(parts: zikaron_glue::grantfile::Parts) -> Result<(Vec<u8>, usize), Fault> {
    built(parts).map(|(b, files, _)| (b, files))
}

pub(crate) fn built(parts: zikaron_glue::grantfile::Parts) -> Result<(Vec<u8>, usize, String), Fault> {
    let (bytes, landed) = zikaron_glue::grantfile::lay(parts).map_err(|t| match t {
        zikaron_glue::grantfile::Unbuilt::Name(n) => Fault::of_landing(zikaron_glue::pack::Trouble::BadPath(n)),
        zikaron_glue::grantfile::Unbuilt::Kit(t) => Fault::of_landing(t),
        zikaron_glue::grantfile::Unbuilt::Package(t, at) => Fault::known(Known::GrantFileKit, format!("{t}{}", at.map(|x| format!(":{x}")).unwrap_or_default())),
    })?;
    Ok((bytes, landed.files, landed.kit_id))
}

pub fn parts(chain: &[Vec<u8>], ledger: &[Vec<u8>], terms: &[(String, Vec<u8>)], publish: Option<&str>) -> Result<zikaron_glue::grantfile::Parts, Fault> {
    Ok(zikaron_glue::grantfile::Parts {
        root: zikaron_glue::grantfile::first_root(chain, ledger),
        code: crate::badgex::payload_text(chain)?,
        hops: chain.to_vec(),
        ledger: ledger.to_vec(),
        terms: terms.to_vec(),
        publish: publish.map(str::to_string),
        ..Default::default()
    })
}

#[derive(Clone, Debug)]
pub struct Exported {
    pub path: PathBuf,
    pub chosen: crate::home::Chosen,
    pub hops: usize,
    pub terms: usize,
    pub ledger: bool,
    pub files: usize,
    pub kit_id: String,
    pub original: bool,
    pub taken: Vec<String>,
    pub left_out: Vec<String>,
    pub sealed: bool,
}

fn original_of(path: &Path, work: &str) -> Result<zikaron_glue::grantfile::Original, Fault> {
    let digests = crate::kitx::digest_of(path)?;
    if !digests.iter().any(|d| zikaron::hexfmt::encode(d).eq_ignore_ascii_case(work.trim())) {
        return Err(Fault::known(Known::NotAnOriginal, path.display().to_string()));
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| Fault::known(Known::FileMissing, path.display().to_string()))?;
    if path.is_dir() {
        let mut got: Vec<(String, Vec<u8>)> = Vec::new();
        zikaron_glue::pack::gather(path, "", &mut got).map_err(Fault::of_landing)?;
        return Ok(zikaron_glue::grantfile::Original::Dir(name, got));
    }
    let bytes = std::fs::read(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    Ok(zikaron_glue::grantfile::Original::File(name, bytes))
}

pub fn listed_original(home: &Home, grant: &str) -> Option<String> {
    let mine = home.ledger().ok()?.pile().ok()?.items;
    let work = mine.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).find(|e| e.id_hex().eq_ignore_ascii_case(grant.trim()))?.body.member("work")?.as_str()?.to_string();
    zikaron_seal::records::take(&crate::recordsx::read(&crate::home::machine_dir().ok()?).ok()?, &mine, &work).place
}

#[derive(Clone, Copy, Debug)]
pub enum Pick<'a> {
    No,
    Listed,
    At(&'a Path),
}

pub fn export(_pass: &crate::exitgate::Pass, home: &Home, id: &str, folder: &Path, original: Pick<'_>, to: Option<&[u8; 33]>) -> Result<Exported, Fault> {
    let pool = crate::badgex::pool(home)?;
    let chain = crate::badgex::chain_for(&pool, id)?;
    let mine: Vec<Vec<u8>> = home.ledger()?.pile()?.items;
    let ledger: Vec<Vec<u8>> = chain.iter().any(|h| mine.contains(h)).then(|| mine.clone()).unwrap_or_default();
    let work = chain.last().and_then(|h| zikaron_glue::book::check(h).ok()).and_then(|e| e.body.member("work").and_then(|v| v.as_str()).map(str::to_string)).unwrap_or_default();
    let settings = crate::settings::Settings::read(home)?;
    let last = crate::lastread::load_anchored(home)?.map(|a| zikaron_seal::lastaudit::Anchored { rows: a.rows, at: a.at, whole: a.whole });
    let upstream: Vec<Vec<Vec<u8>>> = kept(home)?.into_iter().map(|(_, o)| o.ledger).collect();
    let mut parts = zikaron_seal::terms::package_parts(
        &chain,
        &ledger,
        &mine,
        &upstream,
        crate::badgex::payload_text(&chain)?,
        |hid| match crate::termsx::record(home, hid)? {
            Some(r) => crate::termsx::doc_bytes(home, &r),
            None => Ok(None),
        },
        &settings.ground(),
        last.as_ref(),
    )?;
    let terms = parts.terms.len();
    let (taken, left_out) = match original {
        Pick::No => (Vec::new(), Vec::new()),
        Pick::At(p) => {
            let o = original_of(p, &work)?;
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            parts.original = Some(o);
            (vec![zikaron_glue::kitnames::kit_segment(&name).unwrap_or(name)], Vec::new())
        }
        Pick::Listed => {
            let t = zikaron_seal::records::take(&crate::recordsx::read(&crate::home::machine_dir()?)?, &mine, &work);
            let left_out = zikaron_glue::grantfile::left_out(&work, t.original.is_some(), t.left_out);
            parts.original = t.original;
            (t.taken, left_out)
        }
    };
    let delivered = parts.original.is_some();
    let (bytes, files, kit_id) = built(parts)?;
    let chosen = crate::home::choose(&crate::home::Kind::File { stem: stem_of(id, delivered, to.is_some()), ext: zikaron_glue::container::EXT.to_string() }, folder);
    if let Some(d) = chosen.at.parent() {
        std::fs::create_dir_all(d).map_err(|e| crate::fault::classify(&e, &d.display().to_string()))?;
    }
    match to {
        None => zikaron_glue::landing::land_bytes(&chosen.at, &bytes).map_err(Fault::of_landing)?,
        Some(k) => {
            crate::addressedx::seal_out(k, &chosen.at, |plain| zikaron_glue::landing::land_bytes(plain, &bytes).map_err(Fault::of_landing))?;
        }
    }
    Ok(Exported { path: chosen.at.clone(), chosen, hops: chain.len(), terms, ledger: !ledger.is_empty(), files, kit_id, original: delivered, taken, left_out, sealed: to.is_some() })
}

pub fn kept_path(home: &Home, kit_id: &str) -> Result<PathBuf, Fault> {
    let name = crate::names::key()?.name(crate::names::Logical::Kept(kit_id));
    Ok(home.dir(Slot::GrantsHeld).join(KEPT).join(format!("{name}.{}", zikaron_glue::container::EXT)))
}

pub fn keep(home: &Home, bytes: &[u8], kit_id: &str) -> Result<PathBuf, Fault> {
    let at = kept_path(home, kit_id)?;
    if at.exists() {
        return Ok(at);
    }
    if let Some(d) = at.parent() {
        std::fs::create_dir_all(d).map_err(|e| crate::fault::classify(&e, &d.display().to_string()))?;
    }
    crate::local::land(&at, crate::local::Doc::KeptGrant, bytes)?;
    Ok(at)
}

pub fn kept(home: &Home) -> Result<Vec<(PathBuf, Opened)>, Fault> {
    let dir = home.dir(Slot::GrantsHeld).join(KEPT);
    let listing = match std::fs::read_dir(&dir) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(crate::fault::classify(&e, &dir.display().to_string())),
    };
    let mut paths: Vec<PathBuf> = listing.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_file()).collect();
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        if let Some(bytes) = crate::local::read(&p, crate::local::Doc::KeptGrant)? {
            if let Ok(o) = open_bytes(&bytes) {
                out.push((p, o));
            }
        }
    }
    Ok(out)
}
