use crate::node::{self, Endpoint};
use crate::{Code, Trouble, Why};
use zikaron::json::Value;
use zikaron_anchor::book::{self, Call};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Off,
    Wrong,
    At([u8; 20]),
}

pub fn state(eps: &[Endpoint], chain: u64) -> Result<State, Trouble> {
    let Some(at) = book::at(chain) else { return Ok(State::Off) };
    let _read = node::OneRead::open();
    let eps = serving(eps, chain)?;
    let (height, _) = node::head_block(&eps, chain)?;
    let r = node::ask(&eps, "eth_getCode", &Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(&at)), Value::Str(format!("0x{height:x}"))]))?;
    let code = r.value.as_str().and_then(zikaron::hexfmt::decode).ok_or_else(|| Trouble::new(Code::ChainShape, Why::Text(format!("eth_getCode {:?}", r.value))))?;
    Ok(if book::is_the_book(&code) { State::At(at) } else { State::Wrong })
}

pub fn ahead(eps: &[Endpoint], chain: u64, views: &[(Call, [u8; 20])], more: &(dyn Fn(u64) -> Vec<zikaron_anchor::rpc::Ask<'static>> + Sync)) {
    let Some(at) = book::at(chain) else { return };
    node::ask_ahead(eps, chain, &|_, lowest, _| {
        let h = format!("0x{lowest:x}");
        let mut asks: Vec<zikaron_anchor::rpc::Ask<'static>> = vec![("eth_getCode", Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(&at)), Value::Str(h.clone())]))];
        asks.extend(views.iter().map(|(call, who)| ("eth_call", book::call_params(&at, &call.data(who), &h))));
        asks.extend(more(lowest));
        asks
    });
}

fn view(eps: &[Endpoint], chain: u64, at: &[u8; 20], call: Call, who: &[u8; 20]) -> Result<String, Trouble> {
    let _read = node::OneRead::open();
    let eps = serving(eps, chain)?;
    let (height, _) = node::head_block(&eps, chain)?;
    let r = node::ask(&eps, "eth_call", &book::call_params(at, &call.data(who), &format!("0x{height:x}")))?;
    r.value.as_str().map(str::to_string).ok_or_else(|| Trouble::new(Code::ChainShape, Why::Text(format!("eth_call {:?}", r.value))))
}

fn serving(eps: &[Endpoint], chain: u64) -> Result<Vec<Endpoint>, Trouble> {
    node::serving(eps, chain).map(|(kept, _)| kept)
}

pub fn address(eps: &[Endpoint], chain: u64, at: &[u8; 20], call: Call, who: &[u8; 20]) -> Result<[u8; 20], Trouble> {
    let answer = view(eps, chain, at, call, who)?;
    book::address_word(&answer).ok_or_else(|| Trouble::new(Code::ChainShape, Why::Text(format!("{} {answer}", call.signature()))))
}

pub fn entered(eps: &[Endpoint], chain: u64, at: &[u8; 20], who: &[u8; 20]) -> Result<bool, Trouble> {
    let answer = view(eps, chain, at, Call::Entered, who)?;
    book::bool_word(&answer).ok_or_else(|| Trouble::new(Code::ChainShape, Why::Text(format!("entered {answer}"))))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held {
    pub root: [u8; 20],
    pub current: [u8; 20],
}

impl Held {
    pub fn key_under(&self, root: &[u8; 20]) -> Option<[u8; 20]> {
        (self.root == *root).then_some(self.current)
    }

    pub fn holds(&self, root: &[u8; 20], key: &[u8; 20]) -> bool {
        self.key_under(root) == Some(*key)
    }
}

pub fn held(eps: &[Endpoint], chain: u64, at: &[u8; 20], root: &[u8; 20]) -> Result<Held, Trouble> {
    Ok(Held { root: address(eps, chain, at, Call::Root, root)?, current: address(eps, chain, at, Call::Current, root)? })
}

pub fn seated(eps: &[Endpoint], chain: u64, root: &[u8; 20], key: &[u8; 20], lineage: &[[u8; 20]]) -> Result<(), Trouble> {
    ahead(eps, chain, &[(Call::Root, *root), (Call::Current, *root)], &|_| Vec::new());
    let at = match state(eps, chain)? {
        State::At(at) => at,
        _ => return Err(Trouble::text(Code::BookCode, chain.to_string())),
    };
    let h = held(eps, chain, &at, root)?;
    if h.holds(root, key) {
        return Ok(());
    }
    let said = format!("{chain} root={} office={} book={} rootOfRoot={}", zikaron::hexfmt::encode(root), zikaron::hexfmt::encode(key), zikaron::hexfmt::encode(&h.current), zikaron::hexfmt::encode(&h.root));
    let code = match h.key_under(root) {
        Some(k) if k == *root => Code::RootNotInBook,
        Some(k) if lineage.contains(&k) => Code::NotAcceded,
        _ => Code::OfficeElsewhere,
    };
    Err(Trouble::text(code, said))
}
