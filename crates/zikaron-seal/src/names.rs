#[derive(Clone, Copy, Debug)]
pub enum Logical<'a> {
    Home(&'a str),
    Entry(&'a str),
    Held(&'a str),
    Kept(&'a str),
    TermsDir(&'a str),
    TermsDoc(&'a str),
    TermsRecord(&'a str),
    Slot(&'a str),
}

impl Logical<'_> {
    fn tag(&self) -> &'static [u8] {
        match self {
            Logical::Home(_) => b"home",
            Logical::Entry(_) => b"entry",
            Logical::Held(_) => b"held",
            Logical::Kept(_) => b"kept",
            Logical::TermsDir(_) => b"terms-dir",
            Logical::TermsDoc(_) => b"terms-doc",
            Logical::TermsRecord(_) => b"terms-record",
            Logical::Slot(_) => b"slot",
        }
    }

    fn value(&self) -> String {
        let v = match self {
            Logical::Home(x) | Logical::Entry(x) | Logical::Held(x) | Logical::Kept(x) | Logical::TermsDir(x) | Logical::TermsDoc(x) | Logical::TermsRecord(x) => x.trim().trim_start_matches("0x").to_ascii_lowercase(),
            Logical::Slot(a) => return a.to_string(),
        };
        v
    }

    fn width(&self) -> usize {
        match self {
            Logical::Home(_) => 40,
            _ => 64,
        }
    }
}

pub struct NameKey([u8; 32]);

impl NameKey {
    pub fn new(k: [u8; 32]) -> NameKey {
        NameKey(k)
    }

    pub fn name(&self, l: Logical) -> String {
        let mut msg = Vec::with_capacity(64);
        msg.extend_from_slice(l.tag());
        msg.push(0);
        msg.extend_from_slice(l.value().as_bytes());
        let mac = crate::crypto::hmac_sha256(&self.0, &msg);
        let hex = zikaron::hexfmt::encode(&mac);
        hex.trim_start_matches("0x")[..l.width()].to_string()
    }
}

impl Drop for NameKey {
    fn drop(&mut self) {
        zikaron_os::wipe(&mut self.0);
    }
}

pub fn is_home_name(n: &str) -> bool {
    n.len() == 40 && n.bytes().all(|b| b.is_ascii_hexdigit())
}
