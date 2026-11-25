use crate::fault::{Fault, Known};
use crate::home::Home;
use std::collections::BTreeSet;
use std::path::Path;
use zikaron_glue::pack::{self, Bundle};
use zikaron_glue::select::{self, Selection};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pick {
    pub from: Option<u64>,
    pub to: Option<u64>,
    pub ids: Vec<String>,
}

impl Pick {
    pub fn selection(&self) -> Selection {
        Selection { from: self.from, to: self.to, work: None, ids: self.ids.clone() }
    }

    pub fn parse_ids(from: &str, to: &str, ids: &str) -> Result<Pick, Fault> {
        let mut pick = Pick::parse(from, to)?;
        for one in ids.split(|c: char| c.is_whitespace() || c == ',').filter(|x| !x.trim().is_empty()) {
            let id = one.trim().to_ascii_lowercase();
            if !zikaron::hexfmt::is_hex32(&id) {
                return Err(Fault::known(Known::ContentShape, id));
            }
            if !pick.ids.contains(&id) {
                pick.ids.push(id);
            }
        }
        Ok(pick)
    }

    pub fn parse(from: &str, to: &str) -> Result<Pick, Fault> {
        let num = |s: &str, what: &str| -> Result<Option<u64>, Fault> {
            let t = s.trim();
            if t.is_empty() {
                return Ok(None);
            }
            t.parse::<u64>().map(Some).map_err(|_| {
                Fault::known(Known::SettingsShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail155, &[&(what).to_string(), &format!("{:?}", t)])))
            })
        };
        Ok(Pick { from: num(from, crate::lang::t(crate::lang::Key::Tail178))?, to: num(to, crate::lang::t(crate::lang::Key::Tail179))?, ids: Vec::new() })
    }
}

pub struct Chosen {
    pub items: Vec<Vec<u8>>,
    pub pulled: Vec<String>,
    pub withdrawn: Vec<String>,
}

pub fn whole(home: &Home) -> Result<Vec<Vec<u8>>, Fault> {
    Ok(home.ledger()?.pile()?.items)
}

pub fn landing_stem(rows: &[crate::ledgerx::Row], pick: &Pick) -> String {
    let mut chosen: Vec<&crate::ledgerx::Row> = rows
        .iter()
        .filter(|r| {
            if !pick.ids.is_empty() {
                pick.ids.iter().any(|x| x.eq_ignore_ascii_case(&r.id))
            } else {
                pick.from.map(|f| r.seq >= f).unwrap_or(true) && pick.to.map(|t| r.seq <= t).unwrap_or(true)
            }
        })
        .collect();
    chosen.sort_by_key(|r| r.seq);
    match chosen.iter().find_map(|r| r.work.as_deref()) {
        Some(w) => format!("kit-{}", w.trim_start_matches("0x").chars().take(8).collect::<String>().to_ascii_lowercase()),
        None => "kit".to_string(),
    }
}

pub fn choose(home: &Home, pick: &Pick) -> Result<Chosen, Fault> {
    chosen_in(&whole(home)?, pick)
}

fn chosen_in(pile: &[Vec<u8>], pick: &Pick) -> Result<Chosen, Fault> {
    let got = select::choose(pile, &pick.selection());
    let chosen_ids: Vec<String> = got.items.iter().map(|b| zikaron::hexfmt::encode(&zikaron::entry::entry_id(b))).collect();
    if let Some(missing) = pick.ids.iter().find(|x| !chosen_ids.iter().any(|c| c.eq_ignore_ascii_case(x))) {
        return Err(Fault::known(Known::SubjectMissing, missing.clone()));
    }
    let gone = zikaron_seal::records::withdrawn(pile);
    let withdrawn = chosen_ids.into_iter().filter(|c| gone.iter().any(|g| g.eq_ignore_ascii_case(c))).collect();
    Ok(Chosen { items: got.items, pulled: got.pulled, withdrawn })
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Originals(BTreeSet<String>);

impl Originals {
    pub fn of(chosen: &Chosen) -> Originals {
        Originals(
            chosen
                .items
                .iter()
                .filter_map(|b| zikaron_glue::book::check(b).ok())
                .filter(|e| !chosen.withdrawn.iter().any(|w| w.eq_ignore_ascii_case(&e.id_hex())))
                .filter_map(|e| crate::ledgerx::work_of(e.kind, &e.body))
                .map(|c| c.to_ascii_lowercase())
                .collect(),
        )
    }

    pub fn admits(&self, digest: &[u8; 32]) -> bool {
        self.0.contains(&zikaron::hexfmt::encode(digest).to_ascii_lowercase())
    }

    pub fn admits_hex(&self, digest: &str) -> bool {
        self.0.contains(&digest.trim().to_ascii_lowercase())
    }

    pub fn admits_any(&self, digests: &[[u8; 32]]) -> bool {
        digests.iter().any(|d| self.admits(d))
    }

    pub fn admits_any_hex(&self, digests: &[String]) -> bool {
        digests.iter().any(|d| self.admits_hex(d))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub id: String,
    pub seq: u64,
    pub name: String,
    pub path: String,
    pub present: bool,
}

pub const ORIGINALS_LISTED: &str = "listed";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OriginalsWord {
    pub gather: bool,
    pub say_withdrawn: bool,
}

pub fn originals_word(word: &str) -> Option<OriginalsWord> {
    match word.trim() {
        "" => Some(OriginalsWord { gather: true, say_withdrawn: true }),
        "none" => Some(OriginalsWord { gather: false, say_withdrawn: false }),
        w if w == ORIGINALS_LISTED => Some(OriginalsWord { gather: false, say_withdrawn: true }),
        _ => None,
    }
}

pub fn originals(home: &Home, pick: &Pick) -> Result<Vec<Listed>, Fault> {
    originals_where(home, pick, false)
}

pub fn withdrawn_originals(home: &Home, pick: &Pick) -> Result<Vec<Listed>, Fault> {
    originals_where(home, pick, true)
}

fn originals_where(home: &Home, pick: &Pick, gone: bool) -> Result<Vec<Listed>, Fault> {
    let chosen = choose(home, pick)?;
    let ids: BTreeSet<String> = chosen
        .items
        .iter()
        .filter_map(|b| zikaron_glue::book::check(b).ok())
        .filter(|e| e.kind == zikaron_glue::book::Kind::History)
        .filter(|e| chosen.withdrawn.iter().any(|w| w.eq_ignore_ascii_case(&e.id_hex())) == gone)
        .map(|e| e.id_hex().to_ascii_lowercase())
        .collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let root = crate::ledgerx::root_of(home)?;
    let rows = crate::recordsx::read(&crate::home::machine_dir()?)?;
    let ids: Vec<String> = ids.into_iter().collect();
    Ok(zikaron_seal::records::originals_for(&rows, &root, &ids)
        .into_iter()
        .map(|r| Listed { present: Path::new(&r.path).is_file(), id: r.id, seq: r.seq, name: r.name, path: r.path })
        .collect())
}

pub fn preview(home: &Home, pick: &Pick) -> Result<(Vec<String>, Vec<String>), Fault> {
    let got = choose(home, pick)?;
    let ids = got
        .items
        .iter()
        .filter_map(|b| zikaron_glue::book::check(b).ok())
        .map(|e| zikaron::hexfmt::encode(&e.id))
        .collect();
    Ok((ids, got.pulled))
}

pub use zikaron_glue::kitnames::{kit_rel, kit_segment, DIGEST_HEX, NAMES_FILE};

pub fn preview_names(path: &Path) -> Result<Vec<(String, Option<String>)>, Fault> {
    let name = path
        .file_name()
        .map(|x| x.to_string_lossy().to_string())
        .ok_or_else(|| Fault::known(Known::FileMissing, path.display().to_string()))?;
    let md = std::fs::symlink_metadata(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    if md.is_dir() {
        let mut got: Vec<String> = Vec::new();
        walk_names(path, &name, &mut got)?;
        Ok(got.into_iter().map(|p| (p.clone(), kit_rel(&p))).collect())
    } else {
        Ok(vec![(name.clone(), kit_segment(&name))])
    }
}

fn walk_names(root: &Path, prefix: &str, out: &mut Vec<String>) -> Result<(), Fault> {
    let bad = |rel: &str| Fault::of_landing(zikaron_glue::pack::Trouble::BadPath(rel.to_string()));
    let io = |rel: &str, at: &Path, what: &str, e: std::io::Error| Fault::of_landing(zikaron_glue::pack::Trouble::Io(rel.to_string(), format!("{what} {}: {e}", at.display())));
    let listing = std::fs::read_dir(root).map_err(|e| io(prefix, root, "read directory", e))?;
    let mut names: Vec<std::ffi::OsString> = Vec::new();
    for item in listing {
        let e = item.map_err(|e| io(prefix, root, "read directory", e))?;
        names.push(e.file_name());
    }
    names.sort_by(|a, b| a.as_encoded_bytes().cmp(b.as_encoded_bytes()));
    for name in names {
        let Some(name_str) = name.to_str() else {
            return Err(bad(&root.join(&name).to_string_lossy()));
        };
        let rel = if prefix.is_empty() { name_str.to_string() } else { format!("{prefix}/{name_str}") };
        let at = root.join(&name);
        let md = std::fs::symlink_metadata(&at).map_err(|e| io(&rel, &at, "read metadata", e))?;
        if md.file_type().is_symlink() || (!md.is_dir() && !md.is_file()) {
            return Err(bad(&rel));
        }
        if md.is_dir() {
            walk_names(&at, &rel, out)?;
        } else {
            out.push(rel);
        }
    }
    Ok(())
}

pub fn digest_of(path: &Path) -> Result<Vec<[u8; 32]>, Fault> {
    let md = std::fs::symlink_metadata(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    if md.is_file() {
        let bytes = std::fs::read(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
        Ok(vec![crate::anchorx::file_digest(&bytes)])
    } else if md.is_dir() {
        crate::anchorx::dir_digests(path)
    } else {
        Err(Fault::known(Known::NotAdoptable, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail027, &[&(path.display()).to_string()]))))
    }
}

pub fn attach(path: &Path, originals: &Originals, into: &mut Bundle) -> Result<usize, Fault> {
    let name = path
        .file_name()
        .map(|x| x.to_string_lossy().to_string())
        .ok_or_else(|| Fault::known(Known::FileMissing, path.display().to_string()))?;
    let md = std::fs::symlink_metadata(path)
        .map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    let before = into.files.len();
    let stranger = || Fault::known(Known::NotAnOriginal, path.display().to_string());
    if md.is_file() {
        let bytes = std::fs::read(path)
            .map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
        if !originals.admits(&crate::anchorx::file_digest(&bytes)) {
            return Err(stranger());
        }
        zikaron_glue::kitnames::put_original(&name, bytes, into).map_err(|n| Fault::of_landing(zikaron_glue::pack::Trouble::BadPath(n)))?;
    } else if md.is_dir() {
        if !originals.admits_any(&crate::anchorx::dir_digests(path)?) {
            return Err(stranger());
        }
        let mut got: Vec<(String, Vec<u8>)> = Vec::new();
        pack::gather(path, "", &mut got)
            .map_err(|t| Fault::of_landing(t))?;
        zikaron_glue::kitnames::put_directory(&name, got, into).map_err(|n| Fault::of_landing(zikaron_glue::pack::Trouble::BadPath(n)))?;
    } else {
        return Err(Fault::known(
            Known::NotAdoptable,
            crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail027, &[&(path.display()).to_string()])),
        ));
    }
    Ok(into.files.len() - before)
}

pub fn latest_kit(kits: &Path) -> Option<std::path::PathBuf> {
    let listing = std::fs::read_dir(kits).ok()?;
    listing
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join(zikaron_glue::names::MANIFEST).is_file())
        .filter_map(|p| std::fs::metadata(p.join(zikaron_glue::names::MANIFEST)).and_then(|m| m.modified()).ok().map(|t| (t, p)))
        .max_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, p)| p)
}

pub fn anchored_on(home: &Home) -> Result<Option<crate::kitsindex::AnchoredOn>, Fault> {
    let s = crate::settings::Settings::read(home)?;
    Ok(match (s.chain_id, s.registry) {
        (Some(chain_id), Some(r)) => Some(crate::kitsindex::AnchoredOn { chain_id, from_block: s.from_block, registry: r.hex() }),
        _ => None,
    })
}

pub fn kit_anchors(home: &Home) -> Result<Vec<zikaron_glue::pack::Anchor>, Fault> {
    Ok(crate::lastread::load_anchored(home)?
        .map(|a| a.rows.into_iter().map(|(entry_id, (chain_id, tx))| zikaron_glue::pack::Anchor { entry_id, chain_id, tx }).collect())
        .unwrap_or_default())
}

pub struct Made {
    pub path: String,
    pub kit_id: String,
    pub entries: usize,
    pub files: usize,
    pub pulled: Vec<String>,
    pub dropped: Vec<String>,
    pub originals: Vec<String>,
    pub facts: crate::kitsindex::RowFacts,
    pub companion: Option<String>,
}

pub fn export(
    _pass: &crate::exitgate::Pass,
    home: &Home,
    pick: &Pick,
    attachments: &[String],
    note: &str,
    out: &Path,
    form: zikaron_glue::pack::Form,
) -> Result<Made, Fault> {
    export_to(_pass, home, pick, attachments, note, out, form, None, false)
}

#[allow(clippy::too_many_arguments)]
pub fn export_to(
    _pass: &crate::exitgate::Pass,
    home: &Home,
    pick: &Pick,
    attachments: &[String],
    note: &str,
    out: &Path,
    form: zikaron_glue::pack::Form,
    to: Option<&[u8; 33]>,
    companion: bool,
) -> Result<Made, Fault> {
    if to.is_some() && form == zikaron_glue::pack::Form::Folder {
        return Err(Fault::known(Known::AddressedFolder, out.display().to_string()));
    }
    crate::trace::mark(crate::feature::Feature::W5);
    let pile = whole(home)?;
    let chosen = chosen_in(&pile, pick)?;
    if chosen.items.is_empty() {
        return Err(Fault::known(
            Known::QueueEmpty,
            crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail180).to_string()),
        ));
    }
    let originals = Originals::of(&chosen);
    let mut b = Bundle {
        entries: chosen.items.clone(),
        note: note.to_string(),
        root: crate::auditx::root_of(&pile).ok(),
        anchored_on: anchored_on(home)?,
        anchors: kit_anchors(home)?,
        ..Default::default()
    };
    let mut attached: Vec<String> = Vec::new();
    for one in attachments {
        let t = one.trim();
        if t.is_empty() {
            continue;
        }
        let before = b.files.len();
        attach(Path::new(t), &originals, &mut b)?;
        attached.extend(b.files[before..].iter().map(|(p, _)| p.clone()));
    }
    for bytes in &chosen.items {
        let Ok(e) = zikaron_glue::book::check(bytes) else { continue };
        if e.kind != zikaron_glue::book::Kind::Grant {
            continue;
        }
        let found = match crate::termsx::record(home, &e.id_hex())? {
            Some(r) => crate::termsx::doc_bytes(home, &r)?,
            None => None,
        };
        if let Some((rel, doc)) = found {
            if !b.files.iter().any(|(p, _)| *p == rel) {
                b.files.push((rel, doc));
            }
        }
    }
    let old = if companion { pack::companion_v1(&b) } else { None };
    let Some(to) = to else {
        let landed = pack::export_as(out, b, form).map_err(|t| {
            Fault::of_landing(t)
        })?;
        let facts = crate::kitsindex::RowFacts::of_kit(out)?;
        let companion = match old {
            Some(c) => Some(write_companion(out, &c, None)?),
            None => None,
        };
        return Ok(Made {
            path: out.display().to_string(),
            kit_id: landed.kit_id,
            entries: landed.entries,
            files: landed.files,
            pulled: chosen.pulled,
            dropped: landed.dropped,
            originals: attached,
            facts,
            companion,
        });
    };
    let pulled = chosen.pulled.clone();
    let ((landed, mut facts), one_time) = crate::addressedx::seal_out(to, out, |plain| {
        let landed = pack::export_as(plain, b, zikaron_glue::pack::Form::File).map_err(Fault::of_landing)?;
        let facts = crate::kitsindex::RowFacts::of_kit(plain)?;
        crate::task::stage_at(crate::task::Kind::Kit, 4);
        Ok((landed, facts))
    })?;
    facts.one_time = Some(zikaron::hexfmt::encode(&one_time));
    let companion = match old {
        Some(c) => Some(write_companion(out, &c, Some(to))?),
        None => None,
    };
    Ok(Made {
        path: out.display().to_string(),
        kit_id: landed.kit_id,
        entries: landed.entries,
        files: landed.files,
        pulled,
        dropped: landed.dropped,
        originals: attached,
        facts,
        companion,
    })
}

pub fn companion_path(out: &Path) -> std::path::PathBuf {
    let stem = out.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    out.with_file_name(format!("{stem}-kit1.{}", zikaron_glue::container::EXT_KIT))
}

fn write_companion(out: &Path, c: &Bundle, to: Option<&[u8; 33]>) -> Result<String, Fault> {
    let at = companion_path(out);
    let bytes = pack::companion_bytes(c).map_err(Fault::of_landing)?;
    match to {
        None => {
            zikaron_glue::landing::land_bytes(&at, &bytes).map_err(Fault::of_landing)?;
            crate::exports::record_plain(&at, crate::exports::Kind::Kit)?;
        }
        Some(k) => {
            crate::addressedx::seal_out(k, &at, |plain| zikaron_glue::landing::land_bytes(plain, &bytes).map_err(Fault::of_landing))?;
        }
    }
    Ok(at.display().to_string())
}


#[cfg(test)]
mod originals_word_tests {
    use super::*;

    #[test]
    fn the_originals_word_says_what_is_gathered_and_what_is_said() {
        assert_eq!(originals_word(""), Some(OriginalsWord { gather: true, say_withdrawn: true }));
        assert_eq!(originals_word("  "), originals_word(""), "blank is none given");
        assert_eq!(originals_word(ORIGINALS_LISTED), Some(OriginalsWord { gather: false, say_withdrawn: true }), "the export page's");
        assert_eq!(originals_word("none"), Some(OriginalsWord { gather: false, say_withdrawn: false }), "the command line's and the door's: none looked for, none said");
        assert_eq!(originals_word(" none "), originals_word("none"));
        for w in ["None", "LISTED", "all", "none listed", "listed,none"] {
            assert_eq!(originals_word(w), None, "{w}");
        }
    }
}

#[cfg(test)]
mod stem_tests {
    use super::*;

    fn row(seq: u64, id: &str, work: Option<&str>) -> crate::ledgerx::Row {
        crate::ledgerx::Row {
            law: zikaron_glue::book::Law::Two,
            seq,
            kind: zikaron_glue::book::Kind::History,
            id: id.to_string(),
            prev: None,
            author: String::new(),
            summary: String::new(),
            lamp: crate::ledgerx::Lamp::Landed,
            tx: None,
            bytes: 0,
            work: work.map(str::to_string),
            facts: Default::default(),
            anchored_at: None,
        }
    }

    #[test]
    fn the_kit_folder_is_named_after_the_first_chosen_record() {
        let rows = vec![row(0, "0x00", None), row(1, "0x01", Some("0xAABBCCDD11223344")), row(2, "0x02", Some("0x5566778899aabbcc"))];
        assert_eq!(landing_stem(&rows, &Pick::default()), "kit-aabbccdd", "全本即首条带摘要的那一条");
        assert_eq!(landing_stem(&rows, &Pick { from: Some(2), to: Some(2), ids: vec![] }), "kit-55667788", "区间照区间");
        assert_eq!(landing_stem(&rows, &Pick { from: None, to: None, ids: vec!["0x02".into(), "0x01".into()] }), "kit-aabbccdd", "点名照点名,取序号最小那一条");
        assert_eq!(landing_stem(&rows, &Pick { from: Some(0), to: Some(0), ids: vec![] }), "kit", "所选里一条带摘要的也没有");
    }
}

#[cfg(test)]
mod names_tests {
    use super::*;

    #[test]
    fn names_are_transliterated_in_one_place() {
        assert_eq!(kit_segment("notes.txt").as_deref(), Some("notes.txt"), "已合法即原样");
        let parts = |seg: &str, ext: &str| -> (String, usize) {
            let stem = seg.strip_suffix(ext).unwrap_or(seg);
            let is_digest = |p: &&str| p.len() == DIGEST_HEX && p.chars().all(|c| c.is_ascii_hexdigit());
            let words: Vec<&str> = stem.split('-').filter(|p| !is_digest(p)).collect();
            (words.join("-"), stem.split('-').filter(is_digest).count())
        };
        let zh = kit_segment("雅煞珥书1至91章简体版.pdf").expect("转得出");
        assert!(zh.ends_with(".pdf") && parts(&zh, ".pdf") == ("1-91".to_string(), 1) && zh.len() == "1-91-".len() + DIGEST_HEX + ".pdf".len(), "{zh}");
        let sp = kit_segment("Race and Reunion.pdf").expect("转得出");
        assert!(sp.ends_with(".pdf") && parts(&sp, ".pdf") == ("race-and-reunion".to_string(), 1), "{sp}");
        let only = kit_segment("雅煞珥书").expect("转得出");
        assert_eq!(only.len(), DIGEST_HEX, "全是集外字即只剩摘要");
        assert_ne!(kit_segment("A b.txt"), kit_segment("A  b.txt"), "摘要保唯一");
        assert_eq!(kit_segment(&"长".repeat(200)), kit_segment(&"长".repeat(200)));
        assert!(kit_segment(&format!("{}.txt", "A".repeat(250))).is_none(), "转写加摘要之后过长,转不了");
        assert_eq!(kit_rel("书/第一章.txt").map(|x| x.split('/').count()), Some(2), "目录逐段");
        for n in ["雅煞珥书1至91章简体版.pdf", "Race and Reunion.pdf", "-lead.txt", "..", "a/b"] {
            if let Some(x) = kit_rel(n) {
                assert!(zikaron_kit::kitdir::is_kit_path(&x), "{n} → {x}");
            }
        }
    }
}

pub fn unanchored(picked: &[&crate::ledgerx::Row]) -> (Vec<(String, crate::ledgerx::Lamp)>, Vec<(String, bool)>) {
    use crate::ledgerx::Lamp;
    let red: Vec<(String, Lamp)> = picked
        .iter()
        .filter(|r| !r.lamp.confirmed() && !matches!(r.lamp, Lamp::Deleted | Lamp::LocalDeletion))
        .map(|r| (r.id.clone(), r.lamp))
        .collect();
    let send: Vec<(String, bool)> = red
        .iter()
        .filter(|(_, l)| matches!(l, Lamp::Landed | Lamp::Queued | Lamp::Refused | Lamp::Reverted))
        .map(|(id, l)| (id.clone(), *l == Lamp::Landed))
        .collect();
    (red, send)
}
