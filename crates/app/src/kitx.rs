//! The assembly half of the disclosure kit maker. Not one byte of kit shape is invented here.
//!
//! ─── This layer does three parameter jobs ───
//!
//! 1. Lay out "which entries" as the glue crate's [`select::Selection`] (a range cell and a named-ids cell).
//! The selection decides which entries go into the kit and which records' originals may be attached.
//! 2. Read "what to attach" into the glue crate's `files` table (through its `gather`, which does not follow
//! symbolic links); the entry first passes [`Originals`]: a digest that is not a selected record's `content`
//! is refused by name.
//! 3. Hand both to the glue crate's [`pack::export`].
//!
//! ─── No kit when self-verification fails ───
//!
//! That rule is not in this layer: the glue crate first lays the kit out in a temporary place under the same
//! parent directory, and renames the whole thing into place only after the kit crate's `verify_kit` judges
//! `KIT_OK`; on failure the temporary place is cleared at once, and not one byte of the caller's path
//! changes. So "write first, check later" has no place on this path, and its owners are the glue and kit
//! crates, not this layer (reuse is mandatory).
//!
//! ─── Revocation closure ───
//!
//! If the chosen entries include a grant, the glue crate also pulls in the revocations that reference it and
//! reports each one pulled in. The face says so plainly: what else went into a kit must be visible to the
//! person.

use crate::fault::{Fault, Known};
use crate::home::Home;
use std::collections::BTreeSet;
use std::path::Path;
use zikaron_glue::pack::{self, Bundle};
use zikaron_glue::select::{self, Selection};

/// The selector's two cells (entry range). There is no record cell: two filters ANDed together often came up
/// empty without the person knowing why; the engine's `Selection.work` member stays, used by the command
/// line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pick {
    /// Lower seq bound (inclusive).
    pub from: Option<u64>,
    /// Upper seq bound (inclusive).
    pub to: Option<u64>,
    /// The named entries (ids, `0x` plus sixty-four digits; any subset toggled one by one). When non-empty
    /// only these are kept; when empty the range applies. Named entries also go through revocation closure
    /// (closure lives only in the glue crate's selector).
    pub ids: Vec<String>,
}

impl Pick {
    /// Lay out in the glue crate's form. Neither bound given means everything (the glue crate's reading; not
    /// redefined here).
    pub fn selection(&self) -> Selection {
        Selection { from: self.from, to: self.to, work: None, ids: self.ids.clone() }
    }

    /// Recognize from the face's cells. Empty bounds mean "no limit"; an unrecognized number is refused by
    /// name, never quietly read as no limit. `ids` has one per line (or comma separated); one that is not an
    /// id is refused by name.
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

    /// Recognize only the two bounds (the named cell empty).
    pub fn parse(from: &str, to: &str) -> Result<Pick, Fault> {
        let num = |s: &str, what: &str| -> Result<Option<u64>, Fault> {
            let t = s.trim();
            if t.is_empty() {
                return Ok(None);
            }
            t.parse::<u64>().map(Some).map_err(|_| {
                Fault::known(Known::SettingsShape, crate::lang::filln(crate::lang::Key::Tail155, &[&(what).to_string(), &format!("{:?}", t)]))
            })
        };
        Ok(Pick { from: num(from, crate::lang::t(crate::lang::Key::Tail178))?, to: num(to, crate::lang::t(crate::lang::Key::Tail179))?, ids: Vec::new() })
    }
}

/// One selection's reading: how many were chosen and which were pulled in by closure.
pub struct Chosen {
    pub items: Vec<Vec<u8>>,
    pub pulled: Vec<String>,
}

/// The whole ledger pile, through the store crate's strict read (the home's entries are sealed: opened through
/// `local::Ledger`). Choosing and exporting read the same place.
fn whole(home: &Home) -> Result<Vec<Vec<u8>>, Fault> {
    Ok(home.ledger()?.pile()?.items)
}

/// Record bundle directory name (one name, one home): `kit-<first eight digits of the first selected record's
/// original digest>`. The first is the selected entry with the smallest seq that has an original digest
/// (`work`); named selection follows the names, a range follows the range, neither means the whole ledger.
/// With no entry carrying a digest (only grants selected) it stays `kit`. Name collisions are numbered by the
/// landing place (`home::choose`). Reads the ledger rows in memory, not the disk (the window frame calls it).
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

/// What a pick takes from this home's ledger, as [`export`] will take it: the front room of the export mouth,
/// reading the same judgement ([`chosen_in`]).
pub fn choose(home: &Home, pick: &Pick) -> Result<Chosen, Fault> {
    chosen_in(&whole(home)?, pick)
}

/// The selection's one judgement: the glue crate chooses, and every entry named by id must be among what it
/// chose. [`export`] and [`choose`] both read it.
fn chosen_in(pile: &[Vec<u8>], pick: &Pick) -> Result<Chosen, Fault> {
    let got = select::choose(pile, &pick.selection());
    // Every entry named by id is one this ledger holds: judged on what the selection actually chose, so a
    // named entry it did not find is refused by name, never left out of the package without a word.
    let chosen_ids: Vec<String> = got.items.iter().map(|b| zikaron::hexfmt::encode(&zikaron::entry::entry_id(b))).collect();
    if let Some(missing) = pick.ids.iter().find(|x| !chosen_ids.iter().any(|c| c.eq_ignore_ascii_case(x))) {
        return Err(Fault::known(Known::SubjectMissing, missing.clone()));
    }
    Ok(Chosen { items: got.items, pulled: got.pulled })
}

/// Which originals the selected records accept: the `content` (hex32, lowercase) of every history in the
/// selected entries.
///
/// Whether an attachment is valid asks only this set: a file by the sha256 of its bytes
/// (`anchorx::file_digest`), a directory by its manifest digest (`anchorx::of_dir`), both the same code that
/// computes `content` at signing. File names, paths and suffixes count for nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Originals(BTreeSet<String>);

impl Originals {
    pub fn of(chosen: &Chosen) -> Originals {
        Originals(
            chosen
                .items
                .iter()
                .filter_map(|b| zikaron::entry::check(b).ok())
                .filter_map(|e| crate::ledgerx::work_of(e.kind, &e.body))
                .map(|c| c.to_ascii_lowercase())
                .collect(),
        )
    }

    pub fn admits(&self, digest: &[u8; 32]) -> bool {
        self.0.contains(&zikaron::hexfmt::encode(digest).to_ascii_lowercase())
    }

    /// As [`Originals::admits`], with the digest as a hex32 string (the form computed in the background). A
    /// malformed one is not accepted.
    pub fn admits_hex(&self, digest: &str) -> bool {
        self.0.contains(&digest.trim().to_ascii_lowercase())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// One original of a selected record in the local index (the attachment area lists them for ticking).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub id: String,
    pub seq: u64,
    pub name: String,
    /// The absolute path at signing time.
    pub path: String,
    /// Whether a file is at that path now (only presence is checked; the digest is computed by the export
    /// gate).
    pub present: bool,
}

/// The originals of the selected records, listed from the local index (`records/index.json`). Only entries of
/// this ledger (same root) within the selection are listed; those the index lacks (a directory or repository
/// was signed, or signed on another machine) are not listed and can be dragged in by the person, still going
/// through [`Originals`]. Sorted by seq.
pub fn originals(home: &Home, pick: &Pick) -> Result<Vec<Listed>, Fault> {
    let chosen = choose(home, pick)?;
    let ids: BTreeSet<String> = chosen
        .items
        .iter()
        .filter_map(|b| zikaron::entry::check(b).ok())
        .filter(|e| e.kind == zikaron::tokens::EntryType::History)
        .map(|e| e.id_hex().to_ascii_lowercase())
        .collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let root = crate::ledgerx::root_of(home)?;
    let rows = crate::recordsx::read(&crate::home::machine_dir()?)?;
    let mut out: Vec<Listed> = rows
        .into_iter()
        .filter(|r| r.root.eq_ignore_ascii_case(&root) && ids.contains(&r.id.to_ascii_lowercase()))
        .map(|r| Listed { present: Path::new(&r.path).is_file(), id: r.id, seq: r.seq, name: r.name, path: r.path })
        .collect();
    out.sort_by(|a, b| a.seq.cmp(&b.seq).then(a.path.cmp(&b.path)));
    out.dedup_by(|a, b| a.id == b.id && a.path == b.path);
    Ok(out)
}

/// Preview. The same selection as export (the glue crate's `select::choose`): ids of the chosen entries and
/// those pulled in by closure. The face's list comes only from here and is not re-judged in the interface
/// layer.
pub fn preview(home: &Home, pick: &Pick) -> Result<(Vec<String>, Vec<String>), Fault> {
    let got = choose(home, pick)?;
    let ids = got
        .items
        .iter()
        .filter_map(|b| zikaron::entry::check(b).ok())
        .map(|e| zikaron::hexfmt::encode(&e.id))
        .collect();
    Ok((ids, got.pulled))
}

// ───────────────────────── Names inside the kit ─────────────────────────

/// The table of original names to kit names, stored in the kit (under `files/`). It is itself a file in the
/// manifest. One name, one home.
pub const NAMES_FILE: &str = "zikaron-names.json";

/// How many hex digits of the original name's digest are appended when transliterating.
pub const DIGEST_HEX: usize = 8;

/// One name segment → a valid kit name segment. The rule is the kit crate's `is_kit_path` (kit law §7.2:
/// lowercase letters, digits, `.` `_` `-`, not starting with `-`, at most 255 bytes per segment); this layer
/// writes no second copy. Already valid stays unchanged; otherwise transliterate: lowercase, replace each run
/// of characters outside the set with one `-`, append the first eight digits of the sha256 of the original
/// name (UTF-8 bytes) to keep it unique, and keep the extension (when valid after lowercasing). Still invalid
/// after transliteration (too long, for example) gives `None`: it really cannot be converted, the face marks
/// it red and the export button is disabled.
pub fn kit_segment(orig: &str) -> Option<String> {
    if zikaron_kit::kitdir::is_kit_path(orig) && !orig.contains('/') {
        return Some(orig.to_string());
    }
    let squash = |x: &str| -> String {
        let mut out = String::new();
        let mut dash = false;
        for c in x.chars().flat_map(|c| c.to_lowercase()) {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.' {
                out.push(c);
                dash = false;
            } else if !dash {
                out.push('-');
                dash = true;
            }
        }
        out.trim_matches(|c| c == '-' || c == '.').to_string()
    };
    let (stem, ext) = match orig.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() && !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()) => (s, Some(e.to_ascii_lowercase())),
        _ => (orig, None),
    };
    let digest = zikaron::hexfmt::encode(&zikaron::cryptox::sha256(orig.as_bytes()));
    let tag = &digest.trim_start_matches("0x")[..DIGEST_HEX];
    let body = squash(stem);
    let name = match (body.is_empty(), ext) {
        (true, Some(e)) => format!("{tag}.{e}"),
        (true, None) => tag.to_string(),
        (false, Some(e)) => format!("{body}-{tag}.{e}"),
        (false, None) => format!("{body}-{tag}"),
    };
    if zikaron_kit::kitdir::is_kit_path(&name) { Some(name) } else { None }
}

/// Transliterate a relative path segment by segment (directory attachments alike). The whole path must still
/// pass `is_kit_path` (the 1024 total length rule is checked here).
pub fn kit_rel(orig: &str) -> Option<String> {
    let segs: Option<Vec<String>> = orig.split('/').map(kit_segment).collect();
    let rel = segs?.join("/");
    if zikaron_kit::kitdir::is_kit_path(&rel) { Some(rel) } else { None }
}

/// What an attachment will be called in the kit: per item (per file for a directory) the original name and
/// kit name; the kit name is `None` for items that cannot be converted. The attachment preview and export
/// read the same place (what the preview shows is what lands in the kit).
pub fn preview_names(path: &Path) -> Result<Vec<(String, Option<String>)>, Fault> {
    let name = path
        .file_name()
        .map(|x| x.to_string_lossy().to_string())
        .ok_or_else(|| Fault::known(Known::FileMissing, path.display().to_string()))?;
    let md = std::fs::symlink_metadata(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    if md.is_dir() {
        // The preview needs names only and reads no content. Walking through the glue crate's `gather` would
        // read every file's bytes into memory only to drop them here: an attachment directory of tens of GB
        // would stall the frame and grow memory. This only lists names (order, symbolic links and irregular
        // files judged as `gather` does); real export still goes through `gather`.
        let mut got: Vec<String> = Vec::new();
        walk_names(path, &name, &mut got)?;
        Ok(got.into_iter().map(|p| (p.clone(), kit_rel(&p))).collect())
    } else {
        Ok(vec![(name.clone(), kit_segment(&name))])
    }
}

/// A pass that lists names only (the same rules as `pack::gather`: names in byte order, symbolic links and
/// irregular files are `E_BAD_PATH`, directories are descended), reading not one byte of content. Used by the
/// attachment preview; the writing pass still goes through `gather` (the owner).
fn walk_names(root: &Path, prefix: &str, out: &mut Vec<String>) -> Result<(), Fault> {
    let bad = |rel: &str| Fault::of_landing(zikaron_glue::pack::Trouble::BadPath(rel.to_string()));
    // Read and write errors take the export pass's form (code and subject as in `gather`): preview and real
    // export say the same sentence.
    let io = |rel: &str| Fault::of_landing(zikaron_glue::pack::Trouble::Io(rel.to_string()));
    let listing = std::fs::read_dir(root).map_err(|_| io(prefix))?;
    let mut names: Vec<std::ffi::OsString> = Vec::new();
    for item in listing {
        let e = item.map_err(|_| io(prefix))?;
        names.push(e.file_name());
    }
    names.sort_by(|a, b| a.as_encoded_bytes().cmp(b.as_encoded_bytes()));
    for name in names {
        let Some(name_str) = name.to_str() else {
            return Err(bad(&root.join(&name).to_string_lossy()));
        };
        let rel = if prefix.is_empty() { name_str.to_string() } else { format!("{prefix}/{name_str}") };
        let at = root.join(&name);
        let md = std::fs::symlink_metadata(&at).map_err(|_| io(&rel))?;
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

/// The digest of an attachment path: a file by its bytes ([`crate::anchorx::file_digest`]), a directory by
/// its manifest digest ([`crate::anchorx::of_dir`]), the same as the [`attach`] gate and as `content` at
/// signing; symbolic links and device files are refused by name. The export page asks it in the background
/// when something is dragged in (`Action::VetAttachments`), so the row can say "not an original of the
/// selected records" early; the export gate still applies.
pub fn digest_of(path: &Path) -> Result<[u8; 32], Fault> {
    let md = std::fs::symlink_metadata(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    if md.is_file() {
        let bytes = std::fs::read(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
        Ok(crate::anchorx::file_digest(&bytes))
    } else if md.is_dir() {
        Ok(crate::anchorx::of_dir(path)?.digest)
    } else {
        Err(Fault::known(Known::NotAdoptable, crate::lang::filln(crate::lang::Key::Tail027, &[&(path.display()).to_string()])))
    }
}

/// Attach. One path gives one thing: a file attaches that file; a directory attaches the whole tree (through
/// the glue crate's `gather`).
///
/// The entry first asks [`Originals`]: a file's sha256 or a directory's manifest digest that is not among the
/// selected records' `content` is refused by name (`NOT_AN_ORIGINAL`, subject the path), and nothing is
/// attached. The interface and the command line pass the same gate.
///
/// Kit names go through [`kit_rel`] item by item (the only transliteration); renamed items are recorded in
/// `names` (original name, kit name) and written as [`NAMES_FILE`] at export. An item that cannot be
/// converted is refused by name (`LANDING` · `E_BAD_PATH`), and nothing is attached.
pub fn attach(path: &Path, originals: &Originals, into: &mut Bundle, names: &mut Vec<(String, String)>) -> Result<usize, Fault> {
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
        let inside = kit_rel(&name).ok_or_else(|| Fault::of_landing(zikaron_glue::pack::Trouble::BadPath(name.to_string())))?;
        if inside != name {
            names.push((name.clone(), inside.clone()));
        }
        into.files.push((inside.clone(), bytes));
        into.contents.push(inside);
    } else if md.is_dir() {
        if !originals.admits(&crate::anchorx::of_dir(path)?.digest) {
            return Err(stranger());
        }
        let mut got: Vec<(String, Vec<u8>)> = Vec::new();
        pack::gather(path, &name, &mut got)
            .map_err(|t| Fault::of_landing(t))?;
        let mut mapped: Vec<(String, String, Vec<u8>)> = Vec::new();
        for (p, b) in got {
            let inside = kit_rel(&p).ok_or_else(|| Fault::of_landing(zikaron_glue::pack::Trouble::BadPath(p.to_string())))?;
            mapped.push((p, inside, b));
        }
        for (p, inside, b) in mapped {
            if inside != p {
                names.push((p, inside.clone()));
            }
            into.contents.push(inside.clone());
            into.files.push((inside, b));
        }
    } else {
        // Other shapes (symbolic links, device files) are not attached: attaching them would pull things
        // outside the kit into it.
        return Err(Fault::known(
            Known::NotAdoptable,
            crate::lang::filln(crate::lang::Key::Tail027, &[&(path.display()).to_string()]),
        ));
    }
    Ok(into.files.len() - before)
}

/// The name table's member names. One name, one home.
pub mod names_member {
    pub const NAMES: &str = "names";
    pub const FROM: &str = "from";
    pub const TO: &str = "to";
}

/// The table of original names to kit names: canonical JSON `{"names":[{"from":original,"to":kit name},…]}`,
/// sorted by kit name.
pub fn names_table(names: &[(String, String)]) -> Vec<u8> {
    use zikaron::json::Value;
    let mut rows: Vec<&(String, String)> = names.iter().collect();
    rows.sort_by(|a, b| a.1.cmp(&b.1));
    let arr = rows
        .into_iter()
        .map(|(from, to)| Value::Obj(vec![(names_member::FROM.to_string(), Value::Str(from.clone())), (names_member::TO.to_string(), Value::Str(to.clone()))]))
        .collect();
    zikaron::json::canon_bytes(&Value::Obj(vec![(names_member::NAMES.to_string(), Value::Arr(arr))]))
}

/// The most recent record bundle in the home's kits room (the directory with a manifest and the latest
/// modification time). "Check publication" uses it when no local kit was chosen.
pub fn latest_kit(kits: &Path) -> Option<std::path::PathBuf> {
    let listing = std::fs::read_dir(kits).ok()?;
    listing
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join(zikaron_glue::names::MANIFEST).is_file())
        .filter_map(|p| std::fs::metadata(p.join(zikaron_glue::names::MANIFEST)).and_then(|m| m.modified()).ok().map(|t| (t, p)))
        .max_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, p)| p)
}

/// This home's anchoring point: its basis (chain id, registry, start block) as the settings hold it now; none
/// when the chain or the registry is not configured.
pub fn anchored_on(home: &Home) -> Result<Option<crate::kitsindex::AnchoredOn>, Fault> {
    let s = crate::settings::Settings::read(home)?;
    Ok(match (s.chain_id, s.registry) {
        (Some(chain_id), Some(r)) => Some(crate::kitsindex::AnchoredOn { chain_id, from_block: s.from_block, registry: r.hex() }),
        _ => None,
    })
}

/// One export's reading.
pub struct Made {
    pub path: String,
    pub kit_id: String,
    pub entries: usize,
    pub files: usize,
    pub pulled: Vec<String>,
    /// Platform junk dropped before export, each named.
    pub dropped: Vec<String>,
}

/// Export. Choose, attach, and hand to the glue crate to write.
///
/// The kit holds only the chosen entries: a selection choosing zero entries is refused by name (nothing is
/// written); otherwise the kit holds the chosen entries plus the revocations pulled in by closure (the glue
/// crate's `select::choose`). The selection also decides [`Originals`] (which records' originals may be
/// attached) and the terms documents that travel with the kit. The verifier sees only these entries.
///
/// Self-verification is carried by the glue and kit crates (see the file header); this layer only lines the
/// three things up and brings the glue crate's refusal back unchanged: "something is already at that path",
/// "two files with one path in the kit" and "self-verification failed" each have their own name, never merged
/// into "cannot export".
/// `_pass` is the exit gate's [`crate::exitgate::Pass`]: there is no way to this effect but through the gate.
pub fn export(
    _pass: &crate::exitgate::Pass,
    home: &Home,
    pick: &Pick,
    attachments: &[String],
    note: &str,
    out: &Path,
) -> Result<Made, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are marked too.
    crate::trace::mark(crate::feature::Feature::W5);
    let pile = whole(home)?;
    let chosen = chosen_in(&pile, pick)?;
    if chosen.items.is_empty() {
        return Err(Fault::known(
            Known::QueueEmpty,
            crate::lang::t(crate::lang::Key::Tail180).to_string(),
        ));
    }
    let originals = Originals::of(&chosen);
    // Where this kit is anchored travels with it: the home's basis now, as the note's fixed last line.
    let note = crate::kitsindex::note_with(note, anchored_on(home)?.as_ref());
    let mut b = Bundle { entries: chosen.items.clone(), note, ..Default::default() };
    let mut names: Vec<(String, String)> = Vec::new();
    for one in attachments {
        let t = one.trim();
        if t.is_empty() {
            continue;
        }
        attach(Path::new(t), &originals, &mut b, &mut names)?;
    }
    // For renamed items, write a table of original names to kit names (itself a file in the manifest, not
    // record content).
    if !names.is_empty() {
        b.files.push((NAMES_FILE.to_string(), names_table(&names)));
    }
    // The terms documents kept when the selected grants were signed travel with the kit: the kit path is the
    // relative path in the `kits` room (assembled only by `termsx::doc_rel`), so a verifier can recompute the
    // digest and read exclusivity by themselves.
    for bytes in &chosen.items {
        let Ok(e) = zikaron::entry::check(bytes) else { continue };
        if e.kind != zikaron::tokens::EntryType::Grant {
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
    let landed = pack::export(out, b).map_err(|t| {
        Fault::of_landing(t)
    })?;
    Ok(Made {
        path: out.display().to_string(),
        kit_id: landed.kit_id,
        entries: landed.entries,
        files: landed.files,
        pulled: chosen.pulled,
        dropped: landed.dropped,
    })
}

#[cfg(test)]
mod stem_tests {
    use super::*;

    fn row(seq: u64, id: &str, work: Option<&str>) -> crate::ledgerx::Row {
        crate::ledgerx::Row {
            seq,
            kind: zikaron::tokens::EntryType::History,
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
        // Whether the digest goes before or after the transliterated part is free and not pinned: only
        // require that the transliterated segments and one digest are present, with the extension kept.
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

/// The selected entries not yet anchored (the export page follows anchor state): lights are read from the
/// table; confirmed ones (asked only through [`crate::ledgerx::Lamp::confirmed`], as the status bar reads)
/// and delete entries do not count. Returns (ids and lights of those not yet anchored, and which of them
/// "anchor now" can send: true when not yet queued).
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
