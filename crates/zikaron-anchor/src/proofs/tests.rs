use super::*;
use crate::rpc::Trouble;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

const CHAIN: u64 = 11_155_111;
const NUMBER: u64 = 0x100;
const EMITTER: [u8; 20] = [0xee; 20];
const HASH: [u8; 32] = [0x5a; 32];

fn hex(b: &[u8]) -> String {
    zikaron::hexfmt::encode(b)
}

fn q(b: &[u8]) -> String {
    let s: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let t = s.trim_start_matches('0');
    format!("0x{}", if t.is_empty() { "0" } else { t })
}

fn qn(n: u64) -> String {
    format!("0x{n:x}")
}

fn w(json: &str) -> W {
    crate::wire::parse(json.as_bytes()).expect("json")
}

fn fees() -> crate::send::Fees {
    crate::send::Fees { max_fee: 2_000_000_000, priority: 1_000_000_000, gas_limit: 60_000, from_chain: true }
}

fn tx_object(raw: &[u8], block: &[u8; 32], index: u64) -> String {
    let item = rlp::decode_all(&raw[1..]).expect("rlp");
    let l = item.list().expect("list");
    let b = |i: usize| l[i].bytes().expect("bytes").to_vec();
    format!(
        r#"{{"blockHash":"{}","blockNumber":"{}","chainId":"{}","nonce":"{}","maxPriorityFeePerGas":"{}","maxFeePerGas":"{}","gas":"{}","to":"{}","value":"{}","input":"{}","accessList":[],"yParity":"{}","v":"{}","r":"{}","s":"{}","type":"0x2","hash":"{}","transactionIndex":"{}"}}"#,
        hex(block),
        qn(NUMBER),
        q(&b(0)),
        q(&b(1)),
        q(&b(2)),
        q(&b(3)),
        q(&b(4)),
        hex(&b(5)),
        q(&b(6)),
        hex(&b(7)),
        q(&b(9)),
        q(&b(9)),
        q(&b(10)),
        q(&b(11)),
        hex(&zikaron::cryptox::keccak256(raw)),
        qn(index)
    )
}

#[derive(Clone)]
pub(super) struct World {
    pub who: [u8; 20],
    pub tx: [u8; 32],
    pub block: [u8; 32],
    pub raws: Vec<Vec<u8>>,
    pub receipts: Vec<String>,
    pub header_obj: String,
    pub parent_obj: String,
    pub header: Vec<u8>,
    pub parent: Vec<u8>,
    pub state_proof: Vec<Vec<u8>>,
}

fn header_json(number: u64, parent: &[u8; 32], state: &[u8; 32], txs: &[u8; 32], receipts: &[u8; 32], time: u64) -> String {
    format!(
        r#""parentHash":"{}","sha3Uncles":"{}","miner":"{}","stateRoot":"{}","transactionsRoot":"{}","receiptsRoot":"{}","logsBloom":"{}","difficulty":"0x0","number":"{}","gasLimit":"0x1c9c380","gasUsed":"0xa410","timestamp":"{}","extraData":"0x","mixHash":"{}","nonce":"0x0000000000000000","baseFeePerGas":"0x7""#,
        hex(parent),
        hex(&zikaron::cryptox::keccak256(&rlp::list(&[]))),
        hex(&[0x99; 20]),
        hex(state),
        hex(txs),
        hex(receipts),
        hex(&[0u8; 256]),
        qn(number),
        qn(time),
        hex(&[0u8; 32])
    )
}

pub(super) fn world() -> World {
    let k1 = [0x11u8; 32];
    let k2 = [0x22u8; 32];
    let who = zikaron::cryptox::address_of_privkey(&k1).expect("an address");
    let mut calldata = vec![0xee, 0xcd, 0xf9, 0x27];
    calldata.extend_from_slice(&HASH);
    let (raw1, h1) = crate::send::sign_plain(&k1, CHAIN, EMITTER, 0, &calldata, fees(), 0).expect("signed");
    let (raw2, _) = crate::send::sign_plain(&k2, CHAIN, [0x33; 20], 5, &[], fees(), 7).expect("signed");
    let raws = vec![raw2, raw1];
    let mut topic1 = [0u8; 32];
    topic1[12..].copy_from_slice(&who);
    let receipt = |cum: &str, logs: &str| format!(r#"{{"type":"0x2","status":"0x1","cumulativeGasUsed":"{cum}","logsBloom":"{}","logs":[{logs}]}}"#, hex(&[0u8; 256]));
    let log = format!(r#"{{"address":"{}","topics":["{}","{}","{}"],"data":"0x"}}"#, hex(&EMITTER), hex(&crate::scan::topic0()), hex(&topic1), hex(&HASH));
    let receipts = vec![receipt("0x5208", ""), receipt("0xa410", &log)];
    let receipt_bytes_all: Vec<Vec<u8>> = receipts.iter().map(|r| receipt_bytes(&w(r)).expect("a receipt")).collect();
    let empty_root = crate::trie::root_of(&[]);
    let account = |nonce: u64, code: &[u8]| rlp::list(&[rlp::quantity(nonce), rlp::quantity(1_000_000), rlp::bytes(&empty_root), rlp::bytes(&zikaron::cryptox::keccak256(code))]);
    let pairs = vec![(zikaron::cryptox::keccak256(&who).to_vec(), account(1, &[])), (zikaron::cryptox::keccak256(&[0x44u8; 20]).to_vec(), account(0, &[0x60, 0x00]))];
    let (state, state_proof) = crate::trie::keyed_root_and_proof(&pairs, &zikaron::cryptox::keccak256(&who)).expect("a state");
    let parent_fields = header_json(NUMBER - 1, &[0x01; 32], &state, &empty_root, &empty_root, 0x5ff0);
    let parent = crate::ahead::header_rlp(&w(&format!("{{{parent_fields}}}"))).expect("a header");
    let parent_hash = zikaron::cryptox::keccak256(&parent);
    let fields = header_json(NUMBER, &parent_hash, &state, &crate::trie::root_of(&raws), &crate::trie::root_of(&receipt_bytes_all), 0x6000);
    let header = crate::ahead::header_rlp(&w(&format!("{{{fields}}}"))).expect("a header");
    let block = zikaron::cryptox::keccak256(&header);
    World {
        who,
        tx: h1,
        block,
        raws,
        receipts,
        header_obj: fields,
        parent_obj: format!(r#"{{{parent_fields},"hash":"{}","transactions":[]}}"#, hex(&parent_hash)),
        header,
        parent,
        state_proof,
    }
}

impl World {
    fn want(&self) -> Want {
        Want { tx: self.tx, block: self.block, number: NUMBER, who: self.who, hash: HASH }
    }

    fn block_obj(&self, full: bool) -> String {
        let txs: Vec<String> = self.raws.iter().enumerate().map(|(i, r)| if full { tx_object(r, &self.block, i as u64) } else { format!("\"{}\"", hex(&zikaron::cryptox::keccak256(r))) }).collect();
        format!(r#"{{{},"hash":"{}","transactions":[{}]}}"#, self.header_obj, hex(&self.block), txs.join(","))
    }

    fn log(&self) -> String {
        let mut topic1 = [0u8; 32];
        topic1[12..].copy_from_slice(&self.who);
        format!(
            r#"{{"address":"{}","topics":["{}","{}","{}"],"data":"0x","blockNumber":"{}","blockHash":"{}","transactionHash":"{}","transactionIndex":"0x1","logIndex":"0x0","removed":false}}"#,
            hex(&EMITTER),
            hex(&crate::scan::topic0()),
            hex(&topic1),
            hex(&HASH),
            qn(NUMBER),
            hex(&self.block),
            hex(&self.tx)
        )
    }

    fn proof(&self) -> AnchorProof {
        let receipts: Vec<Vec<u8>> = self.receipts.iter().map(|r| receipt_bytes(&w(r)).expect("a receipt")).collect();
        let (_, tx_proof) = crate::trie::root_and_proof(&self.raws, 1).expect("built");
        let (_, receipt_proof) = crate::trie::root_and_proof(&receipts, 1).expect("built");
        AnchorProof {
            chain_id: CHAIN,
            hash: HASH,
            emitter: EMITTER,
            header: self.header.clone(),
            parent: Some(self.parent.clone()),
            tx: self.raws[1].clone(),
            tx_proof,
            receipt: receipts[1].clone(),
            receipt_proof,
            index: 1,
            sender_at: Some(self.state_proof.clone()),
            sender_before: Some(self.state_proof.clone()),
        }
    }
}

#[derive(Clone)]
struct Node {
    world: World,
    receipts_too: bool,
    proofs: bool,
    asked: Arc<Mutex<BTreeMap<String, usize>>>,
    place: &'static str,
}

impl Node {
    fn new(world: &World) -> Node {
        Node { world: world.clone(), receipts_too: true, proofs: true, asked: Default::default(), place: "world" }
    }

    fn answer(&self, method: &str, params: &Value) -> Result<W, Trouble> {
        *self.asked.lock().unwrap().entry(method.to_string()).or_default() += 1;
        let ps = params.as_arr().cloned().unwrap_or_default();
        let first = ps.first().and_then(Value::as_str).unwrap_or_default().to_string();
        let wd = &self.world;
        Ok(match method {
            "eth_chainId" => w(&format!("\"{}\"", qn(CHAIN))),
            "eth_blockNumber" => w("\"0x1000\""),
            "eth_getLogs" => w(&format!("[{}]", wd.log())),
            "eth_getTransactionReceipt" if first == hex(&wd.tx) => {
                let r = wd.receipts[1].trim_end_matches('}');
                w(&format!(r#"{r},"blockHash":"{}","blockNumber":"{}","transactionHash":"{}","transactionIndex":"0x1"}}"#, hex(&wd.block), qn(NUMBER), hex(&wd.tx)))
            }
            "eth_getTransactionByHash" if first == hex(&wd.tx) => w(&tx_object(&wd.raws[1], &wd.block, 1)),
            "eth_getBlockByHash" if first == hex(&wd.block) => w(&wd.block_obj(ps.get(1) == Some(&Value::Bool(true)))),
            "eth_getBlockByNumber" if first == qn(NUMBER) => w(&wd.block_obj(false)),
            "eth_getBlockByNumber" if first == qn(NUMBER - 1) => w(&wd.parent_obj),
            "eth_getBlockReceipts" if self.receipts_too => w(&format!("[{}]", wd.receipts.join(","))),
            "eth_getBlockReceipts" => return Err(Trouble::Node(r#"{"code":-32601,"message":"the method eth_getBlockReceipts does not exist/is not available"}"#.into())),
            "eth_getCode" => w("\"0x\""),
            "eth_getProof" if self.proofs => {
                let nodes: Vec<String> = wd.state_proof.iter().map(|n| format!("\"{}\"", hex(n))).collect();
                w(&format!(r#"{{"accountProof":[{}]}}"#, nodes.join(",")))
            }
            "eth_getProof" => return Err(Trouble::Node(r#"{"code":-32601,"message":"the method eth_getProof does not exist/is not available"}"#.into())),
            _ => W::of(crate::wire::Body::Null),
        })
    }

    fn of(&self, methods: &[&str]) -> usize {
        let a = self.asked.lock().unwrap();
        methods.iter().map(|m| a.get(*m).copied().unwrap_or(0)).sum()
    }
}

impl Endpoint for Node {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        self.answer(method, params)
    }
    fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, Trouble>> {
        Some(Ok(asks.iter().map(|(m, p)| Some(self.answer(m, p))).collect()))
    }
    fn name(&self) -> String {
        self.place.into()
    }
    fn place(&self) -> String {
        self.place.into()
    }
}

const SPARED: [&str; 6] = ["eth_getTransactionReceipt", "eth_getTransactionByHash", "eth_getBlockByHash", "eth_getBlockByNumber", "eth_getCode", "eth_getProof"];

#[test]
fn a_proof_is_built_from_a_node_holds_and_comes_back_from_its_value() {
    let wd = world();
    let mut node = Node::new(&wd);
    let built = build(&mut node, CHAIN, &wd.block, &wd.tx, &wd.who, &HASH, &EMITTER).expect("built");
    assert_eq!(built, wd.proof());
    let pr = check(&built, CHAIN, &wd.want()).expect("holds");
    assert_eq!((pr.header.number, pr.header.timestamp, pr.code_at, pr.code_before), (NUMBER, 0x6000, Some(false), Some(false)));
    assert_eq!(pr.tx.hash, wd.tx);
    assert_eq!(AnchorProof::of_value(&built.to_value()), Some(built.clone()));
    let mut no_receipts = Node::new(&wd);
    no_receipts.receipts_too = false;
    assert_eq!(build(&mut no_receipts, CHAIN, &wd.block, &wd.tx, &wd.who, &HASH, &EMITTER), Err(Gap::Unbuilt), "no block receipts: no proof");
    let mut no_proofs = Node::new(&wd);
    no_proofs.proofs = false;
    let thin = build(&mut no_proofs, CHAIN, &wd.block, &wd.tx, &wd.who, &HASH, &EMITTER).expect("built without account proofs");
    assert_eq!((thin.sender_at.is_none(), thin.sender_before.is_none()), (true, true));
    assert_eq!(check(&thin, CHAIN, &wd.want()).map(|p| (p.code_at, p.code_before)).ok(), Some((None, None)), "the boundaries left to be asked");
}

#[test]
fn a_forged_part_of_a_proof_does_not_hold() {
    let wd = world();
    let good = wd.proof();
    let want = wd.want();
    let with = |f: &dyn Fn(&mut AnchorProof)| {
        let mut p = good.clone();
        f(&mut p);
        check(&p, CHAIN, &want).err()
    };
    assert_eq!(with(&|p| p.chain_id = 1), Some(Gap::Chain));
    assert_eq!(with(&|p| p.header = wd.parent.clone()), Some(Gap::Header), "another block's header");
    assert_eq!(with(&|p| p.parent = Some(wd.header.clone())), Some(Gap::Parent), "a header before that is not the parent");
    assert_eq!(with(&|p| p.parent = None), Some(Gap::Parent), "the header before missing");
    assert_eq!(with(&|p| p.tx[20] ^= 1), Some(Gap::Tx), "the transaction changed");
    let mut calldata = vec![0xee, 0xcd, 0xf9, 0x27];
    calldata.extend_from_slice(&HASH);
    let (elsewhere, _) = crate::send::sign_plain(&[0x11; 32], 5, EMITTER, 0, &calldata, fees(), 0).expect("signed");
    assert_eq!(check(&AnchorProof { tx: elsewhere.clone(), ..good.clone() }, CHAIN, &Want { tx: zikaron::cryptox::keccak256(&elsewhere), ..want }).err(), Some(Gap::Tx), "signed for another chain");
    assert_eq!(check(&good, CHAIN, &Want { who: [0x77; 20], ..want }).err(), Some(Gap::Tx), "not the log's sender");
    assert_eq!(with(&|p| p.index = 0), Some(Gap::TxRoot), "the proofs at another index");
    let (_, receipt_at_0) = crate::trie::root_and_proof(&wd.receipts.iter().map(|r| receipt_bytes(&w(r)).expect("r")).collect::<Vec<_>>(), 0).expect("built");
    assert_eq!(with(&|p| p.receipt_proof = receipt_at_0.clone()), Some(Gap::ReceiptRoot), "the receipt's proof at another index than the transaction's");
    let failed = receipt_bytes(&w(&wd.receipts[1].replace(r#""status":"0x1""#, r#""status":"0x0""#))).expect("r");
    assert_eq!(with(&|p| p.receipt = failed.clone()), Some(Gap::ReceiptRoot), "a receipt with status 0 is not the block's");
    assert_eq!(with(&|p| p.hash = [0x5b; 32]), Some(Gap::Receipt), "a hash its receipt does not carry");
    assert_eq!(with(&|p| p.emitter = [0xef; 20]), Some(Gap::Receipt), "another emitter");
    assert_eq!(with(&|p| { let last = p.sender_at.as_ref().unwrap().len() - 1; p.sender_at.as_mut().unwrap()[last][3] ^= 1; }), Some(Gap::AccountProof), "an account proof changed");
}

fn basis(wd: &World) -> Vec<u8> {
    format!(r#"{{"chains":[{{"chainId":{CHAIN},"fromBlock":0,"registries":["{}"],"senders":["{}"],"toBlock":512}}]}}"#, hex(&EMITTER), hex(&wd.who)).into_bytes()
}

fn scanned(node: &mut Node, offered: Option<Vec<AnchorProof>>) -> (Vec<u8>, String, crate::ahead::Tally) {
    let _action = crate::action::open(None);
    if let Some(p) = offered {
        crate::ahead::offer(p);
    }
    let b = basis(&node.world);
    let mut eps: Vec<(u64, &mut dyn Endpoint)> = vec![(CHAIN, node)];
    let (s, _, seen) = crate::scan::run_knowing(&b, &mut eps, &crate::scan::Known::default()).expect("scanned").expect("a basis");
    (zikaron::json::canon_bytes(&crate::scan::fragment(&s)), format!("{seen:?}"), crate::ahead::tally())
}

#[test]
fn a_cold_read_with_the_seats_proofs_asks_the_node_nothing_of_its_anchors_and_reads_as_without() {
    let wd = world();
    let mut bare = Node::new(&wd);
    let (anchors, facts, tally) = scanned(&mut bare, None);
    assert!(bare.of(&SPARED) > 0, "without proofs the anchor's facts are asked");
    assert_eq!(tally, crate::ahead::Tally::default(), "none offered");
    let mut proved = Node::new(&wd);
    let with = scanned(&mut proved, Some(vec![wd.proof()]));
    assert_eq!((&with.0, &with.1), (&anchors, &facts), "read byte for byte as without");
    assert_eq!(proved.of(&SPARED), 0, "nothing of the anchor asked: {:?}", proved.asked.lock().unwrap());
    assert_eq!(with.2, crate::ahead::Tally { used: 1, missing: 0, failed: Vec::new(), loads: 0 });
    let mut forged_node = Node::new(&wd);
    let mut forged = wd.proof();
    forged.receipt_proof.pop();
    let with_forged = scanned(&mut forged_node, Some(vec![forged]));
    assert_eq!((&with_forged.0, &with_forged.1), (&anchors, &facts));
    assert_eq!(forged_node.of(&SPARED), bare.of(&SPARED), "read as before");
    assert_eq!(with_forged.2.failed.len(), 1);
    let mut thin_node = Node::new(&wd);
    let thin = AnchorProof { sender_at: None, sender_before: None, ..wd.proof() };
    let with_thin = scanned(&mut thin_node, Some(vec![thin]));
    assert_eq!((&with_thin.0, &with_thin.1), (&anchors, &facts));
    assert_eq!(thin_node.of(&["eth_getTransactionReceipt", "eth_getTransactionByHash", "eth_getBlockByHash"]), 0, "receipt, transaction, block from the proof");
    assert!(thin_node.of(&["eth_getCode", "eth_getProof"]) > 0, "the boundaries asked as before");
    let mut other_node = Node::new(&wd);
    let other = AnchorProof { tx: wd.raws[0].clone(), ..wd.proof() };
    let with_other = scanned(&mut other_node, Some(vec![other]));
    assert_eq!((&with_other.0, &with_other.1), (&anchors, &facts), "a proof of a transaction the logs do not hold adds no anchor");
    assert_eq!(with_other.2, crate::ahead::Tally { used: 0, missing: 1, failed: Vec::new(), loads: 0 });
}

#[test]
fn the_seats_proofs_are_read_only_as_a_scan_tests_an_anchor() {
    let wd = world();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let offer = |p: Option<AnchorProof>| {
        let calls = calls.clone();
        crate::ahead::offer_by_hash(move |h| {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(*h, HASH, "asked for the tested anchor's hash");
            p.clone().map(|p| vec![p])
        });
    };
    let mut node = Node::new(&wd);
    let b = basis(&wd);
    {
        let _action = crate::action::open(None);
        offer(Some(wd.proof()));
        let mut eps: Vec<(u64, &mut dyn Endpoint)> = vec![(CHAIN, &mut node)];
        let none = crate::scan::Pick::only(|_| false);
        let (s, _, _, _) = crate::scan::run_picked(&b, &mut eps, &crate::scan::Known::default(), &[], &none).expect("scanned").expect("a basis");
        assert_eq!(s.candidates.len(), 1, "the anchor left untested");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0, "an untested anchor's proofs are not read");
    }
    let seen = {
        let _action = crate::action::open(None);
        offer(Some(wd.proof()));
        offer(None);
        let mut eps: Vec<(u64, &mut dyn Endpoint)> = vec![(CHAIN, &mut node)];
        let (_, _, seen) = crate::scan::run_knowing(&b, &mut eps, &crate::scan::Known::default()).expect("scanned").expect("a basis");
        assert_eq!(crate::ahead::tally().loads, 1, "the one document that holds it read");
        assert_eq!(crate::ahead::tally().used, 1);
        seen
    };
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2, "the hash looked for once in each place");
    assert_eq!(node.of(&SPARED), 0, "the cold read asked nothing of the anchor");
    let known = crate::scan::Known(seen.into_iter().collect(), Default::default());
    let _action = crate::action::open(None);
    offer(Some(wd.proof()));
    let mut eps: Vec<(u64, &mut dyn Endpoint)> = vec![(CHAIN, &mut node)];
    let _ = crate::scan::run_knowing(&b, &mut eps, &known).expect("scanned").expect("a basis");
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2, "a warm read reads none");
    assert_eq!(crate::ahead::tally().loads, 0);
}

fn fact_key(wd: &World) -> crate::scan::FactKey {
    (CHAIN, wd.block, wd.tx, 0)
}

#[test]
fn an_anchor_read_from_proofs_is_marked_proved_and_one_read_of_a_nodes_word_is_not() {
    let wd = world();
    let b = basis(&wd);
    for (proofs, marked) in [(true, true), (false, false)] {
        let mut node = Node::new(&wd);
        node.proofs = proofs;
        node.place = if proofs { "world" } else { "world-without-proofs" };
        let _action = crate::action::open(None);
        let mut eps: Vec<(u64, &mut dyn Endpoint)> = vec![(CHAIN, &mut node)];
        let (_, _, seen) = crate::scan::run_knowing(&b, &mut eps, &crate::scan::Known::default()).expect("scanned").expect("a basis");
        assert_eq!(seen.len(), 1, "the anchor read");
        assert_eq!(crate::ahead::fact_proved(&fact_key(&wd)), marked, "account proofs given: {proofs}");
    }
}

#[test]
fn an_anchor_held_proved_but_for_its_receipt_is_asked_its_receipt_alone() {
    let wd = world();
    let b = basis(&wd);
    let mut cold = Node::new(&wd);
    let (anchors, facts, _) = scanned(&mut cold, None);
    let read = {
        let _action = crate::action::open(None);
        let mut eps: Vec<(u64, &mut dyn Endpoint)> = vec![(CHAIN, &mut cold)];
        crate::scan::run_knowing(&b, &mut eps, &crate::scan::Known::default()).expect("scanned").expect("a basis").2
    };
    let known = crate::scan::Known(Default::default(), read.into_iter().collect());
    let mut node = Node::new(&wd);
    let _action = crate::action::open(None);
    let mut eps: Vec<(u64, &mut dyn Endpoint)> = vec![(CHAIN, &mut node)];
    let (s, _, seen) = crate::scan::run_knowing(&b, &mut eps, &known).expect("scanned").expect("a basis");
    assert_eq!((zikaron::json::canon_bytes(&crate::scan::fragment(&s)), format!("{seen:?}")), (anchors, facts), "read as read cold");
    assert_eq!(node.of(&["eth_getTransactionReceipt"]), 1, "its receipt, once");
    assert_eq!(node.of(&SPARED[1..]), 0, "nothing else of the anchor: {:?}", node.asked.lock().unwrap());
    assert!(crate::ahead::fact_proved(&fact_key(&wd)), "marked proved again");
}
