use super::*;

/// Export a badge. Runs on a background thread. Cascading, encoding, self-verification, drawing and writing
/// are all in `badgex`.
pub(super) fn export_badge(shell: &mut Shell, grant: &str, out: &str) -> Result<Spawned, crate::fault::Fault> {
    let g = grant.trim().to_string();
    if !zikaron::hexfmt::is_hex32(&g) {
        return Err(crate::fault::Fault::known(crate::fault::Known::ContentShape, g));
    }
    let root = shell
        .home
        .as_ref()
        .map(|h| h.root().to_path_buf())
        .ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    let out = crate::home::landing(out)?;
    // Each hop of the chain check uses the audit input left by this vault's latest re-check; without any
    // re-check every hop has no input, and the kit crate reads them as undecided. The window check's "now" is
    // asked of the chain now (the input may be from the last pass; now may not); the face states which pass's
    // chain time the inputs came from.
    let (cards, reviewed_at) = shell.cards.clone().unwrap_or_default();
    let eps = shell.endpoints.clone();
    let chain_id = shell.settings.chain_id;
    let ask = crate::exitgate::ask_of(shell)?;
    shell.badge = None;
    Ok(shell.tasks.spawn(Kind::Badge, move || {
        crate::task::stage_at(Kind::Badge, 0);
        let home = crate::home::Home::open(&root)?;
        let pool = crate::badgex::pool(&home)?;
        let chain = crate::badgex::chain_for(&pool, &g)?;
        crate::task::stage_at(Kind::Badge, 1);
        // The exit gate, last before the badge is written.
        let pass = crate::exitgate::pass(&ask)?;
        let mut made = crate::badgex::export(&pass, &chain, &out)?;
        made.grant = g.clone();
        let now = chain_id.and_then(|c| crate::chainx::head_time(&eps, c).ok()).map(|x| x.0).or(reviewed_at);
        made.chain = crate::badgex::chain_verdict(&chain, &cards, now);
        made.input_at = if cards.is_empty() { None } else { reviewed_at };
        Ok(Done::Badge(Box::new(made)))
    }))
}

/// The whole grant code of a grant in this home (`badgex::code_for`: its chain to the root, encoded).
pub(super) fn grant_code(shell: &mut Shell, grant: &str) -> Result<String, crate::fault::Fault> {
    let home = shell.home.as_ref().ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::NoHome, String::new()))?;
    crate::badgex::code_for(&crate::badgex::pool(home)?, grant)
}
