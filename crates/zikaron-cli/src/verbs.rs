use zikaron_glue::door::Rule;
use crate::args::{self, Args};
use crate::verbs_own::{ACCEPTS, ALL_FLAGS, MORE_FLAGS};
pub use crate::verbs_own::{BY_HAND, KEYED_TOO, REPEATED, WRITE};
use crate::chain;
use crate::codes::{Contract, Exit, Field, Key, Reason, Word};
use crate::entropy;
use crate::entry;
use crate::kitout;
use crate::ledger;
use crate::out::{self, s, Answer};
use crate::sent;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::v2::tokens::Label;
use zikaron_glue::book::{self, Kind as EntryType};
use zikaron_kit::v2::doc as badge;
use zikaron_kit::tokens::{CheckVerdict, BADGE_OK, KIT_OK};
use zikaron_store::ledger::{LedgerDir, Stored};

fn flags<'a>(base: &[&'a str], more: &[&'a str]) -> Vec<&'a str> {
    base.iter().chain(more.iter()).copied().collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Need {
    Flag(&'static str),
    OneOf(&'static [&'static str]),
    AnyOf(&'static [&'static str]),
}

impl Need {
    pub fn flags(self) -> Vec<&'static str> {
        match self {
            Need::Flag(f) => vec![f],
            Need::OneOf(fs) | Need::AnyOf(fs) => fs.to_vec(),
        }
    }
}

pub fn required(verb: &str, homed: bool) -> &'static [Need] {
    if homed && (HOME.iter().any(|(name, _)| *name == verb) || BY_HAND.contains(&verb)) {
        return &[];
    }
    ACCEPTS.iter().find(|(name, _, _, _)| *name == verb).map(|(_, _, _, need)| *need).unwrap_or(&[])
}

pub use zikaron_glue::doorverbs::HOME;

pub const HOME_FLAG: &str = "home";

pub fn accepts(verb: &str, homed: bool) -> Option<Vec<&'static str>> {
    if let (true, Some((_, own))) = (homed, HOME.iter().find(|(name, _)| *name == verb)) {
        return Some(std::iter::once(HOME_FLAG).chain(own.iter().map(|(f, _)| *f)).collect());
    }
    ACCEPTS.iter().find(|(name, _, _, _)| *name == verb).map(|(_, shared, own, _)| flags(shared, own))
}


pub(crate) fn proxy_from(a: &Args) {
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
    rest_keys_from(a);
}

pub(crate) fn rest_keys_from(a: &Args) {
    let Some(path) = a.one("rest-keys") else { return };
    use zikaron_anchor::rest::KeysFileBad;
    match zikaron_anchor::rest::keys_file(std::path::Path::new(path.trim())) {
        Ok(keys) => zikaron_anchor::rest::hold_file_keys(keys),
        Err(why) => out::misuse(
            Reason::Args,
            out::Subject::flag("rest-keys"),
            match why {
                KeysFileBad::Unreadable => out::Said::Unreadable,
                KeysFileBad::NotUtf8 => out::Said::NotUtf8,
                KeysFileBad::Shape => out::Said::FileShape,
            },
        ),
    }
}

fn known(name: &str) -> bool {
    ALL_FLAGS.contains(&name) || MORE_FLAGS.contains(&name)
}

pub fn run(a: &Args) -> Answer {
    crate::seam();
    let answer = {
        let _action = zikaron_anchor::action::open(None);
        run_in(a)
    };
    let unkept = zikaron_anchor::settled::unkept();
    if unkept.is_empty() {
        return answer;
    }
    let line = out::pick(format!("这台机器没留下这一趟读到的:{}", unkept.join(";")), format!("This machine kept none of: {}", unkept.join("; ")));
    let told = match &answer.told {
        Some(t) => format!("{t}\n{line}"),
        None => line,
    };
    answer.with(vec![(Key::Unkept, Value::Arr(unkept.iter().map(|w| s(w)).collect()))]).telling(told)
}

fn run_in(a: &Args) -> Answer {
    if let Some(allowed) = accepts(a.verb(), a.homed()) {
        a.close(&allowed, required(a.verb(), a.homed()));
    }
    if a.homed() && accepts(a.verb(), true).is_some() {
        if BY_HAND.contains(&a.verb()) {
            return crate::desk::on_desktop();
        }
        return crate::desk::ask(a);
    }
    crate::verbs_own::dispatch(a)
}

pub(crate) fn keygen(_a: &Args) -> Answer {
    let key = match entropy::bytes32() {
        Some(k) => k,
        None => return out::unanswered(Reason::Random, vec![]),
    };
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

pub(crate) fn init(a: &Args) -> Answer {
    let key = a.key("key");
    let author = match entry::address(&key) {
        Some(x) => x,
        None => return out::denied(Reason::Key, vec![]),
    };
    let at = a.need("ledger");
    let dir = match ledger::open_or_create(&at) {
        Ok(d) => d,
        Err(t) => return ledger_trouble(&t),
    };
    let survey = match dir.survey() {
        Ok(x) => x,
        Err(t) => return ledger_trouble(&t),
    };
    if !survey.items.is_empty() || !survey.skipped.is_empty() {
        let found = Value::Int((survey.items.len() + survey.skipped.len()) as u64);
        let skipped: Vec<Value> = survey.skipped.iter().map(|x| s(&x.name)).collect();
        let mut roots: Vec<(String, String)> = survey
            .items
            .iter()
            .filter_map(|b| book::check(b).ok())
            .filter(|e| e.seq == 0)
            .map(|e| (e.id_hex(), e.author))
            .collect();
        roots.sort();
        roots.dedup_by(|a, b| a.0 == b.0);
        if let Ok(l) = book::read(&survey.items, &book::empty_fragment()) {
            if matches!(l.form, book::Form::Continued | book::Form::Pending) {
                return out::denied(Reason::AlreadyRooted, vec![(Key::Author, s(&l.root)), (Key::Count, found)]);
            }
        }
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
    if crate::sealed::is_home(&at) {
        return crate::desk::on_desktop();
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

pub(crate) fn history(a: &Args) -> Answer {
    let mut body: Vec<(Field, Value)> = Vec::new();
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
        (EntryType::Annotation, "subject") => one("--subject"),
        (EntryType::Annotation, "note_md") => one("--note"),
        (EntryType::Retraction, "subject") => one("--subject"),
        (EntryType::Retraction, "note_md") => one("--note"),
        _ => None,
    }
}

pub(crate) fn grant(a: &Args) -> Answer {
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
    if let Some(x) = a.one("upstream") {
        body.push((Field::Upstream, s(&x)));
    }
    if let Some(x) = a.one("supersedes") {
        body.push((Field::Supersedes, s(&x.trim().to_ascii_lowercase())));
    }
    write_entry(a, EntryType::Grant, body)
}

pub(crate) fn revoke(a: &Args) -> Answer {
    let mut body: Vec<(Field, Value)> = Vec::new();
    if let Some(x) = a.one("grant") {
        body.push((Field::Grant, s(&x)));
    }
    if let Some(x) = a.one("case") {
        body.push((Field::Case, s(&x)));
    }
    write_entry(a, EntryType::Revocation, body)
}

pub(crate) fn annotate(a: &Args) -> Answer {
    let mut body: Vec<(Field, Value)> = Vec::new();
    if let Some(x) = a.one("subject") {
        body.push((Field::Subject, s(&x)));
    }
    if let Some(x) = a.one("note") {
        body.push((Field::NoteMd, s(&x)));
    }
    write_entry(a, EntryType::Annotation, body)
}

pub(crate) fn retract(a: &Args) -> Answer {
    use zikaron_glue::retraction as convention;
    let at = a.need("ledger");
    let mut link = None;
    let home = if crate::sealed::is_home(&at) {
        if a.one("key").is_some() || a.one(args::KEY_FILE).is_some() {
            out::misuse(Reason::Args, out::Subject::Flags(vec!["key", args::KEY_FILE]), out::Said::SealedNoKey);
        }
        let now = now_secs();
        match crate::sealedsend::open_to_write(&at, now, |h| next_link(a, &h.items)) {
            Ok((h, l)) => {
                link = Some(l);
                Some((h, now))
            }
            Err(answer) => return answer,
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
    let (seq, prev) = match link.map_or_else(|| next_link(a, &items), Ok) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
    let root = match root_named(a, &items) {
        Ok(x) => x,
        Err(answer) => return answer,
    };
    let lines: Vec<convention::Line> = match ledger::entries(&root, &items) {
        Ok(es) => convention::lines_of(&es),
        Err(r) => return out::denied(r, vec![]),
    };
    let target = match convention::may_retract(&lines, &a.one("subject").unwrap_or_default()) {
        Ok(id) => id,
        Err(why) => return rule_refused(Rule::Retraction, Some(why.token()), None, None),
    };
    let then = match &home {
        Some((h, _)) => match h.retracting(&target) {
            Ok(Some(plan)) => Then::Retracted(target.clone(), plan),
            Ok(None) => return rule_refused(Rule::Retraction, Some(zikaron_glue::door::RETRACT_NOT_AUDITED), Some(&target), None),
            Err(t) => return crate::sealed::refused(&t),
        },
        None => Then::Nothing,
    };
    let landing = match (&home, &dir) {
        (Some((h, now)), _) => Landing::Sealed(h, *now, then),
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

pub(crate) fn migrate(a: &Args) -> Answer {
    let at = a.need("ledger");
    if crate::sealed::is_home(&at) {
        return crate::desk::on_desktop();
    }
    let key = a.key("key");
    let author = match entry::address(&key) {
        Some(x) => x,
        None => return out::denied(Reason::Key, vec![]),
    };
    let effective = a.within_ceiling("effective").unwrap_or_else(now_secs);
    let dir = match ledger::open(&at) {
        Ok(d) => d,
        Err(t) => return ledger_trouble(&t),
    };
    let items = match ledger::pile(&dir) {
        Ok(x) => x,
        Err(t) => return ledger_trouble(&t),
    };
    if let Some(refused) = adopted_here(&items).or_else(|| handed_here(&items)) {
        return refused;
    }
    let root = match ledger::root_of(&items) {
        Ok(r) => r,
        Err(r) => return out::denied(r, vec![]),
    };
    let read = match ledger::read(&root, &items) {
        Ok(r) => r,
        Err(r) => return out::denied(r, vec![]),
    };
    let statement = a.one("statement");
    let mut items = items;
    let (closing, body) = match book::next_move(&read, &author, effective, statement.as_deref()) {
        Err(book::NoMove::Moved) => return out::denied(Reason::AlreadyMoved, vec![]),
        Err(book::NoMove::TipAbsent) => return out::denied(Reason::TipAbsent, vec![]),
        Err(book::NoMove::TipForked) => return out::denied(Reason::TipForked, vec![]),
        Ok(book::Move::Genesis { closing, body }) => (closing, body),
        Ok(book::Move::Close { seq, prev, body }) => {
            let sealed = match entry::seal_under(book::Law::One, &author, EntryType::Succession.as_str(), seq, Some(&prev), body, &key) {
                Ok(x) => x,
                Err(t) => return out::denied(Reason::Entry, vec![(Key::Token, s(t.as_str()))]),
            };
            if let Some(refused) = breaks(&items, &sealed, &root) {
                return refused;
            }
            if let Err(t) = ledger::append(&dir, &sealed.entry, &sealed.bytes) {
                return ledger_trouble(&t);
            }
            items.push(sealed.bytes.clone());
            let id = sealed.entry.id_hex();
            (id.clone(), book::genesis_for(&read, &id, statement.as_deref()))
        }
    };
    let old = &read.entries;
    let genesis = match entry::seal(&author, EntryType::Genesis.as_str(), 0, None, body, &key) {
        Ok(x) => x,
        Err(t) => return out::denied(Reason::Entry, vec![(Key::Token, s(t.as_str()))]),
    };
    if let Some(refused) = breaks(&items, &genesis, &root) {
        return refused;
    }
    if let Err(t) = ledger::append(&dir, &genesis.entry, &genesis.bytes) {
        return ledger_trouble(&t);
    }
    let n = old.len().to_string();
    let (c, g) = (closing.clone(), genesis.entry.id_hex());
    let order = [
        out::pick(format!("1. 旧账本 {n} 条条目上链"), format!("1. Anchor the {n} old entries")),
        out::pick(format!("2. 链头时刻晚于上一条后,上链继任 {c}"), format!("2. Once the chain's head is later than the last, anchor the succession {c}")),
        out::pick(format!("3. 链头时刻晚于继任后,上链创世 {g}"), format!("3. Once the chain's head is later than the succession, anchor the genesis {g}")),
    ];
    out::affirmed(vec![
        (Key::Closing, s(&c)),
        (Key::Entries, Value::Int(old.len() as u64)),
        (Key::Continuing, s(&g)),
        (Key::Ledger, s(&dir.root().to_string_lossy())),
    ])
    .telling(order.join("\n"))
}

fn breaks(items: &[Vec<u8>], sealed: &entry::Sealed, root: &str) -> Option<Answer> {
    match ledger::new_findings(items, &sealed.bytes, Some(root)) {
        Ok(names) if names.is_empty() => None,
        Ok(names) => Some(would_break(&sealed.entry.id_hex(), &names)),
        Err(b) => Some(barred(b, &sealed.entry.id_hex())),
    }
}

pub(crate) fn barred(b: book::Barred, entry_id: &str) -> Answer {
    match b {
        book::Barred::Tip(t) => out::denied(ledger::reason_of(t), vec![]),
        book::Barred::Signer { counted, key, office } => out::denied(
            Reason::WouldBreak,
            vec![
                (Key::Token, s(counted.as_str())),
                (Key::EntryId, s(entry_id)),
                (Key::Names, Value::Arr(Vec::new())),
                (Key::Detail, s(&format!("key={key} office={office}"))),
            ],
        )
        .telling(out::pick(
            "所给之钥写的条目不算这本账的(它不在这本账的世系里),没有写;用这本账的现行钥".to_string(),
            "Entries signed by the key given would not count for this ledger (it is outside the ledger's lineage); nothing was written. Use the ledger's key in office".to_string(),
        )),
    }
}

pub(crate) fn anchor(a: &Args) -> Answer {
    let mut folder: Option<Folder> = None;
    if let Some(at) = a.one("ledger").filter(|at| !crate::sealed::is_home(at)) {
        match folder_batch(a, &at) {
            Ok(f) => folder = Some(f),
            Err(answer) => return answer,
        }
    } else if let Some(at) = a.one("ledger") {
        if a.one("key").is_some() || a.one(args::KEY_FILE).is_some() {
            out::misuse(Reason::Args, out::Subject::Flags(vec!["key", args::KEY_FILE]), out::Said::SealedNoKey);
        }
        for f in ["endpoint", "form", "registry", "hash", "calldata", "proxy"] {
            if !a.many(f).is_empty() {
                out::misuse(Reason::Args, out::Subject::flag(f), out::Said::HomeSettingsSay);
            }
        }
        rest_keys_from(a);
        return crate::sealedsend::anchor(&at, std::time::Duration::from_secs(a.u64_of("wait-secs").unwrap_or(90)));
    }
    proxy_from(a);
    use zikaron_anchor::send;
    let specs = chain::endpoint_specs(&a.many_at("endpoint"));
    let chain_id = specs[0].0;
    if specs.iter().any(|(c, _)| *c != chain_id) {
        out::misuse(Reason::Args, out::Subject::flag("endpoint"), out::Said::OneEndpoint);
    }
    let mut table: Option<Vec<zikaron_send::node::Endpoint>> = None;
    let mut node: Box<dyn zikaron_anchor::rpc::Endpoint + Send> = match &specs[..] {
        [(_, url)] => match zikaron_anchor::rpc::Http::new(url) {
            _ if zikaron_anchor::rest::of_lane(url).is_some() => match zikaron_send::node::endpoint_at(url) {
                Some(h) => h,
                None => out::misuse(Reason::Args, out::Subject::flag("endpoint"), out::Said::RestRowBase),
            },
            Some(h) => zikaron_anchor::endpoints::remembering(url, Box::new(h)),
            None => out::misuse(Reason::Args, out::Subject::flag("endpoint"), out::address_said(url)),
        },
        _ => {
            let eps: Vec<zikaron_send::node::Endpoint> = specs.iter().map(|(c, u)| zikaron_send::node::Endpoint::at(*c, zikaron_send::node::NodeAddr::new(u.clone()))).collect();
            match zikaron_send::node::serving(&eps, chain_id) {
                Ok((kept, _)) => {
                    table = Some(kept.clone());
                    Box::new(zikaron_send::node::Table::new(kept, chain_id))
                }
                Err(t) => return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&crate::sealedsend::said(&t.why))), (Key::Token, s(t.code.as_str()))]),
            }
        }
    };
    let ep: &mut dyn zikaron_anchor::rpc::Endpoint = node.as_mut();
    let by_hashes = folder.as_ref().is_some_and(|f| f.carried.is_some());
    let (key, form, registry, hashes, calldata) = match folder {
        Some(f) => (f.key, send::Form::Registry, Some(f.registry), f.hashes, f.carried.as_deref().map(send::carried_calldata)),
        None => {
            let key = a.key("key");
            let form_raw = a.need("form");
            let form = if form_raw == Word::Registry.as_str() {
                send::Form::Registry
            } else {
                out::misuse(Reason::Args, out::typed(&form_raw), out::Said::FormWord)
            };
            let registry = a.one("registry").map(|x| chain::h20(&x));
            if registry.is_none() {
                out::misuse(Reason::Args, out::Subject::flag("registry"), out::Said::Missing);
            }
            let hashes: Vec<[u8; 32]> = a.many("hash").iter().map(|x| chain::h32(x)).collect();
            let calldata = a.one("calldata").map(|x| match hexfmt::decode(&x) {
                Some(b) => b,
                None => out::misuse(Reason::Args, out::Subject::flag("calldata"), out::Said::NotHex),
            });
            if hashes.is_empty() && calldata.is_none() {
                out::misuse(Reason::Args, out::Subject::flag("hash"), out::Said::NeedHashOrCalldata);
            }
            (key, form, registry, hashes, calldata)
        }
    };
    let wait = std::time::Duration::from_secs(a.u64_of("wait-secs").unwrap_or(90));
    let Some(from) = zikaron::cryptox::address_of_privkey(&key) else { out::misuse(Reason::Key, out::Subject::flag("key"), out::Said::KeyOutOfRange) };
    let dir = match send::target(form, registry, &hashes, calldata.clone(), from) {
        Ok((to, data)) => sent::machine().map(|m| sent::place(&m, chain_id, &from, &to, &data)),
        Err(t) => Err(format!("{t:?}")),
    };
    let dir = match dir {
        Ok(d) => d,
        Err(said) => return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&said))]),
    };
    let seal = sent::Seal::of(&key, &dir);
    if let (Some(reg), Ok(m)) = (registry, sent::machine()) {
        zikaron_anchor::action::keep_in(std::sync::Arc::new(sent::logs_at(&m, chain_id, &from, &reg, &key)));
    }
    let before = match sent::read(&dir, &seal) {
        Ok(x) => x,
        Err(said) => return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&said))]),
    };
    zikaron_anchor::ahead::ask_ahead(&mut *ep, &[("eth_chainId", Value::Arr(Vec::new())), ("eth_blockNumber", Value::Arr(Vec::new()))]);
    let served = zikaron_anchor::patience::ask(&mut *ep, "eth_chainId", &Value::Arr(Vec::new())).ok();
    if zikaron_anchor::scan::serves_another(served.as_ref().and_then(zikaron_anchor::scan::chain_served), chain_id).is_some() {
        let r = zikaron_anchor::scan::Refusal::ChainIdMismatch { declared: chain_id, served: served.as_ref().map(zikaron_anchor::wire::write).unwrap_or_default() };
        return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&r.detail())), (Key::Token, s(r.code()))]);
    }
    let answered = |tx: &[u8; 32], confirm: &send::Confirm| -> Answer {
        if matches!(confirm, send::Confirm::Included { status, .. } if *status != 1) {
            let _ = sent::read(&dir, &seal).and_then(|held| sent::void(&dir, &seal, &held));
        }
        sent_answer(tx, confirm, wait)
    };
    let mut nonce = None;
    if !before.is_empty() {
        match send::earlier(&mut *ep, &from, &before) {
            send::Earlier::Included { tx, status, block_number } => return answered(&tx, &send::Confirm::Included { status, block_number }),
            send::Earlier::Held { tx } => {
                let c = send::confirm(&mut *ep, &tx, wait);
                return answered(&tx, &c);
            }
            send::Earlier::Unread { tx, said } => return tx_unheard(&hexfmt::encode(&tx), &said),
            send::Earlier::Unused { nonce: n } => nonce = Some(n),
            send::Earlier::Void => {
                if let Err(said) = sent::void(&dir, &seal, &before) {
                    return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&said))]);
                }
                let last = before.last().map(|(_, h)| hexfmt::encode(h)).unwrap_or_default();
                return tx_void(&last);
            }
        }
    }
    let mut latest = None;
    if let (send::Form::Registry, Some(reg), true, Some(eps)) = (form, registry, calldata.is_none() || by_hashes, table.as_ref()) {
        let urls: Vec<zikaron_send::node::NodeAddr> = eps.iter().map(|e| e.url.clone()).collect();
        match zikaron_send::batch::on_chain(&urls, chain_id, &reg, &from, &hashes, zikaron_anchor::deploy::start_of(chain_id, &reg), &[]) {
            Ok(l) => latest = l,
            Err(t) => match &t.why {
                zikaron_send::Why::OnChain { names, tx, block } => return on_chain(names, tx, *block),
                _ => return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&crate::sealedsend::said(&t.why)))]),
            },
        }
    } else if let (send::Form::Registry, Some(reg), true) = (form, registry, calldata.is_none() || by_hashes) {
        match send::held(&mut *ep, chain_id, &zikaron_anchor::revoke::registries(chain_id, &[reg]), &from, &hashes, zikaron_anchor::deploy::start_of(chain_id, &reg)) {
            Ok(h) if !h.found.is_empty() => {
                let first = h.found[0];
                let names: Vec<String> = h.found.iter().map(|f| hexfmt::encode(&f.hash)).collect();
                return on_chain(&names, &hexfmt::encode(&first.tx), first.block);
            }
            Ok(h) => latest = h.latest,
            Err(not) => return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&not.evidence()))]),
        }
    }
    if let Some(block) = latest {
        let after = match send::block_time(&mut *ep, block) {
            Ok(t) => t,
            Err(t) => return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&format!("eth_getBlockByNumber {t:?}")))]),
        };
        let head = || match &table {
            Some(eps) => zikaron_send::node::head_time_now(eps, chain_id).map_err(|t| crate::sealedsend::said(&t.why)),
            None => send::head_of(&mut *ep).and_then(|h| send::block_time(&mut *ep, h)).map_err(|t| format!("{t:?}")),
        };
        match zikaron_send::batch::await_fence(&[(chain_id, after)], head) {
            Ok(()) => {}
            Err(zikaron_send::batch::Fenced::Unread(t)) => return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&t))]),
            Err(zikaron_send::batch::Fenced::TooEarly { head_time, after }) => return too_early(&fence_detail(chain_id, head_time, after)),
        }
    }
    let mut land = |n: u64, tx: &[u8; 32]| sent::land(&dir, &seal, n, tx);
    let mut refused: Option<Result<(zikaron_send::Trouble, [u8; 20]), String>> = None;
    let mut funds = |ep: &mut dyn zikaron_anchor::rpc::Endpoint, need: u128, from: &[u8; 20]| {
        let params = Value::Arr(vec![Value::Str(hexfmt::encode(from)), Value::Str("latest".into())]);
        match zikaron_anchor::patience::ask(ep, "eth_getBalance", &params) {
            Ok(v) => match v.as_str().and_then(zikaron_send::node::wei) {
                Some(have) => zikaron_send::batch::funds_check(need, have).map_err(|t| refused = Some(Ok((t, *from)))),
                None => {
                    refused = Some(Err(format!("eth_getBalance: {}", zikaron_anchor::wire::write(&v))));
                    Err(())
                }
            },
            Err(t) => {
                refused = Some(Err(format!("eth_getBalance: {t:?}")));
                Err(())
            }
        }
    };
    let went = send::anchor_landed(&mut *ep, &key, chain_id, form, registry, &hashes, calldata, wait, nonce, &mut land, &mut funds);
    match went {
        Ok((sent, _limit)) => answered(&sent.tx, &sent.confirm),
        Err(send::NotSent::Gas(no)) => match no {
            send::NoGas::Refused(t) => {
                let why = match &t {
                    zikaron_anchor::rpc::Trouble::Node(e) if zikaron_anchor::said::refusal_of(e) == zikaron_anchor::said::Refusal::Reverted => {
                        zikaron_anchor::said::reverted(e).map(|r| format!(" \u{b7} {}", r.evidence())).unwrap_or_default()
                    }
                    _ => String::new(),
                };
                out::denied(Reason::GasRefused, vec![(Key::Detail, s(&format!("{t:?}{why}")))])
            }
            send::NoGas::OverCap(n) => out::denied(Reason::GasRefused, vec![(Key::Detail, s(&format!("{n} > {}", send::GAS_LIMIT)))]),
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
        Err(send::NotSent::Nonce(v)) => {
            let said = v.map(|v| String::from_utf8_lossy(&zikaron::json::canon_bytes(&v)).into_owned()).unwrap_or_default();
            out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&format!("eth_getTransactionCount: {said}")))])
        }
        Err(send::NotSent::Land(said)) => out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&said))]),
        Err(send::NotSent::Funds) => match refused {
            Some(Ok((t, from))) => {
                let detail = format!("{}, key {}", crate::sealedsend::said(&t.why), hexfmt::encode(&from));
                crate::sealedsend::send_refused(t.code.as_str(), &detail, false, out::Said::SendRefused.text().to_string())
            }
            Some(Err(said)) => out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&said))]),
            None => out::unanswered(Reason::Unreachable, vec![(Key::Detail, s("eth_getBalance"))]),
        },
        Err(send::NotSent::Broadcast(tx, e)) => tx_unheard(&hexfmt::encode(&tx), &format!("{e:?}")),
    }
}

struct Folder {
    key: [u8; 32],
    registry: [u8; 20],
    hashes: Vec<[u8; 32]>,
    carried: Option<Vec<u8>>,
}

fn folder_batch(a: &Args, at: &str) -> Result<Folder, Answer> {
    for f in ["form", "registry", "hash", "calldata"] {
        if !a.many(f).is_empty() {
            out::misuse(Reason::Args, out::Subject::flag(f), out::Said::AnchorFolderSays);
        }
    }
    let key = a.key("key");
    let raw = a.many_at("endpoint");
    if raw.is_empty() {
        out::misuse(Reason::Args, out::Subject::flag("endpoint"), out::Said::Missing);
    }
    let specs = chain::endpoint_specs(&raw);
    let chain_id = specs[0].0;
    if specs.iter().any(|(c, _)| *c != chain_id) {
        out::misuse(Reason::Args, out::Subject::flag("endpoint"), out::Said::OneEndpoint);
    }
    let Some((row, registry)) = zikaron_anchor::deploy::ROWS.iter().find(|r| r.chain_id == chain_id).and_then(|r| Some((r, zikaron_seal::identities::address(r.registry)?))) else {
        out::misuse(Reason::Args, out::Subject::flag("endpoint"), out::Said::ChainNotDeployed);
    };
    let items = match ledger::open(at).and_then(|d| ledger::pile(&d)) {
        Ok(x) => x,
        Err(t) => return Err(ledger_trouble(&t)),
    };
    proxy_from(a);
    let eps: Vec<zikaron_send::node::Endpoint> = specs.iter().map(|(c, u)| zikaron_send::node::Endpoint::at(*c, zikaron_send::node::NodeAddr::new(u.as_str()))).collect();
    let (head, _) = zikaron_send::node::head_block(&eps, chain_id).map_err(|t| crate::sealedsend::chain_refused(&t, at))?;
    let senders = zikaron_glue::basis::senders_of(&items);
    let regs = zikaron_glue::notices::registries_of(chain_id, &hexfmt::encode(&registry));
    let window = zikaron_glue::basis::Window { chain: chain_id, registries: &regs, from_block: row.from_block, to_block: head.max(row.from_block), senders: &senders };
    let basis = json::canon_bytes(&zikaron_glue::basis::basis_of(&window));
    let scanned = match crate::hopin::bare_record() {
        Some(record) => zikaron_send::agree::scan(&eps, &basis, &record),
        None => zikaron_send::agree::scan(&eps, &basis, &zikaron_send::Bare),
    };
    let fragment = match scanned {
        Ok(zikaron_send::agree::Scan::Basis(got)) => got.fragment,
        Ok(zikaron_send::agree::Scan::NoLabel { report, .. }) => return Err(out::verbatim(Exit::Denied, report)),
        Err(t) => match (&t.code, &t.why) {
            (zikaron_send::Code::ScanRefused, zikaron_send::Why::Scan { refused, .. }) => return Err(scan_unanswered(refused)),
            _ => return Err(crate::sealedsend::chain_refused(&t, at)),
        },
    };
    let read = match book::read(&items, &fragment) {
        Ok(x) => x,
        Err(book::Unread::Root(_)) => return Err(out::denied(Reason::TipAbsent, vec![])),
        Err(_) => return Err(out::verbatim(Exit::Denied, zikaron::v2::audit::no_label())),
    };
    let unsent = book::unanchored_in(&read, chain_id);
    if unsent.is_empty() {
        return Err(held_already(&fragment));
    }
    let sender = entry::address(&key).unwrap_or_default();
    let counted = book::sender_now(&read, &sender);
    if counted != book::Sender::Counted {
        let office = read.office.clone().unwrap_or_default();
        let detail = format!("key={sender} office={office}");
        return Err(crate::sealedsend::send_refused(counted.as_str(), &detail, false, out::pick(
            "所给之钥发的锚不算这本账的(它不在这本账的世系里,或已交出席位),没有发;用这本账的现行钥".to_string(),
            "Anchors sent by the key given would not count for this ledger (it is outside the ledger's lineage, or has handed the seat on); nothing was sent. Use the ledger's key in office".to_string(),
        )));
    }
    let places = zikaron_seal::queue::places_of(&items);
    let queue = zikaron_seal::queue::Queue {
        chain: chain_id,
        items: unsent.iter().enumerate().map(|(i, id)| zikaron_seal::queue::Queued { id: id.clone(), at: i as u64, step: zikaron_seal::queue::Step::Queued, chain: chain_id }).collect(),
        ..Default::default()
    };
    let (_, ids) = queue.next_batch(usize::MAX, |id| places.get(&id.to_ascii_lowercase()).copied()).unwrap_or_default();
    let carried = zikaron_send::batch::carried_of(&items, &ids, chain_id);
    if let Some(e) = &carried {
        let urls: Vec<zikaron_send::node::NodeAddr> = eps.iter().map(|x| x.url.clone()).collect();
        let from = zikaron::cryptox::address_of_privkey(&key).unwrap_or([0; 20]);
        if let Err(t) = zikaron_send::batch::face_gate(&urls, chain_id, &registry, e, &items, &from) {
            return Err(crate::named::of_trouble(zikaron_glue::door::Own::SendRefused, &t, at));
        }
    }
    Ok(Folder { key, registry, hashes: ids.iter().map(|x| chain::h32(x)).collect(), carried })
}

fn held_already(fragment: &Value) -> Answer {
    let mut rows: Vec<(u64, String, String)> = fragment
        .member("anchors")
        .and_then(Value::as_arr)
        .map(|rs| rs.iter().filter_map(|r| Some((r.member("blockNumber").and_then(Value::as_int)?, r.member("hash").and_then(Value::as_str)?.to_string(), r.member("tx").and_then(Value::as_str)?.to_string()))).collect())
        .unwrap_or_default();
    rows.sort();
    let names: Vec<String> = rows.iter().map(|(_, h, _)| h.clone()).collect();
    let (block, tx) = rows.first().map(|(b, _, t)| (*b, t.clone())).unwrap_or_default();
    on_chain(&names, &tx, block)
}

fn sent_answer(tx: &[u8; 32], confirm: &zikaron_anchor::send::Confirm, wait: std::time::Duration) -> Answer {
    use zikaron_anchor::send::Confirm;
    let tx = hexfmt::encode(tx);
    match confirm {
        Confirm::Included { status: 1, block_number } => anchored(&tx, Some(*block_number)),
        Confirm::Included { status, .. } => tx_failed(&tx, *status),
        Confirm::NotYet | Confirm::Split { .. } => tx_not_yet(&tx, wait.as_secs()),
        Confirm::Unreachable(why) => tx_unheard(&tx, why),
    }
}

pub(crate) fn on_chain(names: &[String], tx: &str, block: u64) -> Answer {
    out::denied(Reason::OnChain, vec![(Key::BlockNumber, Value::Int(block)), (Key::Names, Value::Arr(names.iter().map(|n| s(n)).collect())), (Key::Tx, s(tx))])
        .telling(out::Said::OnChain.text().to_string())
}

pub(crate) fn too_early(detail: &str) -> Answer {
    out::unanswered(Reason::TooEarly, vec![(Key::Detail, s(detail))]).telling(out::pick(
        "链头时刻还没过这把钥上一笔锚的块时刻,这一次没有发;过一块再发".to_string(),
        "The chain has not yet passed the block time of this key's last anchor; nothing was sent. Send again after the next block".to_string(),
    ))
}

pub(crate) fn fence_detail(chain: u64, head_time: u64, after: u64) -> String {
    zikaron_send::said(&zikaron_send::Why::TooEarly { chain, head_time, after })
}

pub(crate) fn queued(count: u64) -> Answer {
    out::partial(Reason::Queued, vec![(Key::Count, Value::Int(count))]).telling(out::Said::Queued.text().to_string())
}

pub(crate) fn anchored(tx: &str, block: Option<u64>) -> Answer {
    out::affirmed(vec![(Key::BlockNumber, block.map(Value::Int).unwrap_or(Value::Null)), (Key::Tx, s(tx))])
}

pub(crate) fn tx_failed(tx: &str, status: u64) -> Answer {
    out::denied(Reason::TxStatus, vec![(Key::Tx, s(tx)), (Key::State, Value::Int(status))])
}

pub(crate) fn tx_not_yet(tx: &str, waited: u64) -> Answer {
    out::unanswered(Reason::TxNotYet, vec![(Key::Tx, s(tx)), (Key::Count, Value::Int(waited))])
}

pub(crate) fn tx_void(tx: &str) -> Answer {
    out::denied(Reason::TxVoid, vec![(Key::Tx, s(tx))])
}

pub(crate) fn rule_refused(rule: Rule, token: Option<&str>, entry_id: Option<&str>, names: Option<&[String]>) -> Answer {
    let reason = match rule {
        Rule::Retraction => Reason::Retraction,
        Rule::Withheld => Reason::GrantWithheld,
        Rule::Revocation => Reason::Revocation,
        Rule::Annotation => Reason::Annotation,
        Rule::WouldBreak => Reason::WouldBreak,
    };
    let mut m = Vec::new();
    if let Some(t) = token {
        m.push((Key::Token, s(t)));
    }
    if let Some(e) = entry_id {
        m.push((Key::EntryId, s(e)));
    }
    if let Some(ns) = names {
        m.push((Key::Names, Value::Arr(ns.iter().map(|x| s(x)).collect())));
    }
    out::denied(reason, m)
}

pub(crate) fn would_break(entry_id: &str, names: &[String]) -> Answer {
    let a = rule_refused(Rule::WouldBreak, None, Some(entry_id), Some(names));
    if names.iter().any(|n| n == zikaron::tokens::FindingName::AuthorityMismatch.as_str()) {
        return a.telling(out::pick(
            "这把钥已不持这本账的席位(账本已归新钥),没有写".to_string(),
            "This key no longer holds this ledger's seat (the ledger is the new key's); nothing was written".to_string(),
        ));
    }
    a
}

pub(crate) fn tx_unheard(tx: &str, detail: &str) -> Answer {
    out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(detail)), (Key::Tx, s(tx))])
}

pub(crate) fn scan(a: &Args) -> Answer {
    proxy_from(a);
    let fixtures = a.many("fixture");
    let endpoints_given = a.many("endpoint");
    let at_home = match a.one("ledger") {
        Some(at) if fixtures.is_empty() && crate::sealed::is_home(&at) => Some(at),
        _ => None,
    };
    if at_home.is_none() && fixtures.is_empty() == endpoints_given.is_empty() {
        out::misuse(Reason::Args, out::Subject::Flags(vec!["fixture", "endpoint"]), out::Said::OneOf);
    }
    if let Some(at) = &at_home {
        if a.one("basis").is_some() {
            out::misuse(Reason::Args, out::Subject::Flags(vec!["basis", "ledger"]), out::Said::OneOf);
        }
        let given = if endpoints_given.is_empty() { Vec::new() } else { chain::endpoint_specs(&a.many_at("endpoint")) };
        return scan_home(at, given.is_empty() && a.one("proxy").is_none(), &given);
    }
    if !fixtures.is_empty() {
        for f in ["basis", "proxy", "ledger"] {
            if !a.many(f).is_empty() {
                out::misuse(Reason::Args, out::Subject::flag(f), out::Said::NotWithFixture);
            }
        }
    }
    let mut runs: Vec<(String, Value)> = Vec::new();
    let thin: Vec<u64>;
    if !fixtures.is_empty() {
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
        let specs = chain::endpoint_specs(&a.many_at("endpoint"));
        let basis = match (a.one("basis"), a.one("ledger")) {
            (Some(p), None) => chain::basis_bytes(&p),
            (None, Some(_)) => out::misuse(Reason::Args, out::Subject::flag("ledger"), out::Said::NotAHome),
            _ => out::misuse(Reason::Args, out::Subject::Flags(vec!["basis", "ledger"]), out::Said::OneOf),
        };
        let eps: Vec<zikaron_send::node::Endpoint> = specs.iter().map(|(c, u)| zikaron_send::node::Endpoint::at(*c, zikaron_send::node::NodeAddr::new(u.as_str()))).collect();
        return match crate::hopin::bare_record() {
            Some(record) => scan_agreed(&basis, &eps, Vec::new(), "", &record),
            None => scan_agreed(&basis, &eps, Vec::new(), "", &zikaron_send::Bare),
        };
    }
    for (_, v) in &runs {
        if v.member("basis").is_none() {
            return out::verbatim(Exit::Denied, v.clone());
        }
    }
    let book = vec![(Key::Book, Value::Arr(Vec::new())), (Key::BookUnread, Value::Arr(runs.iter().map(|(p, _)| s(&format!("{p} NOT_ASKED"))).collect()))];
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
            (Key::SingleSourceHistory, Value::Arr(Vec::new())),
            (
                Key::Sources,
                Value::Arr(reading.sources.iter().map(|x| s(x)).collect()),
            ),
        ]
        .into_iter()
        .chain(book)
        .collect()),
        Err(d) => disagreed(&zikaron_send::Why::Differ { count: d.fragments.len(), sources: d.sources.join(" ") }, &d.sources),
    }
}

fn disagreed(why: &zikaron_send::Why, sources: &[String]) -> Answer {
    out::unanswered(
        Reason::EndpointsDisagree,
        vec![(Key::Token, s(zikaron_send::Code::Disagree.as_str())), (Key::Detail, s(&crate::sealedsend::said(why))), (Key::Sources, Value::Arr(sources.iter().map(|x| s(x)).collect()))],
    )
}

fn scan_agreed(basis: &[u8], eps: &[zikaron_send::node::Endpoint], mut unanswered: Vec<Value>, at: &str, facts: &dyn zikaron_send::Facts) -> Answer {
    let sources: Vec<String> = eps.iter().map(|e| format!("{}={}", e.chain, e.url.said())).collect();
    match zikaron_send::agree::scan(eps, basis, facts) {
        Ok(zikaron_send::agree::Scan::Basis(got)) => {
            unanswered.extend(got.unanswered.iter().map(|w| s(&crate::sealedsend::said(w))));
            out::affirmed(
                vec![
                    (Key::Fragment, got.fragment),
                    (Key::SingleSource, Value::Bool(got.single_source)),
                    (Key::SingleSourceChains, Value::Arr(got.single_source_chains.iter().map(|c| Value::Int(*c)).collect())),
                    (Key::SingleSourceHistory, Value::Arr(got.kept_single_chains.iter().map(|c| Value::Int(*c)).collect())),
                    (Key::Sources, Value::Arr(sources.iter().map(|x| s(x)).collect())),
                    (Key::Unanswered, Value::Arr(unanswered)),
                    (Key::Unproven, Value::Arr(got.unproven.iter().map(|w| s(&crate::sealedsend::said(w))).collect())),
                    (Key::Differed, Value::Arr(got.differed.iter().map(|w| s(&crate::sealedsend::said(w))).collect())),
                ]
                .into_iter()
                .chain(book_read(&json::parse(basis).unwrap_or(Value::Null), eps))
                .collect(),
            )
        }
        Ok(zikaron_send::agree::Scan::NoLabel { report, .. }) => out::verbatim(Exit::Denied, report),
        Err(t) => match (&t.code, &t.why) {
            (zikaron_send::Code::ScanRefused, zikaron_send::Why::Scan { refused, .. }) => scan_unanswered(refused),
            (zikaron_send::Code::Disagree, why) => disagreed(why, &sources),
            _ => crate::sealedsend::chain_refused(&t, at),
        },
    }
}

fn scan_unanswered(refused: &[zikaron_anchor::scan::Refusal]) -> Answer {
    let token = refused.first().map(|r| r.code().to_string()).unwrap_or_else(|| "E_UNANSWERED".to_string());
    let detail = refused.iter().map(|r| r.detail()).collect::<Vec<_>>().join("; ");
    let a = out::unanswered(Reason::Scan, vec![(Key::Token, s(&token)), (Key::Detail, s(&detail))]);
    if refused.iter().any(|r| r.over_asks().is_some()) { a.telling(out::over_asks_line()) } else { a }
}

fn scan_home(at: &str, their_proxy: bool, given: &[(u64, String)]) -> Answer {
    use zikaron_send::node::Endpoint;
    if !crate::sealed::is_home(at) {
        out::misuse(Reason::Args, out::Subject::flag("ledger"), out::Said::NotAHome);
    }
    let r = match crate::sealed::read(at, now_secs()) {
        Ok(r) => r,
        Err(t) => return crate::sealed::refused(&t),
    };
    let ground = match r.ground() {
        Ok(g) => g,
        Err(t) => return crate::sealed::refused(&t),
    };
    let (Some(chain), Some(registry)) = (ground.chain_id, ground.registry) else {
        return out::unanswered(Reason::Scan, vec![(Key::Token, s("E_NO_BASIS")), (Key::Detail, s(at))]);
    };
    if their_proxy {
        match r.machine_settings() {
            Ok(cli) => zikaron_send::node::take_machine_route(cli.proxy.as_deref()),
            Err(t) => return crate::sealed::refused(&t),
        }
    }
    let senders = zikaron_glue::basis::senders_of(&r.items);
    let mut lanes: Vec<(u64, [u8; 20], u64)> = vec![(chain, registry, ground.from_block)];
    lanes.extend(ground.other_chains().into_iter().filter_map(|c| ground.window_of(c).map(|(reg, from)| (c, reg, from))));
    let (mut windows, mut eps, mut unanswered): (Vec<(u64, String, u64, u64)>, Vec<Endpoint>, Vec<Value>) = (Vec::new(), Vec::new(), Vec::new());
    for (i, (c, reg, from)) in lanes.into_iter().enumerate() {
        let mine: Vec<Endpoint> = if given.is_empty() { crate::sealedsend::nodes_of(&ground, c) } else { given.iter().filter(|(x, _)| *x == c).map(|(x, u)| Endpoint::at(*x, zikaron_send::node::NodeAddr::new(u.as_str()))).collect() };
        if mine.is_empty() && i == 0 {
            return out::unanswered(Reason::Unreachable, vec![(Key::Detail, s(&format!("NO_ENDPOINT {c}")))]);
        }
        let head = if mine.is_empty() { Err(zikaron_send::Trouble::text(zikaron_send::Code::NoEndpoint, c.to_string())) } else { zikaron_send::node::head_block(&mine, c) };
        match head {
            Ok((h, _)) => {
                windows.push((c, hexfmt::encode(&reg), from, h.max(from)));
                eps.extend(mine);
            }
            Err(t) if i == 0 => return crate::sealedsend::chain_refused(&t, at),
            Err(t) => unanswered.push(s(&format!("chainId={c} {} {}", t.code.as_str(), crate::sealedsend::said(&t.why)))),
        }
    }
    let regs: Vec<Vec<String>> = windows.iter().map(|(c, reg, _, _)| zikaron_glue::notices::registries_of(*c, reg)).collect();
    let ws: Vec<zikaron_glue::basis::Window> = windows.iter().zip(&regs).map(|((c, _, f, t), regs)| zikaron_glue::basis::Window { chain: *c, registries: regs, from_block: *f, to_block: *t, senders: &senders }).collect();
    let basis = json::canon_bytes(&zikaron_glue::basis::basis_of_all(&ws));
    scan_agreed(&basis, &eps, unanswered, at, &r.checked())
}

fn book_read(basis: &Value, eps: &[zikaron_send::node::Endpoint]) -> Vec<(Key, Value)> {
    use zikaron_send::recipient::Net;
    let chains = basis.member("chains").and_then(Value::as_arr).cloned().unwrap_or_default();
    let mut nets: Vec<Net> = Vec::new();
    let mut senders: Vec<String> = Vec::new();
    for c in &chains {
        let revocation = c.member("chainId").and_then(Value::as_int).and_then(zikaron_anchor::revoke::at).map(|a| zikaron::hexfmt::encode(&a));
        let registry = c.member("registries").and_then(Value::as_arr).and_then(|r| r.iter().filter_map(Value::as_str).find(|x| Some(x.to_ascii_lowercase()) != revocation));
        let (Some(Value::Int(chain)), Some(reg)) = (c.member("chainId"), registry.and_then(zikaron_seal::identities::address)) else { continue };
        let from_block = match c.member("fromBlock") {
            Some(Value::Int(n)) => *n,
            _ => 0,
        };
        nets.push(Net { chain: *chain, registry: reg, from_block, nodes: eps.iter().filter(|e| e.chain == *chain).map(|e| e.url.clone()).collect() });
        senders.extend(c.member("senders").and_then(Value::as_arr).cloned().unwrap_or_default().iter().filter_map(|x| x.as_str().map(str::to_ascii_lowercase)));
    }
    senders.sort();
    senders.dedup();
    let (mut rows, mut unread): (Vec<Value>, Vec<String>) = (Vec::new(), Vec::new());
    for root in &senders {
        let Some(r) = zikaron_seal::identities::address(root) else { continue };
        let (key, missed) = zikaron_send::recipient::book_current(&r, &nets);
        if let Some(k) = key {
            rows.push(Value::Obj(vec![(Key::InOffice.as_str().to_string(), s(&hexfmt::encode(&k))), (Key::Root.as_str().to_string(), s(&hexfmt::encode(&r)))]));
        }
        unread.extend(missed);
    }
    unread.sort();
    unread.dedup();
    vec![(Key::Book, Value::Arr(rows)), (Key::BookUnread, Value::Arr(unread.iter().map(|x| s(x)).collect()))]
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

fn audit_input(a: &Args) -> Result<Value, Answer> {
    audit_read(a).map(|r| r.input)
}

struct AuditRead {
    input: Value,
    pile: Option<(String, Vec<Vec<u8>>, Value)>,
}

fn audit_read(a: &Args) -> Result<AuditRead, Answer> {
    if let Some(p) = a.one("input") {
        let bytes = args::slurp(&p);
        return match json::parse_tests_1_3(&bytes) {
            Ok(v) => {
                let pile = pile_of_input(&v);
                Ok(AuditRead { input: v, pile })
            }
            Err(_) => Err(out::verbatim(Exit::Denied, zikaron::v2::audit::no_label())),
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
            v.member(Key::Fragment.as_str()).cloned().unwrap_or(v)
        }
        None => book::empty_fragment(),
    };
    match zikaron_anchor::input::assemble(&fragment, &root, &ledger::pile_hex(&items)) {
        Some(v) => Ok(AuditRead { input: v, pile: Some((root, items, fragment)) }),
        None => Err(out::denied(Reason::Fragment, vec![])),
    }
}

fn pile_of_input(v: &Value) -> Option<(String, Vec<Vec<u8>>, Value)> {
    let root = v.member("root")?.as_str()?.to_string();
    let items: Vec<Vec<u8>> = v.member("pile")?.as_arr()?.iter().map(|x| x.as_str().and_then(zikaron::hexfmt::decode)).collect::<Option<_>>()?;
    let fragment = Value::Obj(vec![("anchors".to_string(), v.member("anchors")?.clone()), ("basis".to_string(), v.member("basis")?.clone())]);
    Some((root, items, fragment))
}

pub(crate) fn audit_verb(a: &Args) -> Answer {
    let AuditRead { input, pile } = match audit_read(a) {
        Ok(v) => v,
        Err(ans) => return ans,
    };
    if let Some(p) = a.one("out") {
        if let Err(t) = zikaron_glue::landing::land_bytes(std::path::Path::new(&p), &json::canon_bytes(&input)) {
            return out::denied(Reason::Ledger, vec![(Key::Detail, said_detail(t.code(), t.said())), (Key::Path, s(t.subject()))]);
        }
    }
    if let Some((root, items, fragment)) = &pile {
        match book::read_under(root, items, fragment) {
            Ok(l) => {
                if let (book::Form::Old, Some(report)) = (l.form, l.old_report) {
                    return out::verbatim(label_exit(l.label), report);
                }
            }
            Err(book::Unread::NoLabel) => return out::verbatim(Exit::Denied, zikaron::v2::audit::no_label()),
            Err(_) => {}
        }
    }
    match zikaron::v2::audit::audit_full(&input) {
        Some(o) => out::verbatim(label_exit(o.label), o.report),
        None => out::verbatim(Exit::Denied, zikaron::v2::audit::no_label()),
    }
}

fn label_exit(l: Label) -> Exit {
    match l {
        Label::Complete => Exit::Affirmed,
        Label::Gaps | Label::Unproven => Exit::Partial,
        Label::BrokenChain => Exit::Denied,
    }
}

fn input_given(a: &Args) -> Result<Option<Value>, Answer> {
    if a.one("input").is_none() && a.one("ledger").is_none() {
        return Ok(None);
    }
    audit_input(a).map(Some)
}

pub(crate) fn check_grant(a: &Args) -> Answer {
    let grant = match grant_of(&args::slurp(&a.need("grant"))) {
        Ok(g) => g,
        Err(ans) => return ans,
    };
    let input = match input_given(a) {
        Ok(i) => i,
        Err(ans) => return ans,
    };
    if !a.many("endpoint").is_empty() {
        proxy_from(a);
    }
    let keys = book_keys(a);
    let read = crate::hopin::hops(a, vec![crate::hopin::Given { grant, input, stated: None }], &|g, i| book_key(&keys, g, i));
    if let Some(ans) = read.refused(false) {
        return crate::hopin::telling(ans, read.told);
    }
    let (grant, input, notices) = read.hops.first().cloned().unwrap_or_default();
    let value = book::grant_check(&grant, input.as_ref(), notices.as_ref(), read.now());
    let gaps = crate::hopin::gaps(&read, &value);
    let answer = out::verbatim(verdict_exit(&value), value);
    crate::hopin::telling(match gaps {
        Some(g) => answer.with(vec![(Key::Gaps, g)]),
        None => answer,
    }, read.told)
}

pub(crate) fn book_keys(a: &Args) -> Vec<(String, String)> {
    let bare = |k: &str| k.trim().trim_start_matches("0x").to_ascii_lowercase();
    let mut out = Vec::new();
    for p in a.one("fragment").into_iter().chain(a.many("book")) {
        let v = args::slurp_json(&p);
        for r in v.member(Key::Book.as_str()).and_then(Value::as_arr).cloned().unwrap_or_default() {
            if let (Some(root), Some(key)) = (r.member(Key::Root.as_str()).and_then(Value::as_str), r.member(Key::InOffice.as_str()).and_then(Value::as_str)) {
                out.push((bare(root), bare(key)));
            }
        }
    }
    out
}

pub(crate) fn book_key(keys: &[(String, String)], grant: &[u8], input: Option<&Value>) -> Option<String> {
    let bare = |k: &str| k.trim().trim_start_matches("0x").to_ascii_lowercase();
    let root = input.and_then(|i| i.member("root")).and_then(Value::as_str).map(str::to_string).or_else(|| {
        book::check(grant).ok().map(|e| e.body.member("ledger").and_then(Value::as_str).map(str::to_string).unwrap_or(e.author))
    })?;
    keys.iter().find(|(r, _)| *r == bare(&root)).map(|(_, k)| format!("0x{k}"))
}

pub(crate) fn verdict_exit(value: &Value) -> Exit {
    match value.member(zikaron_kit::tokens::Key::Verdict.as_str()).and_then(|x| x.as_str()) {
        Some(x) if x == CheckVerdict::Green.as_str() => Exit::Affirmed,
        Some(x) if x == CheckVerdict::Partial.as_str() => Exit::Partial,
        _ => Exit::Denied,
    }
}

fn grant_of(bytes: &[u8]) -> Result<Vec<u8>, Answer> {
    if zikaron_glue::addressed::is_addressed(bytes) {
        return Err(out::denied(Reason::Addressed, vec![(Key::Token, s(zikaron_seal::addressed::Why::Elsewhere.code())), (Key::Detail, Value::Str(String::new()))]).telling(out::Said::AddressedGrantElsewhere.text().to_string()));
    }
    if !zikaron_glue::container::is_container(bytes) {
        return Ok(bytes.to_vec());
    }
    match zikaron_glue::grantfile::open(bytes) {
        Ok(o) => o.hops.last().cloned().ok_or_else(|| out::denied(Reason::GrantFile, vec![(Key::Token, s("E_GRANT_FILE_CODE"))])),
        Err(r) => Err(grant_file_refused(r)),
    }
}

pub(crate) fn grant_file_refused(r: zikaron_glue::grantfile::Refused) -> Answer {
    use zikaron_glue::grantfile::Refused;
    let ms = match r {
        Refused::Shape(b) => vec![(Key::Token, s(b.code())), (Key::Detail, s(&b.subject()))],
        Refused::Kit(verdict, subject) => {
            let mut m = vec![(Key::Token, s(&verdict))];
            if let Some(x) = subject {
                m.push((Key::Detail, s(&x)));
            }
            m
        }
        Refused::NoCode(at) => vec![(Key::Token, s("E_GRANT_FILE_CODE")), (Key::Detail, s(&at))],
        Refused::Code(r) => vec![(Key::Token, s(r.token.as_str()))],
        Refused::Uncarried(id) => vec![(Key::Token, s("E_GRANT_FILE_CHAIN")), (Key::EntryId, s(&id))],
        Refused::Package(token, subject) => grant_package_refusal(&token, subject.as_deref()),
    };
    out::denied(Reason::GrantFile, ms)
}

fn grant_package_refusal(token: &str, subject: Option<&str>) -> Vec<(Key, Value)> {
    use zikaron_kit::v2::tokens::GpToken;
    let mut m = vec![(Key::Token, s(token))];
    match subject {
        Some(x) if token == GpToken::Uncarried.as_str() => m.push((Key::EntryId, s(x))),
        Some(x) if token == GpToken::Content.as_str() || token == GpToken::Terms.as_str() => m.push((Key::Path, s(x))),
        _ => {}
    }
    m
}

pub(crate) fn chain_check(a: &Args) -> Answer {
    let shared: Option<Value> = match input_given(a) {
        Ok(v) => v,
        Err(ans) => return ans,
    };
    let mut given: Vec<crate::hopin::Given> = Vec::new();
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
        given.push(crate::hopin::Given { grant: args::slurp(&g), input, stated: None });
    }
    if !a.many("endpoint").is_empty() {
        proxy_from(a);
    }
    let keys = book_keys(a);
    let read = crate::hopin::hops(a, given, &|g, i| book_key(&keys, g, i));
    if let Some(ans) = read.refused(true) {
        return crate::hopin::telling(ans, read.told);
    }
    let value = book::chain_check(&read.hops, read.now());
    let gaps = crate::hopin::gaps(&read, &value);
    let answer = out::verbatim(verdict_exit(&value), value);
    crate::hopin::telling(match gaps {
        Some(g) => answer.with(vec![(Key::Gaps, g)]),
        None => answer,
    }, read.told)
}

pub(crate) fn badge_verb(a: &Args) -> Answer {
    let encode = a.many("encode");
    let decode = a.one("decode");
    if encode.is_empty() == decode.is_none() {
        out::misuse(Reason::Args, out::Subject::Flags(vec!["encode", "decode"]), out::Said::OneOf);
    }
    if !encode.is_empty() {
        let entries: Vec<Vec<u8>> = encode.iter().map(|p| args::slurp(p)).collect();
        return match badge::badge_encode(&entries) {
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
    match badge::badge_decode(&payload) {
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

pub(crate) fn form_of(a: &Args) -> kitout::Form {
    match a.one("as") {
        None => kitout::Form::File,
        Some(w) => kitout::Form::of(&w).unwrap_or_else(|| out::misuse(Reason::Args, out::Subject::flag("as"), out::Said::AsWord)),
    }
}

pub(crate) fn originals_wanted(a: &Args) -> bool {
    match a.one("originals").as_deref() {
        None => true,
        Some("none") => false,
        Some(_) => out::misuse(Reason::Args, out::Subject::flag("originals"), out::Said::OriginalsWord),
    }
}

pub(crate) fn kit_export(a: &Args) -> Answer {
    let at = a.need("ledger");
    let form = form_of(a);
    let out_dir = a.need("out");
    let at_home = crate::sealed::is_home(&at);
    if !at_home && a.one("originals").is_some() {
        out::misuse(Reason::Args, out::Subject::flag("originals"), out::Said::OriginalsHomeOnly);
    }
    let with_originals = originals_wanted(a);
    let to = seal_to(a);
    let seek = seek_of(a, to, at_home);
    if at_home && !matches!(seek, Some(Seek::Home)) {
        rest_keys_from(a);
    }
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
    let root = book::root_entry(&items).map(|e| e.id_hex()).unwrap_or_default();
    let withdrawn = zikaron_seal::records::withdrawn(&items);
    let wanted = a.many("entry");
    let entries: Vec<Vec<u8>> = if wanted.is_empty() {
        items
    } else {
        let asked: Vec<String> = wanted.iter().map(|x| entry_id_form(x)).collect();
        let items_all = items.clone();
        let picked: Vec<Vec<u8>> = items
            .into_iter()
            .filter(|b| asked.contains(&hexfmt::encode(&book::entry_id(b))))
            .collect();
        let have: Vec<String> = picked
            .iter()
            .map(|b| hexfmt::encode(&book::entry_id(b)))
            .collect();
        let missing: Vec<Value> = asked
            .iter()
            .filter(|x| !have.contains(x))
            .map(|x| s(x))
            .collect();
        if !missing.is_empty() {
            return out::denied(Reason::EntryAbsent, vec![(Key::Names, Value::Arr(missing))]);
        }
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
        let (kit_path, src) = match spec.split_once('=') {
            Some(x) => x,
            None => out::misuse(Reason::Args, out::typed(&spec), out::Said::FileShape),
        };
        files.push((kit_path.to_string(), args::slurp(src)));
        contents.push(kit_path.to_string());
    }
    let ledger_root = a.one("root").or_else(|| book::root_of(&entries).ok());
    let mut bundle = kitout::Bundle {
        entries,
        files,
        contents,
        root: ledger_root,
        note: a.one("note").unwrap_or_default(),
        ..Default::default()
    };
    let taken = match &home {
        Some(r) => match from_home(r, &root, &mut bundle, with_originals, &withdrawn) {
            Ok(x) => Some(x),
            Err(answer) => return answer,
        },
        None => None,
    };
    let key = match (to, seek) {
        (None, _) => None,
        (Some(SealTo::Key(k)), _) => Some(k),
        (Some(SealTo::Address(addr)), Some(seek)) => match key_by(seek, &addr, home.as_ref(), a.one("proxy").is_some()) {
            Ok(k) => Some(k),
            Err(answer) => return answer,
        },
        (Some(SealTo::Address(_)), None) => None,
    };
    if let Some(r) = &home {
        if let Err(answer) = crate::sealedsend::export_gate(r) {
            return answer;
        }
    }
    let out_path = std::path::Path::new(&out_dir);
    let landed = match key {
        None => kitout::export(out_path, bundle, form).map_err(|t| kit_out_refused(&t)),
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
    let sealed = key.is_some();
    if let (Ok(_), Some(r), false) = (&landed, &home, sealed) {
        crate::sealedsend::proofs_beside(r, out_path);
    }
    match landed {
        Ok(l) => {
            let written = kit_written(&l.dropped, l.entries as u64, l.files as u64, &l.kit_id, &out_dir);
            let written = match taken {
                Some((originals, left_out)) => written.with(vec![
                    (Key::Originals, Value::Arr(originals.iter().map(|x| s(x)).collect())),
                    (Key::LeftOut, Value::Arr(left_out.iter().map(|x| s(x)).collect())),
                ]),
                None => written,
            };
            listed_plain(written, home.as_ref(), sealed, zikaron_seal::exports::Kind::Kit, out_path)
        }
        Err(answer) => answer,
    }
}

pub(crate) fn listed_plain(written: Answer, home: Option<&crate::sealed::Reading>, sealed: bool, kind: zikaron_seal::exports::Kind, path: &std::path::Path) -> Answer {
    match home {
        Some(r) if !sealed => match r.list_export(kind, path, now_secs()) {
            Ok(()) => written,
            Err(t) => written.telling(format!("{}: {}", out::say(NOT_LISTED.0, NOT_LISTED.1), t.tail)),
        },
        _ => written,
    }
}

const NOT_LISTED: (&str, &str) = ("明文包已写,但没有登进本机的导出清单,清理看不到它", "The plain package was written but not listed for clean-up");

pub(crate) fn kit_out_refused(t: &kitout::Trouble) -> Answer {
    if let kitout::Trouble::BadPath(p) = t {
        if let Some(f) = zikaron_kit::kitdir::kit_path_fault(p) {
            return kit_path_refused(p, &f);
        }
    }
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
                Some(w) => out::denied(Reason::Ledger, vec![(Key::Detail, said_detail(t.code(), Some(w))), (Key::Path, s(p))]),
                None => out::denied(Reason::Ledger, vec![(Key::Path, s(p))]),
            },
            _ => out::denied(Reason::Ledger, vec![(Key::Detail, s(&t.subject()))]),
    }
}

fn kit_path_refused(path: &str, f: &zikaron_kit::kitdir::PathFault) -> Answer {
    use zikaron_kit::kitdir::PathFault as F;
    let zh = out::lang() == out::Lang::Zh;
    let at = match (f, zh) {
        (F::Empty, true) => "包内路径是空的".to_string(),
        (F::Empty, false) => "The package path is empty".to_string(),
        (F::TooLong { len }, true) => format!("包内路径 {len} 字节,太长"),
        (F::TooLong { len }, false) => format!("The package path is {len} bytes, too long"),
        (F::LeadingSlash, true) => "包内路径以 / 起头".to_string(),
        (F::LeadingSlash, false) => "The package path starts with /".to_string(),
        (F::EmptySegment { segment }, true) => format!("包内路径第 {segment} 段是空的"),
        (F::EmptySegment { segment }, false) => format!("Segment {segment} of the package path is empty"),
        (F::SegmentTooLong { segment, len }, true) => format!("包内路径第 {segment} 段 {len} 字节,太长"),
        (F::SegmentTooLong { segment, len }, false) => format!("Segment {segment} of the package path is {len} bytes, too long"),
        (F::Dot { segment }, true) => format!("包内路径第 {segment} 段是 . 或 .."),
        (F::Dot { segment }, false) => format!("Segment {segment} of the package path is . or .."),
        (F::LeadingDash { segment }, true) => format!("包内路径第 {segment} 段以 - 起头"),
        (F::LeadingDash { segment }, false) => format!("Segment {segment} of the package path starts with -"),
        (F::Char { segment, c }, true) => format!("包内路径第 {segment} 段有不收的字「{c}」"),
        (F::Char { segment, c }, false) => format!("Segment {segment} of the package path has a character it does not take: “{c}”"),
    };
    let rule = out::say("包内名只许小写字母、数字与 . _ -,每段 1 至 255 字节,不以 - 起头,不是 . 或 ..,全长至多 1024 字节", "package names take only lowercase letters, digits and . _ -, each segment 1 to 255 bytes, not starting with -, not . or .., at most 1024 bytes in all");
    let sep = if zh { ";" } else { "; " };
    out::denied(Reason::KitPath, vec![(Key::Path, s(path)), (Key::Token, s(f.as_str())), (Key::Detail, s(&f.evidence()))]).telling(format!("{at}{sep}{rule}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SealTo {
    Address([u8; 20]),
    Key([u8; 33]),
}

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

pub(crate) enum Seek {
    Home,
    Given(Vec<zikaron_send::recipient::Net>),
}

pub(crate) fn seek_of(a: &Args, to: Option<SealTo>, at_home: bool) -> Option<Seek> {
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
        } else {
            rest_keys_from(a);
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

pub(crate) fn key_by(seek: Seek, address: &[u8; 20], home: Option<&crate::sealed::Reading>, proxy_given: bool) -> Result<[u8; 33], Answer> {
    use zikaron_send::recipient::Net;
    let held = match home {
        Some(r) => r.held_office(address).map_err(|t| crate::sealed::refused(&t))?,
        None => None,
    };
    let (nets, readonly, unread) = match (seek, home) {
        (Seek::Given(nets), _) => (nets, Vec::new(), Vec::new()),
        (Seek::Home, Some(r)) => {
            let ground = r.ground().map_err(|t| crate::sealed::refused(&t))?;
            if !proxy_given {
                let cli = r.machine_settings().map_err(|t| crate::sealed::refused(&t))?;
                zikaron_send::node::take_machine_route(cli.proxy.as_deref());
            }
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
    let kept = home.map(|r| r.checked());
    let bare = if home.is_none() { crate::hopin::bare_record() } else { None };
    let keys = kept.as_ref().map(|k| k as &dyn zikaron_send::recipient::Keys).or(bare.as_ref().map(|k| k as &dyn zikaron_send::recipient::Keys));
    zikaron_send::recipient::key_for_root(address, held, nets, &readonly, unread, zikaron_anchor::deploy::CODE_HASH, keys).map(|(k, _)| k).map_err(|(why, words)| addressed_refused(why, &words))
}

pub const KEY_NOT_RECIPIENT: (&str, &str) = ("给出的钥不是这份记录包的接收方", "The key given is not this record package's recipient");

pub(crate) fn addressed_refused(why: zikaron_seal::addressed::Why, evidence: &str) -> Answer {
    let members = vec![(Key::Token, s(why.code())), (Key::Detail, s(evidence))];
    let (zh, en) = why.words();
    let a = if why.unanswered() { out::unanswered(Reason::Addressed, members) } else { out::denied(Reason::Addressed, members) };
    a.telling(out::say(zh, en).to_string())
}

fn from_home(r: &crate::sealed::Reading, root: &str, b: &mut kitout::Bundle, with_originals: bool, withdrawn: &[String]) -> Result<(Vec<String>, Vec<String>), Answer> {
    let rows = r.records().map_err(|t| crate::sealed::refused(&t))?;
    let chosen: Vec<String> = b
        .entries
        .iter()
        .filter_map(|x| book::check(x).ok())
        .filter(|e| zikaron_glue::recordcheck::work_of(e.kind, &e.body).is_some())
        .map(|e| e.id_hex())
        .collect();
    let (mut taken, mut left_out, mut contents): (Vec<String>, Vec<String>, Vec<String>) = (Vec::new(), Vec::new(), Vec::new());
    let chosen = if with_originals { chosen } else { Vec::new() };
    let (gone, chosen): (Vec<String>, Vec<String>) = chosen.into_iter().partition(|c| withdrawn.iter().any(|w| w.eq_ignore_ascii_case(c)));
    for row in zikaron_seal::records::originals_for(&rows, root, &gone) {
        if !left_out.contains(&row.path) {
            left_out.push(row.path.clone());
        }
    }
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
        match zikaron_glue::kitnames::put_original(&name, bytes, b) {
            Ok(inside) => {
                contents.push(content);
                taken.push(inside);
            }
            Err(_) => left_out.push(row.path.clone()),
        }
    }
    let grants: Vec<String> = b.entries.iter().filter_map(|x| book::check(x).ok()).filter(|e| e.kind == EntryType::Grant).map(|e| e.id_hex()).collect();
    for g in grants {
        if let Some((rel, doc)) = r.terms_of(&g).map_err(|t| crate::sealed::refused(&t))? {
            if !b.files.iter().any(|(p, _)| *p == rel) {
                b.files.push((rel, doc));
            }
        }
    }
    let ground = r.ground().map_err(|t| crate::sealed::refused(&t))?;
    if let (Some(chain_id), Some(registry)) = (ground.chain_id, ground.registry) {
        b.anchored_on = Some(zikaron_glue::anchoredon::AnchoredOn { chain_id, from_block: ground.from_block, registry: hexfmt::encode(&registry) });
    }
    if let Some(a) = r.last_anchored().map_err(|t| crate::sealed::refused(&t))? {
        b.anchors = a.rows.into_iter().map(|(entry_id, (chain_id, tx))| zikaron_glue::pack::Anchor { entry_id, chain_id, tx }).collect();
    }
    Ok((taken, left_out))
}

pub(crate) fn kit_written(dropped: &[String], entries: u64, files: u64, kit_id: &str, path: &str) -> Answer {
    out::affirmed(vec![
        (Key::Dropped, Value::Arr(dropped.iter().map(|x| s(x)).collect())),
        (Key::Entries, Value::Int(entries)),
        (Key::Files, Value::Int(files)),
        (Key::KitId, s(kit_id)),
        (Key::Path, s(path)),
        (Key::State, s(KIT_OK)),
    ])
}

pub(crate) fn kit_verify(a: &Args) -> Answer {
    let at = a.need("kit");
    let fragment = a.one("fragment").map(|p| {
        let v = args::slurp_json(&p);
        v.member(Key::Fragment.as_str()).cloned().unwrap_or(v)
    });
    let given = std::path::Path::new(&at);
    let opened = match open_addressed(a, given) {
        Ok(o) => o,
        Err(answer) => return answer,
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
    let answer = records_of(dir, fragment.as_ref());
    drop(unpacked);
    let answer = match a.one("originals-to") {
        Some(to) => originals_to(answer, p, std::path::Path::new(&to)),
        None => answer,
    };
    drop(opened);
    answer
}

pub(crate) fn open_addressed(a: &Args, given: &std::path::Path) -> Result<Option<zikaron_seal::addressed::Opened>, Answer> {
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
    let opened = if zikaron_glue::addressed::is_addressed_at(given) {
        let mut passed: Option<zikaron_seal::Trouble> = None;
        let keys: Vec<zikaron_seal::addressed::Bytes> = match (&ledger, key) {
            (Some(l), _) => match crate::sealed::identity_keys(l, now_secs()) {
                Ok((ks, p)) => {
                    passed = p;
                    ks.into_iter().map(zikaron_seal::addressed::Bytes).collect()
                }
                Err(t) => return Err(crate::sealed::refused(&t)),
            },
            (None, Some(k)) => vec![k],
            (None, None) => out::misuse(Reason::Args, out::Subject::Flags(vec!["key", "ledger", "home"]), out::Said::AddressedNeedsKey),
        };
        let lent: Vec<&dyn zikaron_seal::addressed::Recipient> = keys.iter().map(|k| k as &dyn zikaron_seal::addressed::Recipient).collect();
        match (zikaron_seal::addressed::open(given, &lent), passed) {
            (Ok(o), _) => Some(o),
            (Err(zikaron_seal::addressed::Refused::NotRecipient), Some(t)) => return Err(crate::sealed::refused(&t)),
            (Err(r @ zikaron_seal::addressed::Refused::NotRecipient), None) if ledger.is_none() => {
                let why = zikaron_seal::addressed::Why::NotRecipient;
                let members = vec![(Key::Token, s(why.code())), (Key::Detail, s(r.code()))];
                return Err(out::denied(Reason::Addressed, members).telling(out::say(KEY_NOT_RECIPIENT.0, KEY_NOT_RECIPIENT.1).to_string()));
            }
            (Err(r), _) => return Err(addressed_open_refused(&r)),
        }
    } else {
        None
    };
    Ok(opened)
}

pub(crate) fn addressed_open_refused(r: &zikaron_seal::addressed::Refused) -> Answer {
    match zikaron_seal::addressed::Why::of(r) {
        Some(why) => addressed_refused(why, format!("{} {}", r.code(), r.tail()).trim()),
        None => out::denied(Reason::Unreadable, vec![(Key::Token, s(r.code())), (Key::Detail, s(r.tail()))]),
    }
}

pub(crate) fn originals_to(records: Answer, kit: &std::path::Path, to: &std::path::Path) -> Answer {
    if records.value.member(Key::Reason.as_str()).and_then(|v| v.as_str()) == Some(Reason::Kit.as_str()) {
        return records;
    }
    match zikaron_glue::extract::take(kit, to) {
        Ok((_, landed)) => with_taken(records, &landed.names, &to.display().to_string()),
        Err(r) => extract_refused(r),
    }
}

pub(crate) fn extract_refused(r: zikaron_glue::extract::Refused) -> Answer {
    use zikaron_glue::extract::Refused;
    match r {
        Refused::Kit { verdict, subject } => kit_refused(&verdict, subject.as_deref()),
        Refused::Unpack { code, subject } => out::denied(Reason::Kit, vec![(Key::Detail, s(&subject)), (Key::State, s(&code))]),
        r => {
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

fn records_of(dir: &std::path::Path, fragment: Option<&Value>) -> Answer {
    let kit_id = match book::kit_verdict(dir) {
        Ok(id) => id,
        Err((verdict, subject)) => return kit_refused(&verdict, subject.as_deref()),
    };
    let items = match zikaron_glue::read::kit_entries(dir) {
        Ok(x) => x,
        Err(p) => return out::denied(Reason::Kit, vec![(Key::Detail, s(&p)), (Key::State, s(zikaron_kit::tokens::KitFailToken::Unreadable.as_str()))]),
    };
    let originals = zikaron_glue::recordcheck::originals_in_dir(dir);
    let first = fragment.and_then(|f| book::read(&items, f).ok()).map(|r| r.first);
    let rows: Vec<zikaron_glue::door::Record> = zikaron_glue::recordcheck::records(&items, &originals, first.as_ref())
        .into_iter()
        .map(|r| zikaron_glue::door::Record { entry_id: r.id, content: r.content, original: r.original, chain: r.chain, deleted: r.deleted.map(|d| d.by) })
        .collect();
    records_answer(&kit_id, items.len() as u64, &rows, &[])
}

pub(crate) fn kit_refused(verdict: &str, subject: Option<&str>) -> Answer {
    let mut ms = vec![(Key::State, s(verdict))];
    if let Some(x) = subject {
        ms.push((Key::Detail, s(x)));
    }
    out::denied(Reason::Kit, ms)
}

pub(crate) fn records_answer(kit_id: &str, entries: u64, rows: &[zikaron_glue::door::Record], unanswered: &[String]) -> Answer {
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
            if let Some(by) = &r.deleted {
                ms.push((Key::Deleted.as_str().to_string(), s(by)));
            }
            Value::Obj(ms)
        })
        .collect();
    let mut members = vec![(Key::KitId, s(kit_id)), (Key::Entries, Value::Int(entries)), (Key::Records, Value::Arr(listed))];
    if !unanswered.is_empty() {
        members.push((Key::Unanswered, Value::Arr(unanswered.iter().map(|x| s(x)).collect())));
    }
    if rows.iter().any(|r| r.original == Original::Mismatch) {
        out::denied(Reason::Records, members).telling(out::Said::RecordsNotAll.text().to_string())
    } else if rows.iter().all(|r| r.original == Original::Match && matches!(r.chain, OnChain::Anchored { .. })) {
        out::affirmed(members)
    } else {
        out::partial(Reason::Records, members).telling(out::Said::RecordsNotAll.text().to_string())
    }
}

pub(crate) fn with_taken(records: Answer, names: &[String], to: &str) -> Answer {
    records.with(vec![(Key::Taken, Value::Arr(names.iter().map(|n| s(n)).collect())), (Key::Path, s(to))])
}

pub(crate) fn entry_id_form(x: &str) -> String {
    let bare = x.strip_prefix("0x").unwrap_or(x);
    match zikaron_store::layout::EntryName::parse(bare) {
        Some(n) => format!("0x{}", n.as_str()),
        None => out::misuse(Reason::Args, out::typed(x), out::Said::EntryNotId),
    }
}

pub(crate) fn show(a: &Args) -> Answer {
    let bytes = match (a.one("path"), a.one("entry")) {
        (Some(p), None) => args::slurp(&p),
        (None, Some(id)) => {
            let form = entry_id_form(&id);
            let at = a.need("ledger");
            let p = std::path::Path::new(&at);
            if zikaron_glue::mirror::is_bundle(p) || zikaron_glue::read::is_kit(p) || zikaron_glue::container::is_container_at(p) || zikaron_glue::addressed::is_addressed_at(p) || crate::sealed::is_home(&at) {
                let items = match ledger::read_any(&at, now_secs()) {
                    Ok(x) => x,
                    Err(r) => return read_refused(r),
                };
                match items.into_iter().find(|b| hexfmt::encode(&book::entry_id(b)) == form) {
                    Some(b) => b,
                    None => return out::denied(Reason::EntryAbsent, vec![(Key::EntryId, s(&form))]),
                }
            } else {
                let dir = match ledger::open(&at) {
                    Ok(d) => d,
                    Err(t) => return ledger_trouble(&t),
                };
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
    match book::check(&bytes) {
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

pub(crate) fn contract(_a: &Args) -> Answer {
    let m = |k: Contract, v: Value| (k.as_str().to_string(), v);
    let names = |xs: &[&str]| Value::Arr(xs.iter().map(|x| s(x)).collect());
    let exits = Exit::ALL
        .iter()
        .map(|e| Value::Obj(vec![m(Contract::Code, Value::Int(e.code() as u64)), m(Contract::Name, s(e.name()))]))
        .collect();
    let reasons = Reason::ALL.iter().map(|r| r.as_str()).chain(crate::codes_own::desk_names()).map(s).collect();
    let stands = args::STANDS_FOR
        .iter()
        .map(|(flag, of)| Value::Obj(vec![m(Contract::Flag, s(flag)), m(Contract::For, s(of))]))
        .collect();
    let verbs = ACCEPTS
        .iter()
        .map(|(name, shared, own, need)| {
            let mut ms = vec![m(Contract::Name, s(name)), m(Contract::Flags, names(&flags(shared, own)))];
            if let Some((_, home)) = HOME.iter().find(|(v, _)| v == name) {
                ms.push(m(Contract::HomeFlags, names(&zikaron_glue::doorverbs::names(home))));
            }
            ms.push(m(Contract::Required, Value::Arr(need.iter().map(|n| names(&n.flags())).collect())));
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

fn write_entry(a: &Args, kind: EntryType, body: Vec<(Field, Value)>) -> Answer {
    let at = a.need("ledger");
    if crate::sealed::is_home(&at) {
        if a.one("key").is_some() || a.one(args::KEY_FILE).is_some() {
            out::misuse(Reason::Args, out::Subject::Flags(vec!["key", args::KEY_FILE]), out::Said::SealedNoKey);
        }
        let now = now_secs();
        let (home, (seq, prev)) = match crate::sealedsend::open_to_write(&at, now, |h| next_link(a, &h.items)) {
            Ok(x) => x,
            Err(answer) => return answer,
        };
        let body = with_ledger_root(kind, body, &home.items, a.one("root").as_deref());
        let records = match home.terms_records() {
            Ok(r) => r,
            Err(t) => return crate::sealed::refused(&t),
        };
        let unread = match settle_withdrawals(a, kind, &body, &home, &records) {
            Ok(u) => u,
            Err(answer) => return answer,
        };
        if let Some(refused) = object_gate(kind, &body, &home.items, &records, &zikaron_send::checked::noticed(&home.checked().0), &unread) {
            return refused;
        }
        let road = crate::issuing::Road::Home(&home);
        let mut body = body;
        if kind == EntryType::Grant && shape_holds(&body) {
            match crate::issuing::member(a, &road, &home.items) {
                Ok(m) => body.push((Field::Revocation, m.value())),
                Err(answer) => return answer,
            }
        }
        if let Some(pair) = noticed_pair(kind, &body, &home.items) {
            let author = home.author.clone();
            let mut land = |noticed: bool, body: Value, seq: u64, prev: Option<&str>| {
                let items = home.pile().map_err(|t| crate::sealed::refused(&t))?;
                seal_land(Landing::Sealed(&home, now, if noticed { Then::Noticed } else { Then::Nothing }), &items, &author, EntryType::Revocation.as_str(), seq, prev, body, &home.key, a.one("root").as_deref())
            };
            return crate::issuing::revoke(a, &road, &home.items, pair, a.one("case"), &home.key, (seq, prev), &mut land);
        }
        let original = match (kind, a.one("file")) {
            (EntryType::History, Some(p)) => body.iter().find(|(f, _)| *f == Field::Content).and_then(|(_, v)| v.as_str().map(str::to_string)).map(|content| {
                let at = std::fs::canonicalize(&p).unwrap_or_else(|_| std::path::PathBuf::from(&p));
                Original { name: at.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), path: at.display().to_string(), content }
            }),
            _ => None,
        };
        return seal_and_append(
            Landing::Sealed(&home, now, original.map_or(Then::Nothing, Then::Original)),
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
    let body = with_ledger_root(kind, body, &items, a.one("root").as_deref());
    if let Some(refused) = object_gate(kind, &body, &items, &[], &[], &[]) {
        return refused;
    }
    let road = crate::issuing::Road::Folder;
    let mut body = body;
    if kind == EntryType::Grant && shape_holds(&body) {
        match crate::issuing::member(a, &road, &items) {
            Ok(m) => body.push((Field::Revocation, m.value())),
            Err(answer) => return answer,
        }
    }
    if let Some(pair) = noticed_pair(kind, &body, &items) {
        let mut land = |_noticed: bool, body: Value, seq: u64, prev: Option<&str>| {
            let items = ledger::pile(&dir).map_err(|t| ledger_trouble(&t))?;
            seal_land(Landing::Dir(&dir), &items, &author, EntryType::Revocation.as_str(), seq, prev, body, &key, a.one("root").as_deref())
        };
        return crate::issuing::revoke(a, &road, &items, pair, a.one("case"), &key, (seq, prev), &mut land);
    }
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

fn noticed_pair(kind: EntryType, body: &[(Field, Value)], items: &[Vec<u8>]) -> Option<zikaron_send::withdraw::Pair> {
    if kind != EntryType::Revocation {
        return None;
    }
    let g = body.iter().find(|(f, _)| *f == Field::Grant).and_then(|(_, v)| v.as_str())?;
    zikaron_send::withdraw::pair(items, g)
}

fn settle_withdrawals(a: &Args, kind: EntryType, body: &[(Field, Value)], home: &crate::sealed::Opened, records: &[zikaron_seal::terms::Record]) -> Result<Vec<u64>, Answer> {
    if kind != EntryType::Grant {
        return Ok(Vec::new());
    }
    let text = |f: Field| body.iter().find(|(x, _)| *x == f).and_then(|(_, v)| v.as_str()).map(str::to_string);
    let window = body.iter().find(|(f, _)| *f == Field::Window).and_then(|(_, w)| match (w.member("from"), w.member("to")) {
        (Some(Value::Int(a)), Some(Value::Int(b))) => Some((*a, *b)),
        _ => None,
    });
    let noticed = zikaron_send::checked::noticed(&home.checked().0);
    let standing = zikaron_send::withdraw::standing(&home.items, &noticed);
    let Err(zikaron_seal::terms::Withheld::Clash(ids)) = zikaron_seal::terms::may_grant(&standing, records, &text(Field::Work).unwrap_or_default(), window, text(Field::Supersedes).as_deref()) else { return Ok(Vec::new()) };
    let waiting = zikaron_send::withdraw::unsettled(&home.items, &ids);
    if waiting.is_empty() {
        return Ok(Vec::new());
    }
    let mut chains: Vec<u64> = waiting.iter().filter_map(|g| zikaron_send::withdraw::member_chain(&home.items, g)).collect();
    chains.sort_unstable();
    chains.dedup();
    let road = crate::issuing::Road::Home(home);
    let mut eps = Vec::new();
    for chain in chains {
        let (_, mine) = crate::issuing::venue(a, &road, Some(chain))?;
        eps.extend(mine.into_iter().filter(|e| e.chain == chain));
    }
    zikaron_send::withdraw::read_back(&eps, &home.items, &waiting, &noticed, |n| zikaron_send::checked::add_noticed(&home.checked().0, n)).map_err(|t| crate::sealed::refused(&t))
}

fn object_gate(kind: EntryType, body: &[(Field, Value)], items: &[Vec<u8>], records: &[zikaron_seal::terms::Record], noticed: &[zikaron_send::checked::Notice], unread: &[u64]) -> Option<Answer> {
    let text = |f: Field| body.iter().find(|(x, _)| *x == f).and_then(|(_, v)| v.as_str()).map(str::to_string);
    let read = book::read(items, &book::empty_fragment()).ok()?;
    let lines = zikaron_glue::retraction::lines_of(&read.entries);
    match kind {
        EntryType::Grant => {
            let window = body.iter().find(|(f, _)| *f == Field::Window).and_then(|(_, w)| match (w.member("from"), w.member("to")) {
                (Some(Value::Int(a)), Some(Value::Int(b))) => Some((*a, *b)),
                _ => None,
            });
            let standing = zikaron_send::withdraw::standing(items, noticed);
            match zikaron_seal::terms::may_grant(&standing, records, &text(Field::Work).unwrap_or_default(), window, text(Field::Supersedes).as_deref()).map_err(|w| undecided(w, items, unread)) {
                Ok(()) => None,
                Err(w) => {
                    let names = match &w {
                        zikaron_seal::terms::Withheld::Clash(ids) | zikaron_seal::terms::Withheld::Undecided(ids) => ids.clone(),
                        zikaron_seal::terms::Withheld::Deleted => Vec::new(),
                    };
                    Some(rule_refused(Rule::Withheld, Some(w.token()), None, Some(&names)))
                }
            }
        }
        EntryType::Revocation => {
            let g = text(Field::Grant)?;
            zikaron_glue::retraction::revocation::may_revoke(&lines, &g).err().map(|w| rule_refused(Rule::Revocation, Some(w.token()), Some(&g), None))
        }
        EntryType::Annotation => {
            let x = text(Field::Subject)?;
            zikaron_glue::retraction::annotation::may_name(&lines, &x).err().map(|w| rule_refused(Rule::Annotation, Some(w.token()), Some(&x), None))
        }
        _ => None,
    }
}

pub fn undecided(w: zikaron_seal::terms::Withheld, items: &[Vec<u8>], unread: &[u64]) -> zikaron_seal::terms::Withheld {
    zikaron_send::withdraw::undecided(w, items, unread)
}

fn with_ledger_root(kind: EntryType, mut body: Vec<(Field, Value)>, items: &[Vec<u8>], root: Option<&str>) -> Vec<(Field, Value)> {
    if kind != EntryType::Grant || body.iter().any(|(f, _)| *f == Field::Ledger) {
        return body;
    }
    let named = match root {
        Some(r) => Some(r.trim().to_ascii_lowercase()),
        None => book::root_of(items).ok(),
    };
    if let Some(r) = named {
        body.push((Field::Ledger, s(&r)));
    }
    body
}

pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub(crate) enum Landing<'a> {
    Dir(&'a LedgerDir),
    Sealed(&'a crate::sealed::Opened, u64, Then),
}

pub(crate) enum Then {
    Nothing,
    Noticed,
    Original(Original),
    Retracted(String, zikaron_seal::queue::Retracting),
}

pub(crate) struct Original {
    path: String,
    name: String,
    content: String,
}

fn shape_holds(body: &[(Field, Value)]) -> bool {
    zikaron::v2::entry::body_fault(zikaron::v2::tokens::EntryType::Grant, &entry::shape(body.to_vec())).is_none()
}

pub(crate) fn seal_and_append(
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
    match seal_land(landing, items, author, entry_type, seq, prev, body, key, root) {
        Ok((id, ledger, written)) => wrote(&id, &ledger, seq, written),
        Err(answer) => answer,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn seal_land(
    landing: Landing,
    items: &[Vec<u8>],
    author: &str,
    entry_type: &str,
    seq: u64,
    prev: Option<&str>,
    body: Value,
    key: &[u8; 32],
    root: Option<&str>,
) -> Result<(String, String, bool), Answer> {
    let answer = |a: Answer| -> Result<(String, String, bool), Answer> { Err(a) };
    if let Landing::Dir(_) = landing {
        if let Some(refused) = adopted_here(items) {
            return answer(refused);
        }
    }
    if let Some(refused) = handed_here(items) {
        return answer(refused);
    }
    let sealed = match entry::seal(author, entry_type, seq, prev, body.clone(), key) {
        Ok(x) => x,
        Err(t) => {
            let answer = out::denied(Reason::Entry, vec![(Key::Token, s(t.as_str()))]);
            let kind = book::Kind::of_literal(entry_type);
            return Err(match (t, zikaron::v2::entry::body_fault(zikaron::v2::tokens::EntryType::of(entry_type), &body)) {
                (zikaron::tokens::Token::BodyField, Some(member)) if body_flags(kind, member).is_some() => {
                    answer.telling(out::body_line(member, body_flags(kind, member).unwrap_or_default()))
                }
                _ => answer,
            });
        }
    };
    match ledger::new_findings(items, &sealed.bytes, root) {
        Ok(names) if names.is_empty() => {}
        Ok(names) => return Err(would_break(&sealed.entry.id_hex(), &names)),
        Err(b) => return Err(barred(b, &sealed.entry.id_hex())),
    }
    match landing {
        Landing::Dir(dir) => match ledger::append(dir, &sealed.entry, &sealed.bytes) {
            Ok(stored) => Ok((sealed.entry.id_hex(), dir.root().to_string_lossy().into_owned(), matches!(stored, Stored::Written))),
            Err(t) => Err(ledger_trouble(&t)),
        },
        Landing::Sealed(home, now, then) => {
            if let Then::Original(o) = &then {
                let o = Original { path: o.path.clone(), name: o.name.clone(), content: o.content.clone() };
                let root = book::root_entry(items).map(|e| e.id_hex()).unwrap_or_default();
                let row = zikaron_seal::records::Row { root, content: o.content, id: sealed.entry.id_hex(), seq, name: o.name, path: o.path };
                if let Err(t) = home.index_original(row) {
                    return Err(crate::sealed::refused(&t));
                }
            }
            let landed = match &then {
                Then::Noticed => home.take(&sealed.entry.id_hex(), &sealed.bytes),
                _ => home.append(&sealed.entry.id_hex(), &sealed.bytes, now),
            };
            let written = match landed {
                Ok(w) => w,
                Err(t) => return Err(crate::sealed::refused(&t)),
            };
            if let (Then::Retracted(target, plan), true) = (&then, written) {
                if let Err(t) = home.amend_queue(|q| q.retracted(target, &sealed.entry.id_hex(), *plan)) {
                    return Err(crate::sealed::refused(&t));
                }
            }
            Ok((sealed.entry.id_hex(), home.ledger_root(), written))
        }
    }
}

fn adopted_here(items: &[Vec<u8>]) -> Option<Answer> {
    let root = book::root_entry(items)?.id;
    let (machine, _) = zikaron_os::machine::here().ok()?;
    match zikaron_seal::adopted::contains(&machine, &root) {
        Ok(false) => None,
        Ok(true) => Some(out::denied(Reason::Adopted, vec![]).telling(out::Said::Adopted.text().to_string())),
        Err(t) => Some(crate::sealed::refused(&t)),
    }
}

pub(crate) fn handed_here(items: &[Vec<u8>]) -> Option<Answer> {
    let root = book::root_entry(items)?.id;
    let (machine, _) = zikaron_os::machine::here().ok()?;
    zikaron_seal::pens::may_write(&machine, &root).err().map(|t| crate::sealed::refused(&t))
}

pub(crate) fn wrote(entry_id: &str, ledger: &str, seq: u64, written: bool) -> Answer {
    out::affirmed(vec![(Key::EntryId, s(entry_id)), (Key::Ledger, s(ledger)), (Key::Seq, Value::Int(seq)), (Key::Written, Value::Bool(written))])
}

pub(crate) fn next_link(a: &Args, items: &[Vec<u8>]) -> Result<(u64, Option<String>), Answer> {
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

fn root_named(a: &Args, items: &[Vec<u8>]) -> Result<String, Answer> {
    match a.one("root") {
        Some(x) => Ok(x),
        None => ledger::root_of(items).map_err(|r| out::denied(r, vec![])),
    }
}

fn read_refused(r: ledger::ReadRefused) -> Answer {
    match r {
        ledger::ReadRefused::Ledger(t) => ledger_trouble(&t),
        ledger::ReadRefused::Kit(verdict, subject) => {
            let mut ms = vec![(Key::State, s(&verdict))];
            if let Some(x) = subject {
                ms.push((Key::Detail, s(&x)));
            }
            out::denied(Reason::Kit, ms)
        }
        ledger::ReadRefused::Mirror(said) => out::denied(Reason::Ledger, vec![(Key::Detail, s(&said))]),
        ledger::ReadRefused::Unreadable(p) => out::misuse(Reason::Unreadable, out::typed(&p), out::Said::Unreadable),
        ledger::ReadRefused::File(b) => out::denied(Reason::Kit, vec![(Key::Detail, s(&b.subject())), (Key::State, s(b.code()))]),
        ledger::ReadRefused::Sealed(t) => crate::sealed::refused(&t),
        ledger::ReadRefused::Addressed => {
            out::denied(Reason::Addressed, vec![(Key::Token, s(zikaron_seal::addressed::Why::Elsewhere.code())), (Key::Detail, Value::Str(String::new()))]).telling(out::Said::AddressedElsewhere.text().to_string())
        }
    }
}

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
    out::unanswered(r.reason, vec![(Key::Detail, s(&r.detail)), (Key::Token, s(&r.code))])
}

pub fn all_flag_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = ALL_FLAGS.to_vec();
    v.extend(MORE_FLAGS.iter().copied());
    v
}

pub fn is_known_flag(x: &str) -> bool {
    known(x)
}

pub fn flag_named(x: &str) -> Option<&'static str> {
    ALL_FLAGS.iter().chain(MORE_FLAGS.iter()).find(|f| **f == x).copied()
}

pub const VERBS: [&str; ACCEPTS.len()] = {
    let mut names = [""; ACCEPTS.len()];
    let mut i = 0;
    while i < ACCEPTS.len() {
        names[i] = ACCEPTS[i].0;
        i += 1;
    }
    names
};
