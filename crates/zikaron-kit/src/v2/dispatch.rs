use super::kit::{self, Enumeration};
use crate::doc::{self as doc1, Ack, Fpm, Reject};
use crate::kitdir::{self, KitVerdict as KitVerdict1};
use crate::tokens as t1;
use std::path::Path;
use zikaron::json::{self, Value};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Law {
    Kit1,
    Kit2,
}

fn spec_of(b: &[u8]) -> Option<String> {
    json::accept(b).ok()?.member("spec").and_then(Value::as_str).map(str::to_string)
}

pub fn manifest_law(manifest: Option<&[u8]>) -> Law {
    match manifest.and_then(spec_of) {
        Some(sp) if sp == t1::SPEC_KIT => Law::Kit1,
        _ => Law::Kit2,
    }
}

#[derive(Clone, Debug)]
pub enum KitReading {
    Kit1(KitVerdict1),
    Kit2(kit::KitVerdict),
}

pub fn read_kit(en: &Enumeration) -> KitReading {
    match manifest_law(en.get(super::tokens::MANIFEST).map(Vec::as_slice)) {
        Law::Kit1 => {
            let pairs: Vec<(String, Vec<u8>)> = en.iter().map(|(p, b)| (p.clone(), b.clone())).collect();
            KitReading::Kit1(kitdir::verify_enumeration(&pairs))
        }
        Law::Kit2 => KitReading::Kit2(kit::verify_kit(en)),
    }
}

pub fn read_kit_file(bytes: &[u8]) -> KitReading {
    match kit::parse_one_file(bytes) {
        Ok(en) => read_kit(&en),
        Err(subject) => KitReading::Kit2(kit::KitVerdict::Fail { token: super::tokens::KitToken::Unreadable, subject: Some(subject) }),
    }
}

pub fn read_kit_dir(dir: &Path) -> KitReading {
    match kit::walk(dir) {
        Ok(en) => read_kit(&en),
        Err(subject) => KitReading::Kit2(kit::KitVerdict::Fail { token: super::tokens::KitToken::Unreadable, subject: Some(subject) }),
    }
}

#[derive(Clone, Debug)]
pub enum OldDocument {
    Fpm(Result<Fpm, Reject>),
    Ack(Result<Ack, Reject>),
}

pub fn read_old_document(b: &[u8]) -> Option<OldDocument> {
    match spec_of(b)?.as_str() {
        x if x == t1::SPEC_FPM => Some(OldDocument::Fpm(doc1::check_fpm(b))),
        x if x == t1::SPEC_ACK => Some(OldDocument::Ack(doc1::check_ack(b))),
        _ => None,
    }
}
