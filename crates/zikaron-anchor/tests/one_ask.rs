use zikaron::json::Value;
use zikaron_anchor::rpc::{Broke, Endpoint, Trouble};
use zikaron_anchor::send::{confirm_each, Confirm, ONE_ASK};
use zikaron_anchor::wire::{self, Body, W};
use std::time::Duration;

const LONG: Duration = Duration::from_secs(30);

struct Hung {
    name: &'static str,
    hangs_receipt: bool,
    hangs_head: bool,
    receipt: Option<u64>,
    given: Vec<(&'static str, Option<Duration>)>,
}

impl Hung {
    fn new(name: &'static str, hangs_receipt: bool, hangs_head: bool, receipt: Option<u64>) -> Hung {
        Hung { name, hangs_receipt, hangs_head, receipt, given: Vec::new() }
    }
    fn ask(&mut self, method: &str, within: Option<Duration>) -> Result<W, Trouble> {
        let late = || Trouble::Transport(Broke::Late { node: self.name.into(), deadline: within.unwrap_or(LONG) });
        match method {
            "eth_blockNumber" => {
                self.given.push(("head", within));
                if self.hangs_head {
                    return Err(late());
                }
                Ok(W::of(Body::Str("0x10".into())))
            }
            "eth_getTransactionReceipt" => {
                let headed = self.given.iter().any(|(m, _)| *m == "head");
                self.given.push(("receipt", within));
                if self.hangs_receipt {
                    return Err(late());
                }
                Ok(match self.receipt.filter(|_| headed || !self.hangs_head) {
                    Some(b) => wire::parse(format!("{{\"blockHash\":\"0x{}\",\"blockNumber\":\"0x{b:x}\",\"status\":\"0x1\"}}", "ab".repeat(32)).as_bytes()).expect("json"),
                    None => W::of(Body::Null),
                })
            }
            _ => Ok(W::of(Body::Null)),
        }
    }
    fn given(&self, what: &str) -> Vec<Option<Duration>> {
        self.given.iter().filter(|(m, _)| *m == what).map(|(_, d)| *d).collect()
    }
}

impl Endpoint for Hung {
    fn call(&mut self, method: &str, _: &Value) -> Result<W, Trouble> {
        self.ask(method, None)
    }
    fn call_within(&mut self, method: &str, _: &Value, within: Duration) -> Result<W, Trouble> {
        self.ask(method, Some(within))
    }
    fn name(&self) -> String {
        self.name.into()
    }
    fn place(&self) -> String {
        self.name.into()
    }
}

#[test]
fn a_node_hung_on_the_receipt_costs_one_ask_and_the_others_are_asked() {
    let mut first = Hung::new("first", true, false, None);
    let mut second = Hung::new("second", false, false, Some(0x11));
    let mut third = Hung::new("third", false, false, Some(0x11));
    let got = confirm_each(&mut [&mut first, &mut second, &mut third], &[1; 32], LONG, &[]);
    assert_eq!(got, Confirm::Included { status: 1, block_number: 0x11 }, "the others' receipt, never out of sight");
    let asked = first.given("receipt");
    assert_eq!(asked.len(), 1, "one round: {asked:?}");
    assert!(asked.iter().all(|d| d.is_some_and(|d| d <= ONE_ASK)), "one question, one ask: {asked:?}");
    assert_eq!((second.given("receipt").len(), third.given("receipt").len()), (1, 1), "both others asked in that round");
}

#[test]
fn a_node_hung_on_the_head_costs_one_ask_and_the_receipt_is_asked_next() {
    let _action = zikaron_anchor::action::open(None);
    zikaron_anchor::action::note("held", "eth_blockNumber", &Value::Arr(Vec::new()), &W::of(Body::Str("0x10".into())));
    zikaron_anchor::action::sent("held", Some(&[1; 32]));
    let mut first = Hung::new("first", false, true, Some(0x11));
    let got = confirm_each(&mut [&mut first], &[1; 32], LONG, &[]);
    assert_eq!(got, Confirm::Included { status: 1, block_number: 0x11 });
    assert_eq!(first.given.iter().map(|(m, _)| *m).collect::<Vec<_>>(), ["receipt", "head", "receipt"], "the receipt asked after the head");
    let heads = first.given("head");
    assert!(heads.iter().all(|d| d.is_some_and(|d| d <= ONE_ASK)), "one head question, one ask: {heads:?}");
}

#[test]
fn a_question_is_never_given_more_than_the_wait_left() {
    let wait = Duration::from_secs(1);
    let mut first = Hung::new("first", true, false, None);
    let mut second = Hung::new("second", false, false, Some(0x11));
    let got = confirm_each(&mut [&mut first, &mut second], &[1; 32], wait, &[]);
    assert_eq!(got, Confirm::Included { status: 1, block_number: 0x11 });
    let asked = first.given("receipt");
    assert!(!asked.is_empty() && asked.iter().all(|d| d.is_some_and(|d| d <= wait)), "never past the wait: {asked:?}");
    let mut first = Hung::new("first", true, false, None);
    let mut second = Hung::new("second", false, false, Some(0x11));
    let _ = confirm_each(&mut [&mut first, &mut second], &[1; 32], Duration::ZERO, &[]);
    assert_eq!(first.given("receipt"), [None], "a look with no wait: the channel's own deadline");
}
