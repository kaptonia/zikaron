use super::*;

pub(super) fn check_delivery(shell: &mut Shell, path: &str, expect: &str) -> Result<Spawned, crate::fault::Fault> {
    crate::deliveryx::expected(expect)?;
    let p = std::path::PathBuf::from(path.trim());
    let typed = expect.to_string();
    shell.delivery = None;
    Ok(shell.tasks.spawn(Kind::Delivery, move || {
        Ok(Done::Delivery(crate::deliveryx::check(&p, &typed)?))
    }))
}
