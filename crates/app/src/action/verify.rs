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
    let path = path.trim().to_string();
    shell.verified = None;
    Ok(shell.tasks.spawn(Kind::Verify, move || {
        crate::task::stage_at(Kind::Verify, 0);
        // Kit verification: a directory kit goes to `verify_kit`; grant files and publish addresses are
        // enumerations and go to `verify_enumeration` (the same verification). A publish address is fetched
        // once: the fetched enumeration is material for both kit verification and bytes. The original each
        // `contents` row points to in the kit (material for the per-record verification table).
        let (kit, bytes, rejected, originals) = match &source {
            crate::verifyx::Source::Kit(d) => {
                let (b, r) = crate::verifyx::bytes_at(&source)?;
                (Some(crate::verifyx::verify_kit_at(d)), b, r, crate::verifyx::originals_in_dir(d))
            }
            crate::verifyx::Source::Bytes(_) => {
                let (b, r) = crate::verifyx::bytes_at(&source)?;
                (None, b, r, Vec::new())
            }
            crate::verifyx::Source::File(p) => {
                let raw = std::fs::read(p).map_err(|e| crate::fault::classify(&e, &p.display().to_string()))?;
                let pairs = zikaron_glue::container::decode(&raw).map_err(|b| {
                    crate::fault::Fault::known(crate::fault::Known::GrantFileBad, format!("{}:{}", b.code(), b.subject()))
                })?;
                let (b, r) = crate::verifyx::entries_of_pairs(&pairs);
                (Some(crate::verifyx::kit_facts_of(&pairs)), b, r, crate::verifyx::originals_in_pairs(&pairs))
            }
            crate::verifyx::Source::Remote(base) => {
                let got = crate::fetchx::fetch_kit(base)?;
                let (b, r) = crate::verifyx::entries_of_pairs(&got.pairs);
                (Some(crate::verifyx::kit_facts_of(&got.pairs)), b, r, crate::verifyx::originals_in_pairs(&got.pairs))
            }
        };
        // Anchor review: the lineage is computed from these bytes themselves, scanned once, and the core
        // produces the report.
        let review: Result<(crate::verifyx::AnchorReview, zikaron::json::Value), String> = (|| {
            let g = ground.map_err(|f| f.evidence())?;
            if eps.is_empty() {
                return Err(crate::fault::Fault::known(
                    crate::fault::Known::NoEndpoint,
                    crate::lang::t(crate::lang::Key::Tail057).to_string(),
                )
                .said().to_string());
            }
            let root = crate::auditx::root_of(&bytes).map_err(|f| f.evidence())?;
            let who = crate::readerx::who(&root).map_err(|f| f.evidence())?;
            crate::task::stage_at(Kind::Verify, 1);
            let g = to_head(&eps, g).map_err(|f| f.evidence())?;
            let g = crate::readerx::basis_for(&g, &who, &bytes);
            let scanned = crate::auditx::scan_once(&eps, &g).map_err(|f| f.evidence())?;
            let v = crate::auditx::ask_from(&bytes, &scanned.fragment, Vec::new(), scanned.asked, true)
                .map_err(|f| f.evidence())?;
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
            Ok((
                crate::verifyx::AnchorReview {
                    label: v.label,
                    anchors: scanned.anchors,
                    asked: scanned.asked,
                    unanchored,
                    anchored,
                },
                scanned.fragment,
            ))
        })();
        let (review, fragment, read_chain) = match review {
            Ok((r, f)) => (Ok(r), f, true),
            Err(said) => (Err(said), crate::auditx::empty_fragment(), false),
        };
        // Per record: whether the original matches, whether anchored, the first anchor's block time (same
        // fragment, computed once); without a chain read the last two cells say "chain not read".
        crate::task::stage_at(Kind::Verify, 2);
        let records = if kit.is_some() { crate::verifyx::records(&bytes, &originals, read_chain.then_some(&fragment)) } else { Vec::new() };
        // Depth: the same implementation as the author side (`depthx::read`, numbers from the kit
        // crate), computed on the same fragment.
        let depth = if work.is_empty() || bytes.is_empty() {
            None
        } else {
            Some(crate::verifyx::depth_of(&bytes, &fragment, &work)?)
        };
        let mismatches = crate::verifyx::mismatches(kit.as_ref(), &rejected, &review, depth.as_ref());
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
        })))
    }))
}
