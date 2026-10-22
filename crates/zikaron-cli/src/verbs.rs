//! The twenty-six verbs. Each but `mcp` does the same three steps: arrange arguments, call the public API below,
//! render the returned state on one line (`mcp` runs the others, `crate::mcp`).
//!
//! The shell only requires the flags it needs itself (where the ledger is, whose key, where to write).
//! Body members the `zikaron/1` spec requires are enforced by the core: a `history` without `mode` or a `grant`
//! without `terms` has no such member in its body, and the core's thirteen steps return `E_BODY_FIELD`. A
//! second check here would be a second copy of the spec, and copies drift apart; it also means every
//! entry-writing verb's refusal is the spec's own token. For people, that refusal carries one stderr line
//! naming the missing member (the core's `entry::body_fault`, the same table) and the flags that give it
//! ([`body_flags`]); stdout and the exit code are the core's answer alone.

use crate::args::{self, Args};
use crate::chain;
use crate::codes::{Contract, Exit, Field, Key, Reason, Word};
use crate::docs;
use crate::entropy;
use crate::entry;
use crate::kitout;
use crate::ledger;
use crate::out::{self, s, Answer};
use crate::sent;
use zikaron::audit::{self, Outcome};
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::tokens::{EntryType, Label};
use zikaron_kit::badge;
use zikaron_kit::check::{self, Hop};
use zikaron_kit::reading;
use zikaron_kit::tokens::{CheckVerdict, PairVerdict, BADGE_OK, KIT_OK};
use zikaron_store::ledger::{LedgerDir, Stored};

/// Flags shared by the entry-writing verbs.
const WRITE: [&str; 5] = ["ledger", "key", "root", "seq", "prev"];

/// Join two flag lists (a verb's list is the shared flags plus its own).
fn flags<'a>(base: &[&'a str], more: &[&'a str]) -> Vec<&'a str> {
    base.iter().chain(more.iter()).copied().collect()
}

/// The verbs and each verb's closed flag list, one row per verb: its name, the shared list it starts from
/// ([`WRITE`] or none) and its own flags. This is the one place a verb's flags are spelled: [`run`] closes
/// every verb's arguments by its row before the verb runs (the crate's one `close`), [`VERBS`] is this table's
/// first column, and the `contract` verb prints it. `--key-file` is taken wherever `key` is
/// (`args::STANDS_FOR`, read by `close`).
const ACCEPTS: [(&str, &[&str], &[&str]); 26] = [
    ("keygen", &[], &[]),
    ("init", &[], &["ledger", "key", "statement", "home"]),
    ("history", &WRITE, &["content", "mark", "toolchain", "note", "file", "home"]),
    ("grant", &WRITE, &["grantee", "work", "terms", "history", "window-from", "window-to", "scope", "upstream", "home"]),
    ("revoke", &WRITE, &["grant", "case", "home"]),
    ("adopt", &WRITE, &["anchors", "attestor", "attestation", "home"]),
    ("attest", &[], &["key", "author", "anchors", "prev", "home"]),
    ("succeed", &WRITE, &["to", "kind", "effective", "statement", "home"]),
    ("annotate", &WRITE, &["subject", "note", "home"]),
    ("retract", &WRITE, &["subject", "note", "home"]),
    ("anchor", &[], &["key", "endpoint", "form", "registry", "hash", "calldata", "wait-secs", "proxy", "home", "ledger"]),
    ("scan", &[], &["endpoint", "fixture", "basis", "adoptions", "proxy", "ledger"]),
    ("audit", &[], &["ledger", "root", "input", "fragment", "unavailable", "out"]),
    ("check-grant", &[], &["ledger", "root", "input", "fragment", "unavailable", "now", "grant"]),
    ("chain-check", &[], &["ledger", "root", "hop", "now", "input", "fragment", "unavailable"]),
    ("depth", &[], &["ledger", "root", "work", "input", "fragment", "unavailable"]),
    ("fpm-sign", &[], &["key", "work", "grant", "note", "rows", "out"]),
    ("ack-sign", &[], &["key", "note", "fpm", "fpm-doc", "variant", "out"]),
    ("badge", &[], &["encode", "decode"]),
    ("kit-export", &[], &["ledger", "root", "note", "out", "entry", "file", "proof", "as", "home", "to", "to-key", "endpoint", "proxy", "originals"]),
    ("kit-verify", &[], &["kit", "fragment", "unavailable", "originals-to", "home", "ledger", "key"]),
    ("hand-over", &[], &["ledger", "out"]),
    ("take-over", &[], &["ledger", "handoff"]),
    ("show", &[], &["ledger", "entry", "path"]),
    ("contract", &[], &[]),
    // The Model Context Protocol on stdin and stdout (`mcp`, CLI-SCHEMA §13): it takes no flag.
    ("mcp", &[], &[]),
];

/// The flags a verb takes more than once (read with `Args::many`), by verb: the readings that need to know
/// whether a flag repeats (`mcp`'s tool parameters) read it here.
pub const REPEATED: [(&str, &str); 14] = [
    ("anchor", "hash"),
    ("scan", "fixture"),
    ("scan", "endpoint"),
    ("audit", "unavailable"),
    ("check-grant", "unavailable"),
    ("chain-check", "unavailable"),
    ("chain-check", "hop"),
    ("depth", "unavailable"),
    ("badge", "encode"),
    ("kit-export", "entry"),
    ("kit-export", "file"),
    ("kit-export", "proof"),
    ("kit-export", "endpoint"),
    ("kit-verify", "unavailable"),
];

/// The verbs the desktop does when `--home` names its home, and the flags each takes beside `--home` then (the
/// desktop writes with its own identity, into its own ledger: the flags that name a key, a ledger or a link are
/// not taken). Closed: a verb not here is not handed to the desktop (a verb in [`BY_HAND`] takes `--home` and is
/// refused here). `CLI-SCHEMA.md` §11 is the readable form.
pub const HOME: [(&str, &[&str]); 11] = [
    ("init", &["statement"]),
    ("history", &["file", "note"]),
    ("grant", &["grantee", "work", "terms", "history", "window-from", "window-to", "scope", "upstream"]),
    ("revoke", &["grant", "case"]),
    ("adopt", &["anchors", "attestor", "attestation"]),
    ("attest", &[]),
    ("annotate", &["subject", "note"]),
    ("retract", &["subject", "note"]),
    ("anchor", &[]),
    ("kit-export", &["entry", "note", "out", "as", "to", "to-key", "originals"]),
    ("kit-verify", &["kit", "originals-to"]),
];

/// The verbs that take `--home` but are done on the desktop by hand only: a final, rare change of who holds the
/// ledger. With `--home` they answer `E_ON_DESKTOP` here (exit 1) and nothing is asked of the desktop ([`run`]).
/// Closed; a verb takes `--home` exactly when it is in [`HOME`] or here.
pub const BY_HAND: [&str; 1] = ["succeed"];

/// The flag that hands a verb to the desktop.
pub const HOME_FLAG: &str = "home";

/// A verb's closed flag list: with `--home` given (`homed`) and the verb one the desktop does, `home` and its
/// row in [`HOME`]; otherwise its row in [`ACCEPTS`]. A name that is not a verb has none.
pub fn accepts(verb: &str, homed: bool) -> Option<Vec<&'static str>> {
    if let (true, Some((_, own))) = (homed, HOME.iter().find(|(name, _)| *name == verb)) {
        return Some(std::iter::once(HOME_FLAG).chain(own.iter().copied()).collect());
    }
    ACCEPTS.iter().find(|(name, _, _)| *name == verb).map(|(_, shared, own)| flags(shared, own))
}

/// The closed set of all flag names (each spelled once; verbs pick from here).
const ALL_FLAGS: [&str; 34] = [
    "ledger", "key", "root", "seq", "prev", "statement", "content", "mark", "toolchain", "note",
    "grantee", "work", "terms", "history", "window-from", "window-to", "scope", "upstream",
    "grant", "case", "anchors", "attestor", "attestation", "author", "to", "kind", "effective",
    "subject", "endpoint", "form", "registry", "hash", "calldata", "wait-secs",
];

const MORE_FLAGS: [&str; 28] = [
    "fixture", "basis", "adoptions", "input", "fragment", "unavailable", "now", "hop", "entry",
    "path", "out", "file", "proof", "variant", "fpm", "fpm-doc", "rows", "encode", "decode", "proxy",
    "key-file", "home", "as", "kit", "handoff", "originals-to", "to-key", "originals",
];

/// The way out for the verbs that reach nodes: `--proxy system` (or no flag) follows the terminal's proxy
/// variables when any is set, else the system's proxy settings (as programs in a terminal do, on every system);
/// `--proxy none` goes straight, an address (`http://host:port`, `socks5://host:port`) is that proxy; anything
/// else is misuse. Loopback nodes are always reached straight.
fn proxy_from(a: &Args) {
    use zikaron_anchor::rpc::{follow_the_terminal, proxy_of, set_choice, Choice};
    follow_the_terminal();
    let choice = match a.one("proxy").as_deref().map(str::trim) {
        None | Some("system") => Choice::System,
        Some("none") => Choice::Off,
        Some(p) => match proxy_of(p) {
            Ok(p) => Choice::Manual(p),
            Err(_) => out::misuse(Reason::Args, out::Subject::flag("proxy"), out::Said::ProxyShape),
        },
    };
    set_choice(choice);
}

fn known(name: &str) -> bool {
    ALL_FLAGS.contains(&name) || MORE_FLAGS.contains(&name)
}

// Dispatch.

pub fn run(a: &Args) -> Answer {
    crate::seam();
    // Each verb's own flag list is closed here by its row, before the verb runs: a flag outside it is misuse.
    // A name that is not a verb has no row and is refused below.
    if let Some(allowed) = accepts(a.verb(), a.homed()) {
        a.close(&allowed);
    }
    // `--home`: the desktop does the verb (`desk`); a verb done on the desktop by hand only ([`BY_HAND`]) is
    // refused here and nothing is asked of it.
    if a.homed() {
        if BY_HAND.contains(&a.verb()) {
            return crate::desk::on_desktop();
        }
        return crate::desk::ask(a);
    }
    match a.verb() {
        "keygen" => keygen(a),
        "init" => init(a),
        "history" => history(a),
        "grant" => grant(a),
        "revoke" => revoke(a),
        "adopt" => adopt(a),
        "attest" => attest(a),
        "succeed" => succeed(a),
        "annotate" => annotate(a),
        "retract" => retract(a),
        "anchor" => anchor(a),
        "scan" => scan(a),
        "audit" => audit_verb(a),
        "check-grant" => check_grant(a),
        "chain-check" => chain_check(a),
        "depth" => depth(a),
        "fpm-sign" => fpm_sign(a),
        "ack-sign" => ack_sign(a),
        "badge" => badge_verb(a),
        "kit-export" => kit_export(a),
        "kit-verify" => kit_verify(a),
        "hand-over" => crate::handoff::hand_over(a),
        "take-over" => crate::handoff::take_over(a),
        "show" => show(a),
        "contract" => contract(a),
        "mcp" => crate::mcp::serve(a),
        // A verb not in the table is named by its place and length, never echoed (it may be anything pasted).
        other => out::misuse(Reason::Args, out::Subject::Placed { at: 1, bytes: other.len() }, out::Said::NotAVerb),
    }
}

// Keys and ledger creation.

fn keygen(_a: &Args) -> Answer {
    let key = match entropy::bytes32() {
        Some(k) => k,
        None => return out::unanswered(Reason::Random, vec![]),
    };
    // Random 32 bytes may fall outside the curve range (very unlikely, which is not never). The core judges
    // the range; outside it is said so, never replaced by another number.
    if !zikaron::cryptox::in_range(&key) {
        return out::unanswered(Reason::Random, vec![]);
    }
    match entry::address(&key) {
        Some(addr) => out::affirmed(vec![
            (Key::Address, s(&addr)),
            (Key::Privkey, s(&hexfmt::encode(&key))),
        ]),
        None => out::unanswered(Reason::Random, vec![]),
    }
}

fn init(a: &Args) -> Answer {
    let key = a.key("key");
    let author = match entry::address(&key) {
        Some(x) => x,
        None => return out::denied(Reason::Key, vec![]),
    };
    let dir = match ledger::open_or_create(&a.need("ledger")) {
        Ok(d) => d,
        Err(t) => return ledger_trouble(&t),
    };
    // One ledger, one root. Genesis is the first entry of a ledger, so it is written only when the ledger is
    // empty (the lenient read finds no pile and no skipped items); there is no other way to write it. The
    // refusal is named by what the core says:
    //
    // * exactly one seq 0 entry recognized: `E_ALREADY_ROOTED` (with its author);
    // * several seq 0 entries with different ids: `E_TIP_FORKED` (the ledger is already broken). Ids are
    // counted, not authors or files: two genesis entries signed by one key are two ids and two roots
    // (`zikaron/1` §8 records EQUIVOCATION); one genesis saved under two file names is one id, merged by the
    // core (§8.1), and one root;
    // * anything else non-empty: `E_NOT_EMPTY` (unreadable or skipped stray files, entry-named bytes the core
    // refuses, entries without a root).
    //
    // Counting only recognized seq 0 entries would miss a malformed genesis file that the store reads into
    // the pile and the core refuses, and a second root would be written beside it. Asking "is it empty"
    // leaves no such case.
    //
    // The store and the core decide (the storage naming rule and lenient read, the core's thirteen steps);
    // the shell counts and reports. A refusal writes no entry file (`open_or_create` only creates the
    // directory itself, before the question).
    let survey = match dir.survey() {
        Ok(x) => x,
        Err(t) => return ledger_trouble(&t),
    };
    if !survey.items.is_empty() || !survey.skipped.is_empty() {
        let found = Value::Int((survey.items.len() + survey.skipped.len()) as u64);
        let skipped: Vec<Value> = survey.skipped.iter().map(|x| s(&x.name)).collect();
        // The seq 0 entries the core recognizes, deduplicated by id (one per id; authors are not
        // deduplicated).
        let mut roots: Vec<(String, String)> = survey
            .items
            .iter()
            .filter_map(|b| zikaron::entry::check(b).ok())
            .filter(|e| e.seq == 0)
            .map(|e| (e.id_hex(), e.author))
            .collect();
        roots.sort();
        roots.dedup_by(|a, b| a.0 == b.0);
        return match roots.len() {
            1 => out::denied(Reason::AlreadyRooted, vec![(Key::Author, s(&roots[0].1)), (Key::Count, found)]),
            0 => {
                let mut ms = vec![(Key::Count, found)];
                if !skipped.is_empty() {
                    ms.push((Key::Names, Value::Arr(skipped)));
                }
                out::denied(Reason::NotEmpty, ms)
            }
            _ => out::denied(Reason::TipForked, vec![(Key::Count, found)]),
        };
    }
    let mut body: Vec<(Field, Value)> = Vec::new();
    if let Some(x) = a.one("statement") {
        body.push((Field::StatementMd, s(&x)));
    }
    seal_and_append(
        Landing::Dir(&dir),
        &[],
        &author,
        EntryType::Genesis.as_str(),
        0,
        None,
        entry::shape(body),
        &key,
        None,
    )
}

// The seven entry types.

fn history(a: &Args) -> Answer {
    let mut body: Vec<(Field, Value)> = Vec::new();
    // `--file`: the record convention (`zikaron_glue::recording`, the app's own) fills `content` with the
    // file's sha256 and `mode` with the convention's two cells. Only when given; it stands instead of the three
    // flags it fills, never beside them.
    if let Some(path) = a.one("file") {
        if a.one("content").is_some() || a.one("mark").is_some() || a.one("toolchain").is_some() {
            out::misuse(Reason::Args, out::Subject::Flags(vec!["file", "content", "mark", "toolchain"]), out::Said::OneOf);
        }
        use zikaron_glue::recording;
        let bytes = args::slurp(&path);
        body.push((Field::Content, s(&hexfmt::encode(&recording::content_of(&bytes)))));
        body.push((
            Field::Mode,
            entry::shape(vec![(Field::Mark, s(recording::FAMILY)), (Field::Toolchain, s(&hexfmt::encode(&recording::toolchain())))]),
        ));
        if let Some(x) = a.one("note") {
            body.push((Field::NoteMd, s(&x)));
        }
        return write_entry(a, EntryType::History, body);
    }
    if let Some(x) = a.one("content") {
        body.push((Field::Content, s(&x)));
    }
    // `mode` is a member the spec requires. With both flags absent it does not appear, and the core refuses:
    // the requirement lives in the core's body table.
    let mark = a.one("mark");
    let toolchain = a.one("toolchain");
    if mark.is_some() || toolchain.is_some() {
        let mut mode: Vec<(Field, Value)> = Vec::new();
        if let Some(x) = mark {
            mode.push((Field::Mark, s(&x)));
        }
        if let Some(x) = toolchain {
            mode.push((Field::Toolchain, s(&x)));
        }
        body.push((Field::Mode, entry::shape(mode)));
    }
    if let Some(x) = a.one("note") {
        body.push((Field::NoteMd, s(&x)));
    }
    write_entry(a, EntryType::History, body)
}

/// Which flags give a body member the core refused (`entry::body_fault`'s path), per entry type: the words of
/// the stderr line, in both languages (Chinese, English) as the rest of that line is; the line takes the
/// system's (`out::body_line`). A member no flag gives (an unlisted type has no table) says none.
fn body_flags(kind: EntryType, member: &str) -> Option<(&'static str, &'static str)> {
    let one = |f: &'static str| Some((f, f));
    match (kind, member) {
        (EntryType::Genesis, "statement_md") => one("--statement"),
        (EntryType::History, "content") => Some(("--content(或 --file)", "--content (or --file)")),
        (EntryType::History, "mode") => Some(("--mark 与 --toolchain(或 --file)", "--mark and --toolchain (or --file)")),
        (EntryType::History, "mode.mark") => one("--mark"),
        (EntryType::History, "mode.toolchain") => one("--toolchain"),
        (EntryType::History, "note_md") => one("--note"),
        (EntryType::Grant, "grantee") => one("--grantee"),
        (EntryType::Grant, "work") => one("--work"),
        (EntryType::Grant, "terms") => one("--terms"),
        (EntryType::Grant, "history") => one("--history"),
        (EntryType::Grant, "window" | "window.from" | "window.to") => Some(("--window-from 与 --window-to", "--window-from and --window-to")),
        (EntryType::Grant, "scope_md") => one("--scope"),
        (EntryType::Revocation, "grant") => one("--grant"),
        (EntryType::Revocation, "case") => one("--case"),
        (EntryType::Adoption, "attestor") => one("--attestor"),
        (EntryType::Adoption, "attestation") => one("--attestation"),
        (EntryType::Adoption, _) => one("--anchors"),
        (EntryType::Succession, "to") => one("--to"),
        (EntryType::Succession, "kind") => one("--kind"),
        (EntryType::Succession, "effective") => one("--effective"),
        (EntryType::Succession, "statement_md") => one("--statement"),
        (EntryType::Annotation, "subject") => one("--subject"),
        (EntryType::Annotation, "note_md") => one("--note"),
        _ => None,
    }
}

fn grant(a: &Args) -> Answer {
    let mut body: Vec<(Field, Value)> = Vec::new();
    for (flag, field) in [
        ("grantee", Field::Grantee),
        ("work", Field::Work),
        ("terms", Field::Terms),
        ("history", Field::History),
        ("scope", Field::ScopeMd),
    ] {
        if let Some(x) = a.one(flag) {
            body.push((field, s(&x)));
        }
    }
    let from = a.within_ceiling("window-from");
    let to = a.within_ceiling("window-to");
    if from.is_some() || to.is_some() {
        let mut w: Vec<(Field, Value)> = Vec::new();
        if let Some(x) = from {
            w.push((Field::From, Value::Int(x)));
        }
        if let Some(x) = to {
            w.push((Field::To, Value::Int(x)));
        }
        body.push((Field::Window, entry::shape(w)));
    }
    // `upstream` is the member the kit reading adds to a grant body (`zikaron/1` §6.10 treats it as data). The
    // shell fills in the caller's bytes; how the per-hop checks read it is the kit core's business.
    if let Some(x) = a.one("upstream") {
        body.push((Field::Upstream, s(&x)));
    }
    write_entry(a, EntryType::Grant, body)
}

fn revoke(a: &Args) -> Answer {
    let mut body: Vec<(Field, Value)> = Vec::new();
    if let Some(x) = a.one("grant") {
        body.push((Field::Grant, s(&x)));
    }
    // `case` is optional: present only when given, never an empty placeholder.
    if let Some(x) = a.one("case") {
        body.push((Field::Case, s(&x)));
    }
    write_entry(a, EntryType::Revocation, body)
}

fn adopt(a: &Args) -> Answer {
    let mut body: Vec<(Field, Value)> = Vec::new();
    if let Some(p) = a.one("anchors") {
        body.push((Field::Anchors, args::slurp_json(&p)));
    }
    if let Some(x) = a.one("attestor") {
        body.push((Field::Attestor, s(&x)));
    }
    if let Some(x) = a.one("attestation") {
        body.push((Field::Attestation, s(&x)));
    }
    write_entry(a, EntryType::Adoption, body)
}

/// The `zikaron/1` §6.6 cosignature: over the canonical bytes of `{adopter, anchors, prev}`, domain
/// `zikaron/1-adoption`. The preimage includes `prev` and comes from the core's `adoption_preimage`.
fn attest(a: &Args) -> Answer {
    let key = a.key("key");
    let attestor = match entry::address(&key) {
        Some(x) => x,
        None => return out::denied(Reason::Key, vec![]),
    };
    let author = a.need("author");
    let prev = a.need("prev");
    let anchors = args::slurp_json(&a.need("anchors"));
    match entry::attestation(&author, &anchors, &prev, &key) {
        Ok(sig) => out::affirmed(vec![
            (Key::Attestation, s(&sig)),
            (Key::Attestor, s(&attestor)),
        ]),
        Err(t) => out::denied(Reason::Entry, vec![(Key::Token, s(t.as_str()))]),
    }
}

fn succeed(a: &Args) -> Answer {
    let mut body: Vec<(Field, Value)> = Vec::new();
    if let Some(x) = a.one("to") {
        body.push((Field::To, s(&x)));
    }
    // `kind` is free: any token is accepted; a whitelist here would be a closed table the spec does not have.
    if let Some(x) = a.one("kind") {
        body.push((Field::Kind, s(&x)));
    }
    if let Some(x) = a.within_ceiling("effective") {
        body.push((Field::Effective, Value::Int(x)));
    }
    if let Some(x) = a.one("statement") {
        body.push((Field::StatementMd, s(&x)));
    }
    write_entry(a, EntryType::Succession, body)
}

fn annotate(a: &Args) -> Answer {
    let mut body: Vec<(Field, Value)> = Vec::new();
    if let Some(x) = a.one("subject") {
        body.push((Field::Subject, s(&x)));
    }
    if let Some(x) = a.one("note") {
        body.push((Field::NoteMd, s(&x)));
    }
    write_entry(a, EntryType::Annotation, body)
}

/// Retract a work record (the retraction convention over the open entry types of `zikaron/1` §6.9). The type
/// literal, body keys and writing rule live in `zikaron_glue::retraction`, shared with the app: a missing or non-hex32
/// subject, one not on this ledger's lineage, not a `history`, or already deleted is refused by the
/// convention's token (`E_RETRACTION` + `token`) and the ledger is unchanged. A passing retraction goes the
/// same way as `annotate`: the core builds, signs and checks it, the storage crate lands it.
fn retract(a: &Args) -> Answer {
    use zikaron_glue::retraction as convention;
    let at = a.need("ledger");
    // A home the desktop keeps sealed is retracted in with the credential, as the other entry-writing verbs
    // write it (`write_entry`); a key given for it is misuse.
    let home = if crate::sealed::is_home(&at) {
        if a.one("key").is_some() || a.one(args::KEY_FILE).is_some() {
            out::misuse(Reason::Args, out::Subject::Flags(vec!["key", args::KEY_FILE]), out::Said::SealedNoKey);
        }
        let now = now_secs();
        match crate::sealed::open(&at, now) {
            Ok(h) => Some((h, now)),
            Err(t) => return crate::sealed::refused(&t),
        }
    } else {
        None
    };
    let (key, author, dir, items) = match &home {
        Some((h, _)) => (h.key, h.author.clone(), None, h.items.clone()),
        None => {
            let key = a.key("key");
            let author = match entry::address(&key) {
                Some(x) => x,
                None => return out::denied(Reason::Key, vec![]),
            };
            let dir = match ledger::open(&at) {
                Ok(d) => d,
                Err(t) => return ledger_trouble(&t),
            };
            let items = match ledger::pile(&dir) {
                Ok(x) => x,
                Err(t) => return ledger_trouble(&t),
            };
            (key, author, Some(dir), items)
        }
    };
    // Ask which entry to follow first: giving only one of `--seq` and `--prev` is misuse, and misuse goes out
    // before any answer (same order as `write_entry`).
    let (seq, prev) = match next_link(a, &items) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
    // A home's write gates, as for every entry written with the credential.
    if let Some(refused) = home.as_ref().and_then(|(h, _)| crate::sealedsend::write_gates(h)) {
        return refused;
    }
    let root = match root_named(a, &items) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
    // The convention reads the lineage ledger the core's audit returns (the same one as the tip question),
    // not stray files in the pile.
    let lines: Vec<convention::Line> = match ledger::entries(&root, &items) {
        Ok(es) => es.iter().map(convention::Line::of).collect(),
        Err(r) => return out::denied(r, vec![]),
    };
    let target = match convention::may_retract(&lines, &a.one("subject").unwrap_or_default()) {
        Ok(id) => id,
        Err(why) => return out::denied(Reason::Retraction, vec![(Key::Token, s(why.token()))]),
    };
    let landing = match (&home, &dir) {
        (Some((h, now)), _) => Landing::Sealed(h, *now, None),
        (None, Some(d)) => Landing::Dir(d),
        (None, None) => return out::denied(Reason::Ledger, vec![]),
    };
    seal_and_append(
        landing,
        &items,
        &author,
        convention::ENTRY_TYPE,
        seq,
        prev.as_deref(),
        convention::body(&target, &a.one("note").unwrap_or_default()),
        &key,
        a.one("root").as_deref(),
    )
}

// Chain.

fn anchor(a: &Args) -> Answer {
    // A home's queue, sent with the credential (`sealedsend`): what it sends and how come from the home's settings
    // and this machine's, so the flags that would name them are misuse, as a key is for any sealed write.
    if let Some(at) = a.one("ledger") {
        if !crate::sealed::is_home(&at) {
            out::misuse(Reason::Args, out::Subject::flag("ledger"), out::Said::AnchorNotAHome);
        }
        if a.one("key").is_some() || a.one(args::KEY_FILE).is_some() {
            out::misuse(Reason::Args, out::Subject::Flags(vec!["key", args::KEY_FILE]), out::Said::SealedNoKey);
        }
        for f in ["endpoint", "form", "registry", "hash", "calldata", "proxy"] {
            if !a.many(f).is_empty() {
                out::misuse(Reason::Args, out::Subject::flag(f), out::Said::HomeSettingsSay);
            }
        }
        return crate::sealedsend::anchor(&at, std::time::Duration::from_secs(a.u64_of("wait-secs").unwrap_or(90)));
    }
    proxy_from(a);
    use zikaron_anchor::send;
    let specs = chain::endpoint_specs(&a.many_at("endpoint"));
    // Anchoring uses exactly one endpoint. Taking the first and silently ignoring the rest would drop the
    // others; there is no failover here, so a second url for the same chain would never be used either.
    // (Reading needs multi-endpoint agreement; that is `scan`.)
    if specs.len() != 1 {
        out::misuse(Reason::Args, out::Subject::flag("endpoint"), out::Said::OneEndpoint);
    }
    let (chain_id, url) = specs[0].clone();
    let mut ep = match zikaron_anchor::rpc::Http::new(&url) {
        Some(h) => h,
        // Read whole by `endpoint_specs` before; named, never echoed, all the same.
        None => out::misuse(Reason::Args, out::Subject::flag("endpoint"), out::address_said(&url)),
    };
    let key = a.key("key");
    let form_raw = a.need("form");
    let form = if form_raw == Word::Registry.as_str() {
        send::Form::Registry
    } else if form_raw == Word::Bare.as_str() {
        send::Form::Bare
    } else {
        out::misuse(Reason::Args, out::typed(&form_raw), out::Said::FormWord)
    };
    // The registry goes with the registry form and only with it: judged here, before any node is asked.
    let registry = a.one("registry").map(|x| chain::h20(&x));
    match (form, registry.is_some()) {
        (send::Form::Registry, false) => out::misuse(Reason::Args, out::Subject::flag("registry"), out::Said::Missing),
        (send::Form::Bare, true) => out::misuse(Reason::Args, out::Subject::flag("registry"), out::Said::RegistryOnlyWithRegistryForm),
        _ => {}
    }
    let hashes: Vec<[u8; 32]> = a.many("hash").iter().map(|x| chain::h32(x)).collect();
    let calldata = a.one("calldata").map(|x| match hexfmt::decode(&x) {
        Some(b) => b,
        None => out::misuse(Reason::Args, out::Subject::flag("calldata"), out::Said::NotHex),
    });
    if hashes.is_empty() && calldata.is_none() {
        out::misuse(Reason::Args, out::Subject::flag("hash"), out::Said::NeedHashOrCalldata);
    }
    // The caller sets the wait: a fixed number would impose one block time on every chain.
    let wait = std::time::Duration::from_secs(a.u64_of("wait-secs").unwrap_or(90));
    // What this very anchoring sent before is asked about first, by its hashes (`sent`, the record kept in the
    // machine folder; `send::earlier`, the one rule the app's queue judges by too): included, it is answered as
    // included; held by the node, its receipt is awaited; either way nothing new is signed. A record that cannot
    // be placed or read, or a node that cannot say where it stands, signs nothing either: a transaction already
    // out is never sent a second time on a guess.
    let Some(from) = zikaron::cryptox::address_of_privkey(&key) else { out::misuse(Reason::Key, out::Subject::flag("key"), out::Said::KeyOutOfRange) };
    let dir = match send::target(form, registry, &hashes, calldata.clone(), from) {
        Ok((to, data)) => sent::machine().map(|m| sent::place(&m, chain_id, &from, &to, &data)),
        Err(t) => Err(format!("{t:?}")),
    };
    let dir = match dir {
        Ok(d) => d,
        Err(said) => return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&said))]),
    };
    let before = match sent::read(&dir) {
        Ok(x) => x,
        Err(said) => return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&said))]),
    };
    // A transaction included with a status other than 1 will never anchor, as one void never will: the answer
    // says so (`E_TX_STATUS`), and the record marks what it holds void, so the next run sends the anchoring
    // afresh rather than answering that failure for ever.
    // The chain's answer stands whether or not the mark lands: a mark that cannot be written leaves the record as
    // it was, and the next run finds the same inclusion and tries the mark again.
    let answered = |tx: &[u8; 32], confirm: &send::Confirm| -> Answer {
        if matches!(confirm, send::Confirm::Included { status, .. } if *status != 1) {
            let _ = sent::read(&dir).and_then(|held| sent::void(&dir, &held));
        }
        sent_answer(tx, confirm, wait)
    };
    let mut nonce = None;
    if !before.is_empty() {
        match send::earlier(&mut ep, &from, &before) {
            send::Earlier::Included { tx, status, block_number } => return answered(&tx, &send::Confirm::Included { status, block_number }),
            send::Earlier::Held { tx } => {
                let c = send::confirm(&mut ep, &tx, wait);
                return answered(&tx, &c);
            }
            send::Earlier::Unread { tx, said } => return tx_unheard(&hexfmt::encode(&tx), &said),
            // No node holds them and their nonce is unused: signed again at that very nonce, so at most one of
            // them can ever be included.
            send::Earlier::Unused { nonce: n } => nonce = Some(n),
            // Void (their nonce used by another transaction, none included): said as void, nothing sent now, as
            // the desktop says it; the record marks them void, so the next run sends the anchoring afresh.
            send::Earlier::Void => {
                if let Err(said) = sent::void(&dir, &before) {
                    return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&said))]);
                }
                let last = before.last().map(|(_, h)| hexfmt::encode(h)).unwrap_or_default();
                return tx_void(&last);
            }
        }
    }
    // Anchored already, by whichever side: the registry's logs from this key, for these hashes, from the
    // registry's deployment block (`deploy::start_of`; block 0 for a registry no row names) to the head
    // (`send::on_chain`). Any hash the chain holds already: nothing is sent, each named. A chain that does not say
    // is no answer, and nothing is sent on a guess. The bare form and raw calldata name no registry log to ask by.
    if let (send::Form::Registry, Some(reg), None) = (form, registry, calldata.as_ref()) {
        match send::on_chain(&mut ep, chain_id, &reg, &from, &hashes, zikaron_anchor::deploy::start_of(chain_id, &reg)) {
            Ok(found) if !found.is_empty() => {
                let first = found[0];
                let names: Vec<String> = found.iter().map(|f| hexfmt::encode(&f.hash)).collect();
                return on_chain(&names, &hexfmt::encode(&first.tx), first.block);
            }
            Ok(_) => {}
            Err(not) => return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&not.evidence()))]),
        }
    }
    // The transaction is estimated before it is sent, by the rule the app takes (`send::estimate_gas`), and
    // carries the limit that estimate gives (`send::limit_for`). A call the endpoint refuses to estimate would
    // revert, and one estimated above the ceiling would run out of gas, either with the fee paid: neither is
    // broadcast. Its hash is landed in the record once it is signed and before it is broadcast: a hash that
    // cannot be landed sends nothing.
    let mut land = |n: u64, tx: &[u8; 32]| sent::land(&dir, n, tx);
    match send::anchor_landed(&mut ep, &key, chain_id, form, registry, &hashes, calldata, wait, nonce, &mut land) {
        Ok((sent, _limit)) => answered(&sent.tx, &sent.confirm),
        Err(send::NotSent::Gas(no)) => match no {
            send::NoGas::Refused(t) => {
                // A revert says what the contract said, read from the error's data (the reason stays the same).
                let why = match &t {
                    zikaron_anchor::rpc::Trouble::Node(e) if zikaron_anchor::said::refusal_of(e) == zikaron_anchor::said::Refusal::Reverted => {
                        zikaron_anchor::said::reverted(e).map(|r| format!(" \u{b7} {}", r.evidence())).unwrap_or_default()
                    }
                    _ => String::new(),
                };
                out::denied(Reason::GasRefused, vec![(Key::Detail, s(&format!("{t:?}{why}")))])
            }
            send::NoGas::OverCap(n) => out::denied(Reason::GasRefused, vec![(Key::Detail, s(&format!("{n} > {}", send::GAS_LIMIT)))]),
            // The endpoint never answered the question, refused it for its own reasons (the one table:
            // rate limited, no such method, credentials, wrong chain, an error without a numeric `code`, a
            // page at an HTTP status), or answered it in another shape: no answer about the call. The detail
            // names which, by that table.
            send::NoGas::Network(t) => {
                let said = match zikaron_anchor::said::refusal(&t) {
                    Some(r) => format!("{r:?}"),
                    None => format!("{t:?}"),
                };
                out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&said))])
            }
            send::NoGas::NotText(v) => out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&String::from_utf8_lossy(&zikaron::json::canon_bytes(&v))))]),
            send::NoGas::Unreadable(hex) => out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&hex))]),
        },
        Err(send::NotSent::Send(e)) => out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&format!("{e:?}")))]),
        // The node's answer to the nonce question, as it came, named by the question.
        Err(send::NotSent::Nonce(v)) => {
            let said = v.map(|v| String::from_utf8_lossy(&zikaron::json::canon_bytes(&v)).into_owned()).unwrap_or_default();
            out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&format!("eth_getTransactionCount: {said}")))])
        }
        // The hash could not be landed: nothing was broadcast.
        Err(send::NotSent::Land(said)) => out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&said))]),
        // The broadcast failed after the hash was landed: the bytes may be in a pool, so the answer carries the
        // hash, and a rerun asks about it before anything new is signed.
        Err(send::NotSent::Broadcast(tx, e)) => tx_unheard(&hexfmt::encode(&tx), &format!("{e:?}")),
    }
}

/// `anchor`'s answer for a transaction that went out, by where it stands: only "included with status 1" is
/// anchored (`zikaron/1` §9.1); the other three states are named, and the transaction hash is always present
/// because the bytes were broadcast.
fn sent_answer(tx: &[u8; 32], confirm: &zikaron_anchor::send::Confirm, wait: std::time::Duration) -> Answer {
    use zikaron_anchor::send::Confirm;
    let tx = hexfmt::encode(tx);
    match confirm {
        Confirm::Included { status: 1, block_number } => anchored(&tx, Some(*block_number)),
        // Included with a status other than 1: the chain answered "this failed". A negative answer.
        Confirm::Included { status, .. } => tx_failed(&tx, *status),
        // Not included by the deadline, or out of sight: the chain did not answer. (One endpoint is asked here,
        // so a split never comes; were it to, it is "not yet".)
        Confirm::NotYet | Confirm::Split { .. } => tx_not_yet(&tx, wait.as_secs()),
        Confirm::Unreachable(why) => tx_unheard(&tx, why),
    }
}

/// `anchor`'s answer when the chain already holds anchors of hashes it was to send, from this key through this
/// registry (`names`, the earliest in `tx` at `block`): nothing was sent. Built here only: the direct `anchor`, the
/// desktop's under `--home` and the credential's with `--ledger` all answer by it.
pub(crate) fn on_chain(names: &[String], tx: &str, block: u64) -> Answer {
    out::denied(Reason::OnChain, vec![(Key::BlockNumber, Value::Int(block)), (Key::Names, Value::Arr(names.iter().map(|n| s(n)).collect())), (Key::Tx, s(tx))])
        .telling(out::Said::OnChain.text().to_string())
}

/// `anchor`'s answer when the switch leaves sending to the person on the desktop: nothing was sent, `count`
/// entries wait in the queue (exit 3). The desktop's under `--home` and the credential's with `--ledger` answer by
/// it.
pub(crate) fn queued(count: u64) -> Answer {
    out::partial(Reason::Queued, vec![(Key::Count, Value::Int(count))]).telling(out::Said::Queued.text().to_string())
}

/// `anchor`'s answers once the transaction went out, built here only (the command line's own sending, the
/// desktop's under `--home` and the credential's with `--ledger` all answer by these): included with status 1, at
/// that block.
pub(crate) fn anchored(tx: &str, block: Option<u64>) -> Answer {
    out::affirmed(vec![(Key::BlockNumber, block.map(Value::Int).unwrap_or(Value::Null)), (Key::Tx, s(tx))])
}

/// Included with another status: the chain said the call failed (a negative answer).
pub(crate) fn tx_failed(tx: &str, status: u64) -> Answer {
    out::denied(Reason::TxStatus, vec![(Key::Tx, s(tx)), (Key::State, Value::Int(status))])
}

/// Not in a block after `waited` seconds: no answer yet.
pub(crate) fn tx_not_yet(tx: &str, waited: u64) -> Answer {
    out::unanswered(Reason::TxNotYet, vec![(Key::Tx, s(tx)), (Key::Count, Value::Int(waited))])
}

/// Void: no node holds what was sent and its nonce was used by another transaction; it will never be included
/// (a negative answer about that transaction; nothing new was sent by this answer).
pub(crate) fn tx_void(tx: &str) -> Answer {
    out::denied(Reason::TxVoid, vec![(Key::Tx, s(tx))])
}

/// No node answered the receipt question within the wait: what was heard.
pub(crate) fn tx_unheard(tx: &str, detail: &str) -> Answer {
    out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(detail)), (Key::Tx, s(tx))])
}

fn scan(a: &Args) -> Answer {
    proxy_from(a);
    let fixtures = a.many("fixture");
    let endpoints_given = a.many("endpoint");
    // A home's own nodes: `--ledger` names a home the desktop keeps and neither `--endpoint` nor `--fixture` is
    // given; the nodes come from its settings (`home_nodes`).
    let at_home = match a.one("ledger") {
        Some(at) if fixtures.is_empty() && endpoints_given.is_empty() && crate::sealed::is_home(&at) => Some(at),
        _ => None,
    };
    if at_home.is_none() && fixtures.is_empty() == endpoints_given.is_empty() {
        out::misuse(Reason::Args, out::Subject::Flags(vec!["fixture", "endpoint"]), out::Said::OneOf);
    }
    // A run over recordings reads no basis, no adoptions and no network: any of them given (once or more) is
    // misuse, never silently left unread.
    if !fixtures.is_empty() {
        for f in ["basis", "adoptions", "proxy", "ledger"] {
            if !a.many(f).is_empty() {
                out::misuse(Reason::Args, out::Subject::flag(f), out::Said::NotWithFixture);
            }
        }
    }
    let mut runs: Vec<(String, Value)> = Vec::new();
    let thin: Vec<u64>;
    if !fixtures.is_empty() {
        // The offline endpoint rule: a recording is everything one endpoint said; the same recording twice is
        // one source.
        let mut distinct: Vec<String> = fixtures
            .iter()
            .map(|p| {
                std::fs::canonicalize(p)
                    .map(|x| x.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| p.clone())
            })
            .collect();
        distinct.sort();
        distinct.dedup();
        let mut chains: Vec<u64> = distinct.iter().flat_map(|p| chains_in(p)).collect();
        chains.sort_unstable();
        chains.dedup();
        thin = chains
            .into_iter()
            .filter(|c| distinct.iter().filter(|p| chains_in(p).contains(c)).count() < 2)
            .collect();
        for p in &fixtures {
            match chain::replay_fragment(p) {
                Ok(v) => runs.push((p.clone(), v)),
                Err(r) => return refused(r),
            }
        }
    } else {
        let specs = match &at_home {
            Some(at) => match home_nodes(at, a.one("proxy").is_none()) {
                Ok(specs) => specs,
                Err(answer) => return answer,
            },
            None => chain::endpoint_specs(&a.many_at("endpoint")),
        };
        thin = chain::thin_chains(&specs);
        // The basis: the file `--basis` names, or the one a home the desktop keeps declares (`--ledger`).
        let basis = match (a.one("basis"), a.one("ledger")) {
            (Some(p), None) => chain::basis_bytes(&p),
            (None, Some(at)) => match home_basis(&at, &specs) {
                Ok(b) => b,
                Err(answer) => return answer,
            },
            _ => out::misuse(Reason::Args, out::Subject::Flags(vec!["basis", "ledger"]), out::Said::OneOf),
        };
        let adoptions = a.one("adoptions");
        for k in 0..chain::rounds(&specs) {
            match chain::live_fragment(&basis, adoptions.as_deref(), &specs, k) {
                Ok((who, v)) => runs.push((who, v)),
                Err(r) => return refused(r),
            }
        }
    }
    // When the core says "this is not a basis", the verb must not exit green.
    //
    // The scan first asks the core about the basis; a malformed basis gets no label, which is not a fragment
    // (no `anchors` / `basis` / `evidence`). Wrapping it as a fragment and exiting 0 would make a misspelled
    // basis look like a successful scan until assembly failed downstream with a different subject. So the
    // core's value goes out as a negative answer at once.
    for (_, v) in &runs {
        if v.member("basis").is_none() {
            return out::verbatim(Exit::Denied, v.clone());
        }
    }
    match zikaron_anchor::endpoints::agree_over(runs, thin) {
        Ok(reading) => out::affirmed(vec![
            (Key::Fragment, reading.fragment.clone()),
            (
                Key::SingleSource,
                Value::Bool(reading.single_source),
            ),
            (
                Key::SingleSourceChains,
                Value::Arr(
                    reading
                        .single_source_chains
                        .iter()
                        .map(|c| Value::Int(*c))
                        .collect(),
                ),
            ),
            (
                Key::Sources,
                Value::Arr(reading.sources.iter().map(|x| s(x)).collect()),
            ),
        ]),
        Err(d) => out::unanswered(
            Reason::EndpointsDisagree,
            vec![
                (
                    Key::Detail,
                    s(&format!("{} 份读数不一致", d.fragments.len())),
                ),
                (
                    Key::Sources,
                    Value::Arr(d.sources.iter().map(|x| s(x)).collect()),
                ),
            ],
        ),
    }
}

/// The basis a home the desktop keeps declares, as the desktop's self-audit declares it (`zikaron_glue::basis`):
/// the chain, the registry and the block to scan from in the home's settings (read with the credential,
/// `sealed::read`), the senders of its ledger, and the chain's head as its endpoints say it (the lowest of them)
/// for the window's last block. A ledger folder that is not a home's has no settings to read (misuse, `--ledger`);
/// a home whose settings name no chain or no registry has no basis (`E_SCAN`, `E_NO_BASIS`); no endpoint of that
/// chain given, or none answering its head, is no answer (`E_UNREACHABLE`).
/// A home's nodes for the chain its settings name, read with the credential as a send reads them
/// (`sealedsend::nodes_of`), and, when `--proxy` was not given (`their_proxy`), this machine's proxy for the command
/// line as a send takes it. No chain set is `E_NO_BASIS`, as for the basis; no node for that chain is no answer
/// (`E_UNREACHABLE`), the nodes never said.
fn home_nodes(at: &str, their_proxy: bool) -> Result<Vec<(u64, String)>, Answer> {
    let r = crate::sealed::read(at, now_secs()).map_err(|t| crate::sealed::refused(&t))?;
    let ground = r.ground().map_err(|t| crate::sealed::refused(&t))?;
    let Some(chain) = ground.chain_id else {
        return Err(out::unanswered(Reason::Scan, vec![(Key::Token, s("E_NO_BASIS")), (Key::Detail, s(at))]));
    };
    if their_proxy {
        let cli = r.machine_settings().map_err(|t| crate::sealed::refused(&t))?;
        zikaron_send::node::take_machine_route(cli.proxy.as_deref());
    }
    let nodes = crate::sealedsend::nodes_of(&ground, chain);
    if nodes.is_empty() {
        return Err(out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&format!("NO_ENDPOINT {chain}")))]));
    }
    Ok(nodes.iter().map(|e| (e.chain, e.url.for_transport().to_string())).collect())
}

fn home_basis(at: &str, specs: &[(u64, String)]) -> Result<Vec<u8>, Answer> {
    if !crate::sealed::is_home(at) {
        out::misuse(Reason::Args, out::Subject::flag("ledger"), out::Said::NotAHome);
    }
    let r = crate::sealed::read(at, now_secs()).map_err(|t| crate::sealed::refused(&t))?;
    let ground = r.ground().map_err(|t| crate::sealed::refused(&t))?;
    let (Some(chain), Some(registry)) = (ground.chain_id, ground.registry) else {
        return Err(out::unanswered(Reason::Scan, vec![(Key::Token, s("E_NO_BASIS")), (Key::Detail, s(at))]));
    };
    let Some(head) = chain::least_head(specs, chain) else {
        return Err(out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&format!("eth_blockNumber {chain}")))]));
    };
    let senders = zikaron_glue::basis::senders_of(&r.items);
    let registry = hexfmt::encode(&registry);
    let window = zikaron_glue::basis::Window { chain, registry: &registry, from_block: ground.from_block, to_block: head.max(ground.from_block), senders: &senders };
    Ok(json::canon_bytes(&zikaron_glue::basis::basis_of(&window)))
}

fn chains_in(path: &str) -> Vec<u64> {
    use zikaron_anchor::wire::{self, Body};
    let b = args::slurp(path);
    let Some(fx) = wire::parse(&b) else {
        out::misuse(Reason::Unreadable, out::typed(path), out::Said::NotJson)
    };
    let mut out_ids = Vec::new();
    if let Some(w) = fx.member("rpc") {
        if let Body::Obj(chains) = &w.body {
            for (cid, _) in chains {
                if let Ok(id) = cid.parse::<u64>() {
                    out_ids.push(id);
                }
            }
        }
    }
    out_ids
}

// Audit and readings.

/// An audit input: the raw bytes of the caller's file, or assembled from ledger and fragment.
fn audit_input(a: &Args) -> Result<Value, Answer> {
    if let Some(p) = a.one("input") {
        // The raw bytes go to the core unchanged, so it reads exactly what it must refuse; this layer neither
        // rewrites nor recanonicalizes them.
        let bytes = args::slurp(&p);
        return match json::parse_tests_1_3(&bytes) {
            Ok(v) => Ok(v),
            Err(_) => Err(out::verbatim(Exit::Denied, audit::no_label())),
        };
    }
    let items = match ledger::read_any(&a.need("ledger"), now_secs()) {
        Ok(x) => x,
        Err(r) => return Err(read_refused(r)),
    };
    let root = match a.one("root") {
        Some(x) => x,
        None => match ledger::root_of(&items) {
            Ok(x) => x,
            Err(r) => return Err(out::denied(r, vec![])),
        },
    };
    let fragment = match a.one("fragment") {
        Some(p) => {
            let v = args::slurp_json(&p);
            // A reading wraps the fragment in `fragment`; a bare fragment is accepted too.
            v.member(Key::Fragment.as_str()).cloned().unwrap_or(v)
        }
        None => {
            let empty = zikaron_anchor::scan::Scanned {
                anchors: Vec::new(),
                evidence: Vec::new(),
                basis: ledger::empty_basis(),
            };
            zikaron_anchor::scan::fragment(&empty)
        }
    };
    let unavailable: Vec<String> = a.many("unavailable");
    match zikaron_anchor::input::assemble(&fragment, &root, &ledger::pile_hex(&items), &unavailable)
    {
        Some(v) => Ok(v),
        None => Err(out::denied(Reason::Fragment, vec![])),
    }
}

fn audit_verb(a: &Args) -> Answer {
    let input = match audit_input(a) {
        Ok(v) => v,
        Err(ans) => return ans,
    };
    // `--out`: the audit input this pass assembled (contract 11, its six members, in canonical bytes; the same
    // shape `zka audit-input` prints) lands at that path before the core reads it, whatever the label, so a
    // cross-ledger `chain-check` can take it as a hop's input. Landing goes the one way files land
    // (`zikaron_glue::landing`): an existing name is refused, nothing is overwritten.
    if let Some(p) = a.one("out") {
        if let Err(t) = zikaron_glue::landing::land_bytes(std::path::Path::new(&p), &json::canon_bytes(&input)) {
            return out::denied(Reason::Ledger, vec![(Key::Detail, said_detail(t.code(), t.said())), (Key::Path, s(t.subject()))]);
        }
    }
    match audit::audit_full(&input) {
        // The report passes through unchanged (its byte shape belongs to the core); the label only sets the
        // exit code.
        Some(o) => out::verbatim(label_exit(o.label), o.report),
        None => out::verbatim(Exit::Denied, audit::no_label()),
    }
}

/// Label to exit code. GAPS and UNAVAILABLE are neither green nor red: exit 3.
fn label_exit(l: Label) -> Exit {
    match l {
        Label::Complete => Exit::Affirmed,
        Label::Gaps | Label::Unavailable => Exit::Partial,
        Label::BrokenChain | Label::NoLabel => Exit::Denied,
    }
}

fn outcome_of(a: &Args, grant: Option<&[u8]>) -> Result<Option<Outcome>, Answer> {
    if a.one("input").is_none() && a.one("ledger").is_none() {
        return Ok(None);
    }
    let input = audit_input(a)?;
    // The third check asks whether the grant is in the ledger: as in `chain_check`, the grant is added to the
    // pile before auditing (the kit core's `with_grant_in_pile`).
    let input = match grant {
        Some(g) => check::with_grant_in_pile(&input, g),
        None => input,
    };
    Ok(audit::audit_full(&input))
}

fn check_grant(a: &Args) -> Answer {
    let grant = match grant_of(&args::slurp(&a.need("grant"))) {
        Ok(g) => g,
        Err(ans) => return ans,
    };
    let outcome = match outcome_of(a, Some(&grant)) {
        Ok(o) => o,
        Err(ans) => return ans,
    };
    let checked = check::grant_check_with(&grant, outcome.as_ref(), a.now());
    out::verbatim(verdict_exit(checked.verdict), checked.value)
}

/// The grant to check from what `--grant` holds: entry bytes as they are; a grant file (the app's
/// single-file bundle, recognized by its start) opened by the one reading the app uses
/// (`zikaron_glue::grantfile`), and the grant it is for is its chain's last hop. A grant file that does not
/// open is refused by name (`E_GRANT_FILE` with the refusing layer's token); nothing is checked.
fn grant_of(bytes: &[u8]) -> Result<Vec<u8>, Answer> {
    use zikaron_glue::grantfile::Refused;
    if !zikaron_glue::container::is_container(bytes) {
        return Ok(bytes.to_vec());
    }
    match zikaron_glue::grantfile::open(bytes) {
        Ok(o) => o.hops.last().cloned().ok_or_else(|| out::denied(Reason::GrantFile, vec![(Key::Token, s("E_GRANT_FILE_CODE"))])),
        Err(r) => {
            let ms = match r {
                Refused::Shape(b) => vec![(Key::Token, s(b.code())), (Key::Detail, s(&b.subject()))],
                Refused::Kit(verdict, subject) => {
                    let mut m = vec![(Key::Token, s(verdict.as_str()))];
                    if let Some(x) = subject {
                        m.push((Key::Detail, s(&x)));
                    }
                    m
                }
                Refused::NoCode(at) => vec![(Key::Token, s("E_GRANT_FILE_CODE")), (Key::Detail, s(&at))],
                Refused::Code(r) => vec![(Key::Token, s(r.token.as_str()))],
                Refused::Uncarried(id) => vec![(Key::Token, s("E_GRANT_FILE_CHAIN")), (Key::EntryId, s(&hexfmt::encode(&id)))],
            };
            Err(out::denied(Reason::GrantFile, ms))
        }
    }
}

/// Verdict to exit code. PARTIAL has its own code, merged into neither green nor red.
fn verdict_exit(v: CheckVerdict) -> Exit {
    match v {
        CheckVerdict::Green => Exit::Affirmed,
        CheckVerdict::Partial => Exit::Partial,
        CheckVerdict::Fail => Exit::Denied,
    }
}

fn chain_check(a: &Args) -> Answer {
    // A hop is `<grant file>` or `<grant file>=<audit input file>`.
    //
    // Hops without their own input use the input assembled from `--ledger` (or `--input`) when given: a chain
    // often lives in one ledger, and requiring one identical input file per hop invites mistakes.
    let shared: Option<Value> = if a.one("ledger").is_some() || a.one("input").is_some() {
        match audit_input(a) {
            Ok(v) => Some(v),
            Err(ans) => return ans,
        }
    } else {
        None
    };
    let mut loaded: Vec<(Vec<u8>, Option<Value>)> = Vec::new();
    for spec in a.many("hop") {
        let (g, input) = match spec.split_once('=') {
            Some((g, i)) => {
                let bytes = args::slurp(i);
                match json::parse_tests_1_3(&bytes) {
                    Ok(v) => (g.to_string(), Some(v)),
                    Err(_) => out::misuse(Reason::Unreadable, out::typed(i), out::Said::NotLawJson),
                }
            }
            None => (spec.clone(), shared.clone()),
        };
        loaded.push((args::slurp(&g), input));
    }
    let hops: Vec<Hop> = loaded
        .iter()
        .map(|(g, i)| Hop {
            grant: g.as_slice(),
            input: i.clone(),
        })
        .collect();
    let value = check::chain_check(&hops, a.now());
    let exit = match value
        .member(zikaron_kit::tokens::Key::Verdict.as_str())
        .and_then(|x| x.as_str())
    {
        Some(x) if x == CheckVerdict::Green.as_str() => Exit::Affirmed,
        Some(x) if x == CheckVerdict::Partial.as_str() => Exit::Partial,
        _ => Exit::Denied,
    };
    out::verbatim(exit, value)
}

fn depth(a: &Args) -> Answer {
    let work = a.need("work");
    let outcome = match outcome_of(a, None) {
        Ok(o) => o,
        Err(ans) => return ans,
    };
    // Depth is a reading, not a verdict: a reading made is an answer. What cannot be read is written by the
    // kit core.
    out::verbatim(Exit::Affirmed, reading::depth(outcome.as_ref(), &work))
}

// Documents and payloads.

fn fpm_sign(a: &Args) -> Answer {
    let key = a.key("key");
    let author = match entry::address(&key) {
        Some(x) => x,
        None => return out::denied(Reason::Key, vec![]),
    };
    let rows = match a.one("rows") {
        Some(p) => args::slurp_json(&p),
        None => Value::Arr(Vec::new()),
    };
    let made = docs::fpm(
        &author,
        &a.one("work").unwrap_or_default(),
        a.one("grant").as_deref(),
        rows,
        &a.one("note").unwrap_or_default(),
        &key,
    );
    let bytes = made.as_ref().map(|x| x.bytes.clone()).unwrap_or_default();
    let ans = doc_answer(a, made.map(|x| (x.bytes, hexfmt::encode(&x.read.doc_id))));
    land_doc(a, ans, &bytes)
}

fn ack_sign(a: &Args) -> Answer {
    let key = a.key("key");
    let recipient = match entry::address(&key) {
        Some(x) => x,
        None => return out::denied(Reason::Key, vec![]),
    };
    // The manifest an acknowledgement refers to may be given by id or as the manifest itself. Given the
    // manifest, pairing runs here (`zikaron.kit/1` §5.3): whether the acknowledgement signs this manifest, whether
    // the signer has a row, whether the variant matches are judged by the kit core. Pairing here lets the
    // signer learn at once that the acknowledgement does not pair, before it goes out.
    let doc_path = a.one("fpm-doc");
    let fpm_bytes = doc_path.as_deref().map(args::slurp);
    let fpm_id = match (&fpm_bytes, a.one("fpm")) {
        (Some(b), None) => hexfmt::encode(&zikaron_kit::doc::doc_id(b)),
        (None, Some(x)) => x,
        _ => out::misuse(Reason::Args, out::Subject::Flags(vec!["fpm", "fpm-doc"]), out::Said::OneOf),
    };
    let made = docs::ack(
        &recipient,
        &fpm_id,
        &a.one("variant").unwrap_or_default(),
        &a.one("note").unwrap_or_default(),
        &key,
    );
    let ans = doc_answer(a, made.as_ref().map(|x| (x.bytes.clone(), hexfmt::encode(&x.read.doc_id))).map_err(clone_bad));
    let bytes = made.as_ref().map(|x| x.bytes.clone()).unwrap_or_default();
    let (Some(fpm), Ok(ack)) = (fpm_bytes, &made) else {
        return land_doc(a, ans, &bytes);
    };
    let verdict = zikaron_kit::doc::pair(&fpm, &ack.bytes).verdict();
    let decided = match ans.exit {
        Exit::Affirmed if verdict == PairVerdict::Paired => out::affirmed(pair_fields(&ans, &verdict)),
        Exit::Affirmed => out::denied(Reason::Doc, pair_fields(&ans, &verdict)),
        _ => ans,
    };
    land_doc(a, decided, &bytes)
}

/// Land only after the verdict: only an affirmative answer writes `--out` and adds the location to the
/// answer; a negative answer writes nothing. A file landed before the check could already be taken.
fn land_doc(a: &Args, ans: Answer, bytes: &[u8]) -> Answer {
    let Some(p) = a.one("out") else { return ans };
    if ans.exit != Exit::Affirmed {
        return ans;
    }
    // One way to land files (`zikaron_glue::landing`): write a temporary sibling in full, fsync, move into
    // place; an existing name is refused and nothing is overwritten. A plain `fs::write` would replace what
    // is there and could leave a truncated document that looks whole. `pack::export` and this share that
    // path, and the test suites of both crates scan for it.
    if let Err(t) = zikaron_glue::landing::land_bytes(std::path::Path::new(&p), bytes) {
        return out::denied(
            match t {
                zikaron_glue::landing::Trouble::Occupied(_) => Reason::Ledger,
                zikaron_glue::landing::Trouble::Io(..) => Reason::Ledger,
            },
            vec![(Key::Detail, said_detail(t.code(), t.said())), (Key::Path, s(t.subject()))],
        );
    }
    let Value::Obj(mut ms) = ans.value else { return ans };
    ms.push((Key::Path.as_str().to_string(), s(&p)));
    ms.sort_by(|x, y| x.0.as_bytes().cmp(y.0.as_bytes()));
    out::verbatim(Exit::Affirmed, Value::Obj(ms))
}

/// Put the assembled members back together with the pairing verdict (answer members are built in one place).
fn pair_fields(ans: &Answer, verdict: &PairVerdict) -> Vec<(Key, Value)> {
    let mut ms: Vec<(Key, Value)> = Vec::new();
    if let Value::Obj(members) = &ans.value {
        for (k, v) in members {
            if k == Key::DocId.as_str() {
                ms.push((Key::DocId, v.clone()));
            } else if k == Key::Doc.as_str() {
                ms.push((Key::Doc, v.clone()));
            }
        }
    }
    ms.push((Key::State, s(verdict.as_str())));
    ms
}

/// `Reject` is not `Copy`, and pairing needs another look at `made` after `doc_answer`: copy the refusal.
fn clone_bad(b: &docs::Bad) -> docs::Bad {
    match b {
        docs::Bad::Sign(t) => docs::Bad::Sign(*t),
        docs::Bad::Refused(r) => docs::Bad::Refused(r.clone()),
    }
}

/// Assemble a document answer. Nothing lands here: the caller lands after the verdict (see [`land_doc`]), so
/// an acknowledgement refused by pairing never reaches the caller's path.
fn doc_answer(a: &Args, made: Result<(Vec<u8>, String), docs::Bad>) -> Answer {
    let _ = a;
    match made {
        Ok((bytes, id)) => {
            let ms = vec![
                (Key::DocId, s(&id)),
                (Key::Doc, s(&String::from_utf8_lossy(&bytes))),
            ];
            out::affirmed(ms)
        }
        Err(docs::Bad::Sign(t)) => out::denied(Reason::Entry, vec![(Key::Token, s(t.as_str()))]),
        Err(docs::Bad::Refused(r)) => out::denied(
            Reason::Doc,
            vec![
                (
                    Key::Index,
                    match r.index {
                        Some(i) => Value::Int(i as u64),
                        None => Value::Null,
                    },
                ),
                (Key::Token, s(r.token.as_str())),
            ],
        ),
    }
}

fn badge_verb(a: &Args) -> Answer {
    let encode = a.many("encode");
    let decode = a.one("decode");
    if encode.is_empty() == decode.is_none() {
        out::misuse(Reason::Args, out::Subject::Flags(vec!["encode", "decode"]), out::Said::OneOf);
    }
    if !encode.is_empty() {
        let entries: Vec<Vec<u8>> = encode.iter().map(|p| args::slurp(p)).collect();
        return match badge::encode(&entries) {
            Ok(payload) => out::affirmed(vec![
                (Key::Payload, s(&payload)),
                (Key::State, s(BADGE_OK)),
            ]),
            Err(r) => out::denied(
                Reason::Badge,
                vec![
                    (
                        Key::Index,
                        match r.index {
                            Some(i) => Value::Int(i as u64),
                            None => Value::Null,
                        },
                    ),
                    (Key::Token, s(r.token.as_str())),
                ],
            ),
        };
    }
    let payload = args::slurp(&decode.unwrap_or_default());
    match badge::decode(&payload) {
        Ok(entries) => out::affirmed(vec![
            (Key::Count, Value::Int(entries.len() as u64)),
            (
                Key::Entries,
                Value::Arr(entries.iter().map(|e| s(&e.id_hex())).collect()),
            ),
            (Key::State, s(BADGE_OK)),
        ]),
        Err(r) => out::denied(
            Reason::Badge,
            vec![
                (
                    Key::Index,
                    match r.index {
                        Some(i) => Value::Int(i as u64),
                        None => Value::Null,
                    },
                ),
                (Key::Token, s(r.token.as_str())),
            ],
        ),
    }
}

// Kits and showing entries.

/// The form `--as` names a kit in (`file` or `folder`); without it, one file.
fn form_of(a: &Args) -> kitout::Form {
    match a.one("as") {
        None => kitout::Form::File,
        Some(w) => kitout::Form::of(&w).unwrap_or_else(|| out::misuse(Reason::Args, out::Subject::flag("as"), out::Said::AsWord)),
    }
}

/// `--originals`: a home's chosen records' originals go in unless it is `none`, its one word; any other word is
/// misuse. Judged here for both ways a home is named (`--ledger`, and `--home` before the desktop is asked).
pub(crate) fn originals_wanted(a: &Args) -> bool {
    match a.one("originals").as_deref() {
        None => true,
        Some("none") => false,
        Some(_) => out::misuse(Reason::Args, out::Subject::flag("originals"), out::Said::OriginalsWord),
    }
}

fn kit_export(a: &Args) -> Answer {
    let at = a.need("ledger");
    let form = form_of(a);
    let out_dir = a.need("out");
    // Whom the one file is sealed to, and where an address's key is looked for: arguments, judged before anything
    // is read.
    let at_home = crate::sealed::is_home(&at);
    // A plain ledger folder has no index of signed files: its originals are each given with `--file`.
    if !at_home && a.one("originals").is_some() {
        out::misuse(Reason::Args, out::Subject::flag("originals"), out::Said::OriginalsHomeOnly);
    }
    let with_originals = originals_wanted(a);
    let to = seal_to(a);
    let seek = seek_of(a, to, at_home);
    // A home the desktop keeps sealed is read with the credential, as every read of it is (`sealed::read`).
    let (items, home) = if at_home {
        match crate::sealed::read(&at, now_secs()) {
            Ok(r) => (r.items.clone(), Some(r)),
            Err(t) => return crate::sealed::refused(&t),
        }
    } else {
        let dir = match ledger::open(&at) {
            Ok(d) => d,
            Err(t) => return ledger_trouble(&t),
        };
        match ledger::pile(&dir) {
            Ok(x) => (x, None),
            Err(t) => return ledger_trouble(&t),
        }
    };
    // The ledger's root (its genesis entry's id): which of this machine's signed files are this ledger's.
    let root = items.iter().filter_map(|x| zikaron::entry::check(x).ok()).find(|e| e.seq == 0).map(|e| e.id_hex()).unwrap_or_default();
    let wanted = a.many("entry");
    let entries: Vec<Vec<u8>> = if wanted.is_empty() {
        items
    } else {
        // An unmatched id must not silently become "zero entries": an empty kit would get a valid seal. Ids
        // are recognized in one spelling rule (bare and `0x`-prefixed are the same id), and named ids that
        // are not found are reported as a refusal.
        let asked: Vec<String> = wanted.iter().map(|x| entry_id_form(x)).collect();
        let items_all = items.clone();
        let picked: Vec<Vec<u8>> = items
            .into_iter()
            .filter(|b| asked.contains(&hexfmt::encode(&zikaron::entry::entry_id(b))))
            .collect();
        let have: Vec<String> = picked
            .iter()
            .map(|b| hexfmt::encode(&zikaron::entry::entry_id(b)))
            .collect();
        let missing: Vec<Value> = asked
            .iter()
            .filter(|x| !have.contains(x))
            .map(|x| s(x))
            .collect();
        if !missing.is_empty() {
            return out::denied(Reason::EntryAbsent, vec![(Key::Names, Value::Arr(missing))]);
        }
        // Named choices pass the revocation closure rule too: naming a grant brings its revocations into the
        // kit. The rule lives in `zikaron_glue::select`; this calls the same selector.
        let mut picked = picked;
        let closure = zikaron_glue::select::choose(
            &items_all,
            &zikaron_glue::select::Selection { ids: asked.clone(), ..Default::default() },
        );
        for b in closure.items {
            if !picked.contains(&b) {
                picked.push(b);
            }
        }
        picked
    };
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut contents: Vec<String> = Vec::new();
    for spec in a.many("file") {
        // `<kit path>=<disk path>`: kit paths contain no `=` (`zikaron.kit/1`), so the split is at the first one.
        let (kit_path, src) = match spec.split_once('=') {
            Some(x) => x,
            None => out::misuse(Reason::Args, out::typed(&spec), out::Said::FileShape),
        };
        files.push((kit_path.to_string(), args::slurp(src)));
        contents.push(kit_path.to_string());
    }
    let mut proofs: Vec<(String, String, Vec<u8>)> = Vec::new();
    for spec in a.many("proof") {
        let mut parts = spec.splitn(3, '=');
        let (Some(kit_path), Some(tx), Some(src)) = (parts.next(), parts.next(), parts.next())
        else {
            out::misuse(Reason::Args, out::typed(&spec), out::Said::ProofShape)
        };
        proofs.push((kit_path.to_string(), tx.to_string(), args::slurp(src)));
    }
    let mut bundle = kitout::Bundle {
        entries,
        files,
        proofs,
        contents,
        root: a.one("root"),
        note: a.one("note").unwrap_or_default(),
    };
    // From a home: the chosen records' originals as the desktop takes them, and the note's anchoring line.
    let taken = match &home {
        Some(r) => match from_home(r, &root, &mut bundle, with_originals) {
            Ok(x) => Some(x),
            Err(answer) => return answer,
        },
        None => None,
    };
    // The recipient's key: given, or an address's found on chain (its own proof); every refusal leaves zero bytes
    // written.
    let key = match (to, seek) {
        (None, _) => None,
        (Some(SealTo::Key(k)), _) => Some(k),
        (Some(SealTo::Address(addr)), Some(seek)) => match key_by(seek, &addr, home.as_ref(), a.one("proxy").is_some()) {
            Ok(k) => Some(k),
            Err(answer) => return answer,
        },
        (Some(SealTo::Address(_)), None) => None,
    };
    let out_path = std::path::Path::new(&out_dir);
    let landed = match key {
        None => kitout::export(out_path, bundle, form).map_err(|t| kit_out_refused(&t)),
        // Sealed: the plain one file laid out in a folder of its own and sealed into `--out`, the sealed layer's one
        // way for both ends (`zikaron_seal::addressed::seal_out`).
        Some(k) => {
            use zikaron_seal::addressed::Sealing;
            match zikaron_seal::addressed::seal_out(&k, out_path, |plain| kitout::export(plain, bundle, kitout::Form::File)) {
                Ok((l, _)) => Ok(l),
                Err(Sealing::Lay(t)) => Err(kit_out_refused(&t)),
                Err(Sealing::Land(t)) => Err(kit_out_refused(&kitout::Trouble::from(t))),
                Err(Sealing::Refused(r)) => Err(out::denied(Reason::Ledger, vec![(Key::Detail, said_detail("E_IO", Some(r.tail()))), (Key::Path, s(&out_dir))])),
            }
        }
    };
    match landed {
        Ok(l) => {
            let written = kit_written(&l.dropped, l.entries as u64, l.files as u64, &l.kit_id, &out_dir, l.proofs as u64);
            match taken {
                Some((originals, left_out)) => written.with(vec![
                    (Key::Originals, Value::Arr(originals.iter().map(|x| s(x)).collect())),
                    (Key::LeftOut, Value::Arr(left_out.iter().map(|x| s(x)).collect())),
                ]),
                None => written,
            }
        }
        Err(answer) => answer,
    }
}

/// A kit output failure as `kit-export` answers it, read from the kit output crate's error in one place: when the
/// kit core judges a kit invalid, `state` is the `zikaron.kit/1` verdict and `detail` its subject; disk failures
/// carry `path`.
fn kit_out_refused(t: &kitout::Trouble) -> Answer {
    match (t.verdict(), t.path()) {
            (Some(v), _) => out::denied(
                Reason::Kit,
                vec![
                    (
                        Key::Detail,
                        match t.subject().split_once(':') {
                            Some((_, subj)) if !subj.is_empty() => s(subj),
                            _ => Value::Null,
                        },
                    ),
                    (Key::State, s(v)),
                ],
            ),
            (None, Some(p)) => match t.said() {
                // A disk operation failed: what the system said is in `detail`.
                Some(w) => out::denied(Reason::Ledger, vec![(Key::Detail, said_detail(t.code(), Some(w))), (Key::Path, s(p))]),
                None => out::denied(Reason::Ledger, vec![(Key::Path, s(p))]),
            },
            _ => out::denied(Reason::Ledger, vec![(Key::Detail, s(&t.subject()))]),
    }
}

/// Whom `kit-export` seals its one file to: an address (`--to`), its key looked for on chain, or a key (`--to-key`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SealTo {
    Address([u8; 20]),
    Key([u8; 33]),
}

/// Whom `kit-export` seals to, judged as arguments before anything is read, on every road (the desktop's door
/// too, `desk::read_args`): both ways given, or either with `--as folder`, is misuse in the words of the addressed
/// package's one table (`zikaron_seal::addressed::words`); an address not in its form, or a key that is not a point
/// of the curve (`zikaron_seal::addressed::key_text`, both ends' one reading), is misuse naming its flag. None given,
/// the package is not sealed.
pub(crate) fn seal_to(a: &Args) -> Option<SealTo> {
    let (to, key) = (a.one("to"), a.one("to-key"));
    let flag = match (&to, &key) {
        (None, None) => return None,
        (Some(_), Some(_)) => out::misuse(Reason::Args, out::Subject::Flags(vec!["to", "to-key"]), out::Said::AddressedBoth),
        (Some(_), None) => "to",
        (None, Some(_)) => "to-key",
    };
    if a.one("as").as_deref().and_then(kitout::Form::of) == Some(kitout::Form::Folder) {
        out::misuse(Reason::Args, out::Subject::Flags(vec!["as", flag]), out::Said::AddressedFolder);
    }
    if let Some(t) = to {
        return match zikaron_seal::identities::address(&t) {
            Some(x) => Some(SealTo::Address(x)),
            None => out::misuse(Reason::Args, out::Subject::flag("to"), out::Said::Not20),
        };
    }
    match key.as_deref().and_then(zikaron_seal::addressed::key_text) {
        Some(k) => Some(SealTo::Key(k)),
        None => out::misuse(Reason::Args, out::Subject::flag("to-key"), out::Said::AddressedKeyShape),
    }
}

/// Where `kit-export --to` looks for the address's key.
enum Seek {
    /// A home's ledger: the home's network and this machine's read-only networks, with the home's nodes and this
    /// machine's proxy (`--proxy` given goes by it), as `scan --ledger` reads.
    Home,
    /// A plain ledger folder: the networks `--endpoint` gives, each chain's registry and start block from the known
    /// deployments table.
    Given(Vec<zikaron_send::recipient::Net>),
}

/// Where an address's key is looked for, judged as arguments: `--endpoint` and `--proxy` are taken only with
/// `--to` (misuse otherwise, naming the flag); a home's ledger reads its own networks and takes no `--endpoint`; a
/// plain ledger folder needs `--endpoint`, each chain one the known deployments table names
/// (`zikaron_anchor::deploy`; another is misuse naming the value by its place). The way out is set here
/// (`--proxy`, or the system's on a plain folder; a home without `--proxy` takes this machine's when its key is
/// sought, `key_by`).
fn seek_of(a: &Args, to: Option<SealTo>, at_home: bool) -> Option<Seek> {
    let endpoints = a.many_at("endpoint");
    let proxy_given = a.one("proxy").is_some();
    if !matches!(to, Some(SealTo::Address(_))) {
        for (flag, given) in [("endpoint", !endpoints.is_empty()), ("proxy", proxy_given)] {
            if given {
                out::misuse(Reason::Args, out::Subject::flag(flag), out::Said::OnlyWithTo);
            }
        }
        return None;
    }
    if at_home {
        if !endpoints.is_empty() {
            out::misuse(Reason::Args, out::Subject::flag("endpoint"), out::Said::HomeSettingsSay);
        }
        if proxy_given {
            proxy_from(a);
        }
        return Some(Seek::Home);
    }
    let specs = chain::endpoint_specs(&endpoints);
    for (at, spec) in &endpoints {
        let known = zikaron_anchor::rpc::endpoint_spec(spec).ok().map(|(c, _)| zikaron_anchor::deploy::ROWS.iter().any(|r| r.chain_id == c));
        if known != Some(true) {
            out::misuse(Reason::Args, out::unechoed_endpoint(*at, spec), out::Said::ChainNotDeployed);
        }
    }
    let mut nets = Vec::new();
    for c in chain::chains_of(&specs) {
        let Some((row, registry)) = zikaron_anchor::deploy::ROWS.iter().find(|r| r.chain_id == c).and_then(|r| Some((r, zikaron_seal::identities::address(r.registry)?))) else {
            out::misuse(Reason::Args, out::Subject::flag("endpoint"), out::Said::ChainNotDeployed);
        };
        let nodes = specs.iter().filter(|(x, _)| *x == c).map(|(_, u)| zikaron_send::node::NodeAddr::new(u.as_str())).collect();
        nets.push(zikaron_send::recipient::Net { chain: c, registry, from_block: row.from_block, nodes });
    }
    proxy_from(a);
    Some(Seek::Given(nets))
}

/// The key behind `address`, by the sender's one judgement for both ends (`zikaron_send::recipient::key_for`): on the
/// networks given; or on a home's network (none set is a network not read) and this machine's read-only networks,
/// each past its fingerprint gate, with the home's nodes, and this machine's proxy unless `--proxy` was given (set
/// already, `seek_of`). Not found: `E_ADDRESSED` with the table's code, exit 4 when some network was not read
/// through.
fn key_by(seek: Seek, address: &[u8; 20], home: Option<&crate::sealed::Reading>, proxy_given: bool) -> Result<[u8; 33], Answer> {
    use zikaron_send::recipient::Net;
    let (nets, readonly, unread) = match (seek, home) {
        (Seek::Given(nets), _) => (nets, Vec::new(), Vec::new()),
        (Seek::Home, Some(r)) => {
            let ground = r.ground().map_err(|t| crate::sealed::refused(&t))?;
            if !proxy_given {
                let cli = r.machine_settings().map_err(|t| crate::sealed::refused(&t))?;
                zikaron_send::node::take_machine_route(cli.proxy.as_deref());
            }
            // A home's network with no chain set is no network to read (the sender's judgement counts it for nothing).
            let mut nets = Vec::new();
            if let (Some(chain), Some(registry)) = (ground.chain_id, ground.registry) {
                nets.push(Net { chain, registry, from_block: ground.from_block, nodes: crate::sealedsend::nodes_of(&ground, chain).into_iter().map(|e| e.url).collect() });
            }
            let readonly = r
                .read_networks()
                .map_err(|t| crate::sealed::refused(&t))?
                .into_iter()
                .map(|n| Net { chain: n.chain_id, registry: n.registry, from_block: n.from_block, nodes: n.nodes.iter().map(|u| zikaron_send::node::NodeAddr::new(u.as_str())).collect() })
                .collect();
            (nets, readonly, Vec::new())
        }
        (Seek::Home, None) => (Vec::new(), Vec::new(), Vec::new()),
    };
    zikaron_send::recipient::key_for(address, nets, &readonly, unread, zikaron_anchor::deploy::CODE_HASH).map_err(|(why, words)| addressed_refused(why, &words))
}

/// A record package addressed to one key, refused: `E_ADDRESSED` with the code of the addressed package's one table
/// (`token`, as the desktop spells it) and its evidence (`detail`, never a recipient), in that table's words; exit 4
/// for the one member that is no answer (`Why::unanswered`), 1 otherwise. Built here for every road this command
/// line opens or seals a package on.
pub(crate) fn addressed_refused(why: zikaron_seal::addressed::Why, evidence: &str) -> Answer {
    let members = vec![(Key::Token, s(why.code())), (Key::Detail, s(evidence))];
    let (zh, en) = why.words();
    let a = if why.unanswered() { out::unanswered(Reason::Addressed, members) } else { out::denied(Reason::Addressed, members) };
    a.telling(out::say(zh, en).to_string())
}

/// What a record package exported from a home takes besides its entries, as the desktop's export does: the
/// originals of its chosen records from this machine's index of signed files (`zikaron_seal::records`), each a
/// file still in place whose bytes match the record's `content`, named in the package as the desktop names them
/// (`zikaron_glue::kitnames`, renamed ones listed in its name table); the terms documents of its chosen grants
/// (`zikaron_seal::terms`); and the home's anchoring point as the note's last line (`zikaron_glue::anchoredon`,
/// from the home's settings). Returns the package paths of the originals taken and the paths of those left out
/// (gone, no longer the recorded bytes, or with a name that cannot be made kit-legal); both empty with
/// `with_originals` false (`--originals none`), when no original is looked for.
fn from_home(r: &crate::sealed::Reading, root: &str, b: &mut kitout::Bundle, with_originals: bool) -> Result<(Vec<String>, Vec<String>), Answer> {
    let rows = r.records().map_err(|t| crate::sealed::refused(&t))?;
    let chosen: Vec<String> = b
        .entries
        .iter()
        .filter_map(|x| zikaron::entry::check(x).ok())
        .filter(|e| zikaron_glue::recordcheck::work_of(e.kind, &e.body).is_some())
        .map(|e| e.id_hex())
        .collect();
    let mut names: Vec<(String, String)> = Vec::new();
    let (mut taken, mut left_out, mut contents): (Vec<String>, Vec<String>, Vec<String>) = (Vec::new(), Vec::new(), Vec::new());
    // Left out (`--originals none`): no chosen record is looked up, so none is taken or left out.
    let chosen = if with_originals { chosen } else { Vec::new() };
    for row in zikaron_seal::records::originals_for(&rows, root, &chosen) {
        let content = row.content.to_ascii_lowercase();
        if contents.contains(&content) {
            continue;
        }
        let bytes = match std::fs::read(&row.path) {
            Ok(x) if hexfmt::encode(&zikaron_glue::recording::content_of(&x)).eq_ignore_ascii_case(&content) => x,
            _ => {
                left_out.push(row.path.clone());
                continue;
            }
        };
        let name = std::path::Path::new(&row.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match zikaron_glue::kitnames::put_original(&name, bytes, b, &mut names) {
            Ok(inside) => {
                contents.push(content);
                taken.push(inside);
            }
            Err(_) => left_out.push(row.path.clone()),
        }
    }
    if !names.is_empty() {
        b.files.push((zikaron_glue::kitnames::NAMES_FILE.to_string(), zikaron_glue::kitnames::names_table(&names)));
    }
    // The terms documents kept when the chosen grants were signed travel with them, at their kept path, as the
    // desktop's export carries them.
    let grants: Vec<String> = b.entries.iter().filter_map(|x| zikaron::entry::check(x).ok()).filter(|e| e.kind == EntryType::Grant).map(|e| e.id_hex()).collect();
    for g in grants {
        if let Some((rel, doc)) = r.terms_of(&g).map_err(|t| crate::sealed::refused(&t))? {
            if !b.files.iter().any(|(p, _)| *p == rel) {
                b.files.push((rel, doc));
            }
        }
    }
    let ground = r.ground().map_err(|t| crate::sealed::refused(&t))?;
    if let (Some(chain_id), Some(registry)) = (ground.chain_id, ground.registry) {
        let at = zikaron_glue::anchoredon::AnchoredOn { chain_id, from_block: ground.from_block, registry: hexfmt::encode(&registry) };
        b.note = zikaron_glue::anchoredon::note_with(&b.note, Some(&at));
    }
    Ok((taken, left_out))
}

/// `kit-export`'s affirmative answer, built here only (the command line's own export and the desktop's under
/// `--home` both answer by it).
pub(crate) fn kit_written(dropped: &[String], entries: u64, files: u64, kit_id: &str, path: &str, proofs: u64) -> Answer {
    out::affirmed(vec![
        (Key::Dropped, Value::Arr(dropped.iter().map(|x| s(x)).collect())),
        (Key::Entries, Value::Int(entries)),
        (Key::Files, Value::Int(files)),
        (Key::KitId, s(kit_id)),
        (Key::Path, s(path)),
        (Key::Proofs, Value::Int(proofs)),
        (Key::State, s(KIT_OK)),
    ])
}

/// `kit-verify`: a record package, one file or a folder, record by record, with the same check the desktop's
/// verification uses (`zikaron_glue::recordcheck::records`): is each record's original the one it records
/// (`original`: `match`, `mismatch`, `missing`), and is it anchored on the chain read (`anchored`: `anchored` with
/// `firstAt`, `notAnchored`, or `unread` without `--fragment`). The package must pass the kit law first
/// (`verify_kit`; refused `E_KIT`). Every record holding is exit 0; an original that is not the one recorded is
/// `E_RECORDS` exit 1; otherwise a record not anchored, not read or without its original is `E_RECORDS` exit 3.
fn kit_verify(a: &Args) -> Answer {
    let at = a.need("kit");
    // Every argument is read before the package is read out, so no misuse exit leaves a folder behind.
    let fragment = a.one("fragment").map(|p| {
        let v = args::slurp_json(&p);
        v.member(Key::Fragment.as_str()).cloned().unwrap_or(v)
    });
    let unavailable: Vec<String> = a.many("unavailable");
    // The ways a package addressed to one key opens: this machine's identities with the credential (`--ledger`
    // naming a home the desktop keeps), or the key given (`--key`, `--key-file`); one way, not both.
    let ledger = a.one("ledger");
    let key_given = a.one("key").is_some() || a.one(args::KEY_FILE).is_some();
    if let Some(l) = &ledger {
        if key_given {
            out::misuse(Reason::Args, out::Subject::Flags(vec!["key", args::KEY_FILE]), out::Said::SealedNoKey);
        }
        if !crate::sealed::is_home(l) {
            out::misuse(Reason::Args, out::Subject::flag("ledger"), out::Said::LedgerNotHome);
        }
    }
    let key = key_given.then(|| zikaron_seal::addressed::Bytes(a.key("key")));
    // A package addressed to one key is opened first, into a plain one-file package of its own (removed when this
    // returns), by the first key offered that is its recipient; that package is then verified, and its originals
    // taken, as any other. No key opens it: refused by name, nothing opened kept.
    let given = std::path::Path::new(&at);
    let opened = if zikaron_glue::addressed::is_addressed_at(given) {
        // A seat passed over because its slot did not read may be the recipient: when no key offered opens the
        // package, its trouble is the answer, never "none of them is the recipient".
        let mut passed: Option<zikaron_seal::Trouble> = None;
        let keys: Vec<zikaron_seal::addressed::Bytes> = match (&ledger, key) {
            (Some(l), _) => match crate::sealed::identity_keys(l, now_secs()) {
                Ok((ks, p)) => {
                    passed = p;
                    ks.into_iter().map(zikaron_seal::addressed::Bytes).collect()
                }
                Err(t) => return crate::sealed::refused(&t),
            },
            (None, Some(k)) => vec![k],
            (None, None) => out::misuse(Reason::Args, out::Subject::Flags(vec!["key", "ledger", "home"]), out::Said::AddressedNeedsKey),
        };
        let lent: Vec<&dyn zikaron_seal::addressed::Recipient> = keys.iter().map(|k| k as &dyn zikaron_seal::addressed::Recipient).collect();
        match (zikaron_seal::addressed::open(given, &lent), passed) {
            (Ok(o), _) => Some(o),
            (Err(zikaron_seal::addressed::Refused::NotRecipient), Some(t)) => return crate::sealed::refused(&t),
            (Err(r), _) => return addressed_open_refused(&r),
        }
    } else {
        None
    };
    let p = opened.as_ref().map(|o| o.file.as_path()).unwrap_or(given);
    let unpacked = if zikaron_glue::container::is_container_at(p) {
        match zikaron_glue::read::unpack(p) {
            Ok(u) => Some(u),
            Err(b) => return out::denied(Reason::Kit, vec![(Key::Detail, s(&b.subject())), (Key::State, s(b.code()))]),
        }
    } else {
        None
    };
    let dir = unpacked.as_ref().map(|u| u.dir.as_path()).unwrap_or(p);
    let answer = records_of(dir, fragment.as_ref(), &unavailable);
    drop(unpacked);
    let answer = match a.one("originals-to") {
        Some(to) => originals_to(answer, p, std::path::Path::new(&to)),
        None => answer,
    };
    drop(opened);
    answer
}

/// Opening a package addressed to one key, refused: in the addressed package's one table (`addressed_refused`); a
/// disk or entropy failure, which has no code there, as unreadable with the system's words (`E_UNREADABLE`, exit 1,
/// `token` `IO`).
fn addressed_open_refused(r: &zikaron_seal::addressed::Refused) -> Answer {
    match zikaron_seal::addressed::Why::of(r) {
        Some(why) => addressed_refused(why, format!("{} {}", r.code(), r.tail()).trim()),
        None => out::denied(Reason::Unreadable, vec![(Key::Token, s(r.code())), (Key::Detail, s(r.tail()))]),
    }
}

/// `kit-verify --originals-to <folder>`: a package the kit law refuses gives nothing (its answer as it is);
/// otherwise its originals are taken out into the folder by the one way the desktop takes them
/// (`zikaron_glue::extract::take`: the package read again and judged, the matching originals placed all or nothing),
/// and the records' answer says under which names (`taken`) and where (`path`). Not taken, nothing is written and
/// the answer is `E_EXTRACT` with the refusal's code (`token`), the names it is about (`names`) and what it names
/// besides (`detail`), its words those of `zikaron_glue::extract::words`; a package that no longer passes when read
/// again is refused as the kit law refuses it.
fn originals_to(records: Answer, kit: &std::path::Path, to: &std::path::Path) -> Answer {
    use zikaron_glue::extract::Refused;
    if records.value.member(Key::Reason.as_str()).and_then(|v| v.as_str()) == Some(Reason::Kit.as_str()) {
        return records;
    }
    match zikaron_glue::extract::take(kit, to) {
        Ok((_, landed)) => with_taken(records, &landed.names, &to.display().to_string()),
        Err(Refused::Kit { verdict, subject }) => kit_refused(&verdict, subject.as_deref()),
        Err(Refused::Unpack { code, subject }) => out::denied(Reason::Kit, vec![(Key::Detail, s(&subject)), (Key::State, s(&code))]),
        Err(r) => {
            let mut ms = vec![(Key::Token, s(&r.code()))];
            if !r.names().is_empty() {
                ms.push((Key::Names, Value::Arr(r.names().iter().map(|n| s(n)).collect())));
            }
            if let Some(d) = r.detail() {
                ms.push((Key::Detail, s(&d)));
            }
            let words = r.words().map(|(zh, en)| out::say(zh, en).to_string()).unwrap_or_default();
            out::denied(Reason::Extract, ms).telling(words)
        }
    }
}

/// The record verdicts of a record package folder (see [`kit_verify`]).
fn records_of(dir: &std::path::Path, fragment: Option<&Value>, unavailable: &[String]) -> Answer {
    let kit_id = match zikaron_kit::kitdir::verify_kit(dir) {
        zikaron_kit::kitdir::KitVerdict::Ok { kit_id, .. } => hexfmt::encode(&kit_id),
        zikaron_kit::kitdir::KitVerdict::Fail { verdict, subject } => return kit_refused(verdict.as_str(), subject.as_deref()),
    };
    let items = match zikaron_glue::read::kit_entries(dir) {
        Ok(x) => x,
        Err(p) => return out::denied(Reason::Kit, vec![(Key::Detail, s(&p)), (Key::State, s(zikaron_kit::tokens::KitFailToken::Unreadable.as_str()))]),
    };
    let originals = zikaron_glue::recordcheck::originals_in_dir(dir);
    // The chain as the fragment reads it: the audit outcome of these entries under their root, as the desktop
    // reads it; none without a fragment, or when it does not assemble or audit.
    let outcome = fragment.and_then(|f| {
        let root = ledger::root_of(&items).ok()?;
        let input = zikaron_anchor::input::assemble(f, &root, &ledger::pile_hex(&items), unavailable)?;
        audit::audit_full(&input)
    });
    let read = outcome.as_ref().and_then(|o| fragment.map(|f| (o, f)));
    let rows: Vec<zikaron_glue::door::Record> = zikaron_glue::recordcheck::records(&items, &originals, read)
        .into_iter()
        .map(|r| zikaron_glue::door::Record { entry_id: r.id, content: r.content, original: r.original, chain: r.chain })
        .collect();
    records_answer(&kit_id, items.len() as u64, &rows)
}

/// A record package the kit law refuses: `E_KIT` with its verdict (`state`) and subject (`detail`). Built here for
/// both roads (the package read here, and the desktop's word through `--home`).
pub(crate) fn kit_refused(verdict: &str, subject: Option<&str>) -> Answer {
    let mut ms = vec![(Key::State, s(verdict))];
    if let Some(x) = subject {
        ms.push((Key::Detail, s(x)));
    }
    out::denied(Reason::Kit, ms)
}

/// The answer of `kit-verify` from the record facts (`zikaron_glue::recordcheck`, the one judge): the package's id,
/// how many entries it carries, each record's `entryId`, `content`, `original`, `anchored` and `firstAt`; exit 0
/// when every original matches and every record is anchored, `E_RECORDS` exit 1 when an original is not the one
/// recorded, `E_RECORDS` exit 3 otherwise. Built here for both roads (the package read here, and the desktop's facts
/// through `--home`), so the two cannot answer in two shapes.
pub(crate) fn records_answer(kit_id: &str, entries: u64, rows: &[zikaron_glue::door::Record]) -> Answer {
    use zikaron_glue::recordcheck::{OnChain, Original};
    let listed: Vec<Value> = rows
        .iter()
        .map(|r| {
            let mut ms = vec![
                (Key::EntryId.as_str().to_string(), s(&r.entry_id)),
                (Key::Content.as_str().to_string(), s(&r.content)),
                (Key::Original.as_str().to_string(), s(r.original.word())),
                (Key::Anchored.as_str().to_string(), s(r.chain.word())),
            ];
            if let Some(first_at) = r.chain.first_at() {
                ms.push((Key::FirstAt.as_str().to_string(), Value::Int(first_at)));
            }
            Value::Obj(ms)
        })
        .collect();
    let members = vec![(Key::KitId, s(kit_id)), (Key::Entries, Value::Int(entries)), (Key::Records, Value::Arr(listed))];
    if rows.iter().any(|r| r.original == Original::Mismatch) {
        out::denied(Reason::Records, members).telling(out::Said::RecordsNotAll.text().to_string())
    } else if rows.iter().all(|r| r.original == Original::Match && matches!(r.chain, OnChain::Anchored { .. })) {
        out::affirmed(members)
    } else {
        out::partial(Reason::Records, members).telling(out::Said::RecordsNotAll.text().to_string())
    }
}

/// The records' answer with the originals taken out: under which names (`taken`) and where (`path`).
pub(crate) fn with_taken(records: Answer, names: &[String], to: &str) -> Answer {
    records.with(vec![(Key::Taken, Value::Arr(names.iter().map(|n| s(n)).collect())), (Key::Path, s(to))])
}

/// The one rule for entry ids: strip at most one `0x`, then apply the storage crate's naming rule (64
/// lowercase hex; `EntryName` is its only constructor). Anything else is misuse, so `show` and `kit-export`
/// read the same bytes the same way.
pub(crate) fn entry_id_form(x: &str) -> String {
    let bare = x.strip_prefix("0x").unwrap_or(x);
    match zikaron_store::layout::EntryName::parse(bare) {
        Some(n) => format!("0x{}", n.as_str()),
        None => out::misuse(Reason::Args, out::typed(x), out::Said::EntryNotId),
    }
}

fn show(a: &Args) -> Answer {
    let bytes = match (a.one("path"), a.one("entry")) {
        (Some(p), None) => args::slurp(&p),
        (None, Some(id)) => {
            let form = entry_id_form(&id);
            // A mirror bundle, a record package (a folder or one file) or a home the desktop keeps sealed (with the
            // credential) is read by the read-only reading (`ledger::read_any`), and the entry is the one whose id
            // the core computes as `--entry`; not there is `E_ENTRY_ABSENT`.
            let at = a.need("ledger");
            let p = std::path::Path::new(&at);
            if zikaron_glue::mirror::is_bundle(p) || zikaron_glue::read::is_kit(p) || zikaron_glue::container::is_container_at(p) || zikaron_glue::addressed::is_addressed_at(p) || crate::sealed::is_home(&at) {
                let items = match ledger::read_any(&at, now_secs()) {
                    Ok(x) => x,
                    Err(r) => return read_refused(r),
                };
                match items.into_iter().find(|b| hexfmt::encode(&zikaron::entry::entry_id(b)) == form) {
                    Some(b) => b,
                    None => return out::denied(Reason::EntryAbsent, vec![(Key::EntryId, s(&form))]),
                }
            } else {
                let dir = match ledger::open(&at) {
                    Ok(d) => d,
                    Err(t) => return ledger_trouble(&t),
                };
                // The file name comes from the storage naming rule (`EntryName` is the only constructor).
                let name = match zikaron_store::layout::EntryName::parse(form.trim_start_matches("0x")) {
                    Some(n) => n,
                    None => out::misuse(Reason::Args, out::typed(&id), out::Said::EntryNotId),
                };
                match dir.read_named(&zikaron_store::layout::entry_file_name(&name)) {
                    Ok(b) => {
                        ledger::refuse_sealed(&at, std::slice::from_ref(&b));
                        b
                    }
                    Err(t) => return ledger_trouble(&t),
                }
            }
        }
        _ => out::misuse(Reason::Args, out::Subject::Flags(vec!["entry", "path"]), out::Said::OneOf),
    };
    match zikaron::entry::check(&bytes) {
        Ok(e) => out::affirmed(vec![
            (Key::Author, s(&e.author)),
            (Key::EntryId, s(&e.id_hex())),
            (Key::Prev, match &e.prev {
                Some(p) => s(p),
                None => Value::Null,
            }),
            (Key::Seq, Value::Int(e.seq)),
            (Key::EntryType, s(&e.entry_type)),
            (Key::Value, e.value.clone()),
        ]),
        Err(t) => out::denied(Reason::Entry, vec![(Key::Token, s(t.as_str()))]),
    }
}

// The command line's own contract.

/// The command line's contract as one canonical JSON value, built from the closed tables alone: the verbs and
/// each one's flags ([`ACCEPTS`]), the flags that stand for another (`args::STANDS_FOR`), the exit codes
/// ([`Exit::ALL`]) and the refusal reasons ([`Reason::ALL`]). Member names are [`Contract`] (and `ok`). It reads
/// nothing and writes nothing: a reading of the tables is an answer, exit 0.
fn contract(_a: &Args) -> Answer {
    let m = |k: Contract, v: Value| (k.as_str().to_string(), v);
    let names = |xs: &[&str]| Value::Arr(xs.iter().map(|x| s(x)).collect());
    let exits = Exit::ALL
        .iter()
        .map(|e| Value::Obj(vec![m(Contract::Code, Value::Int(e.code() as u64)), m(Contract::Name, s(e.name()))]))
        .collect();
    let reasons = Reason::ALL.iter().map(|r| s(r.as_str())).collect();
    let stands = args::STANDS_FOR
        .iter()
        .map(|(flag, of)| Value::Obj(vec![m(Contract::Flag, s(flag)), m(Contract::For, s(of))]))
        .collect();
    let verbs = ACCEPTS
        .iter()
        .map(|(name, shared, own)| {
            let mut ms = vec![m(Contract::Name, s(name)), m(Contract::Flags, names(&flags(shared, own)))];
            // A verb the desktop does under `--home`: the flags it takes beside `--home`.
            if let Some((_, home)) = HOME.iter().find(|(v, _)| v == name) {
                ms.push(m(Contract::HomeFlags, names(home)));
            }
            Value::Obj(ms)
        })
        .collect();
    out::verbatim(
        Exit::Affirmed,
        Value::Obj(vec![
            (Key::Ok.as_str().to_string(), Value::Bool(true)),
            m(Contract::Exits, Value::Arr(exits)),
            m(Contract::Reasons, Value::Arr(reasons)),
            m(Contract::StandsFor, Value::Arr(stands)),
            m(Contract::Verbs, Value::Arr(verbs)),
        ]),
    )
}

// Shared parts.

fn write_entry(a: &Args, kind: EntryType, body: Vec<(Field, Value)>) -> Answer {
    let at = a.need("ledger");
    // A home ZIKARON Desk keeps sealed: written with the command line's credential (`sealed`), the key the
    // desktop's vault keeps for the home; a key given here is misuse.
    if crate::sealed::is_home(&at) {
        if a.one("key").is_some() || a.one(args::KEY_FILE).is_some() {
            out::misuse(Reason::Args, out::Subject::Flags(vec!["key", args::KEY_FILE]), out::Said::SealedNoKey);
        }
        let now = now_secs();
        let home = match crate::sealed::open(&at, now) {
            Ok(h) => h,
            Err(t) => return crate::sealed::refused(&t),
        };
        let (seq, prev) = match next_link(a, &home.items) {
            Ok(x) => x,
            Err(answer) => return answer,
        };
        // The home's write gates, as for a send: its read-only mark, a ledger that broke, a ledger handed off.
        if let Some(refused) = crate::sealedsend::write_gates(&home) {
            return refused;
        }
        // A file recorded into the home is this machine's original of that record, indexed as the desktop indexes
        // the files it signs (`--file` of `history`: its place, its name, the content it gave).
        let original = match (kind, a.one("file")) {
            (EntryType::History, Some(p)) => body.iter().find(|(f, _)| *f == Field::Content).and_then(|(_, v)| v.as_str().map(str::to_string)).map(|content| {
                let at = std::fs::canonicalize(&p).unwrap_or_else(|_| std::path::PathBuf::from(&p));
                Original { name: at.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), path: at.display().to_string(), content }
            }),
            _ => None,
        };
        return seal_and_append(
            Landing::Sealed(&home, now, original),
            &home.items,
            &home.author.clone(),
            kind.as_str(),
            seq,
            prev.as_deref(),
            entry::shape(body),
            &home.key,
            a.one("root").as_deref(),
        );
    }
    let key = a.key("key");
    let author = match entry::address(&key) {
        Some(x) => x,
        None => return out::denied(Reason::Key, vec![]),
    };
    let dir = match ledger::open(&at) {
        Ok(d) => d,
        Err(t) => return ledger_trouble(&t),
    };
    let items = match ledger::pile(&dir) {
        Ok(x) => x,
        Err(t) => return ledger_trouble(&t),
    };
    let (seq, prev) = match next_link(a, &items) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
    seal_and_append(
        Landing::Dir(&dir),
        &items,
        &author,
        kind.as_str(),
        seq,
        prev.as_deref(),
        entry::shape(body),
        &key,
        a.one("root").as_deref(),
    )
}

/// Seconds since the epoch now (when a sealed home's credential is checked, and its queue's time).
fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Where a sealed entry lands: a ledger directory the command line keeps (its own bytes), or a home the desktop
/// keeps sealed (the sealed layer's ledger and queue, at the time given, and the original of a file recorded there).
enum Landing<'a> {
    Dir(&'a LedgerDir),
    Sealed(&'a crate::sealed::Opened, u64, Option<Original>),
}

/// A file recorded into a home: where it is, its name, the content it gave its record.
struct Original {
    path: String,
    name: String,
    content: String,
}

/// Seal one entry and land it, after the one write gate: the core audits this pile with the candidate
/// offline, and a chain finding the pile did not already have refuses the write by name (`E_WOULD_BREAK`,
/// the findings' names) with not one byte written (`ledger::new_findings`). Every entry-writing verb comes here,
/// `init` and `retract` included, after the tip and the root are asked.
fn seal_and_append(
    landing: Landing,
    items: &[Vec<u8>],
    author: &str,
    entry_type: &str,
    seq: u64,
    prev: Option<&str>,
    body: Value,
    key: &[u8; 32],
    root: Option<&str>,
) -> Answer {
    // A plain ledger the desktop adopted is written through its sealed copy from then on, never here: checked
    // against this machine's adopted list (`zikaron_seal::adopted`) before anything is written (no passcode, no
    // credential needed).
    if let Landing::Dir(_) = landing {
        if let Some(refused) = adopted_here(items) {
            return refused;
        }
    }
    // A ledger this machine handed off to another place is not written here until its handoff comes back, a plain
    // folder and a sealed home alike (`zikaron_seal::pens`).
    if let Some(refused) = handed_here(items) {
        return refused;
    }
    let sealed = match entry::seal(author, entry_type, seq, prev, body.clone(), key) {
        Ok(x) => x,
        // The core's token passes through unchanged: no translation, no regrouping. A body refusal also tells
        // people which member and which flags, on stderr only.
        Err(t) => {
            let answer = out::denied(Reason::Entry, vec![(Key::Token, s(t.as_str()))]);
            let kind = EntryType::of(entry_type);
            return match (t, zikaron::entry::body_fault(kind, &body)) {
                (zikaron::tokens::Token::BodyField, Some(member)) if body_flags(kind, member).is_some() => {
                    answer.telling(out::body_line(member, body_flags(kind, member).unwrap_or_default()))
                }
                _ => answer,
            };
        }
    };
    match ledger::new_findings(items, &sealed.bytes, root) {
        Ok(names) if names.is_empty() => {}
        Ok(names) => {
            return out::denied(
                Reason::WouldBreak,
                vec![(Key::EntryId, s(&sealed.entry.id_hex())), (Key::Names, Value::Arr(names.iter().map(|n| s(n)).collect()))],
            )
        }
        Err(r) => return out::denied(r, vec![]),
    }
    match landing {
        Landing::Dir(dir) => match ledger::append(dir, &sealed.entry, &sealed.bytes) {
            Ok(stored) => wrote(&sealed.entry.id_hex(), &dir.root().to_string_lossy(), seq, matches!(stored, Stored::Written)),
            Err(t) => ledger_trouble(&t),
        },
        Landing::Sealed(home, now, original) => {
            // The original is indexed before the entry lands: a row whose entry never landed names no chosen
            // record and is never taken; an entry landed without its row would export without its original.
            if let Some(o) = original {
                let root = items.iter().filter_map(|b| zikaron::entry::check(b).ok()).find(|e| e.seq == 0).map(|e| e.id_hex()).unwrap_or_default();
                let row = zikaron_seal::records::Row { root, content: o.content, id: sealed.entry.id_hex(), seq, name: o.name, path: o.path };
                if let Err(t) = home.index_original(row) {
                    return crate::sealed::refused(&t);
                }
            }
            match home.append(&sealed.entry.id_hex(), &sealed.bytes, now) {
                Ok(written) => wrote(&sealed.entry.id_hex(), &home.ledger_root(), seq, written),
                Err(t) => crate::sealed::refused(&t),
            }
        }
    }
}

/// The refusal for a plain ledger this machine's desktop adopted (`zikaron_seal::adopted`), by its root id; `None`
/// when it was not adopted here (or has no root yet, or this user has no machine folder). A list that does not
/// read is refused by name: nothing is written past a check that could not be made.
fn adopted_here(items: &[Vec<u8>]) -> Option<Answer> {
    let root = items.iter().filter_map(|b| zikaron::entry::check(b).ok()).find(|e| e.seq == 0)?.id;
    let (machine, _) = zikaron_os::machine::here().ok()?;
    match zikaron_seal::adopted::contains(&machine, &root) {
        Ok(false) => None,
        Ok(true) => Some(out::denied(Reason::Adopted, vec![]).telling(out::Said::Adopted.text().to_string())),
        Err(t) => Some(crate::sealed::refused(&t)),
    }
}

/// The refusal for a ledger this machine handed off (`zikaron_seal::pens`, by its root id): `E_HANDED_OFF` naming
/// the handoff; `None` when its pen is here (or it has no root yet, or this user has no machine folder). A pen
/// list that does not read is refused by name: nothing is written past a check that could not be made.
pub(crate) fn handed_here(items: &[Vec<u8>]) -> Option<Answer> {
    let root = items.iter().filter_map(|b| zikaron::entry::check(b).ok()).find(|e| e.seq == 0)?.id;
    let (machine, _) = zikaron_os::machine::here().ok()?;
    zikaron_seal::pens::may_write(&machine, &root).err().map(|t| crate::sealed::refused(&t))
}

/// An entry-writing verb's affirmative answer, built here only (the command line's own writing and the
/// desktop's under `--home` both answer by it).
pub(crate) fn wrote(entry_id: &str, ledger: &str, seq: u64, written: bool) -> Answer {
    out::affirmed(vec![(Key::EntryId, s(entry_id)), (Key::Ledger, s(ledger)), (Key::Seq, Value::Int(seq)), (Key::Written, Value::Bool(written))])
}

/// Which entry to follow: the caller's `--seq`/`--prev` when given, otherwise the core's lineage. Shared by
/// every entry-writing verb.
fn next_link(a: &Args, items: &[Vec<u8>]) -> Result<(u64, Option<String>), Answer> {
    match (a.u64_of("seq"), a.one("prev")) {
        (Some(n), Some(p)) => Ok((n, Some(p))),
        (None, None) => {
            let root = root_named(a, items)?;
            match ledger::tip(&root, items) {
                Ok((n, p)) => Ok((n, Some(p))),
                Err(r) => Err(out::denied(r, vec![])),
            }
        }
        _ => out::misuse(Reason::Args, out::Subject::Flags(vec!["seq", "prev"]), out::Said::SeqPrevPair),
    }
}

/// The root: `--root` when given, otherwise the author of the genesis entry in the pile (the core decides
/// which one that is).
fn root_named(a: &Args, items: &[Vec<u8>]) -> Result<String, Answer> {
    match a.one("root") {
        Some(x) => Ok(x),
        None => ledger::root_of(items).map_err(|r| out::denied(r, vec![])),
    }
}

/// A read-only verb's refusal of what `--ledger` names, each by its own layer: a storage refusal passed on in
/// full (code, named files, size and cap), the kit core's verdict, a mirror manifest that is not this kind, a
/// file that does not read.
fn read_refused(r: ledger::ReadRefused) -> Answer {
    match r {
        ledger::ReadRefused::Ledger(t) => ledger_trouble(&t),
        ledger::ReadRefused::Kit(verdict, subject) => {
            let mut ms = vec![(Key::State, s(verdict.as_str()))];
            if let Some(x) = subject {
                ms.push((Key::Detail, s(&x)));
            }
            out::denied(Reason::Kit, ms)
        }
        ledger::ReadRefused::Mirror(said) => out::denied(Reason::Ledger, vec![(Key::Detail, s(&said))]),
        ledger::ReadRefused::Unreadable(p) => out::misuse(Reason::Unreadable, out::typed(&p), out::Said::Unreadable),
        // A record package in one file: the layout's code is the kit's state, as a kit the kit law refused.
        ledger::ReadRefused::File(b) => out::denied(Reason::Kit, vec![(Key::Detail, s(&b.subject())), (Key::State, s(b.code()))]),
        ledger::ReadRefused::Sealed(t) => crate::sealed::refused(&t),
        // The addressed package's code; the line for people says where this command line opens it.
        ledger::ReadRefused::Addressed => {
            out::denied(Reason::Addressed, vec![(Key::Token, s(zikaron_seal::addressed::Why::Elsewhere.code())), (Key::Detail, Value::Str(String::new()))]).telling(out::Said::AddressedElsewhere.text().to_string())
        }
    }
}

/// `detail` for a refusal carrying a storage or landing code: the code, and for a failed disk operation
/// (`E_IO`) what the system said after `: ` (the operation, the path, the system's words).
fn said_detail(code: &str, said: Option<&str>) -> Value {
    match said {
        Some(w) if !w.is_empty() => s(&format!("{code}: {w}")),
        _ => s(code),
    }
}

pub(crate) fn ledger_trouble(t: &zikaron_store::codes::Trouble) -> Answer {
    let mut ms = vec![(Key::Detail, said_detail(t.code.as_str(), t.said.as_deref()))];
    if !t.names.is_empty() {
        ms.push((
            Key::Names,
            Value::Arr(t.names.iter().map(|x| s(x)).collect()),
        ));
    }
    if let Some(n) = t.size {
        ms.push((Key::Count, Value::Int(n as u64)));
    }
    out::denied(Reason::Ledger, ms)
}

fn refused(r: chain::Refused) -> Answer {
    let a = out::unanswered(r.reason, vec![(Key::Detail, s(&r.detail)), (Key::Token, s(&r.code))]);
    match r.line {
        Some(said) => a.telling(said.text().to_string()),
        None => a,
    }
}

/// Every flag a verb uses must be in [`ALL_FLAGS`] or [`MORE_FLAGS`]; the tests scan for it.
pub fn all_flag_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = ALL_FLAGS.to_vec();
    v.extend(MORE_FLAGS.iter().copied());
    v
}

pub fn is_known_flag(x: &str) -> bool {
    known(x)
}

/// A word as a flag of the closed table: that flag's own name (the table's, never the word typed), or none.
pub fn flag_named(x: &str) -> Option<&'static str> {
    ALL_FLAGS.iter().chain(MORE_FLAGS.iter()).find(|f| **f == x).copied()
}

/// The twenty-six verbs: the first column of [`ACCEPTS`], in its order.
pub const VERBS: [&str; ACCEPTS.len()] = {
    let mut names = [""; ACCEPTS.len()];
    let mut i = 0;
    while i < ACCEPTS.len() {
        names[i] = ACCEPTS[i].0;
        i += 1;
    }
    names
};
