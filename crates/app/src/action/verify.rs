use super::*;

/// Verify a record. Runs on a background thread.
///
/// Kit verification and depth work offline; anchor review needs the chain, and when the chain is unreachable
/// or the basis not configured that column records a named refusal while the other two still show: "chain
/// unread" and "not anchored" are different, and the second is never faked by the first.
pub(super) fn verify_work(shell: &mut Shell, path: &str, work: &str) -> Result<Spawned, crate::fault::Fault> {
    let source = crate::verifyx::source_of(path)?;
    let work = work.trim().to_string();
    if !work.is_empty() {
        crate::depthx::work_of(&work)?;
    }
    let ground = ground_bare(shell);
    let eps = shell.endpoints.clone();
    let nets = read_nets_now()?;
    let machine = crate::home::machine_dir().ok();
    let clock = shell.clock;
    let path = path.trim().to_string();
    shell.verified = None;
    Ok(shell.tasks.spawn(Kind::Verify, move || {
        crate::task::stage_at(Kind::Verify, 0);
        // Kit verification: a directory kit goes to `verify_kit`; grant files and publish addresses are
        // enumerations and go to `verify_enumeration` (the same verification). A publish address is fetched
        // once: the fetched enumeration is material for both kit verification and bytes. The original each
        // `contents` row points to in the kit (material for the per-record verification table).
        let (kit, bytes, rejected, originals, manifest) = match &source {
            crate::verifyx::Source::Kit(d) => {
                let (b, r) = crate::verifyx::bytes_at(&source)?;
                let manifest = std::fs::read(d.join(zikaron_glue::names::MANIFEST)).ok();
                (Some(crate::verifyx::verify_kit_at(d)), b, r, crate::verifyx::originals_in_dir(d), manifest)
            }
            crate::verifyx::Source::Bytes(_) => {
                let (b, r) = crate::verifyx::bytes_at(&source)?;
                (None, b, r, Vec::new(), None)
            }
            crate::verifyx::Source::File(p) => {
                let raw = std::fs::read(p).map_err(|e| crate::fault::classify(&e, &p.display().to_string()))?;
                let pairs = zikaron_glue::container::decode(&raw).map_err(|b| {
                    crate::fault::Fault::known(crate::fault::Known::GrantFileBad, format!("{}:{}", b.code(), b.subject()))
                })?;
                let (b, r) = crate::verifyx::entries_of_pairs(&pairs);
                (Some(crate::verifyx::kit_facts_of(&pairs)), b, r, crate::verifyx::originals_in_pairs(&pairs), crate::verifyx::manifest_in(&pairs))
            }
            crate::verifyx::Source::Remote(base) => {
                let got = crate::fetchx::fetch_kit(base)?;
                let (b, r) = crate::verifyx::entries_of_pairs(&got.pairs);
                (Some(crate::verifyx::kit_facts_of(&got.pairs)), b, r, crate::verifyx::originals_in_pairs(&got.pairs), crate::verifyx::manifest_in(&got.pairs))
            }
        };
        // Where the kit says it is anchored: when that network is not among the ones read (the main network
        // and the read-only table), the kit is still verified and shown, and no chain is read.
        let stated = manifest.as_deref().and_then(crate::verifyx::stated_on);
        let listed = crate::widex::listed(stated.as_ref(), ground.as_ref().ok(), &nets);
        let not_added = if listed { None } else { stated.clone() };
        // Anchor review: the lineage is computed from these bytes themselves, scanned once, and the core
        // produces the report.
        let review: Result<(crate::verifyx::AnchorReview, zikaron::json::Value, zikaron_anchor::scan::Emitters, Vec<crate::widex::Missed>), String> = (|| {
            let review_of = |v: crate::auditx::Verdict, anchors: usize, asked: usize| {
                let unanchored: Vec<String> = crate::auditx::rows_of(&v.report, zikaron::tokens::Key::Unanchored)
                    .iter()
                    .filter_map(|r| match r {
                        zikaron::json::Value::Str(s) => Some(s.clone()),
                        zikaron::json::Value::Obj(m) => m.iter().find(|(k, _)| k == "entryId" || k == "id").and_then(|(_, x)| match x {
                            zikaron::json::Value::Str(s) => Some(s.clone()),
                            _ => None,
                        }),
                        _ => None,
                    })
                    .collect();
                let anchored = crate::ledgerx::anchored_of(&v.report).len();
                crate::verifyx::AnchorReview { label: v.label, anchors, asked, unanchored, anchored, unread: Vec::new() }
            };
            if nets.is_empty() || !listed {
                let g = ground.clone().map_err(|f| f.evidence())?;
                if eps.is_empty() {
                    return Err(crate::fault::Fault::known(
                        crate::fault::Known::NoEndpoint,
                        crate::lang::t(crate::lang::Key::Tail057).to_string(),
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
                let g = crate::readerx::basis_for(&g, &who, &bytes);
                let (scanned, emitters) = crate::auditx::scan_once_noting(&eps, &g).map_err(|f| f.evidence())?;
                let v = crate::auditx::ask_from(&bytes, &scanned.fragment, Vec::new(), scanned.asked, true)
                    .map_err(|f| f.evidence())?;
                return Ok((review_of(v, scanned.anchors, scanned.asked), scanned.fragment, emitters, Vec::new()));
            }
            // Across networks: the main network (when configured) and every read-only one, each chain on its
            // own; the senders are this ledger's lineage, as on the main network alone.
            let root = crate::auditx::root_of(&bytes).map_err(|f| f.evidence())?;
            let who = crate::readerx::who(&root).map_err(|f| f.evidence())?;
            crate::task::stage_at(Kind::Verify, 1);
            let senders = crate::readerx::basis_for(&crate::widex::carrier(ground.as_ref().ok(), &nets), &who, &bytes).senders;
            // The main network, when configured, is always one of the windows: with no node of its own it fails by
            // name like any other chain, never dropped without a word.
            let main = ground.as_ref().ok().map(|g| (&eps[..], g));
            let w = crate::widex::scan(main, &nets, &senders, crate::widex::Ask::First).map_err(|f| f.evidence())?;
            let v = crate::auditx::ask_from(&bytes, &w.fragment, Vec::new(), w.asked, true).map_err(|f| f.evidence())?;
            Ok((review_of(v, w.anchors, w.asked), w.fragment, w.emitters, w.missed))
        })();
        let (mut review, fragment, read_chain, emitters, missed) = match review {
            Ok((r, f, e, m)) => (Ok(r), f, true, e, m),
            Err(said) => (Err(said), crate::auditx::empty_fragment(), false, Default::default(), Vec::new()),
        };
        // Per record: whether the original matches, whether anchored, the first anchor's block time (same
        // fragment, computed once); without a chain read the last two cells say "chain not read".
        crate::task::stage_at(Kind::Verify, 2);
        let mut records = if kit.is_some() { crate::verifyx::records(&bytes, &originals, read_chain.then_some(&fragment)) } else { Vec::new() };
        for r in records.iter_mut() {
            if let Some(f) = r.first.as_mut() {
                crate::auditx::name_registry(f, &emitters);
            }
        }
        // A network left out this pass: what it alone could reach reads "chain not read", never "not anchored".
        crate::verifyx::unread_where_missed(&missed, &mut review, &mut records);
        // Depth: the same implementation as the author side (`depthx::read`, numbers from the kit
        // crate), computed on the same fragment.
        let depth = if work.is_empty() || bytes.is_empty() {
            None
        } else {
            Some(crate::verifyx::depth_of(&bytes, &fragment, &work)?)
        };
        let mismatches = crate::verifyx::mismatches(kit.as_ref(), &rejected, &review, depth.as_ref());
        // The result file speaks only of chains read: a kit that holds is filed when this pass read a chain (so
        // never when its stated network was not added, nor when no chain could be read); a kit that does not
        // hold is filed as it is.
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
                                emitters: &emitters,
                                missed: &missed,
                            },
                        )
                    })
                    .map(|p| p.display().to_string())
                    .map_err(|f| f.evidence()),
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
            depth,
            mismatches,
            records,
            not_added,
            missed,
            read: if read_chain { crate::verifyx::chains_of(&fragment) } else { Vec::new() },
            filed,
        })))
    }))
}
