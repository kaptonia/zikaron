use super::*;

/// Delivery verification. The file is read in the background (no disk in the frame); the comparison lives in
/// `deliveryx::check`.
pub(super) fn check_delivery(shell: &mut Shell, path: &str, expect: &str) -> Result<Spawned, crate::fault::Fault> {
    // The terms cell is recognized first: empty or malformed is refused by name at once, without starting the
    // background task.
    crate::deliveryx::expected(expect)?;
    let p = std::path::PathBuf::from(path.trim());
    let typed = expect.to_string();
    shell.delivery = None;
    Ok(shell.tasks.spawn(Kind::Delivery, move || {
        Ok(Done::Delivery(crate::deliveryx::check(&p, &typed)?))
    }))
}
