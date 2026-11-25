use super::*;

fn asked(shell: &Shell) -> (Vec<String>, Vec<String>, Vec<(String, String)>) {
    let g = shell.settings.ground();
    let rest: Vec<String> = g.rest_asked().to_vec();
    let services: Vec<String> = rest.iter().filter_map(|b| zikaron_anchor::rest::service_key(b)).collect();
    let mut keys: Vec<(String, String)> = shell.machine.rest_keys.0.iter().filter(|(s, _)| services.iter().any(|x| x.eq_ignore_ascii_case(s))).cloned().collect();
    keys.sort();
    (g.endpoints.clone(), rest, keys)
}

fn landed(shell: &mut Shell, before: &(Vec<String>, Vec<String>, Vec<(String, String)>)) {
    if asked(shell) != *before {
        super::archive::endpoints_changed(shell);
    }
}

pub(super) fn set_rest_mode(shell: &mut Shell, on: bool) -> Result<bool, crate::fault::Fault> {
    let before = asked(shell);
    shell.commit_settings(|s| s.rest_on = on)?;
    landed(shell, &before);
    Ok(on)
}

pub enum Added {
    Row(String),
    Keyed(String),
}

pub fn add_refused(shell: &Shell, base: &str, key: &str) -> Option<crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    use crate::lang::Key as L;
    use zikaron_anchor::rest::NotABase;
    let target = match zikaron_anchor::rest::base_of(base) {
        Ok(t) => t,
        Err(why) => {
            let f = Fault::known(Known::SettingsShape, crate::lang::both_of(|| crate::chainx::said_of(&zikaron_send::node::address_said(&zikaron_anchor::rest::lane(base, 0)))));
            return Some(match why {
                NotABase::Scheme | NotABase::Query => f.worded(Some(L::NetRestBaseBad), None),
                NotABase::Address(_) => f,
            });
        }
    };
    let key = key.trim();
    let service = target.authority();
    if (key.is_empty() && !zikaron_anchor::rest::file_gives(&service)) || key.chars().any(char::is_whitespace) {
        return Some(Fault::known(Known::SettingsShape, String::new()).worded(Some(L::NetRestKeyBad), None));
    }
    let listed = shell.settings.rest.iter().any(|b| zikaron_anchor::rest::service_key(b).is_some_and(|s| s == service));
    (listed && (key.is_empty() || shell.machine.rest_keys.of(&service).is_some_and(|k| !k.is_empty()))).then(|| Fault::known(Known::NetworkListed, service).worded(Some(L::NetRestListed), Some(L::NetRestListedNext)))
}

pub(super) fn add_rest(shell: &mut Shell, base: &str, key: &crate::secret::Secret) -> Result<Added, crate::fault::Fault> {
    if let Some(f) = add_refused(shell, base, key.expose()) {
        return Err(f);
    }
    let target = zikaron_anchor::rest::base_of(base).map_err(|_| crate::fault::Fault::known(crate::fault::Known::SettingsShape, String::new()))?;
    let was = asked(shell);
    let key = key.expose().trim();
    let service = target.authority();
    if let Some(listed) = shell.settings.rest.iter().find(|b| zikaron_anchor::rest::service_key(b).is_some_and(|s| s == service)).cloned() {
        let key = key.to_string();
        shell.machine = crate::machine::update(|m| {
            m.rest_keys.0.retain(|(s, _)| !s.eq_ignore_ascii_case(&service));
            m.rest_keys.0.push((service.clone(), key));
        })?;
        landed(shell, &was);
        return Ok(Added::Keyed(listed));
    }
    shell.may_save_settings()?;
    let written = target.url();
    if key.is_empty() {
        shell.commit_settings(|s| s.rest.push(written.clone()))?;
        landed(shell, &was);
        return Ok(Added::Row(written));
    }
    let key = key.to_string();
    let before = shell.machine.rest_keys.clone();
    shell.machine = crate::machine::update(|m| {
        m.rest_keys.0.retain(|(s, _)| !s.eq_ignore_ascii_case(&service));
        m.rest_keys.0.push((service.clone(), key));
    })?;
    if let Err(f) = shell.commit_settings(|s| s.rest.push(written.clone())) {
        if let Ok(m) = crate::machine::update(|m| m.rest_keys = before) {
            shell.machine = m;
        }
        return Err(f);
    }
    landed(shell, &was);
    Ok(Added::Row(written))
}

pub(super) fn remove_rest(shell: &mut Shell, base: &str) -> Result<usize, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    let service = zikaron_anchor::rest::service_key(base).ok_or_else(|| Fault::known(Known::SettingsShape, crate::lang::both_of(|| crate::chainx::said_of(&zikaron_send::node::address_said(&zikaron_anchor::rest::lane(base, 0))))))?;
    let ours = |b: &String| zikaron_anchor::rest::service_key(b).is_some_and(|s| s == service);
    if !shell.settings.rest.iter().any(ours) {
        return Err(Fault::known(Known::SettingsShape, service));
    }
    let before = asked(shell);
    shell.commit_settings(|s| s.rest.retain(|b| !ours(b)))?;
    let keys = crate::machine::update(|m| m.rest_keys.0.retain(|(s, _)| !s.eq_ignore_ascii_case(&service)));
    landed(shell, &before);
    shell.machine = keys?;
    Ok(shell.settings.rest.len())
}
