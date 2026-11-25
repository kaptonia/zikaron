use crate::DOMAIN;
use crate::shapes::text;
use zikaron::cryptox;
use zikaron::entry as v1;
use zikaron::hexfmt;
use zikaron::json::Value;

pub fn declaration(root: &str, address: &str, role: &str) -> Vec<u8> {
    format!("{DOMAIN}\nroot:{root}\naddress:{address}\nrole:{role}").into_bytes()
}

pub fn message(declaration: &[u8]) -> Vec<u8> {
    v1::message(DOMAIN, &cryptox::sha256(declaration))
}

pub fn digest(declaration: &[u8]) -> [u8; 32] {
    v1::eip191_digest(&message(declaration))
}

pub fn holds(body: &Value) -> bool {
    let d = digest(&declaration(text(body, "root"), text(body, "address"), text(body, "role")));
    v1::verify_signature(text(body, "sig"), &d, text(body, "address")).is_ok()
}

pub fn sign(privkey: &[u8; 32], root: &str, address: &str, role: &str) -> Option<[u8; 65]> {
    let (r, s, v) = cryptox::sign_digest(privkey, &digest(&declaration(root, address, role)))?;
    let mut sig = [0u8; 65];
    sig[..32].copy_from_slice(&r);
    sig[32..64].copy_from_slice(&s);
    sig[64] = v;
    Some(sig)
}

pub fn sig_hex(sig: &[u8; 65]) -> String {
    hexfmt::encode(sig)
}
