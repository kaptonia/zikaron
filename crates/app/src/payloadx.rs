use crate::fault::{Fault, Known};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    Payload,
    PayloadFile,
    EntryFile,
    GrantFile,
}

impl Form {
    pub fn as_str(self) -> &'static str {
        match self {
            Form::Payload => "payload",
            Form::PayloadFile => "payload_file",
            Form::EntryFile => "entry_file",
            Form::GrantFile => "grant_file",
        }
    }
}

pub fn decode(payload: &[u8]) -> Result<Vec<Vec<u8>>, Fault> {
    match zikaron_kit::v2::doc::badge_decode(payload) {
        Ok(entries) => Ok(entries.iter().map(|e| e.bytes().to_vec()).collect()),
        Err(r) => Err(refused(r)),
    }
}

pub fn refused(r: zikaron_kit::v2::doc::DecodeReject) -> Fault {
    Fault::known(
        Known::PayloadRefused,
        crate::lang::both_of(|| format!(
            "{}{}{}",
            r.token.as_str(),
            r.index.map(|i| crate::lang::filln(crate::lang::Key::Tail087, &[&(i).to_string()])).unwrap_or_default(),
            r.inner.map(|t| format!(" {t:?}")).unwrap_or_default()
        )),
    )
}

pub fn take(typed: &str) -> Result<(Form, Vec<Vec<u8>>), Fault> {
    let t = take_full(typed)?;
    Ok((t.form, t.hops))
}

pub struct Taken {
    pub form: Form,
    pub hops: Vec<Vec<u8>>,
    pub file: Option<(std::path::PathBuf, crate::grantfilex::Opened)>,
    pub unsealed: Option<Vec<u8>>,
}

impl Taken {
    pub fn package_bytes(&self) -> Option<Result<Vec<u8>, Fault>> {
        let (p, _) = self.file.as_ref()?;
        Some(match &self.unsealed {
            Some(b) => Ok(b.clone()),
            None => std::fs::read(p).map_err(|e| crate::fault::classify(&e, &p.display().to_string())),
        })
    }
}

fn written_as_code(b: &[u8]) -> bool {
    let p = zikaron_kit::tokens::BADGE_PREFIX.as_bytes();
    b.len() >= p.len() && b[..p.len()].eq_ignore_ascii_case(p)
}

fn code_in(b: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(b).ok()?;
    let code: Vec<u8> = text.chars().filter(|c| !c.is_whitespace() && !unseen(*c)).collect::<String>().into_bytes();
    written_as_code(&code).then_some(code)
}

fn unseen(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{00AD}' | '\u{FEFF}')
}

pub fn take_full(typed: &str) -> Result<Taken, Fault> {
    let t = typed.trim();
    if t.is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail199).to_string())));
    }
    if let Some(code) = code_in(t.as_bytes()) {
        return Ok(Taken { form: Form::Payload, hops: decode(&code)?, file: None, unsealed: None });
    }
    let p = Path::new(t);
    if zikaron_glue::addressed::is_addressed_at(p) {
        let opened = crate::addressedx::open(p)?;
        let at = opened.file.display().to_string();
        let bytes = std::fs::read(&opened.file).map_err(|e| crate::fault::classify(&e, &at))?;
        let is_grant = zikaron_glue::container::decode(&bytes).is_ok_and(|pairs| zikaron_glue::grantfile::is_package(&pairs));
        if !is_grant {
            return Err(Fault::known(Known::AddressedElsewhere, p.display().to_string()));
        }
        let o = crate::grantfilex::open_bytes(&bytes)?;
        return Ok(Taken { form: Form::GrantFile, hops: o.hops.clone(), file: Some((p.to_path_buf(), o)), unsealed: Some(bytes) });
    }
    if crate::grantfilex::is_grant_file(p) {
        let o = crate::grantfilex::open(p)?;
        return Ok(Taken { form: Form::GrantFile, hops: o.hops.clone(), file: Some((p.to_path_buf(), o)), unsealed: None });
    }
    let bytes = std::fs::read(p).map_err(|e| crate::fault::classify(&e, &p.display().to_string()))?;
    if let Some(code) = code_in(&bytes) {
        return Ok(Taken { form: Form::PayloadFile, hops: decode(&code)?, file: None, unsealed: None });
    }
    Ok(Taken { form: Form::EntryFile, hops: vec![bytes], file: None, unsealed: None })
}
