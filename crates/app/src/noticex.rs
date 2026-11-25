use zikaron::json::Value;
use crate::chainx::NoticeUnread as Unread;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notes {
    pub per: Vec<Option<Value>>,
    pub said: Vec<Option<String>>,
}

pub fn said(u: &Unread) -> String {
    match u {
        Unread::NoVenue => crate::lang::t(crate::lang::Key::NoticesNoVenue).to_string(),
        Unread::NoNode => crate::lang::t(crate::lang::Key::NoticesNoNode).to_string(),
        Unread::Contract(g) => format!("{} ({})", crate::lang::t(crate::lang::Key::NoticesContract), g.code()),
        Unread::Book => crate::lang::t(crate::lang::Key::NoticesBook).to_string(),
        Unread::Trouble(t) => crate::fault::Fault::from(t.clone()).evidence(),
    }
}

pub fn for_grants(grants: &[&[u8]], eps: &[crate::chainx::Endpoint]) -> Notes {
    let mut read: Vec<(u64, Result<Value, String>)> = Vec::new();
    let mut notes = Notes::default();
    for g in grants {
        let Some(chain) = zikaron_glue::notices::chain_of(g) else {
            notes.per.push(None);
            notes.said.push(None);
            continue;
        };
        if !read.iter().any(|(c, _)| *c == chain) {
            let got = crate::chainx::notices(eps, chain).map(|r| r.value).map_err(|u| format!("{} {chain}: {}", crate::lang::t(crate::lang::Key::NoticesUnread), said(&u)));
            read.push((chain, got));
        }
        let got = read.iter().find(|(c, _)| *c == chain).map(|(_, v)| v.clone());
        notes.per.push(got.clone().and_then(Result::ok));
        notes.said.push(got.and_then(Result::err));
    }
    notes
}

pub fn basis_line(n: &Value) -> String {
    let Some(b) = n.member("basis") else { return String::new() };
    let int = |k: &str| b.member(k).and_then(Value::as_int).map(|x| x.to_string()).unwrap_or_default();
    let text = |k: &str| b.member(k).and_then(Value::as_str).unwrap_or_default().to_string();
    format!("{} \u{b7} {} \u{b7} {}\u{2013}{} \u{b7} {} \u{b7} {}", int("chainId"), text("revocationContract"), int("fromBlock"), int("toBlock"), text("book"), int("bookFromBlock"))
}
