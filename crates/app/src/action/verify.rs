use super::*;

pub(super) fn verify_work(shell: &mut Shell, path: &str, work: &str) -> Result<Spawned, crate::fault::Fault> {
    shell.verified = None;
    let source = crate::verifyx::source_of(path)?;
    let work = work.trim().to_string();
    if !work.is_empty() {
        crate::verifyx::work_of(&work)?;
    }
    let ground = ground_bare(shell);
    let eps = shell.endpoints.clone();
    let nets = read_nets_now()?;
    let machine = crate::home::machine_dir().ok();
    let mut shelf = super::check::shelf_of(shell, "");
    shelf.reads = nets.clone();
    let clock = shell.clock;
    let path = path.trim().to_string();
    Ok(shell.tasks.spawn(Kind::Verify, move || {
        crate::task::stage_at(Kind::Verify, 0);
        let pairs_of = |d: &std::path::Path| zikaron_kit::v2::kit::walk(d).ok().map(|en| en.into_iter().collect::<Vec<_>>());
        match &source {
            crate::verifyx::Source::Kit(p) | crate::verifyx::Source::File(p) | crate::verifyx::Source::Bytes(p) => zikaron_glue::anchorproofs::offer_beside(p),
            crate::verifyx::Source::Remote(base) => crate::supplyx::offer_fetched(base.as_str()),
            crate::verifyx::Source::Addressed(_) => {}
        }
        let (kit, bytes, rejected, originals, manifest, takes, pairs) = match &source {
            crate::verifyx::Source::Kit(d) => {
                let (b, r) = crate::verifyx::bytes_at(&source)?;
                let manifest = std::fs::read(d.join(zikaron_glue::names::MANIFEST)).ok();
                let kit = crate::verifyx::verify_kit_at(d);
                let held = zikaron_glue::extract::originals_in_dir(d);
                let takes = kit.ok.then(|| zikaron_glue::extract::plan_in(d, &b, &held));
                let pairs = pairs_of(d);
                (Some(kit), b, r, crate::verifyx::originals_of(&held), manifest, takes, pairs)
            }
            crate::verifyx::Source::Bytes(_) => {
                let (b, r) = crate::verifyx::bytes_at(&source)?;
                (None, b, r, Vec::new(), None, None, None)
            }
            crate::verifyx::Source::File(p) => {
                let out = zikaron_glue::read::unpack(p).map_err(|b| crate::fault::Fault::known(crate::fault::Known::KitFileBad, format!("{}:{}", b.code(), b.subject())))?;
                let d = out.dir.clone();
                let (b, r) = crate::verifyx::bytes_at(&crate::verifyx::Source::Kit(d.clone()))?;
                let manifest = std::fs::read(d.join(zikaron_glue::names::MANIFEST)).ok();
                let kit = crate::verifyx::verify_kit_at(&d);
                let held = zikaron_glue::extract::originals_in_dir(&d);
                let takes = kit.ok.then(|| zikaron_glue::extract::plan_in(&d, &b, &held));
                let read = (Some(kit), b, r, crate::verifyx::originals_of(&held), manifest, takes, pairs_of(&d));
                drop(out);
                read
            }
            crate::verifyx::Source::Addressed(p) => {
                let opened = crate::addressedx::open(p)?;
                let out = zikaron_glue::read::unpack(&opened.file).map_err(|b| crate::fault::Fault::known(crate::fault::Known::KitFileBad, format!("{}:{}", b.code(), b.subject())))?;
                let d = out.dir.clone();
                let (b, r) = crate::verifyx::bytes_at(&crate::verifyx::Source::Kit(d.clone()))?;
                let manifest = std::fs::read(d.join(zikaron_glue::names::MANIFEST)).ok();
                let kit = crate::verifyx::verify_kit_at(&d);
                let held = zikaron_glue::extract::originals_in_dir(&d);
                let takes = kit.ok.then(|| zikaron_glue::extract::plan_in(&d, &b, &held));
                let read = (Some(kit), b, r, crate::verifyx::originals_of(&held), manifest, takes, pairs_of(&d));
                drop(out);
                drop(opened);
                read
            }
            crate::verifyx::Source::Remote(base) => {
                let got = crate::fetchx::fetch_kit(base)?;
                let (b, r) = crate::verifyx::entries_of_pairs(&got.pairs);
                (Some(crate::verifyx::kit_facts_of(&got.pairs)), b, r, crate::verifyx::originals_in_pairs(&got.pairs), crate::verifyx::manifest_in(&got.pairs), None, Some(got.pairs.clone()))
            }
        };
        let stated = manifest.as_deref().and_then(crate::verifyx::stated_on);
        let listed = crate::widex::listed(stated.as_ref(), ground.as_ref().ok(), &nets);
        let not_added = if listed { None } else { stated.clone() };
        let offer = crate::readerx::book_offer(ground.as_ref().ok().filter(|_| !eps.is_empty()).map(|g| (&eps[..], g)), &nets);
        let mut unanswered: Vec<String> = Vec::new();
        let pick = crate::verifyx::own_pick(&bytes);
        let review: Result<(crate::verifyx::AnchorReview, zikaron::json::Value, zikaron_anchor::scan::Emitters, Vec<crate::widex::Missed>), String> = (|| {
            if nets.is_empty() || !listed {
                let g = ground.clone().map_err(|f| f.evidence())?;
                if eps.is_empty() {
                    return Err(crate::fault::Fault::known(
                        crate::fault::Known::NoEndpoint,
                        crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail057).to_string()),
                    )
                    .said().to_string());
                }
                if !listed {
                    return Err(crate::lang::t(crate::lang::Key::VerifyNotAddedSaid).to_string());
                }
                let root = crate::auditx::root_of(&bytes).map_err(|f| f.evidence())?;
                let who = crate::readerx::who(&root).map_err(|f| f.evidence())?;
                crate::task::stage_at(Kind::Verify, 1);
                let g = to_head(&eps, g).map_err(|f| f.evidence())?;
                let mut g = crate::readerx::basis_for(&g, &who, &bytes);
                g.senders = crate::readerx::senders_with_book(&root, g.senders, &offer);
                let (scanned, emitters, said) = crate::auditx::scan_table_picked(&eps, &zikaron::json::canon_bytes(&crate::auditx::basis_of(&g)), &pick).map_err(|u| {
                    unanswered = u.unanswered;
                    u.fault.evidence()
                })?;
                unanswered = said;
                let review = crate::verifyx::own_anchors(&bytes, &scanned.fragment, scanned.anchors, scanned.asked);
                return Ok((review, scanned.fragment, emitters, Vec::new()));
            }
            let root = crate::auditx::root_of(&bytes).map_err(|f| f.evidence())?;
            let who = crate::readerx::who(&root).map_err(|f| f.evidence())?;
            crate::task::stage_at(Kind::Verify, 1);
            let senders = crate::readerx::basis_for(&crate::widex::carrier(ground.as_ref().ok(), &nets), &who, &bytes).senders;
            let senders = crate::readerx::senders_with_book(&root, senders, &offer);
            let main = ground.as_ref().ok().map(|g| (&eps[..], g));
            let w = crate::widex::scan_picked(main, &nets, &senders, crate::widex::Ask::First, &pick).map_err(|f| {
                unanswered = vec![f.evidence()];
                f.evidence()
            })?;
            unanswered = w.unanswered.clone();
            let review = crate::verifyx::own_anchors(&bytes, &w.fragment, w.anchors, w.asked);
            Ok((review, w.fragment, w.emitters, w.missed))
        })();
        let (mut review, fragment, read_chain, emitters, missed) = match review {
            Ok((r, f, e, m)) => (Ok(r), f, true, e, m),
            Err(said) => (Err(said), crate::auditx::empty_fragment(), false, Default::default(), Vec::new()),
        };
        crate::task::stage_at(Kind::Verify, 2);
        let mut records = if kit.is_some() { crate::verifyx::records(&bytes, &originals, read_chain.then_some(&fragment)) } else { Vec::new() };
        for r in records.iter_mut() {
            if let Some(f) = r.first.as_mut() {
                crate::auditx::name_registry(f, &emitters);
            }
        }
        crate::verifyx::unread_where_missed(&missed, &mut review, &mut records);
        let grant = match (&kit, &pairs) {
            (Some(k), Some(p)) if k.ok && zikaron_glue::grantfile::is_package(p) => {
                let chain = ground.as_ref().ok().map(|g| g.chain);
                let at = std::path::PathBuf::from(&path);
                let check = |hops: crate::checkx::Hops| crate::checkx::run(hops, shelf.clone(), eps.clone(), ground.clone(), None, chain);
                crate::verifyx::grant_read(p, &at, read_chain.then_some(&check as &dyn Fn(crate::checkx::Hops) -> crate::checkx::Checked))
            }
            _ => None,
        };
        let work_found = (!work.is_empty() && !bytes.is_empty()).then(|| crate::verifyx::work_present(&bytes, &work));
        let mismatches = crate::verifyx::mismatches(kit.as_ref(), &rejected, &review, work_found);
        let filed = match (&kit, &manifest) {
            (Some(k), Some(m)) if !k.ok || read_chain => Some(
                machine
                    .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHomeDir, String::new()))
                    .and_then(|dir| {
                        crate::verifiedx::write(
                            &dir,
                            &crate::verifiedx::Found {
                                manifest: m,
                                at: clock(),
                                kit: k,
                                records: &records,
                                fragment: read_chain.then_some(&fragment),
                            },
                        )
                    })
                    .map(|p| p.display().to_string())
                    .map_err(|f| [f.human().to_string(), f.shown()].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" \u{b7} ")),
            ),
            _ => None,
        };
        Ok(Done::Verified(Box::new(crate::verifyx::Verified {
            path,
            source,
            kit,
            entries: bytes.len(),
            rejected,
            review,
            work,
            work_found,
            mismatches,
            records,
            takes,
            not_added,
            missed,
            unanswered,
            read: if read_chain { crate::verifyx::chains_of(&fragment) } else { Vec::new() },
            basis: if read_chain { crate::verifyx::basis_read(&fragment) } else { Vec::new() },
            grant,
            filed,
        })))
    }))
}

pub(super) fn extract_originals(shell: &mut Shell, kit: &str, to: &str) -> Applied {
    let (kit, to) = (std::path::PathBuf::from(kit.trim()), std::path::PathBuf::from(to.trim()));
    match shell.tasks.spawn(Kind::Extract, move || {
        let opened = if zikaron_glue::addressed::is_addressed_at(&kit) { Some(crate::addressedx::open(&kit)?) } else { None };
        let from = opened.as_ref().map(|o| o.file.clone()).unwrap_or_else(|| kit.clone());
        match zikaron_glue::extract::take(&from, &to) {
            Ok((plan, landed)) => Ok(Done::Extracted { landed, left: plan.left }),
            Err(r) => Err(extract_fault(&r)),
        }
    }) {
        Spawned::Started => Applied::Started(Kind::Extract),
        Spawned::InFlight => Applied::Refused(Kind::Extract),
    }
}

pub fn extract_fault(r: &zikaron_glue::extract::Refused) -> crate::fault::Fault {
    use crate::fault::Known;
    use zikaron_glue::extract::Refused;
    let known = match r {
        Refused::Kit { .. } | Refused::Unpack { .. } => Known::ExtractKit,
        Refused::Path(_) => Known::ExtractPath,
        Refused::Nothing => Known::ExtractNothing,
        Refused::Clash(_) => Known::ExtractClash,
        Refused::Name(_) => Known::ExtractName,
        Refused::NotAFolder(_) => Known::ExtractNotFolder,
        Refused::Exists(_) => Known::ExtractExists,
        Refused::Link(_) => Known::ExtractLink,
        Refused::Changed(_) => Known::ExtractChanged,
        Refused::Landing(..) => Known::ExtractLanding,
    };
    let mut tail: Vec<String> = vec![r.code()];
    tail.extend(r.names().iter().cloned());
    tail.extend(r.detail());
    crate::fault::Fault::known(known, tail.join(" \u{b7} "))
}
