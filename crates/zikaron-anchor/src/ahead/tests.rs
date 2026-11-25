use super::*;
use crate::wire::{self, Body};

fn doc() -> W {
    wire::parse(include_bytes!("sepolia.json")).expect("the fixture reads")
}

fn blocks() -> Vec<W> {
    doc().member("blocks").and_then(W::as_arr).expect("blocks").to_vec()
}

fn h32(w: &W, k: &str) -> [u8; 32] {
    crate::tx::data(w, k).and_then(|b| <[u8; 32]>::try_from(b).ok()).expect("32 bytes")
}

fn block(n: &str) -> W {
    blocks().into_iter().find(|b| b.member("number").and_then(W::as_str) == Some(n)).expect("that block")
}

fn proof(who: &str, n: &str) -> Vec<Vec<u8>> {
    let d = doc();
    let p = d.member("proofs").and_then(W::as_arr).expect("proofs").iter().find(|p| p.member("address").and_then(W::as_str) == Some(who) && p.member("block").and_then(W::as_str) == Some(n)).cloned().expect("that proof");
    p.member("accountProof").and_then(W::as_arr).expect("nodes").iter().map(|x| x.as_str().and_then(zikaron::hexfmt::decode).expect("hex")).collect()
}

fn with(w: &W, k: &str, v: &str) -> W {
    let Body::Obj(ms) = &w.body else { panic!("an object") };
    let ms = ms.iter().map(|(n, x)| (n.clone(), if n == k { W::of(Body::Str(v.to_string())) } else { x.clone() })).collect();
    W::of(Body::Obj(ms))
}

const REVOCATION: [u8; 20] = [0x50, 0x6e, 0x53, 0x6d, 0x4b, 0x46, 0x3f, 0x48, 0xbb, 0x05, 0x3f, 0xe7, 0xc3, 0xdf, 0x26, 0x5b, 0xc2, 0x1b, 0x02, 0xfe];
const SENDER: [u8; 20] = [0x84, 0xa0, 0xfa, 0x07, 0xd9, 0x66, 0xb2, 0xba, 0x1f, 0xae, 0x86, 0xce, 0x79, 0xa0, 0xe2, 0x1e, 0x97, 0xa8, 0xdc, 0x8f];

#[test]
fn a_real_header_hashes_to_its_block_and_one_byte_changed_does_not() {
    for b in blocks() {
        let hash = h32(&b, "hash");
        let h = header_of(&b, &hash).expect("proved");
        assert_eq!(h.number, crate::tx::qty_u64(&b, "number").expect("number"));
        assert_eq!(h.timestamp, crate::tx::qty_u64(&b, "timestamp").expect("time"));
        assert_eq!(h.parent, h32(&b, "parentHash"));
        for (k, v) in [("timestamp", "0x1"), ("stateRoot", &format!("0x{}", "11".repeat(32))), ("extraData", "0x00"), ("slotNumber", "0x1")] {
            assert_eq!(header_of(&with(&b, k, v), &hash), Err(ProofGap::HeaderHash), "{k} changed");
        }
        let Body::Obj(ms) = &b.body else { panic!("an object") };
        let lacking = W::of(Body::Obj(ms.iter().filter(|(n, _)| n != "stateRoot").cloned().collect()));
        assert_eq!(header_of(&lacking, &hash), Err(ProofGap::HeaderUnbuilt), "a required field missing");
    }
    assert_eq!(header_of(&block("0xb8a704"), &h32(&block("0xb8a704"), "hash")).expect("child").parent, h32(&block("0xb8a703"), "hash"));
}

#[test]
fn the_code_at_a_block_is_read_from_a_real_account_proof_and_nothing_else_proves() {
    let root = |n: &str| h32(&block(n), "stateRoot");
    assert_eq!(code_in(&root("0xb8a703"), &REVOCATION, &proof("0x506e536d4b463f48bb053fe7c3df265bc21b02fe", "0xb8a703")), Ok(true), "made in that block");
    assert_eq!(code_in(&root("0xb8a702"), &REVOCATION, &proof("0x506e536d4b463f48bb053fe7c3df265bc21b02fe", "0xb8a702")), Ok(false), "absent before");
    assert_eq!(code_in(&root("0xb8a704"), &SENDER, &proof("0x84a0fa07d966b2ba1fae86ce79a0e21e97a8dc8f", "0xb8a704")), Ok(false), "a key: no code");
    assert_eq!(code_in(&root("0xb8a703"), &SENDER, &proof("0x84a0fa07d966b2ba1fae86ce79a0e21e97a8dc8f", "0xb8a703")), Ok(false));
    let good = proof("0x84a0fa07d966b2ba1fae86ce79a0e21e97a8dc8f", "0xb8a704");
    for i in 0..good.len() {
        let mut bad = good.clone();
        let mid = bad[i].len() / 2;
        bad[i][mid] ^= 1;
        assert_eq!(code_in(&root("0xb8a704"), &SENDER, &bad), Err(ProofGap::AccountProof), "node {i} changed");
    }
    let mut other = SENDER;
    other[19] ^= 1;
    assert_eq!(code_in(&root("0xb8a704"), &other, &good), Err(ProofGap::AccountProof), "another account");
    assert_eq!(code_in(&root("0xb8a703"), &SENDER, &good), Err(ProofGap::AccountProof), "another root");
    let mut coded = good.clone();
    let last = coded.len() - 1;
    let empty = zikaron::cryptox::keccak256(&[]);
    let at = coded[last].windows(32).position(|w| w == empty).expect("the code hash in the leaf");
    coded[last][at] ^= 1;
    assert_eq!(code_in(&root("0xb8a704"), &SENDER, &coded), Err(ProofGap::AccountProof), "the code hash changed");
    assert_eq!(code_in(&root("0xb8a704"), &SENDER, &good[..good.len() - 1]), Err(ProofGap::AccountProof), "cut short");
}

struct Batching {
    takes: bool,
    lies: bool,
    batches: Vec<(String, usize)>,
    singles: Vec<String>,
}

impl Batching {
    fn answer(&self, method: &str, params: &Value) -> W {
        let Value::Arr(ps) = params else { return W::of(Body::Null) };
        match method {
            "eth_getBlockByHash" => {
                let h = ps.first().and_then(Value::as_str).unwrap_or_default();
                let b = blocks().into_iter().find(|b| b.member("hash").and_then(W::as_str) == Some(h));
                match (b, self.lies) {
                    (Some(b), true) => with(&b, "timestamp", "0x1"),
                    (Some(b), false) => b,
                    (None, _) => W::of(Body::Null),
                }
            }
            "eth_getProof" => {
                let (who, n) = (ps.first().and_then(Value::as_str).unwrap_or_default(), ps.get(2).and_then(Value::as_str).unwrap_or_default());
                let d = doc();
                d.member("proofs").and_then(W::as_arr).and_then(|all| all.iter().find(|p| p.member("address").and_then(W::as_str) == Some(who) && p.member("block").and_then(W::as_str) == Some(n)).cloned()).unwrap_or(W::of(Body::Null))
            }
            _ => W::of(Body::Null),
        }
    }
}

impl Endpoint for Batching {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        self.singles.push(method.to_string());
        Ok(self.answer(method, params))
    }
    fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, Trouble>> {
        let mut kinds: Vec<&str> = asks.iter().map(|(m, _)| *m).collect();
        kinds.dedup();
        self.batches.push((kinds.join("+"), asks.len()));
        if !self.takes {
            return Some(Err(Trouble::Transport(crate::rpc::Broke::Status { node: self.place(), code: 400 })));
        }
        Some(Ok(asks.iter().map(|(m, p)| Some(Ok(self.answer(m, p)))).collect()))
    }
    fn name(&self) -> String {
        self.place()
    }
    fn place(&self) -> String {
        format!("batching-{}-{}", self.takes, self.lies)
    }
}

fn wants() -> Vec<Want> {
    let b = block("0xb8a704");
    let p = block("0xb8a703");
    vec![
        Want { tx: [0xd2; 32], block: h32(&b, "hash"), number: 0xb8a704, who: SENDER, hash: [0x5a; 32] },
        Want { tx: [0xd3; 32], block: h32(&p, "hash"), number: 0xb8a703, who: SENDER, hash: [0x5a; 32] },
    ]
}

#[test]
fn a_window_is_read_ahead_in_batches_kept_once_proved_and_a_refusal_is_remembered() {
    let wants = wants();
    {
        let _action = crate::action::open(None);
        let mut node = Batching { takes: true, lies: false, batches: Vec::new(), singles: Vec::new() };
        read_ahead(&mut node, 11155111, &wants, Share::WHOLE);
        let kinds: Vec<&str> = node.batches.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(kinds, ["eth_getTransactionReceipt", "eth_getBlockByHash+eth_getProof", "eth_getTransactionByHash+eth_getCode"], "the receipts, then the blocks and every proof in one batch (no block before: each proof checks against a block read by its hash, #606), then the transactions no round holds and the boundary no block read ahead proves");
        assert!(node.singles.is_empty(), "no single question");
        let _at = reading_at(Some(wants[0].block));
        assert_eq!(time_at(11155111, 0xb8a704), crate::tx::qty_u64(&block("0xb8a704"), "timestamp"), "the block's time proved");
        assert_eq!((code_at(11155111, &SENDER, 0xb8a704), code_at(11155111, &SENDER, 0xb8a703)), (Some(false), Some(false)), "both boundaries proved");
        assert_eq!(time_at(11155111, 0xb8a705), None, "only the log's own block");
        assert!(unproven().is_empty());
    }
    {
        let _action = crate::action::open(None);
        let mut node = Batching { takes: false, lies: false, batches: Vec::new(), singles: Vec::new() };
        read_ahead(&mut node, 11155111, &wants, Share::WHOLE);
        read_ahead(&mut node, 11155111, &wants, Share::WHOLE);
        assert_eq!(node.batches.len(), 1, "refused once, not asked in batch again");
        assert!(unbatched().iter().any(|(p, m)| *p == node.place() && m == WHOLE), "a batch refused whole: the node, every method");
        let _at = reading_at(Some(wants[0].block));
        assert_eq!(time_at(11155111, 0xb8a704), None, "nothing read ahead: asked as before");
    }
    {
        let _action = crate::action::open(None);
        let mut node = Batching { takes: true, lies: true, batches: Vec::new(), singles: Vec::new() };
        read_ahead(&mut node, 11155111, &wants, Share::WHOLE);
        let _at = reading_at(Some(wants[0].block));
        assert_eq!(time_at(11155111, 0xb8a704), None, "a block that does not prove is not kept");
        assert!(unproven().iter().any(|(_, p, g)| *p == node.place() && *g == ProofGap::HeaderHash), "the node named as not proving: {:?}", unproven());
        assert_eq!(node.batches.len(), 3, "{:?}", node.batches);
        assert_eq!(node.batches[2].1, 2 + 2 + 3, "two transactions, two times, three boundaries: {:?}", node.batches);
        assert!(node.singles.is_empty(), "no single question: {:?}", node.singles);
    }
    let mut node = Batching { takes: true, lies: false, batches: Vec::new(), singles: Vec::new() };
    read_ahead(&mut node, 11155111, &wants, Share::WHOLE);
    assert!(node.batches.is_empty());
}

struct Scripted {
    place: &'static str,
    script: std::collections::VecDeque<Result<bool, Trouble>>,
    asked: usize,
}

impl Endpoint for Scripted {
    fn call(&mut self, _: &str, _: &Value) -> Result<W, Trouble> {
        Ok(W::of(Body::Null))
    }
    fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, Trouble>> {
        self.asked += 1;
        Some(match self.script.pop_front().unwrap_or(Ok(true)) {
            Ok(true) => Ok(asks.iter().map(|_| Some(Ok(W::of(Body::Str("answered".into()))))).collect()),
            Ok(false) => Ok(asks.iter().map(|_| Some(Err(Trouble::Node(r#"{"code":-32000,"message":"rate limit exceeded"}"#.into())))).collect()),
            Err(t) => Err(t),
        })
    }
    fn name(&self) -> String {
        self.place.to_string()
    }
    fn place(&self) -> String {
        self.place.to_string()
    }
}

#[test]
fn a_passing_fault_is_not_remembered_as_a_node_that_takes_no_batch_and_what_came_is_kept() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let two: Vec<crate::rpc::Ask<'_>> = vec![("eth_getProof", Value::Null), ("eth_getProof", Value::Null)];
    let status = |code: u16| Trouble::Transport(Broke::Status { node: "n".into(), code });
    let late = Trouble::Transport(Broke::Late { node: "n".into(), deadline: std::time::Duration::from_secs(1) });
    for (place, first) in [("late", Err(late)), ("busy", Err(status(503))), ("limited", Err(status(429))), ("limited-each", Ok(false))] {
        let script: std::collections::VecDeque<_> = std::iter::repeat_with(|| first.clone()).take(4).collect();
        let mut node = Scripted { place, script, asked: 0 };
        let _ = in_batches(&mut node, &two);
        assert!(!unbatched().iter().any(|(p, _)| p == place), "{place}: not remembered");
        node.script.clear();
        let got = in_batches(&mut node, &two).expect("batched again");
        assert!(got.iter().all(|a| value(a).is_some()), "{place}: asked in batch again and answered");
    }
    for (place, refusal) in [("refuses-400", status(400)), ("refuses-said", Trouble::Node(r#"{"code":-32600,"message":"batch requests are not supported"}"#.into()))] {
        let mut node = Scripted { place, script: [Err(refusal)].into_iter().collect(), asked: 0 };
        assert!(in_batches(&mut node, &two).is_none());
        assert!(unbatched().iter().any(|(p, m)| p == place && m == WHOLE), "{place}: remembered");
        assert!(in_batches(&mut node, &two).is_none());
        assert_eq!(node.asked, 1, "{place}: not asked in batch again");
    }
    let many: Vec<crate::rpc::Ask<'_>> = (0..BATCH_MAX + 5).map(|_| ("eth_getBlockByHash", Value::Null)).collect();
    let late = Trouble::Transport(Broke::Late { node: "n".into(), deadline: std::time::Duration::from_secs(1) });
    let mut node = Scripted { place: "half", script: [Ok(true), Err(late)].into_iter().collect(), asked: 0 };
    let got = in_batches(&mut node, &many).expect("the first batch kept");
    assert_eq!(got.len(), many.len());
    assert!(got[..BATCH_MAX].iter().all(|a| value(a).is_some()) && got[BATCH_MAX..].iter().all(Option::is_none), "the twenty kept, the five to ask one by one");
    assert!(!unbatched().iter().any(|(p, _)| p == "half"));
}

struct Refusing {
    place: &'static str,
    refuses: &'static [&'static str],
}

impl Endpoint for Refusing {
    fn call(&mut self, _: &str, _: &Value) -> Result<W, Trouble> {
        Ok(W::of(Body::Null))
    }
    fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, Trouble>> {
        Some(Ok(asks
            .iter()
            .map(|(m, _)| Some(if self.refuses.contains(m) { Err(Trouble::Node(r#"{"code":-32603,"message":"execution reverted"}"#.into())) } else { Ok(W::of(Body::Str("answered".into()))) }))
            .collect()))
    }
    fn name(&self) -> String {
        self.place.to_string()
    }
    fn place(&self) -> String {
        self.place.to_string()
    }
}

#[test]
fn one_question_refused_in_a_batch_leaves_its_method_batched() {
    let mut node = Refusing { place: "refusing-estimate", refuses: &["eth_estimateGas"] };
    let mixed: Vec<crate::rpc::Ask<'_>> = vec![("eth_estimateGas", Value::Null), ("eth_getBlockByNumber", Value::Null)];
    let got = in_batches(&mut node, &mixed).expect("batched");
    assert!(matches!(got[0], Some(Err(_))) && value(&got[1]).is_some());
    assert!(!unbatched().iter().any(|(p, _)| p == "refusing-estimate"), "one refused question remembers nothing: {:?}", unbatched());
    let mut node = Refusing { place: "refusing-proofs", refuses: &["eth_getProof"] };
    let proofs: Vec<crate::rpc::Ask<'_>> = vec![("eth_getProof", Value::Null), ("eth_getProof", Value::Arr(Vec::new())), ("eth_getBlockByHash", Value::Null)];
    let _ = in_batches(&mut node, &proofs);
    let kept: Vec<(String, String)> = unbatched().into_iter().filter(|(p, _)| p == "refusing-proofs").collect();
    assert_eq!(kept, [("refusing-proofs".to_string(), "eth_getProof".to_string())], "that method alone");
}

#[test]
fn a_batch_limited_past_its_table_is_halved_and_the_halves_answer() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let status = |code: u16| Trouble::Transport(Broke::Status { node: "n".into(), code });
    let many: Vec<crate::rpc::Ask<'_>> = (0..BATCH_MAX + 5).map(|_| ("eth_getCode", Value::Null)).collect();
    let mut node = Scripted { place: "limited-every-time", script: [Ok(true), Err(status(429)), Err(status(429)), Err(status(429)), Ok(true), Ok(true)].into_iter().collect(), asked: 0 };
    let r = in_batches_said(&mut node, &many, false);
    let got = r.got.expect("answered");
    assert!(got.iter().all(|a| value(a).is_some()), "every answer kept: {got:?}");
    assert_eq!(node.asked, 6, "the twenty, the five limited three times, then its two halves");
    assert!(r.limited.iter().all(|l| !l), "nothing left limited");
    assert!(!unbatched().iter().any(|(p, _)| p.starts_with("limited-")), "a rate limit is the moment's");
}

#[derive(Default)]
struct Seen {
    out: usize,
    most: usize,
    posts: Vec<Vec<(String, String)>>,
    singles: Vec<String>,
}

#[derive(Clone)]
struct Wide {
    place: String,
    doubles: bool,
    proofs: bool,
    limits_proofs: Option<&'static str>,
    limits_in_place: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    limited: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    pause: u64,
    seen: std::sync::Arc<std::sync::Mutex<Seen>>,
}

impl Wide {
    fn new(place: &str, doubles: bool) -> Wide {
        Wide { place: place.to_string(), doubles, proofs: true, limits_proofs: None, limits_in_place: Default::default(), limited: Default::default(), pause: 30, seen: Default::default() }
    }

    fn answer(&self, method: &str, params: &Value) -> Result<W, Trouble> {
        let Value::Arr(ps) = params else { return Ok(W::of(Body::Null)) };
        let first = ps.first().and_then(Value::as_str).unwrap_or_default().to_string();
        Ok(match method {
            "eth_getProof" if !self.proofs => return Err(Trouble::Node(r#"{"code":-32601,"message":"the method eth_getProof does not exist/is not available"}"#.into())),
            "eth_getBlockByHash" => blocks().into_iter().find(|b| b.member("hash").and_then(W::as_str) == Some(first.as_str())).unwrap_or(W::of(Body::Null)),
            "eth_getBlockByNumber" => blocks().into_iter().find(|b| b.member("number").and_then(W::as_str) == Some(first.as_str())).unwrap_or(W::of(Body::Null)),
            "eth_getProof" => {
                let n = ps.get(2).and_then(Value::as_str).unwrap_or_default();
                doc().member("proofs").and_then(W::as_arr).and_then(|all| all.iter().find(|p| p.member("address").and_then(W::as_str) == Some(first.as_str()) && p.member("block").and_then(W::as_str) == Some(n)).cloned()).unwrap_or(W::of(Body::Null))
            }
            "eth_getTransactionReceipt" | "eth_getTransactionByHash" | "eth_getCode" => W::of(Body::Null),
            _ => W::of(Body::Str(first)),
        })
    }

    fn seen(&self) -> std::sync::MutexGuard<'_, Seen> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn asked(&self, method: &str) -> Vec<String> {
        self.seen().posts.iter().flatten().filter(|(m, _)| m == method).map(|(_, p)| p.clone()).collect()
    }
}

impl Endpoint for Wide {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        self.seen().singles.push(method.to_string());
        self.answer(method, params)
    }
    fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, Trouble>> {
        {
            let mut s = self.seen();
            s.out += 1;
            s.most = s.most.max(s.out);
            let post = asks.iter().map(|(m, p)| (m.to_string(), match p {
                Value::Arr(ps) => ps.first().and_then(Value::as_str).unwrap_or_default().to_string(),
                _ => String::new(),
            })).collect();
            s.posts.push(post);
        }
        std::thread::sleep(std::time::Duration::from_millis(self.pause));
        let limited = self.limited.fetch_update(std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst, |n| n.checked_sub(1)).is_ok();
        let in_place = self.limits_in_place.fetch_update(std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst, |n| n.checked_sub(1)).is_ok();
        let proof = |m: &str| m == "eth_getProof";
        let words = |w: &str| Trouble::Node(format!(r#"{{"code":-32005,"message":"{w}"}}"#));
        let first = asks.iter().position(|(m, _)| proof(m));
        let out = if limited || (self.limits_proofs == Some("whole") && first.is_some()) {
            if limited { Err(Trouble::Transport(Broke::Status { node: self.place.clone(), code: 429 })) } else { Err(words("rate limit exceeded")) }
        } else {
            Ok(asks
                .iter()
                .enumerate()
                .map(|(i, (m, p))| match self.limits_proofs {
                    _ if in_place && matches!(p, Value::Arr(ps) if ps.first().and_then(Value::as_str).and_then(|s| s.chars().last()).is_some_and(|c| "02468".contains(c))) => Some(Err(words("rate-limit exceeded"))),
                    Some("each") if proof(m) => Some(Err(words("rate-limit exceeded"))),
                    Some("first") if proof(m) && Some(i) != first => Some(Err(words("rate-limit exceeded"))),
                    _ => Some(self.answer(m, p)),
                })
                .collect())
        };
        self.seen().out -= 1;
        Some(out)
    }
    fn another(&self) -> Option<Box<dyn Endpoint + Send>> {
        self.doubles.then(|| Box::new(self.clone()) as Box<dyn Endpoint + Send>)
    }
    fn name(&self) -> String {
        self.place.clone()
    }
    fn place(&self) -> String {
        self.place.clone()
    }
}

fn echoes(n: usize) -> Vec<crate::rpc::Ask<'static>> {
    (0..n).map(|i| ("zikaron_echo", Value::Arr(vec![Value::Str(format!("q{i}"))]))).collect()
}

fn bytes_of(got: &crate::rpc::Batched) -> Vec<Option<String>> {
    got.iter().map(|a| value(a).map(crate::wire::write)).collect()
}

#[test]
fn the_batches_to_one_node_go_out_together_within_its_limit_and_answer_as_one_after_another() {
    let asks = echoes(7 * BATCH_MAX);
    let wide = Wide::new("parallel-wide", true);
    let got = in_batches(&mut wide.clone(), &asks).expect("answered");
    let most = wide.seen().most;
    let posts = wide.seen().posts.len();
    assert!((2..=PARALLEL_START).contains(&most), "out at once: {most}");
    assert_eq!(posts, 7, "as many posts as one after another");
    let narrow = Wide::new("parallel-narrow", false);
    let one_by_one = in_batches(&mut narrow.clone(), &asks).expect("answered");
    assert_eq!(narrow.seen().most, 1, "a channel that cannot be doubled: one batch at a time");
    assert_eq!(narrow.seen().posts.len(), 7);
    assert_eq!(bytes_of(&got), bytes_of(&one_by_one), "the same answers either way");
    assert!(got.iter().all(|a| value(a).is_some()));
}

#[test]
fn a_rate_limit_halves_how_many_batches_go_to_that_node_at_once_and_it_is_remembered() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let asks = echoes(7 * BATCH_MAX);
    let wide = Wide::new("parallel-halving", true);
    wide.limited.store(1, std::sync::atomic::Ordering::SeqCst);
    let got = in_batches(&mut wide.clone(), &asks).expect("answered");
    assert!(got.iter().all(|a| value(a).is_some()), "the limited batch sent again within the table");
    assert_eq!(parallel("parallel-halving"), PARALLEL_START / 2);
    let again = Wide::new("parallel-halving", true);
    let _ = in_batches(&mut again.clone(), &asks);
    assert!(again.seen().most <= PARALLEL_START / 2, "two at once at most now: {}", again.seen().most);
    again.limited.store(1, std::sync::atomic::Ordering::SeqCst);
    let _ = in_batches(&mut again.clone(), &asks);
    assert_eq!(parallel("parallel-halving"), 1, "halved again: one at a time, as before");
}

#[test]
fn a_node_limiting_every_batch_is_split_no_further_than_two_halves_and_rests() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let asks = echoes(7 * BATCH_MAX);
    let wide = Wide::new("parallel-limited", false);
    wide.limited.store(usize::MAX, std::sync::atomic::Ordering::SeqCst);
    let mut ep = crate::endpoints::remembering("parallel-limited", Box::new(wide.clone()));
    let r = in_batches_said(ep.as_mut(), &asks, false);
    let posts = wide.seen().posts.clone();
    assert_eq!(posts.len(), 5, "three posts of the first batch, its two halves once each: {:?}", posts.iter().map(Vec::len).collect::<Vec<_>>());
    assert_eq!(posts.iter().map(Vec::len).collect::<Vec<_>>(), [20, 20, 20, 10, 10]);
    assert!(r.got.as_ref().is_none_or(|g| g.iter().all(|a| value(a).is_none())), "nothing answered");
    assert!(r.limited.iter().all(|l| *l), "every question marked limited");
    assert_eq!(r.stopped, Stopped::Limited);
    assert_eq!(parallel("parallel-limited"), 1);
    assert!(crate::endpoints::resting("parallel-limited", "zikaron_echo").is_some(), "the method rests from the node");
    assert!(wide.seen().singles.is_empty(), "the batches ask no single question");
}

#[test]
fn a_node_taking_half_a_batch_answers_every_question_in_halves() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    struct Small {
        posts: Vec<usize>,
    }
    impl Endpoint for Small {
        fn call(&mut self, _: &str, _: &Value) -> Result<W, Trouble> {
            Ok(W::of(Body::Null))
        }
        fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, Trouble>> {
            self.posts.push(asks.len());
            Some(if asks.len() > 10 { Err(Trouble::Transport(Broke::Status { node: "small".into(), code: 429 })) } else { Ok(asks.iter().map(|_| Some(Ok(W::of(Body::Str("x".into()))))).collect()) })
        }
        fn name(&self) -> String {
            "small-batches".into()
        }
        fn place(&self) -> String {
            "small-batches".into()
        }
    }
    let mut node = Small { posts: Vec::new() };
    let r = in_batches_said(&mut node, &echoes(2 * BATCH_MAX), false);
    assert!(r.got.expect("answered").iter().all(|a| value(a).is_some()), "every question answered");
    assert_eq!(node.posts.iter().filter(|n| **n == 10).count(), 4, "four halves: {:?}", node.posts);
    assert_eq!(r.stopped, Stopped::No);
}

#[test]
fn a_share_limited_once_is_handed_to_another_node_at_once() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    for form in ["each", "whole"] {
        let a = Wide::new(&format!("handed-{form}-a"), true);
        let b = Wide { limits_proofs: Some(form), ..Wide::new(&format!("handed-{form}-b"), true) };
        {
            let _action = crate::action::open(None);
            two_rounds(&a, &b);
            let proof_posts = b.seen().posts.iter().filter(|p| p.iter().any(|(m, _)| m == "eth_getProof")).count();
            assert_eq!(proof_posts, 1, "{form}: one post of its proofs, no pause, no half: {:?}", b.seen().posts);
            let _at = reading_at(Some(wants()[1].block));
            assert_eq!(code_at(11155111, &SENDER, 0xb8a703), Some(false), "{form}: the other node proved its share");
            assert!(!proves(&b.place, true), "{form}: limited in this action: dealt no proof");
            assert!(crate::endpoints::resting(&b.place, "eth_getProof").is_none(), "{form}: no rest, the table never spent");
        }
        let _action = crate::action::open(None);
        assert!(proves(&b.place, true), "{form}: another action: dealt proofs again");
    }
}

#[test]
fn a_node_resting_from_proofs_is_dealt_them_again_once_it_answers() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let node = Wide { limits_proofs: Some("each"), ..Wide::new("recovers-proofs", false) };
    {
        let _action = crate::action::open(None);
        let mut ep = crate::endpoints::remembering("recovers-proofs", Box::new(node.clone()));
        read_ahead(ep.as_mut(), 11155111, &wants(), Share::WHOLE);
    }
    assert!(!proves("recovers-proofs", false), "resting: dealt no proof");
    assert!(no_proof().contains(&"recovers-proofs".to_string()));
    crate::endpoints::move_limit_clock(crate::endpoints::REST_FIRST);
    assert!(proves("recovers-proofs", false), "the rest over: dealt proofs again");
    let well = Wide::new("recovers-proofs", false);
    let _action = crate::action::open(None);
    let mut ep = crate::endpoints::remembering("recovers-proofs", Box::new(well.clone()));
    read_ahead(ep.as_mut(), 11155111, &wants(), Share::WHOLE);
    assert!(!well.asked("eth_getProof").is_empty(), "asked its proofs");
    assert!(proves("recovers-proofs", false), "answered: kept on proofs");
}

#[test]
fn a_halving_holds_as_a_rest_does_and_is_forgotten_once_answered() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let wide = Wide::new("halving-recovers", true);
    wide.limited.store(1, std::sync::atomic::Ordering::SeqCst);
    let _ = in_batches(&mut wide.clone(), &echoes(7 * BATCH_MAX));
    assert_eq!(parallel("halving-recovers"), PARALLEL_START / 2, "halved");
    crate::endpoints::move_limit_clock(crate::endpoints::REST_FIRST);
    assert_eq!(parallel("halving-recovers"), PARALLEL_START, "the hold over: the full number");
    let _ = in_batches(&mut Wide::new("halving-recovers", true), &echoes(7 * BATCH_MAX));
    assert_eq!(parallel("halving-recovers"), PARALLEL_START, "answered: forgotten");
}

fn two_rounds(a: &Wide, b: &Wide) {
    let wants = wants();
    let action = crate::action::current();
    std::thread::scope(|s| {
        for (k, node) in [a, b].into_iter().enumerate() {
            let (wants, action, mut node) = (&wants, action.clone(), node.clone());
            s.spawn(move || crate::action::within(action, || read_ahead(&mut node, 11155111, wants, Share { k, n: 2 })));
        }
    });
}

#[test]
fn two_nodes_bear_a_share_each_of_the_blocks_and_proofs_and_each_is_asked_its_receipts() {
    let _action = crate::action::open(None);
    let (a, b) = (Wide::new("share-a", true), Wide::new("share-b", true));
    two_rounds(&a, &b);
    let w = wants();
    let enc = |h: &[u8; 32]| zikaron::hexfmt::encode(h);
    assert_eq!(a.asked("eth_getBlockByHash"), [enc(&w[0].block)], "the first share's block of the first node alone");
    assert_eq!(b.asked("eth_getBlockByHash"), [enc(&w[1].block)], "the second share's block of the second node alone");
    for node in [&a, &b] {
        let mut receipts = node.asked("eth_getTransactionReceipt");
        receipts.sort();
        assert_eq!(receipts, [enc(&w[0].tx), enc(&w[1].tx)], "{}: every receipt its own", node.place);
    }
    let _at = reading_at(Some(w[0].block));
    assert_eq!(time_at(11155111, 0xb8a704), crate::tx::qty_u64(&block("0xb8a704"), "timestamp"));
    assert_eq!((code_at(11155111, &SENDER, 0xb8a704), code_at(11155111, &SENDER, 0xb8a703)), (Some(false), Some(false)));
}

#[test]
fn a_node_that_gives_no_proof_is_asked_none_nor_a_block_before_and_another_takes_its_share() {
    let (a, mut b) = (Wide::new("noproof-a", true), Wide::new("noproof-b", true));
    b.proofs = false;
    {
        let _action = crate::action::open(None);
        two_rounds(&a, &b);
        assert!(b.asked("eth_getBlockByNumber").is_empty() && !b.seen().singles.iter().any(|m| m == "eth_getBlockByNumber"), "no block before asked of a node that gave no proof");
        assert!(no_proof().contains(&"noproof-b".to_string()), "known to give none: {:?}", no_proof());
        let _at = reading_at(Some(wants()[1].block));
        assert_eq!(code_at(11155111, &SENDER, 0xb8a703), Some(false), "the other node proved its share's boundary");
    }
    let _action = crate::action::open(None);
    let alone = Wide { proofs: false, ..Wide::new("noproof-b", true) };
    read_ahead(&mut alone.clone(), 11155111, &wants(), Share::WHOLE);
    assert!(alone.asked("eth_getProof").is_empty() && alone.asked("eth_getBlockByNumber").is_empty(), "asked no proof and no block before: {:?}", alone.seen().posts);
    assert_eq!(alone.asked("eth_getCode").len(), 3, "the boundaries left to the code, as before");
}

#[test]
fn a_nodes_share_is_read_from_its_place_in_the_list() {
    assert_eq!(Share::of("b", Some("a b c")), Share { k: 1, n: 3 });
    assert_eq!(Share::of("z", Some("a b c")), Share::WHOLE);
    assert_eq!(Share::of("a", Some("a")), Share::WHOLE);
    assert_eq!(Share::of("a", None), Share::WHOLE);
}

#[test]
fn one_proof_that_came_keeps_the_node_dealt_proofs() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let a = Wide::new("limits-first-a", true);
    let b = Wide { limits_proofs: Some("first"), ..Wide::new("limits-first-b", true) };
    let _action = crate::action::open(None);
    two_rounds(&a, &b);
    assert!(b.asked("eth_getProof").len() >= 2, "a batch of its proofs: {:?}", b.seen().posts);
    assert!(!no_proof().contains(&b.place), "one proof came: kept on proofs: {:?}", no_proof());
}

#[test]
fn a_transaction_is_asked_ahead_of_one_node_once() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let (a, b) = (Wide::new("tx-once-a", true), Wide::new("tx-once-b", true));
    {
        let _action = crate::action::open(None);
        two_rounds(&a, &b);
    }
    for node in [&a, &b] {
        let mut txs = node.asked("eth_getTransactionByHash");
        let all = txs.len();
        txs.sort();
        txs.dedup();
        assert_eq!(all, txs.len(), "{}: each transaction asked of it once: {:?}", node.place, node.seen().posts);
    }
    let _action = crate::action::open(None);
    let mut alone = Batching { takes: true, lies: false, batches: Vec::new(), singles: Vec::new() };
    read_ahead(&mut alone, 11155111, &wants(), Share::WHOLE);
    let txs: usize = alone.batches.iter().filter(|(m, _)| m.contains("eth_getTransactionByHash")).count();
    assert_eq!(txs, 1, "a node alone: its transactions in one post, once: {:?}", alone.batches);
}

#[test]
fn questions_limited_in_a_batch_taken_are_posted_again_after_the_tables_pause() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let asks = echoes(6);
    let lets_go = Wide::new("in-place-once", false);
    lets_go.limits_in_place.store(1, std::sync::atomic::Ordering::SeqCst);
    let r = in_batches_said(&mut lets_go.clone(), &asks, false);
    let got = r.got.expect("answered");
    assert!(got.iter().all(|a| value(a).is_some()), "every answer in its place: {:?}", bytes_of(&got));
    assert_eq!(bytes_of(&got), bytes_of(&in_batches(&mut Wide::new("in-place-none", false), &asks).expect("plain")), "the same answers as a node that limits nothing");
    let posts = lets_go.seen().posts.clone();
    assert_eq!(posts.len(), 2, "one post more: {posts:?}");
    assert_eq!(posts[1].iter().map(|(_, p)| p.as_str()).collect::<Vec<_>>(), vec!["q0", "q2", "q4"], "only the limited ones again");
    assert!(lets_go.seen().singles.is_empty(), "no question asked alone");
    assert_eq!(r.stopped, Stopped::No);
    let never = Wide::new("in-place-ever", false);
    never.limits_in_place.store(usize::MAX, std::sync::atomic::Ordering::SeqCst);
    let r = in_batches_said(&mut never.clone(), &asks, false);
    let table = crate::patience::pauses(crate::patience::Class::RateLimited).len();
    let posts = never.seen().posts.clone();
    assert_eq!(posts.len(), 1 + table, "asked again once a pause: {posts:?}");
    assert!(posts[1..].iter().all(|p| p.iter().map(|(_, q)| q.as_str()).collect::<Vec<_>>() == ["q0", "q2", "q4"]), "only the limited ones again: {posts:?}");
    assert_eq!(r.stopped, Stopped::Limited, "told as stopped for a rate limit");
    let got = r.got.expect("what came is kept");
    assert!(got.iter().enumerate().filter(|(i, _)| i % 2 == 1).all(|(_, a)| value(a).is_some()), "what came kept: {:?}", bytes_of(&got));
    assert_eq!(r.limited, (0..6).map(|i| i % 2 == 0).collect::<Vec<_>>(), "the limited marked, the rest not");
    assert!(never.seen().singles.is_empty(), "no question asked alone");
}
