use super::*;

/// Exports a badge on a background thread. The grant chain, encoding, self-verification, drawing and writing
/// all live in `badgex`.
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
    // Each hop of the chain check uses the audit inputs from this vault's latest re-check; with no re-check,
    // every hop is undecided. The validity window's "now" is read from the chain at export time (falling back
    // to the re-check time), and the result records which re-check the inputs came from.
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
