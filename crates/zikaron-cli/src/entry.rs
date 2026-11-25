use crate::codes::Field;
use zikaron::entry as k1;
use zikaron::hexfmt;
use zikaron::json::Value;
use zikaron::tokens::Token;
use zikaron_glue::book::{self, Law};

pub struct Sealed {
    pub bytes: Vec<u8>,
    pub entry: book::Entry,
}

pub fn shape(members: Vec<(Field, Value)>) -> Value {
    Value::Obj(
        members
            .into_iter()
            .map(|(k, v)| (k.as_str().to_string(), v))
            .collect(),
    )
}

pub fn address(key: &[u8; 32]) -> Option<String> {
    crate::seam();
    zikaron::cryptox::address_of_privkey(key).map(|a| hexfmt::encode(&a))
}

pub fn seal(author: &str, entry_type: &str, seq: u64, prev: Option<&str>, body: Value, key: &[u8; 32]) -> Result<Sealed, Token> {
    seal_under(Law::Two, author, entry_type, seq, prev, body, key)
}

pub fn seal_under(law: Law, author: &str, entry_type: &str, seq: u64, prev: Option<&str>, body: Value, key: &[u8; 32]) -> Result<Sealed, Token> {
    crate::seam();
    match book::seal(law, author, entry_type, seq, prev, body, |pre, domain| sign(pre, domain, key)) {
        book::Sealed::Done(e) => Ok(Sealed { bytes: e.bytes.clone(), entry: e }),
        book::Sealed::Refused(t) | book::Sealed::Signer(t) => Err(t),
    }
}

pub fn sign(preimage: &[u8], domain: &str, key: &[u8; 32]) -> Result<String, Token> {
    crate::seam();
    let (_, digest) = k1::presig_and_digest(preimage, domain);
    let (r, s, v) = match zikaron::cryptox::sign_digest(key, &digest) {
        Some(x) => x,
        None => return Err(Token::SigRecover),
    };
    let mut raw = Vec::with_capacity(65);
    raw.extend_from_slice(&r);
    raw.extend_from_slice(&s);
    raw.push(v);
    Ok(hexfmt::encode(&raw))
}
