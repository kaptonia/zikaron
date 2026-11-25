use crate::fault::Fault;
use crate::key::Address;
use zikaron::json::Value;
use zikaron_anchor::rpc;
pub use zikaron_send::Why;

pub use zikaron_send::batch::{before_of, fence, fence_open, Batch, Fenced, OnChain, FENCE_PAUSES};
pub use zikaron_send::book;
pub use zikaron_send::one;
pub use zikaron_send::sent::{machine as sent_machine, DIR as SENT_DIR};
pub use zikaron_send::checked::PLAIN_DIR;
pub use zikaron_send::batch::Lane;
pub use zikaron_send::node::{forget_serving, refusal_of, route_of, said, wei, Endpoint, Got, Https, Kind, Layer, NodeAddr, Refusal, Said, Wire, FEE_FACTS, MAX_HEAD_LAG, SEND_BACKOFF, TIP_FACTS, TX_FACTS};

pub fn speak() {
    zikaron_send::node::say_layers(|l, node, detail| crate::lang::filln(layer_key(l), &[node, detail]));
    zikaron_anchor::rest::keys_from(crate::machine::rest_keys_now);
}

pub const REST_KEYS_ENV: &str = "ZIKARON_REST_KEYS";

static KEYS_FILE: std::sync::OnceLock<Option<zikaron_anchor::rest::KeysFileBad>> = std::sync::OnceLock::new();

pub fn keys_file() -> Option<zikaron_anchor::rest::KeysFileBad> {
    *KEYS_FILE.get_or_init(|| {
        let path = std::env::var_os(REST_KEYS_ENV).filter(|p| !p.is_empty())?;
        match zikaron_anchor::rest::keys_file(std::path::Path::new(&path)) {
            Ok(keys) => {
                zikaron_anchor::rest::hold_file_keys(keys);
                None
            }
            Err(why) => Some(why),
        }
    })
}

pub fn keys_file_said() -> Option<String> {
    use crate::lang::{fill1, Key};
    use zikaron_anchor::rest::KeysFileBad;
    let key = match keys_file()? {
        KeysFileBad::Unreadable => Key::NetRestFileUnreadable,
        KeysFileBad::NotUtf8 => Key::NetRestFileNotUtf8,
        KeysFileBad::Shape => Key::NetRestFileShape,
    };
    Some(fill1(key, REST_KEYS_ENV))
}

pub fn nodes_of(s: &crate::settings::Settings) -> Vec<Endpoint> {
    zikaron_send::node::settings_nodes(&s.ground())
}

fn layer_key(l: Layer) -> crate::lang::Key {
    match l {
        Layer::Name => crate::lang::Key::Tail232,
        Layer::Connect => crate::lang::Key::Tail233,
        Layer::Handshake => crate::lang::Key::Tail234,
        Layer::Certificate => crate::lang::Key::Tail235,
    }
}

pub fn typed(spec: &str) -> Result<Endpoint, String> {
    Endpoint::typed(spec).map_err(|w| said_of(&w))
}

#[derive(Clone, Debug)]
pub struct Reading {
    pub value: Value,
    pub sources: usize,
    pub single_source: bool,
    pub unanswered: Vec<Why>,
    pub differed: Vec<Why>,
}

fn reading_of(r: zikaron_send::node::Reading) -> Reading {
    Reading { value: r.value, sources: r.sources, single_source: r.single_source, unanswered: r.unanswered, differed: r.differed }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Unreachable,
    Tls,
    Timeout,
    Unserved,
    NoKey,
    Limited,
    Credentials,
    OtherChain,
    Unreadable,
    Refused,
    Differed,
    Behind,
    BadAddress,
    Unproven,
    NoAnswer,
}

impl NodeKind {
    pub fn of_trouble(node: &str, t: &rpc::Trouble) -> NodeKind {
        use zikaron_send::Code as C;
        if matches!(t, rpc::Trouble::Transport(rpc::Broke::Proof(_))) {
            return NodeKind::Unproven;
        }
        use zikaron_send::node::{Layer, Said, Wire};
        if let Said::Wire(Wire::Tls(Layer::Name | Layer::Connect)) = zikaron_send::node::said(t) {
            return NodeKind::Unreachable;
        }
        match zikaron_send::node::said_trouble(node, t).code {
            C::Unreachable | C::ProxyUnreachable => NodeKind::Unreachable,
            C::NodeTls => NodeKind::Tls,
            C::NodeTimeout => NodeKind::Timeout,
            C::ServiceUnserved | C::MethodMissing => NodeKind::Unserved,
            C::ServiceKeyMissing => NodeKind::NoKey,
            C::RateLimited => NodeKind::Limited,
            C::NodeAuth | C::ProxyCredentialsMissing | C::ProxyCredentialsRefused => NodeKind::Credentials,
            C::WrongChain => NodeKind::OtherChain,
            C::AnswerTooLong | C::AnswerNotJson | C::ChainShape => NodeKind::Unreadable,
            C::NodeRefused | C::ContractRefused => NodeKind::Refused,
            _ => NodeKind::NoAnswer,
        }
    }

    pub fn said(self) -> &'static str {
        use crate::lang::{t, Key};
        t(match self {
            NodeKind::Unreachable => Key::NetSaidUnreachable,
            NodeKind::Tls => Key::NetSaidTls,
            NodeKind::Timeout => Key::NetSaidTimeout,
            NodeKind::Unserved => Key::NetSaidUnserved,
            NodeKind::NoKey => Key::NetSaidNoKey,
            NodeKind::Limited => Key::NetSaidLimited,
            NodeKind::Credentials => Key::NetSaidCredentials,
            NodeKind::OtherChain => Key::NetSaidOtherChain,
            NodeKind::Unreadable => Key::NetSaidUnreadable,
            NodeKind::Refused => Key::NetSaidRefused,
            NodeKind::Differed => Key::NetSaidDiffered,
            NodeKind::Behind => Key::NetSaidBehind,
            NodeKind::BadAddress => Key::NetSaidBadAddress,
            NodeKind::Unproven => Key::NetSaidUnproven,
            NodeKind::NoAnswer => Key::NetSaidNoAnswer,
        })
    }
}

pub fn kinds_of(w: &Why) -> Vec<(String, NodeKind)> {
    match w {
        Why::Node { node, trouble } => vec![(node.clone(), NodeKind::of_trouble(node, trouble))],
        Why::Unreadable { node } | Why::NotHeight { node, .. } | Why::NoTime { node } | Why::NonceUnread { node } => vec![(node.clone(), NodeKind::Unreadable)],
        Why::Differs { node } => vec![(node.clone(), NodeKind::Differed)],
        Why::Scheme { node } | Why::Port { node } | Why::NotAddress { node } | Why::NotSpec { node } => vec![(node.clone(), NodeKind::BadAddress)],
        Why::Behind { node, .. } => vec![(node.clone(), NodeKind::Behind)],
        Why::WrongChainNode { node, .. } => vec![(node.clone(), NodeKind::OtherChain)],
        Why::AtChain { why, .. } => kinds_of(why),
        Why::Each(all) => all.iter().flat_map(kinds_of).collect(),
        Why::Scan { unanswered, .. } => unanswered.iter().flat_map(kinds_of).collect(),
        _ => Vec::new(),
    }
}

pub fn plain_of(ws: &[Why]) -> Vec<String> {
    ws.iter().flat_map(plain_lines).collect()
}

fn plain_lines(w: &Why) -> Vec<String> {
    match w {
        Why::Node { node, trouble } => {
            let kind = NodeKind::of_trouble(node, trouble);
            vec![match (kind, zikaron_send::node_said(trouble)) {
                (NodeKind::Refused, Some(said)) => format!("{node}: {} \u{b7} {said}", kind.said()),
                _ => format!("{node}: {}", kind.said()),
            }]
        }
        Why::AtChain { why, .. } => plain_lines(why),
        Why::Each(all) => all.iter().flat_map(plain_lines).collect(),
        Why::Scan { unanswered, refused } => {
            let lines: Vec<String> = unanswered.iter().flat_map(|w| if let Why::Text(said) = w { vec![said.clone()] } else { plain_lines(w) }).collect();
            if lines.is_empty() {
                refused.iter().map(|r| r.detail()).collect()
            } else {
                lines
            }
        }
        other => kinds_of(other).into_iter().map(|(node, kind)| format!("{node}: {}", kind.said())).collect(),
    }
}

pub fn said_of(w: &Why) -> String {
    use crate::lang::{filln, t, Key};
    match w {
        Why::Text(s) => s.clone(),
        Why::Each(all) => all.iter().map(said_of).collect::<Vec<_>>().join(" · "),
        Why::Node { node, trouble } => node_words(node, trouble),
        Why::Unreadable { node } => filln(Key::Tail094, &[node]),
        Why::Differs { node } => filln(Key::TailDiffers, &[node]),
        Why::Scheme { node } => filln(Key::Tail093, &[node]),
        Why::Port { node } => filln(Key::TailNodePort, &[node]),
        Why::NotAddress { node } => filln(Key::TailNodeAddress, &[node]),
        Why::NotSpec { node } => filln(Key::Tail008, &[node]),
        Why::AtChain { chain, why } => format!("{chain}={}", said_of(why)),
        Why::NoHeadNode { chain } => filln(Key::Tail095, &[&chain.to_string()]),
        Why::NoTimeNode { chain } => filln(Key::Tail097, &[&chain.to_string()]),
        Why::NotHeight { node, text } => filln(Key::Tail096, &[node, text]),
        Why::Behind { node, lag, high } => filln(Key::TailNodeBehind, &[node, &lag.to_string(), &high.to_string()]),
        Why::NoTime { node } => filln(Key::Tail098, &[node]),
        Why::NotYetAt { has, not_yet } => filln(Key::TailNotYetAt, &[has, not_yet]),
        Why::Differ { count, sources } => filln(Key::Tail086, &[&count.to_string(), sources]),
        Why::NoHistory { nodes } => filln(Key::TailNoHistory, &[nodes]),
        Why::WrongChainNode { node, serves, chain } => filln(Key::TailWrongChainNode, &[node, &serves.to_string(), &chain.to_string()]),
        Why::BalanceNotText(s) => filln(Key::Tail099, &[s]),
        Why::BalanceNotQty(s) => filln(Key::Tail100, &[s]),
        Why::GasNotText(s) => filln(Key::Tail033, &[s]),
        Why::GasNotQty(s) => filln(Key::Tail034, &[s]),
        Why::NoAddress => t(Key::Tail013).to_string(),
        Why::Funds { need, have } => crate::action::funds_tail(*need, *have),
        Why::OnChain { names, tx, block } => format!("{} · {tx} · {block}", names.join(" ")),
        Why::TooEarly { chain, head_time, after } => format!("chainId={chain} headTime={head_time} after={after}"),
        Why::NoNodes => t(Key::Tail084).to_string(),
        Why::NonceUnread { node } => filln(Key::TailNonceUnread, &[node]),
        Why::Scan { unanswered, .. } => unanswered.iter().map(said_of).collect::<Vec<_>>().join(" · "),
    }
}

pub fn trouble_said(url: &str, t: &rpc::Trouble) -> String {
    node_words(&zikaron_net::sayable(url), t)
}

fn node_words(url: &str, t: &rpc::Trouble) -> String {
    let (what, inner) = match t {
        rpc::Trouble::Node(e) => (crate::lang::t(crate::lang::Key::Tail089), e.clone()),
        rpc::Trouble::Transport(b @ rpc::Broke::Status { .. }) => (crate::lang::t(crate::lang::Key::Tail089), b.evidence()),
        rpc::Trouble::Transport(b @ rpc::Broke::NoKey { .. }) => (crate::lang::t(crate::lang::Key::TailRestNoKey), b.evidence()),
        rpc::Trouble::Transport(b @ rpc::Broke::Unserved { .. }) => (crate::lang::t(crate::lang::Key::TailRestNotServed), b.evidence()),
        rpc::Trouble::Transport(rpc::Broke::LogsBehind { node }) => (crate::lang::t(crate::lang::Key::TailRestLogsBehind), node.clone()),
        rpc::Trouble::Transport(b @ rpc::Broke::HeadBehind { .. }) => (crate::lang::t(crate::lang::Key::TailHeadBehind), b.evidence()),
        rpc::Trouble::Transport(b) => (crate::lang::t(crate::lang::Key::Tail090), b.evidence()),
        rpc::Trouble::NotServed(k) => (crate::lang::t(crate::lang::Key::Tail091), k.clone()),
        rpc::Trouble::Contradiction(k) => (crate::lang::t(crate::lang::Key::Tail092), k.clone()),
    };
    let reason = match t {
        rpc::Trouble::Node(e) if refusal_of(e) == Refusal::Reverted => zikaron_anchor::said::reverted(e).map(|r| format!(" \u{b7} {}", r.evidence())).unwrap_or_default(),
        _ => String::new(),
    };
    if inner.contains(url) {
        format!("{what}:{inner}{reason}")
    } else {
        format!("{url}: {what}:{inner}{reason}")
    }
}

pub fn said_fault(url: &str, t: &rpc::Trouble) -> Fault {
    zikaron_send::node::said_trouble(url, t).into()
}

pub fn address_said(url: &str) -> String {
    said_of(&zikaron_send::node::address_said(url))
}

pub fn fee_source_line(fees: Option<&zikaron_anchor::send::Fees>) -> Option<crate::lang::Key> {
    match fees {
        Some(f) if f.from_chain => None,
        _ => Some(crate::lang::Key::U3FeeFallback),
    }
}

pub fn endpoint_at(url: &str) -> Option<Box<dyn rpc::Endpoint + Send>> {
    speak();
    zikaron_send::node::endpoint_at(url)
}

pub fn https(url: &str) -> Option<Https> {
    speak();
    Https::new(url)
}

pub use zikaron_send::node::OneRead;

pub fn ask(eps: &[Endpoint], method: &str, params: &Value) -> Result<Reading, Fault> {
    speak();
    zikaron_send::node::ask(eps, method, params).map(reading_of).map_err(Fault::from)
}

pub fn ask_call(eps: &[Endpoint], method: &str, params: &Value) -> Result<Reading, (Fault, bool)> {
    speak();
    zikaron_send::node::ask_call(eps, method, params).map(reading_of).map_err(|(t, call)| (Fault::from(t), call))
}

pub fn tx_known(urls: &[NodeAddr], chain: u64, hash: &[u8; 32]) -> bool {
    speak();
    zikaron_send::node::tx_known(urls, chain, hash)
}

pub use zikaron_send::node::AsHeld;

pub fn tx_as_held(urls: &[NodeAddr], chain: u64, tx: &str) -> AsHeld {
    speak();
    zikaron_send::node::tx_as_held(urls, chain, tx)
}

pub fn head_block(eps: &[Endpoint], chain: u64) -> Result<(u64, usize), Fault> {
    speak();
    zikaron_send::node::head_block(eps, chain).map_err(Fault::from)
}

pub use zikaron_send::notices::NowFrom;

pub fn now_of(injected: Option<u64>, eps: &[Endpoint], chain: Option<u64>) -> (Option<u64>, NowFrom) {
    speak();
    zikaron_send::notices::now_of(injected, eps, chain)
}

pub fn head_time(eps: &[Endpoint], chain: u64) -> Result<(u64, u64, usize), Fault> {
    speak();
    zikaron_send::node::head_time(eps, chain).map_err(Fault::from)
}

pub fn serving(eps: &[Endpoint], chain: u64) -> Result<(Vec<Endpoint>, Vec<String>), Fault> {
    speak();
    zikaron_send::node::serving(eps, chain).map(|(kept, left)| (kept, left.iter().map(said_of).collect())).map_err(Fault::from)
}

pub fn recipient_key(nets: &[RecipientNet], author: &[u8; 20]) -> zikaron_send::recipient::Found {
    speak();
    zikaron_send::recipient::key_of(nets, author)
}

pub use zikaron_send::recipient::{held as held_in, Found as RecipientFound, Net as RecipientNet};

pub fn key_for_root(
    root: &[u8; 20],
    held: Option<[u8; 20]>,
    nets: Vec<RecipientNet>,
    readonly: &[RecipientNet],
    unread: Vec<String>,
    pinned: &str,
) -> Result<([u8; 33], zikaron_send::recipient::From), (zikaron_seal::addressed::Why, String)> {
    speak();
    let kept = zikaron_send::checked::Record(crate::sendx::record::Desk);
    zikaron_send::recipient::key_for_root(root, held, nets, readonly, unread, pinned, Some(&kept))
}

pub fn book_current(root: &[u8; 20], offer: &[RecipientNet]) -> Option<[u8; 20]> {
    speak();
    zikaron_send::recipient::book_current(root, offer).0
}

pub fn fingerprint_gate(chain: u64, registry: &[u8; 20], nodes: &[String], pinned: &str) -> FingerprintGate {
    fingerprint_gate_counted(chain, registry, nodes, pinned).0
}

pub fn fingerprint_gate_counted(chain: u64, registry: &[u8; 20], nodes: &[String], pinned: &str) -> (FingerprintGate, Vec<String>) {
    speak();
    zikaron_send::fingerprint::gate_counted(chain, registry, nodes, pinned)
}

pub fn notices(eps: &[Endpoint], chain: u64) -> Result<NoticeRead, NoticeUnread> {
    speak();
    zikaron_send::notices::read(eps, chain)
}

pub use zikaron_send::notices::{Read as NoticeRead, Unread as NoticeUnread};

pub use zikaron_send::fingerprint::{Gate as FingerprintGate, Why as FingerprintMiss};

#[cfg(test)]
pub fn disagree_trouble(words: &str) -> SendTrouble {
    zikaron_send::Trouble::text(zikaron_send::Code::Disagree, words.to_string())
}

pub use zikaron_send::face::{Built as FaceBuilt, Lane as FaceLane, Left as FaceLeft, NotRead as FaceNotRead, NO_ROOT as FACE_NO_ROOT, omitted as face_omitted};
pub use zikaron_send::Trouble as SendTrouble;

pub fn face_input(eps: &[Endpoint], root: &[u8; 20], lanes: &[FaceLane], pile: &[Vec<u8>], facts: &dyn zikaron_send::Facts) -> Result<FaceBuilt, FaceNotRead> {
    speak();
    zikaron_send::face::input(eps, root, lanes, pile, facts)
}

pub fn balance(eps: &[Endpoint], chain: u64, who: &Address) -> Result<(u128, Reading), Fault> {
    crate::trace::mark(crate::feature::Feature::H4);
    speak();
    zikaron_send::node::balance(eps, chain, &who.0).map(|(n, r)| (n, reading_of(r))).map_err(Fault::from)
}

pub fn fees(eps: &[Endpoint], chain: u64) -> (zikaron_anchor::send::Fees, Vec<String>) {
    let r = fee_reading(eps, chain);
    (r.fees, r.left)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeReading {
    pub base: Option<u64>,
    pub fees: zikaron_anchor::send::Fees,
    pub left: Vec<String>,
}

pub fn fee_reading(eps: &[Endpoint], chain: u64) -> FeeReading {
    speak();
    let r = zikaron_send::node::fee_reading(eps, chain);
    FeeReading { base: r.base, fees: r.fees, left: r.left.iter().map(said_of).collect() }
}

#[cfg(test)]
mod plain_tests {
    use super::*;

    #[test]
    fn a_node_is_said_by_its_kind_without_its_code() {
        let node = |n: &str, b: rpc::Broke| Why::Node { node: n.to_string(), trouble: rpc::Trouble::Transport(b) };
        let ws = vec![
            node("https://a", rpc::Broke::Said("TLS_HANDSHAKE https://a: received fatal alert: HandshakeFailure".into())),
            node("http://b", rpc::Broke::Said("http://b: Connection refused (os error 61)".into())),
            node("https://f", rpc::Broke::Said("TLS_CONNECT https://f: Connection refused (os error 61)".into())),
            node("http://c", rpc::Broke::Late { node: "http://c".into(), deadline: std::time::Duration::from_secs(1) }),
            Why::AtChain { chain: 1, why: Box::new(node("http://d", rpc::Broke::Proof(rpc::ProofGap::NotInBlock))) },
            Why::Differs { node: "http://e".into() },
            Why::Text("eth_getLogs".into()),
        ];
        let w = crate::lang::both_of(|| plain_of(&ws).join(" · "));
        assert_eq!(w.zh, "https://a: 建立安全连接失败 · http://b: 连不上 · https://f: 连不上 · http://c: 未在时限内应答 · http://d: 证不出 · http://e: 答得不同");
        assert_eq!(w.en, "https://a: Secure connection failed · http://b: Could not connect · https://f: Could not connect · http://c: No answer in time · http://d: No proof · http://e: Answered differently");
        let evidence = ws.iter().map(said_of).collect::<Vec<_>>().join(" · ");
        assert!(evidence.contains("TLS_HANDSHAKE") && evidence.contains("os error 61"), "the evidence keeps them: {evidence}");
    }
}
