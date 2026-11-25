use crate::vaultx::Card;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Revoked,
    Handed,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Revoked => "revoked",
            Kind::Handed => "handed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alarm {
    pub grant: String,
    pub author: String,
    pub kind: Kind,
    pub to: Option<String>,
    pub at: Option<u64>,
}

pub fn key_of(a: &Alarm) -> String {
    match &a.to {
        Some(to) => format!("{}:{}:{}", a.kind.as_str(), a.grant.to_ascii_lowercase(), to.to_ascii_lowercase()),
        None => format!("{}:{}", a.kind.as_str(), a.grant.to_ascii_lowercase()),
    }
}

pub fn handed_after(upstream: &[Vec<u8>], grant: &str) -> Option<String> {
    let read = zikaron_glue::book::read(upstream, &zikaron_glue::book::empty_fragment()).ok()?;
    let at = read.position(grant)?;
    read.entries[at + 1..].iter().filter(|e| e.hands_off()).last().and_then(|e| e.to().map(str::to_string))
}

pub fn alarms(cards: &[Card], already: &[String], now: Option<u64>) -> (Vec<Alarm>, Vec<String>) {
    crate::trace::mark(crate::feature::Feature::D7);
    let mut out: Vec<Alarm> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for c in cards {
        let mut found: Vec<Alarm> = Vec::new();
        if c.failed.iter().any(|x| x == zikaron_kit::tokens::Check::Revoked.as_str()) {
            found.push(Alarm { grant: c.id.clone(), author: c.author.clone(), kind: Kind::Revoked, to: None, at: now });
        }
        if let Some(to) = &c.handed {
            found.push(Alarm { grant: c.id.clone(), author: c.author.clone(), kind: Kind::Handed, to: Some(to.clone()), at: now });
        }
        for a in found {
            let k = key_of(&a);
            if already.iter().any(|x| *x == k) || keys.iter().any(|x| *x == k) {
                continue;
            }
            keys.push(k);
            out.push(a);
        }
    }
    (out, keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: char, author: &str, revoked: bool, handed: Option<&str>) -> Card {
        let h = crate::vaultx::Held {
            id: format!("0x{}", id.to_string().repeat(64)),
            bytes: b"not a grant".to_vec(),
            author: author.to_string(),
            grantee: String::new(),
            work: String::new(),
            terms: String::new(),
            window: None,
            upstream: None,
            seq: 1,
        };
        let mut c = crate::vaultx::review(&h, &[], &crate::auditx::empty_fragment(), None, 0);
        c.failed = if revoked { vec![zikaron_kit::tokens::Check::Revoked.as_str().to_string()] } else { Vec::new() };
        c.handed = handed.map(str::to_string);
        c
    }

    #[test]
    fn every_held_grant_of_every_issuer_is_watched() {
        let (a, b) = (format!("0x{}", "aa".repeat(20)), format!("0x{}", "bb".repeat(20)));
        let to = format!("0x{}", "cc".repeat(20));
        let cards = vec![card('1', &a, true, None), card('2', &b, false, None), card('3', &b, true, Some(&to)), card('4', &a, false, None)];
        let (fresh, keys) = alarms(&cards, &[], Some(9));
        let seen: Vec<(String, Kind)> = fresh.iter().map(|x| (x.author.clone(), x.kind)).collect();
        assert_eq!(seen, vec![(a.clone(), Kind::Revoked), (b.clone(), Kind::Revoked), (b.clone(), Kind::Handed)]);
        assert_eq!(keys.len(), 3);
        assert!(alarms(&cards, &keys, Some(10)).0.is_empty(), "each once");
    }
}
