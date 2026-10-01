use super::*;

pub(super) fn set_review_every(shell: &mut Shell, secs: &str) -> Result<u64, crate::fault::Fault> {
    let n = secs.trim().parse::<u64>().map_err(|_| {
        crate::fault::Fault::known(
            crate::fault::Known::SettingsShape,
            crate::lang::filln(crate::lang::Key::Tail071, &[&format!("{:?}", secs.trim())]),
        )
    })?;
    shell.commit_settings(|s| s.review_every = n)?;
    Ok(n)
}
