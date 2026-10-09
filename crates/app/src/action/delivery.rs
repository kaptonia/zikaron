use super::*;

/// Delivery verification. The file is read in the background, never on the UI thread; the comparison lives in
/// `deliveryx::check`.
pub(super) fn check_delivery(shell: &mut Shell, path: &str, expect: &str) -> Result<Spawned, crate::fault::Fault> {
    // Validate the expected value first: empty or malformed input is refused before any background work.
    crate::deliveryx::expected(expect)?;
    let p = std::path::PathBuf::from(path.trim());
    let typed = expect.to_string();
    shell.delivery = None;
    Ok(shell.tasks.spawn(Kind::Delivery, move || {
        Ok(Done::Delivery(crate::deliveryx::check(&p, &typed)?))
    }))
}
