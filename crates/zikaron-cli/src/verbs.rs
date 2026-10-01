//! The twenty-one verbs. Each does the same three steps: arrange arguments, call the public API below, render
//! the returned state on one line.
//!
//! The shell only requires the flags it needs itself (where the ledger is, whose key, where to write).
//! Members the law requires are refused by the law when missing: a `history` without `mode` or a `grant`
//! without `terms` has no such member in its body, and the core's thirteen steps return `E_BODY_FIELD`. A
//! second check here would be a second copy of the law, and copies drift apart; it also means every
//! entry-writing verb has a refusal that is the law's own token.

use crate::args::{self, Args};
use crate::chain;
use crate::codes::{Exit, Field, Key, Reason, Word};
use crate::docs;
use crate::entropy;
use crate::entry;
use crate::kitout;
use crate::ledger;
use crate::out::{self, s, Answer};
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

/// The closed set of all flag names (each spelled once; verbs pick from here).
const ALL_FLAGS: [&str; 34] = [
    "ledger", "key", "root", "seq", "prev", "statement", "content", "mark", "toolchain", "note",
    "grantee", "work", "terms", "history", "window-from", "window-to", "scope", "upstream",
    "grant", "case", "anchors", "attestor", "attestation", "author", "to", "kind", "effective",
    "subject", "endpoint", "form", "registry", "hash", "calldata", "wait-secs",
];

const MORE_FLAGS: [&str; 19] = [
    "fixture", "basis", "adoptions", "input", "fragment", "unavailable", "now", "hop", "entry",
    "path", "out", "file", "proof", "variant", "fpm", "fpm-doc", "rows", "encode", "decode",
];

fn known(name: &str) -> bool {
    ALL_FLAGS.contains(&name) || MORE_FLAGS.contains(&name)
}

// Dispatch.

pub fn run(a: &Args) -> Answer {
    crate::seam();
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
        "show" => show(a),
        other => out::misuse(Reason::Args, &format!("{other} 不是一个动词。{}", args::USAGE)),
    }
}

// Keys and ledger creation.

fn keygen(a: &Args) -> Answer {
    a.close(&[]);
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
    a.close(&flags(&["ledger", "key", "statement"], &[]));
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
    // counted, not authors or files: two genesis entries signed by one key are two ids and two roots (law §8
    // records EQUIVOCATION); one genesis saved under two file names is one id, merged by the core (law §8.1),
    // and one root;
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
        &dir,
        &author,
        EntryType::Genesis.as_str(),
        0,
        None,
        entry::shape(body),
        &key,
    )
}

// The seven entry types.

fn history(a: &Args) -> Answer {
    a.close(&flags(&WRITE, &["content", "mark", "toolchain", "note"]));
    let mut body: Vec<(Field, Value)> = Vec::new();
    if let Some(x) = a.one("content") {
        body.push((Field::Content, s(&x)));
    }
    // `mode` is a member the law requires. With both flags absent it does not appear, and the core refuses:
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

fn grant(a: &Args) -> Answer {
    a.close(&flags(
        &WRITE,
        &[
            "grantee",
            "work",
            "terms",
            "history",
            "window-from",
            "window-to",
            "scope",
            "upstream",
        ],
    ));
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
    let from = a.u64_of("window-from");
    let to = a.u64_of("window-to");
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
    // `upstream` is the member the kit reading adds to a grant body (parent law §6.10 treats it as data). The
    // shell fills in the caller's bytes; how the per-hop checks read it is the kit core's business.
    if let Some(x) = a.one("upstream") {
        body.push((Field::Upstream, s(&x)));
    }
    write_entry(a, EntryType::Grant, body)
}

fn revoke(a: &Args) -> Answer {
    a.close(&flags(&WRITE, &["grant", "case"]));
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
    a.close(&flags(&WRITE, &["anchors", "attestor", "attestation"]));
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

/// The law §6.6 cosignature: over the canonical bytes of `{adopter, anchors, prev}`, domain
/// `zikaron/1-adoption`. The preimage includes `prev` and comes from the core's `adoption_preimage`.
fn attest(a: &Args) -> Answer {
    a.close(&flags(&["key", "author", "anchors", "prev"], &[]));
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
    a.close(&flags(&WRITE, &["to", "kind", "effective", "statement"]));
    let mut body: Vec<(Field, Value)> = Vec::new();
    if let Some(x) = a.one("to") {
        body.push((Field::To, s(&x)));
    }
    // `kind` is free: any token is accepted; a whitelist here would be a closed table the law does not have.
    if let Some(x) = a.one("kind") {
        body.push((Field::Kind, s(&x)));
    }
    if let Some(x) = a.u64_of("effective") {
        body.push((Field::Effective, Value::Int(x)));
    }
    if let Some(x) = a.one("statement") {
        body.push((Field::StatementMd, s(&x)));
    }
    write_entry(a, EntryType::Succession, body)
}

fn annotate(a: &Args) -> Answer {
    a.close(&flags(&WRITE, &["subject", "note"]));
    let mut body: Vec<(Field, Value)> = Vec::new();
    if let Some(x) = a.one("subject") {
        body.push((Field::Subject, s(&x)));
    }
    if let Some(x) = a.one("note") {
        body.push((Field::NoteMd, s(&x)));
    }
    write_entry(a, EntryType::Annotation, body)
}

/// Retract a work record (the retraction convention over the open entry types of law §6.9). The type literal,
/// body keys and writing rule live in `zikaron_glue::retraction`, shared with the app: a missing or non-hex32
/// subject, one not on this ledger's lineage, not a `history`, or already deleted is refused by the
/// convention's token (`E_RETRACTION` + `token`) and the ledger is unchanged. A passing retraction goes the
/// same way as `annotate`: the core builds, signs and checks it, the storage crate lands it.
fn retract(a: &Args) -> Answer {
    use zikaron_glue::retraction as convention;
    a.close(&flags(&WRITE, &["subject", "note"]));
    let key = a.key("key");
    let author = match entry::address(&key) {
        Some(x) => x,
        None => return out::denied(Reason::Key, vec![]),
    };
    let dir = match ledger::open(&a.need("ledger")) {
        Ok(d) => d,
        Err(t) => return ledger_trouble(&t),
    };
    let items = match ledger::pile(&dir) {
        Ok(x) => x,
        Err(t) => return ledger_trouble(&t),
    };
    // Ask which entry to follow first: giving only one of `--seq` and `--prev` is misuse, and misuse goes out
    // before any answer (same order as `write_entry`).
    let (seq, prev) = match next_link(a, &items) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
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
    seal_and_append(
        &dir,
        &author,
        convention::ENTRY_TYPE,
        seq,
        prev.as_deref(),
        convention::body(&target, &a.one("note").unwrap_or_default()),
        &key,
    )
}

// Chain.

fn anchor(a: &Args) -> Answer {
    a.close(&flags(
        &["key", "endpoint", "form", "registry", "hash", "calldata"],
        &["wait-secs"],
    ));
    use zikaron_anchor::send;
    let specs = chain::endpoint_specs(&a.many("endpoint"));
    // Anchoring uses exactly one endpoint. Taking the first and silently ignoring the rest would drop the
    // others; there is no failover here, so a second url for the same chain would never be used either.
    // (Reading needs multi-endpoint agreement; that is `scan`.)
    if specs.len() != 1 {
        out::misuse(
            Reason::Args,
            &format!("anchor 只往一条链上发,而给了 {} 个 --endpoint", specs.len()),
        );
    }
    let (chain_id, url) = specs[0].clone();
    let mut ep = match zikaron_anchor::rpc::Http::new(&url) {
        Some(h) => h,
        None => out::misuse(Reason::Args, &format!("端点只认 http://host:port:{url}")),
    };
    let key = a.key("key");
    let form_raw = a.need("form");
    let form = if form_raw == Word::Registry.as_str() {
        send::Form::Registry
    } else if form_raw == Word::Bare.as_str() {
        send::Form::Bare
    } else {
        out::misuse(
            Reason::Args,
            &format!(
                "--form 只认 {} 或 {}",
                Word::Registry.as_str(),
                Word::Bare.as_str()
            ),
        )
    };
    let registry = a.one("registry").map(|x| chain::h20(&x));
    let hashes: Vec<[u8; 32]> = a.many("hash").iter().map(|x| chain::h32(x)).collect();
    let calldata = a.one("calldata").map(|x| match hexfmt::decode(&x) {
        Some(b) => b,
        None => out::misuse(Reason::Args, "--calldata 不是十六进制"),
    });
    if hashes.is_empty() && calldata.is_none() {
        out::misuse(Reason::Args, "至少要一个 --hash,或一段 --calldata");
    }
    // The caller sets the wait: a fixed number would impose one block time on every chain.
    let wait = std::time::Duration::from_secs(a.u64_of("wait-secs").unwrap_or(90));
    match send::anchor(
        &mut ep, &key, chain_id, form, registry, &hashes, calldata, wait,
    ) {
        // Only "included with status 1" is anchored (law §9.1). The other three states are named, and the
        // transaction hash is always present because the bytes were broadcast.
        Ok(sent) if sent.anchored() => {
            let bn = match sent.confirm {
                send::Confirm::Included { block_number, .. } => Value::Int(block_number),
                _ => Value::Null,
            };
            out::affirmed(vec![
                (Key::BlockNumber, bn),
                (Key::Tx, s(&hexfmt::encode(&sent.tx))),
            ])
        }
        Ok(sent) => {
            let tx = (Key::Tx, s(&hexfmt::encode(&sent.tx)));
            match &sent.confirm {
                // Included with a status other than 1: the chain answered "this failed". A negative answer.
                send::Confirm::Included { status, .. } => out::denied(
                    Reason::TxStatus,
                    vec![tx, (Key::State, Value::Int(*status))],
                ),
                // Not included by the deadline, or out of sight: the chain did not answer.
                send::Confirm::NotYet => out::unanswered(
                    Reason::TxNotYet,
                    vec![tx, (Key::Count, Value::Int(wait.as_secs()))],
                ),
                send::Confirm::Unreachable(why) => {
                    out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(why)), tx])
                }
            }
        }
        Err(e) => out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&format!("{e:?}")))]),
    }
}

fn scan(a: &Args) -> Answer {
    a.close(&flags(&["endpoint"], &["fixture", "basis", "adoptions"]));
    let fixtures = a.many("fixture");
    let endpoints_given = a.many("endpoint");
    if fixtures.is_empty() == endpoints_given.is_empty() {
        out::misuse(Reason::Args, "--fixture 与 --endpoint 之中恰要一路");
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
        let specs = chain::endpoint_specs(&endpoints_given);
        thin = chain::thin_chains(&specs);
        let basis = a.need("basis");
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

fn chains_in(path: &str) -> Vec<u64> {
    use zikaron_anchor::wire::{self, Body};
    let b = args::slurp(path);
    let Some(fx) = wire::parse(&b) else {
        out::misuse(Reason::Unreadable, &format!("{path}(不是 JSON)"))
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
    let dir = match ledger::open(&a.need("ledger")) {
        Ok(d) => d,
        Err(t) => return Err(ledger_trouble(&t)),
    };
    let items = match ledger::pile(&dir) {
        Ok(x) => x,
        Err(t) => return Err(ledger_trouble(&t)),
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
    a.close(&flags(
        &["ledger", "root"],
        &["input", "fragment", "unavailable"],
    ));
    let input = match audit_input(a) {
        Ok(v) => v,
        Err(ans) => return ans,
    };
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
    a.close(&flags(
        &["ledger", "root"],
        &["input", "fragment", "unavailable", "now", "grant"],
    ));
    let grant = args::slurp(&a.need("grant"));
    let outcome = match outcome_of(a, Some(&grant)) {
        Ok(o) => o,
        Err(ans) => return ans,
    };
    let checked = check::grant_check_with(&grant, outcome.as_ref(), a.now());
    out::verbatim(verdict_exit(checked.verdict), checked.value)
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
    a.close(&flags(
        &["ledger", "root"],
        &["hop", "now", "input", "fragment", "unavailable"],
    ));
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
                    Err(_) => out::misuse(Reason::Unreadable, i),
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
    a.close(&flags(
        &["ledger", "root", "work"],
        &["input", "fragment", "unavailable"],
    ));
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
    a.close(&flags(&["key", "work", "grant", "note"], &["rows", "out"]));
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
    a.close(&flags(&["key", "note"], &["fpm", "fpm-doc", "variant", "out"]));
    let key = a.key("key");
    let recipient = match entry::address(&key) {
        Some(x) => x,
        None => return out::denied(Reason::Key, vec![]),
    };
    // The manifest an acknowledgement refers to may be given by id or as the manifest itself. Given the
    // manifest, pairing runs here (kit law §5.3): whether the acknowledgement signs this manifest, whether
    // the signer has a row, whether the variant matches are judged by the kit core. Pairing here lets the
    // signer learn at once that the acknowledgement does not pair, before it goes out.
    let doc_path = a.one("fpm-doc");
    let fpm_bytes = doc_path.as_deref().map(args::slurp);
    let fpm_id = match (&fpm_bytes, a.one("fpm")) {
        (Some(b), None) => hexfmt::encode(&zikaron_kit::doc::doc_id(b)),
        (None, Some(x)) => x,
        _ => out::misuse(Reason::Args, "--fpm 与 --fpm-doc 之中恰要一面"),
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
                zikaron_glue::landing::Trouble::Io(_) => Reason::Ledger,
            },
            vec![(Key::Detail, s(t.code())), (Key::Path, s(t.subject()))],
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
    a.close(&flags(&[], &["encode", "decode"]));
    let encode = a.many("encode");
    let decode = a.one("decode");
    if encode.is_empty() == decode.is_none() {
        out::misuse(Reason::Args, "--encode 与 --decode 之中恰要一路");
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

fn kit_export(a: &Args) -> Answer {
    a.close(&flags(
        &["ledger", "root", "note"],
        &["out", "entry", "file", "proof"],
    ));
    let dir = match ledger::open(&a.need("ledger")) {
        Ok(d) => d,
        Err(t) => return ledger_trouble(&t),
    };
    let items = match ledger::pile(&dir) {
        Ok(x) => x,
        Err(t) => return ledger_trouble(&t),
    };
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
        // `<kit path>=<disk path>`: kit paths contain no `=` (kit law), so the split is at the first one.
        let (kit_path, src) = match spec.split_once('=') {
            Some(x) => x,
            None => out::misuse(Reason::Args, "--file 的形是 <包内路径>=<盘上的路>"),
        };
        files.push((kit_path.to_string(), args::slurp(src)));
        contents.push(kit_path.to_string());
    }
    let mut proofs: Vec<(String, String, Vec<u8>)> = Vec::new();
    for spec in a.many("proof") {
        let mut parts = spec.splitn(3, '=');
        let (Some(kit_path), Some(tx), Some(src)) = (parts.next(), parts.next(), parts.next())
        else {
            out::misuse(
                Reason::Args,
                "--proof 的形是 <包内路径>=<tx>=<盘上的路>",
            )
        };
        proofs.push((kit_path.to_string(), tx.to_string(), args::slurp(src)));
    }
    let bundle = kitout::Bundle {
        entries,
        files,
        proofs,
        contents,
        root: a.one("root"),
        note: a.one("note").unwrap_or_default(),
    };
    let out_dir = a.need("out");
    match kitout::export(std::path::Path::new(&out_dir), bundle) {
        Ok(l) => out::affirmed(vec![
            (
                Key::Dropped,
                Value::Arr(l.dropped.iter().map(|x| s(x)).collect()),
            ),
            (Key::Entries, Value::Int(l.entries as u64)),
            (Key::Files, Value::Int(l.files as u64)),
            (Key::KitId, s(&l.kit_id)),
            (Key::Path, s(&out_dir)),
            (Key::Proofs, Value::Int(l.proofs as u64)),
            (Key::State, s(KIT_OK)),
        ]),
        // When the kit core judges a kit invalid, `state` is the kit law verdict and `detail` its subject;
        // disk failures carry `path`. The fields are read from the kit output crate's error in one place.
        Err(t) => match (t.verdict(), t.path()) {
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
            (None, Some(p)) => out::denied(Reason::Ledger, vec![(Key::Path, s(p))]),
            _ => out::denied(Reason::Ledger, vec![(Key::Detail, s(&t.subject()))]),
        },
    }
}

/// The one rule for entry ids: strip at most one `0x`, then apply the storage crate's naming rule (64
/// lowercase hex; `EntryName` is its only constructor). Anything else is misuse, so `show` and `kit-export`
/// read the same bytes the same way.
fn entry_id_form(x: &str) -> String {
    let bare = x.strip_prefix("0x").unwrap_or(x);
    match zikaron_store::layout::EntryName::parse(bare) {
        Some(n) => format!("0x{}", n.as_str()),
        None => out::misuse(
            Reason::Args,
            &format!("--entry 不是六十四位小写十六进制:{x}"),
        ),
    }
}

fn show(a: &Args) -> Answer {
    a.close(&flags(&["ledger"], &["entry", "path"]));
    let bytes = match (a.one("path"), a.one("entry")) {
        (Some(p), None) => args::slurp(&p),
        (None, Some(id)) => {
            let dir = match ledger::open(&a.need("ledger")) {
                Ok(d) => d,
                Err(t) => return ledger_trouble(&t),
            };
            // The file name comes from the storage naming rule (`EntryName` is the only constructor).
            let form = entry_id_form(&id);
            let name = match zikaron_store::layout::EntryName::parse(form.trim_start_matches("0x")) {
                Some(n) => n,
                None => out::misuse(Reason::Args, &format!("--entry 不是六十四位小写十六进制:{id}")),
            };
            match dir.read_named(&zikaron_store::layout::entry_file_name(&name)) {
                Ok(b) => {
                    ledger::refuse_sealed(std::slice::from_ref(&b));
                    b
                }
                Err(t) => return ledger_trouble(&t),
            }
        }
        _ => out::misuse(Reason::Args, "--entry 与 --path 之中恰要一面"),
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

// Shared parts.

fn write_entry(a: &Args, kind: EntryType, body: Vec<(Field, Value)>) -> Answer {
    let key = a.key("key");
    let author = match entry::address(&key) {
        Some(x) => x,
        None => return out::denied(Reason::Key, vec![]),
    };
    let dir = match ledger::open(&a.need("ledger")) {
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
        &dir,
        &author,
        kind.as_str(),
        seq,
        prev.as_deref(),
        entry::shape(body),
        &key,
    )
}

fn seal_and_append(
    dir: &LedgerDir,
    author: &str,
    entry_type: &str,
    seq: u64,
    prev: Option<&str>,
    body: Value,
    key: &[u8; 32],
) -> Answer {
    let sealed = match entry::seal(author, entry_type, seq, prev, body, key) {
        Ok(x) => x,
        // The law's token passes through unchanged: no translation, no regrouping.
        Err(t) => return out::denied(Reason::Entry, vec![(Key::Token, s(t.as_str()))]),
    };
    match ledger::append(dir, &sealed.entry, &sealed.bytes) {
        Ok(stored) => out::affirmed(vec![
            (Key::EntryId, s(&sealed.entry.id_hex())),
            (Key::Ledger, s(&dir.root().to_string_lossy())),
            (Key::Seq, Value::Int(seq)),
            (
                Key::Written,
                Value::Bool(matches!(stored, Stored::Written)),
            ),
        ]),
        Err(t) => ledger_trouble(&t),
    }
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
        _ => out::misuse(Reason::Args, "--seq 与 --prev 要么都给,要么都不给"),
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

/// Pass a storage refusal on in full: code, named files, size and cap (its disclosure is not optional).
fn ledger_trouble(t: &zikaron_store::codes::Trouble) -> Answer {
    let mut ms = vec![(Key::Detail, s(t.code.as_str()))];
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
    out::unanswered(
        r.reason,
        vec![(Key::Detail, s(&r.detail)), (Key::Token, s(&r.code))],
    )
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

/// The twenty-one verbs.
pub const VERBS: [&str; 21] = [
    "keygen",
    "init",
    "history",
    "grant",
    "revoke",
    "adopt",
    "attest",
    "succeed",
    "annotate",
    "retract",
    "anchor",
    "scan",
    "audit",
    "check-grant",
    "chain-check",
    "depth",
    "fpm-sign",
    "ack-sign",
    "badge",
    "kit-export",
    "show",
];
