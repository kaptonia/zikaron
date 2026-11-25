use crate::args::Args;
use crate::codes::{Key, Reason};
use crate::out::{self, s, Answer};
use crate::verbs;
use zikaron::hexfmt;
use zikaron::json::Value;
use zikaron_glue::book::{self, Kind as EntryType};

pub fn grant_package(a: &Args) -> Answer {
    crate::seam();
    match a.one("package") {
        Some(p) => verify(a, &p),
        None => export(a),
    }
}

fn chain_in(items: &[Vec<u8>], grant: &str) -> Result<Vec<Vec<u8>>, Answer> {
    let find = |id: &str| items.iter().find(|b| hexfmt::encode(&book::entry_id(b)).eq_ignore_ascii_case(id)).cloned();
    let absent = |id: &str| out::denied(Reason::EntryAbsent, vec![(Key::Names, Value::Arr(vec![s(id)]))]);
    let mut chain = vec![find(grant).ok_or_else(|| absent(grant))?];
    loop {
        let e = book::check(&chain[0]).map_err(|_| absent(grant))?;
        if e.kind != EntryType::Grant {
            return Err(absent(grant));
        }
        let Some(up) = e.body.member("upstream").and_then(|v| v.as_str()).map(str::to_string) else { break };
        if chain.len() > items.len() {
            return Err(absent(&up));
        }
        chain.insert(0, find(&up).ok_or_else(|| absent(&up))?);
    }
    Ok(chain)
}

fn export(a: &Args) -> Answer {
    let at = a.need("ledger");
    let grant = verbs::entry_id_form(&a.need("grant"));
    let folder = a.need("out");
    let with_original = match a.one("originals").as_deref() {
        None | Some("none") => false,
        Some(w) if w == crate::GRANT_ORIGINALS_ALL => true,
        Some(_) => out::misuse(Reason::Args, out::Subject::flag("originals"), out::Said::PackageOriginalsWord),
    };
    let at_home = crate::sealed::is_home(&at);
    let seal = verbs::seal_to(a);
    let to = match &seal {
        Some(verbs::SealTo::Address(x)) => Some(*x),
        _ => None,
    };
    let seek = verbs::seek_of(a, seal, at_home);
    let now = verbs::now_secs();
    let home = if at_home {
        match crate::sealed::read(&at, now) {
            Ok(r) => Some(r),
            Err(t) => return crate::sealed::refused(&t),
        }
    } else {
        None
    };
    let items = match &home {
        Some(r) => r.items.clone(),
        None => match crate::ledger::open(&at).and_then(|d| crate::ledger::pile(&d)) {
            Ok(x) => x,
            Err(t) => return verbs::ledger_trouble(&t),
        },
    };
    let pool = match &home {
        Some(r) => match r.held() {
            Ok(held) => zikaron_glue::grantfile::chain_pool(held, &items),
            Err(t) => return crate::sealed::refused(&t),
        },
        None => items.clone(),
    };
    let chain = match chain_in(&pool, &grant) {
        Ok(c) => c,
        Err(answer) => return answer,
    };
    let code = match zikaron_kit::v2::doc::badge_encode(&chain) {
        Ok(c) => c,
        Err(r) => return out::denied(Reason::Badge, vec![(Key::Token, s(r.token.as_str()))]),
    };
    let (mut taken, mut left_out) = (Vec::new(), Vec::new());
    let work = book::check(chain.last().map(Vec::as_slice).unwrap_or_default()).ok().and_then(|e| e.body.member("work").and_then(|v| v.as_str()).map(str::to_string)).unwrap_or_default();
    if with_original && home.is_none() {
        left_out = zikaron_glue::grantfile::left_out(&work, false, Vec::new());
    }
    let parts = match &home {
        None => zikaron_glue::grantfile::Parts { root: zikaron_glue::grantfile::first_root(&chain, &items), code, hops: chain.clone(), ledger: items.clone(), ..Default::default() },
        Some(r) => {
            let ground = match r.ground() {
                Ok(g) => g,
                Err(t) => return crate::sealed::refused(&t),
            };
            let last = match r.last_anchored() {
                Ok(l) => l,
                Err(t) => return crate::sealed::refused(&t),
            };
            let upstream = match r.kept_ledgers() {
                Ok(u) => u,
                Err(t) => return crate::sealed::refused(&t),
            };
            let mut parts = match zikaron_seal::terms::package_parts(&chain, &items, &items, &upstream, code, |id| r.terms_of(id), &ground, last.as_ref()) {
                Ok(p) => p,
                Err(t) => return crate::sealed::refused(&t),
            };
            if with_original {
                let rows = match r.records() {
                    Ok(x) => x,
                    Err(t) => return crate::sealed::refused(&t),
                };
                let t = zikaron_seal::records::take(&rows, &items, &work);
                left_out = zikaron_glue::grantfile::left_out(&work, t.original.is_some(), t.left_out);
                parts.original = t.original;
                taken = t.taken;
            }
            parts
        }
    };
    let (bytes, landed) = match zikaron_glue::grantfile::lay(parts) {
        Ok(x) => x,
        Err(zikaron_glue::grantfile::Unbuilt::Name(n)) => return verbs::kit_out_refused(&crate::kitout::Trouble::BadPath(n)),
        Err(zikaron_glue::grantfile::Unbuilt::Kit(t)) => return verbs::kit_out_refused(&t),
        Err(zikaron_glue::grantfile::Unbuilt::Package(token, subject)) => return verbs::grant_file_refused(zikaron_glue::grantfile::Refused::Package(token, subject)),
    };
    let key = match (to, seek) {
        (Some(root), Some(seek)) => match verbs::key_by(seek, &root, home.as_ref(), a.one("proxy").is_some()) {
            Ok(k) => Some(k),
            Err(answer) => return answer,
        },
        _ => None,
    };
    if let Some(r) = &home {
        verbs::rest_keys_from(a);
        if let Err(answer) = crate::sealedsend::export_gate(r) {
            return answer;
        }
    }
    let dir = std::path::PathBuf::from(&folder);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return verbs::kit_out_refused(&crate::kitout::Trouble::Io(folder.clone(), e.to_string()));
    }
    let head: String = grant.trim_start_matches("0x").chars().take(10).collect();
    let path = dir.join(format!("grant-{head}.{}", zikaron_glue::container::EXT));
    let landed_at = match key {
        Some(key) => {
            use zikaron_seal::addressed::Sealing;
            match zikaron_seal::addressed::seal_out(&key, &path, |plain| zikaron_glue::landing::land_bytes(plain, &bytes)) {
                Ok(_) => Ok(()),
                Err(Sealing::Lay(t)) | Err(Sealing::Land(t)) => Err(t),
                Err(Sealing::Refused(r)) => return out::denied(Reason::Ledger, vec![(Key::Detail, s(r.tail())), (Key::Path, s(&path.display().to_string()))]),
            }
        }
        None => zikaron_glue::landing::land_bytes(&path, &bytes),
    };
    if let Err(t) = landed_at {
        return verbs::kit_out_refused(&crate::kitout::Trouble::from(t));
    }
    let written = package_written(&path.display().to_string(), &landed.kit_id, landed.files as u64, &taken, &left_out, to.is_some());
    verbs::listed_plain(written, home.as_ref(), to.is_some(), zikaron_seal::exports::Kind::GrantPackage, &path)
}

pub(crate) fn package_written(path: &str, kit_id: &str, files: u64, originals: &[String], left_out: &[String], sealed: bool) -> Answer {
    out::affirmed(vec![
        (Key::Files, Value::Int(files)),
        (Key::KitId, s(kit_id)),
        (Key::LeftOut, Value::Arr(left_out.iter().map(|x| s(x)).collect())),
        (Key::Originals, Value::Arr(originals.iter().map(|x| s(x)).collect())),
        (Key::Path, s(path)),
        (Key::Sealed, Value::Bool(sealed)),
    ])
}

fn verify(a: &Args, at: &str) -> Answer {
    let given = std::path::Path::new(at);
    let opened = match verbs::open_addressed(a, given) {
        Ok(o) => o,
        Err(answer) => return answer,
    };
    let p = opened.as_ref().map(|o| o.file.as_path()).unwrap_or(given);
    let bytes = match std::fs::read(p) {
        Ok(b) => b,
        Err(e) => return out::denied(Reason::Unreadable, vec![(Key::Detail, s(&e.to_string()))]),
    };
    let pairs = match zikaron_glue::container::decode(&bytes) {
        Ok(x) => x,
        Err(b) => return verbs::grant_file_refused(zikaron_glue::grantfile::Refused::Shape(b)),
    };
    let o = match zikaron_glue::grantfile::open_pairs(&pairs) {
        Ok(o) => o,
        Err(r) => return verbs::grant_file_refused(r),
    };
    let en: zikaron_kit::v2::kit::Enumeration = pairs.iter().cloned().collect();
    let package = zikaron_kit::v2::kit::verify_grant_package(&en);
    let fragment = match a.one("fragment") {
        Some(p) => {
            let v = crate::args::slurp_json(&p);
            v.member(Key::Fragment.as_str()).cloned().unwrap_or(v)
        }
        None => book::empty_fragment(),
    };
    let keys = verbs::book_keys(a);
    let bare = |k: &str| k.trim().trim_start_matches("0x").to_ascii_lowercase();
    let in_office = |root: &str| keys.iter().find(|(r, _)| *r == bare(root)).map(|(_, k)| format!("0x{k}"));
    let given: Vec<crate::hopin::Given> = o
        .hops
        .iter()
        .zip(zikaron_glue::grantfile::hops_judged(&o, &fragment, &in_office))
        .map(|(h, j)| crate::hopin::Given { grant: h.clone(), input: j.input, stated: j.root })
        .collect();
    if !a.many("endpoint").is_empty() {
        verbs::proxy_from(a);
    }
    let read = crate::hopin::hops(a, given, &|_, _| None);
    if let Some(ans) = read.refused(true) {
        return crate::hopin::telling(ans, read.told);
    }
    let check = book::chain_check(&read.hops, read.now());
    let told = read.told.clone();
    let mut members = vec![("check".to_string(), check.clone()), ("package".to_string(), package)];
    if let Some(g) = crate::hopin::gaps(&read, &check) {
        members.push((Key::Gaps.as_str().to_string(), g));
    }
    if let Some(to) = a.one("originals-to") {
        match zikaron_glue::extract::take(p, std::path::Path::new(&to)) {
            Ok((_, landed)) => {
                members.push((Key::Path.as_str().to_string(), s(&to)));
                members.push((Key::Taken.as_str().to_string(), Value::Arr(landed.names.iter().map(|n| s(n)).collect())));
            }
            Err(r) => return verbs::extract_refused(r),
        }
    }
    drop(opened);
    members.sort_by(|x, y| x.0.cmp(&y.0));
    crate::hopin::telling(out::verbatim(verbs::verdict_exit(&check), Value::Obj(members)), told)
}

pub fn clean(a: &Args) -> Answer {
    crate::seam();
    let at = a.need("ledger");
    if !crate::sealed::is_home(&at) {
        out::misuse(Reason::Args, out::Subject::flag("ledger"), out::Said::LedgerNotHome);
    }
    let yes = yes_given(a);
    let r = match crate::sealed::read(&at, verbs::now_secs()) {
        Ok(r) => r,
        Err(t) => return crate::sealed::refused(&t),
    };
    let room = r.machine().join(zikaron_seal::exports::DIR);
    let index = match zikaron_seal::exports::index_at(r.machine()) {
        Ok(x) => x,
        Err(t) => return crate::sealed::refused(&t),
    };
    let list = match r.exports_list() {
        Ok(x) => x,
        Err(t) => return crate::sealed::refused(&t),
    };
    let rows = match index.as_deref().map(zikaron_seal::exports::index_rows).transpose() {
        Ok(x) => x.unwrap_or_default(),
        Err(t) => return crate::sealed::refused(&t),
    };
    let plan = zikaron_seal::exports::plan(&rows, &list);
    if !yes {
        return cleaned(false, &rows_of(&plan).0, &rows_of(&plan).1);
    }
    let done = zikaron_seal::exports::remove(plan);
    let gone = done.removed_paths();
    if let Some(next) = index.as_deref().and_then(|b| zikaron_seal::exports::index_without(b, &gone)) {
        if let Err(t) = zikaron_seal::land::put_at(&room, zikaron_seal::exports::INDEX, &next) {
            return crate::sealed::refused(&t);
        }
    }
    let left: Vec<zikaron_seal::exports::Row> = list.into_iter().filter(|x| !gone.contains(&x.path)).collect();
    if let Err(t) = r.write_exports_list(&left) {
        return crate::sealed::refused(&t);
    }
    let (removed, kept) = rows_of(&done);
    cleaned(true, &removed, &kept)
}

pub(crate) fn yes_given(a: &Args) -> bool {
    match a.one("yes").as_deref() {
        None => false,
        Some(w) if w == crate::CLEAN_YES => true,
        Some(_) => out::misuse(Reason::Args, out::Subject::flag("yes"), out::Said::YesWord),
    }
}

fn rows_of(p: &zikaron_seal::exports::Plan) -> (Vec<zikaron_glue::door::Cleared>, Vec<zikaron_glue::door::Cleared>) {
    let row = |i: &zikaron_seal::exports::Item, why: Option<&zikaron_seal::exports::Why>| zikaron_glue::door::Cleared { kind: i.kind.word().to_string(), path: i.path.clone(), link: i.link.clone(), why: why.map(|w| w.word().to_string()) };
    (p.remove.iter().map(|i| row(i, None)).collect(), p.keep.iter().map(|(i, w)| row(i, Some(w))).collect())
}

pub(crate) fn cleaned(done: bool, removed: &[zikaron_glue::door::Cleared], kept: &[zikaron_glue::door::Cleared]) -> Answer {
    let row = |c: &zikaron_glue::door::Cleared| {
        let mut m = vec![(Key::Kind.as_str().to_string(), s(&c.kind))];
        if let Some(l) = &c.link {
            m.push((Key::Link.as_str().to_string(), s(l)));
        }
        m.push((Key::Path.as_str().to_string(), s(&c.path)));
        if let Some(w) = &c.why {
            m.push((Key::Why.as_str().to_string(), s(w)));
        }
        Value::Obj(m)
    };
    let gone = if done { Key::Removed } else { Key::Planned };
    out::affirmed(vec![
        (Key::Kept, Value::Arr(kept.iter().map(row).collect())),
        (gone, Value::Arr(removed.iter().map(row).collect())),
        (Key::Unlisted, s(crate::CLEAN_UNLISTED)),
    ])
}
