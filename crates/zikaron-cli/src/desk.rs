use crate::args::Args;
use crate::codes::{Door, Key, Reason};
use crate::out::{self, s, Answer, Said};
use crate::verbs;
use zikaron_glue::door::{Reply, Request};

pub fn ask(a: &Args) -> Answer {
    let home = whole_dir(&a.need(verbs::HOME_FLAG));
    if a.verb() == "annotate" {
        if let Some(x) = a.one("subject").filter(|x| x.trim().is_empty()) {
            let token = zikaron_glue::retraction::annotation::Invalid::Shape.token();
            return verbs::rule_refused(zikaron_glue::door::Rule::Annotation, Some(token), Some(&x), None);
        }
    }
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
                std::io::ErrorKind::NotFound if writes_alone(&machine) => absent(Door::Alone, Said::DoorAlone),
                std::io::ErrorKind::NotFound => match crate::sealed::this_machines(&home, &machine) {
                    Err(t) => crate::sealed::refused(&t),
                    Ok(()) => absent(Door::NotOpen, Said::DesktopLocked),
                },
                std::io::ErrorKind::InvalidInput => absent(Door::PathTooLong, Said::DoorPathLong),
                std::io::ErrorKind::PermissionDenied => absent(Door::NotYou, Said::DoorNotYou),
                _ => absent(Door::Broken, Said::DoorBroken),
            };
        }
    };
    if zikaron_glue::door::put(&mut door, &req.to_bytes()).is_err() {
        return absent(Door::Broken, Said::DoorBroken);
    }
    match zikaron_glue::door::take(&mut door).ok().and_then(|b| Reply::of_bytes(&b)) {
        Some(r) => answer_of(r, a.verb(), &home.display().to_string()),
        None => absent(Door::Broken, Said::DoorBroken),
    }
}

pub(crate) fn writes_alone(machine: &std::path::Path) -> bool {
    !matches!(zikaron_seal::credential::find(machine), Err(t) if t.code == zikaron_seal::Code::CredentialMissing)
}

pub fn on_desktop() -> Answer {
    out::denied(Reason::OnDesktop, vec![]).telling(Said::OnDesktop.text().to_string())
}

fn read_args(a: &Args) -> Vec<(String, String)> {
    use zikaron_glue::doorverbs::Flag;
    if a.verb() == "kit-export" {
        let _ = verbs::seal_to(a);
        let _ = verbs::originals_wanted(a);
        let _ = verbs::form_of(a);
    }
    let own = verbs::accepts(a.verb(), true).unwrap_or_default();
    let mut out = Vec::new();
    for flag in own.into_iter().filter(|f| *f != verbs::HOME_FLAG) {
        let values: Vec<String> = match zikaron_glue::doorverbs::kind(a.verb(), flag).unwrap_or(Flag::Value) {
            Flag::File => vec![whole_file(&a.need(flag))],
            Flag::Out => vec![whole_place(&a.need(flag))],
            Flag::Place => a.one(flag).map(|p| whole_place(&p)).into_iter().collect(),
            Flag::Value => match (a.verb(), flag) {
                (_, "window-from" | "window-to" | "effective") | ("face-home", "chain") => a.within_ceiling(flag).map(|n| n.to_string()).into_iter().collect(),
                ("kit-export", "entry") => a.many(flag).iter().map(|x| verbs::entry_id_form(x)).collect(),
                ("grant-package", "grant") => a.one(flag).map(|x| verbs::entry_id_form(&x)).into_iter().collect(),
                ("grant-package", "originals") => match a.one(flag).as_deref() {
                    None | Some("none") => Vec::new(),
                    Some(w) if w == crate::GRANT_ORIGINALS_ALL => vec![w.to_string()],
                    Some(_) => out::misuse(Reason::Args, out::Subject::flag("originals"), Said::PackageOriginalsWord),
                },
                ("grant-package", "to") => a.one(flag).map(|t| match zikaron_seal::identities::address(&t) {
                    Some(_) => t,
                    None => out::misuse(Reason::Args, out::Subject::flag("to"), Said::Not20),
                }).into_iter().collect(),
                ("clean", "yes") => crate::grantpack::yes_given(a).then(|| crate::CLEAN_YES.to_string()).into_iter().collect(),
                _ => a.one(flag).into_iter().collect(),
            },
        };
        out.extend(values.into_iter().map(|v| (flag.to_string(), v)));
    }
    out
}

fn whole_dir(path: &str) -> std::path::PathBuf {
    match std::fs::canonicalize(path) {
        Ok(p) if p.is_dir() => p,
        _ => out::misuse(Reason::Unreadable, out::typed(path), Said::Unreadable),
    }
}

fn whole_file(path: &str) -> String {
    match std::fs::canonicalize(path) {
        Ok(p) if p.is_file() && std::fs::File::open(&p).is_ok() => p.display().to_string(),
        _ => out::misuse(Reason::Unreadable, out::typed(path), Said::Unreadable),
    }
}

fn whole_place(path: &str) -> String {
    match std::path::absolute(path) {
        Ok(p) => p.display().to_string(),
        Err(_) => out::misuse(Reason::Unreadable, out::typed(path), Said::Unreadable),
    }
}

fn absent(why: Door, said: Said) -> Answer {
    out::unanswered(Reason::Desktop, vec![(Key::Detail, s(why.as_str()))]).telling(said.text().to_string())
}

fn answer_of(r: Reply, verb: &str, home: &str) -> Answer {
    match r {
        Reply::Wrote { entry_id, ledger, seq } => verbs::wrote(&entry_id, &ledger, seq, true),
        Reply::Noticed { entry_id, ledger, tx, notice, withdrawn } => crate::issuing::noticed_answer(&entry_id, &ledger, &tx, &notice, withdrawn),
        Reply::Anchored { tx, block } => verbs::anchored(&tx, Some(block)),
        Reply::Reverted { tx, status } => verbs::tx_failed(&tx, status),
        Reply::NotYet { tx, waited } => verbs::tx_not_yet(&tx, waited),
        Reply::Unheard { tx, detail } => verbs::tx_unheard(&tx, &detail),
        Reply::Voided { tx } => verbs::tx_void(&tx),
        Reply::OnChain { names, tx, block } => verbs::on_chain(&names, &tx, block),
        Reply::Kit { kit_id, path, entries, files, dropped, originals, left_out } => verbs::kit_written(&dropped, entries, files, &kit_id, &path).with(vec![
            (Key::Originals, zikaron::json::Value::Arr(originals.iter().map(|x| s(x)).collect())),
            (Key::LeftOut, zikaron::json::Value::Arr(left_out.iter().map(|x| s(x)).collect())),
        ]),
        Reply::Verified { kit_id, entries, records, taken, unanswered } => {
            let a = verbs::records_answer(&kit_id, entries, &records, &unanswered);
            match taken {
                Some(t) => verbs::with_taken(a, &t.names, &t.path),
                None => a,
            }
        }
        Reply::KitRefused { verdict, subject } => verbs::kit_refused(&verdict, subject.as_deref()),
        Reply::Queued { count } => verbs::queued(count),
        Reply::OnDesktop => on_desktop(),
        Reply::Refused { code, tail, network, zh, en, .. } if verb == "anchor" => crate::sealedsend::send_refused(&code, &tail, network, out::pick(zh, en)),
        Reply::Refused { code, tail, network, own: Some(own), .. } => crate::named::of(own, &code, &tail, network, home),
        Reply::Refused { code, tail, network, zh, en, .. } => match crate::codes_own::desk_reason(&code) {
            Some((reason, token)) => {
                let mut members = Vec::new();
                if let Some(t) = token {
                    members.push((Key::Token, s(t)));
                }
                members.push((Key::Detail, s(&tail)));
                let a = if network { out::unanswered(reason, members) } else { out::denied(reason, members) };
                a.telling(out::pick(zh, en))
            }
            None => absent(Door::Broken, Said::DoorBroken),
        },
        Reply::RuleRefused { rule, token, entry_id, names, zh, en } => verbs::rule_refused(rule, token.as_deref(), entry_id.as_deref(), names.as_deref()).telling(out::pick(zh, en)),
        Reply::Closed => absent(Door::Closed, Said::DesktopClosed),
        Reply::GrantPackage { path, kit_id, files, originals, left_out, sealed } => crate::grantpack::package_written(&path, &kit_id, files, &originals, &left_out, sealed),
        Reply::Cleaned { done, removed, kept } => crate::grantpack::cleaned(done, &removed, &kept),
        Reply::Face { table, moment, omitted } => crate::faceverbs::table_answer(table.as_bytes(), moment, &omitted),
        Reply::Joined { joined, read } => crate::faceverbs::joined_answer(joined, &read.iter().map(|r| (r.root.clone(), r.moment, r.omitted.clone())).collect::<Vec<_>>()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zikaron_glue::doorverbs::{kind, Flag, HOME};

    fn args(v: &[&str]) -> Args {
        Args::read(v.iter().map(std::ffi::OsString::from).collect())
    }

    #[test]
    fn every_path_flag_goes_whole_by_the_kind_its_row_declares() {
        let paths: Vec<(&str, &str, Flag)> = HOME.iter().flat_map(|(v, fl)| fl.iter().filter(|(_, k)| k.is_path()).map(move |(f, k)| (*v, *f, *k))).collect();
        assert_eq!(
            paths,
            vec![("history", "file", Flag::File), ("kit-export", "out", Flag::Out), ("kit-verify", "kit", Flag::Place), ("kit-verify", "originals-to", Flag::Place), ("grant-package", "out", Flag::Out)],
            "the five path flags and their kinds"
        );
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let roundabout = |name: &str| dir.join("src").join("..").join(name).display().to_string();
        let file = std::fs::canonicalize(dir.join("Cargo.toml")).expect("there").display().to_string();
        let place = |p: &str| std::path::absolute(p).expect("absolute").display().to_string();
        let got = |v: &[&str]| read_args(&args(v));
        let pick = |a: &[(String, String)], f: &str| a.iter().find(|(k, _)| k == f).map(|(_, v)| v.clone());
        let h = got(&["history", "--home", "/h", "--file", &roundabout("Cargo.toml"), "--note", "n"]);
        assert_eq!(pick(&h, "file"), Some(file), "history --file: the file it is");
        assert_eq!(pick(&h, "note"), Some("n".to_string()), "a value goes as given");
        let k = got(&["kit-export", "--home", "/h", "--out", "rel/out"]);
        assert_eq!(pick(&k, "out"), Some(place("rel/out")), "kit-export --out: absolute");
        let v = got(&["kit-verify", "--home", "/h", "--kit", "a.zkit", "--originals-to", "rel/o"]);
        assert_eq!((pick(&v, "kit"), pick(&v, "originals-to")), (Some(place("a.zkit")), Some(place("rel/o"))), "kit-verify: both absolute");
        let none = got(&["kit-verify", "--home", "/h"]);
        assert!(none.is_empty(), "a place not given goes as nothing");
        let g = got(&["grant-package", "--home", "/h", "--out", "rel/p"]);
        assert_eq!(pick(&g, "out"), Some(place("rel/p")), "grant-package --out: absolute");
        assert_eq!(kind("kit-export", "note"), Some(Flag::Value));
        assert_eq!(kind("kit-export", "nope"), None);
    }
}
