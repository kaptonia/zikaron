//! `--home`: the verb is done by the running desktop, in the home it has open, through the desktop's local IPC
//! endpoint (the door, `zikaron_glue::door`).
//!
//! The arguments are read and misuse is judged here exactly as for any verb, before anything is asked (exit 2,
//! nothing on stdout): the flags the verb takes beside `--home` (`verbs::HOME`), a home that is not a folder, a
//! file or a value that does not read. What is read goes to the desktop as one request (`zikaron_glue::door`):
//! the verb and each flag as given, paths made whole first (the desktop does not share this process's working
//! folder). The desktop does it through its own action layer and sends back what came of it; the answer is built
//! here from those facts by the same functions that build the verb's answer without `--home`
//! (`verbs::wrote`, `verbs::anchored`, `verbs::kit_written` and the others), so the two cannot answer in two
//! shapes.
//!
//! The door is found where the desktop opens it: in this user's machine folder (`zikaron_os::machine`), named
//! by that folder and the home (`zikaron_glue::door::place`). No door there means the desktop is absent
//! (`E_DESKTOP`, exit 4: locked, that home not open, or not running); this path never writes anything.

use crate::args::Args;
use crate::codes::{Door, Key, Reason};
use crate::out::{self, s, Answer, Said};
use crate::verbs;
use zikaron_glue::door::{Reply, Request};

/// Hand the verb to the desktop and answer by what it says. `run` closed the flags by `verbs::HOME` already.
pub fn ask(a: &Args) -> Answer {
    let home = whole_dir(&a.need(verbs::HOME_FLAG));
    let req = Request { verb: a.verb().to_string(), home: home.display().to_string(), args: read_args(a) };
    let machine = match zikaron_os::machine::here() {
        Ok((m, _)) => m,
        Err(_) => return absent(Door::NoMachine, Said::NoMachine),
    };
    let place = zikaron_glue::door::place(&machine, &home);
    let mut door = match zikaron_os::door::connect(&place) {
        Ok(d) => d,
        Err(e) => {
            return match e.kind() {
                std::io::ErrorKind::NotFound => absent(Door::NotOpen, Said::DesktopLocked),
                std::io::ErrorKind::InvalidInput => absent(Door::PathTooLong, Said::DoorPathLong),
                std::io::ErrorKind::PermissionDenied => absent(Door::NotYou, Said::DoorNotYou),
                _ => absent(Door::Broken, Said::DoorBroken),
            };
        }
    };
    if zikaron_glue::door::put(&mut door, &req.to_bytes()).is_err() {
        return absent(Door::Broken, Said::DoorBroken);
    }
    // The desktop answers when what it does is done (a send waits for its receipt): no deadline here.
    match zikaron_glue::door::take(&mut door).ok().and_then(|b| Reply::of_bytes(&b)) {
        Some(r) => answer_of(r),
        None => absent(Door::Broken, Said::DoorBroken),
    }
}

/// Each flag the verb takes beside `--home`, in its row's order (a repeatable flag's values in the order given),
/// read as the verb reads it without `--home`: whole numbers within the spec's ceiling, entry ids in their one
/// form, a file that must exist and read, places made whole.
fn read_args(a: &Args) -> Vec<(String, String)> {
    let own = verbs::accepts(a.verb(), true).unwrap_or_default();
    let mut out = Vec::new();
    for flag in own.into_iter().filter(|f| *f != verbs::HOME_FLAG) {
        let values: Vec<String> = match (a.verb(), flag) {
            (_, "window-from" | "window-to" | "effective") => a.within_ceiling(flag).map(|n| n.to_string()).into_iter().collect(),
            ("history", "file") => vec![whole_file(&a.need(flag))],
            // The anchors file is read as without `--home` (unreadable or not valid JSON is misuse here) and
            // handed over as that JSON.
            ("adopt", "anchors") => a.one(flag).map(|p| String::from_utf8_lossy(&zikaron::json::canon_bytes(&crate::args::slurp_json(&p))).into_owned()).into_iter().collect(),
            ("kit-export", "out") => vec![whole_place(&a.need(flag))],
            ("kit-export", "entry") => a.many(flag).iter().map(|x| verbs::entry_id_form(x)).collect(),
            _ => a.one(flag).into_iter().collect(),
        };
        out.extend(values.into_iter().map(|v| (flag.to_string(), v)));
    }
    out
}

/// A folder that is there, as the system resolves it; anything else is misuse naming it.
fn whole_dir(path: &str) -> std::path::PathBuf {
    match std::fs::canonicalize(path) {
        Ok(p) if p.is_dir() => p,
        _ => out::misuse(Reason::Unreadable, out::typed(path), Said::Unreadable),
    }
}

/// A file that is there and reads, as the system resolves it; anything else is misuse naming it.
fn whole_file(path: &str) -> String {
    match std::fs::canonicalize(path) {
        Ok(p) if p.is_file() && std::fs::File::open(&p).is_ok() => p.display().to_string(),
        _ => out::misuse(Reason::Unreadable, out::typed(path), Said::Unreadable),
    }
}

/// A place to write, whole (it need not be there yet; the desktop refuses one that is).
fn whole_place(path: &str) -> String {
    match std::path::absolute(path) {
        Ok(p) => p.display().to_string(),
        Err(_) => out::misuse(Reason::Unreadable, out::typed(path), Said::Unreadable),
    }
}

/// No answer from the desktop (exit 4), and why, in a line for people.
fn absent(why: Door, said: Said) -> Answer {
    out::unanswered(Reason::Desktop, vec![(Key::Detail, s(why.as_str()))]).telling(said.text().to_string())
}

/// The command line's answer to what the desktop said, by the verb's own answers.
fn answer_of(r: Reply) -> Answer {
    match r {
        // The desktop writes the entry itself: it is written (never "already there") when it says so.
        Reply::Wrote { entry_id, ledger, seq } => verbs::wrote(&entry_id, &ledger, seq, true),
        Reply::Anchored { tx, block } => verbs::anchored(&tx, Some(block)),
        Reply::Reverted { tx, status } => verbs::tx_failed(&tx, status),
        Reply::NotYet { tx, waited } => verbs::tx_not_yet(&tx, waited),
        Reply::Unheard { tx, detail } => verbs::tx_unheard(&tx, &detail),
        Reply::Voided { tx } => verbs::tx_void(&tx),
        // Under `--home` a package takes no proof bundles (they are chosen on the desktop).
        Reply::Kit { kit_id, path, entries, files, dropped } => verbs::kit_written(&dropped, entries, files, &kit_id, &path, 0),
        Reply::Queued { count } => out::partial(Reason::Queued, vec![(Key::Count, zikaron::json::Value::Int(count))]).telling(Said::Queued.text().to_string()),
        Reply::OnDesktop => out::denied(Reason::OnDesktop, vec![]).telling(Said::OnDesktop.text().to_string()),
        Reply::Refused { code, tail, network, zh, en } => {
            let members = vec![(Key::Token, s(&code)), (Key::Detail, s(&tail))];
            // A network refusal is the absence of an answer (`zikaron/1` §9.4), never a negative one.
            let a = if network { out::unanswered(Reason::DesktopRefused, members) } else { out::denied(Reason::DesktopRefused, members) };
            a.telling(out::pick(zh, en))
        }
        Reply::Closed => absent(Door::Closed, Said::DesktopClosed),
    }
}
