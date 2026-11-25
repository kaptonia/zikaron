use zikaron::json::Value;
use zikaron_anchor::judge::{self, Missing, NoReading, Rule, Statistic};
use zikaron_anchor::rpc::Trouble;
use zikaron_anchor::send;
use zikaron_anchor::wire::{self, W};

fn w(s: &str) -> W {
    wire::parse(s.as_bytes()).expect("a wire value")
}

fn table(tips: &[u64]) -> W {
    let reward: Vec<String> = tips.iter().map(|t| format!("[\"0x{t:x}\"]")).collect();
    w(&format!("{{\"oldestBlock\":\"0x2d\",\"gasUsedRatio\":[0.5],\"reward\":[{}]}}", reward.join(",")))
}

fn judged(answers: Vec<(&str, Result<W, Trouble>)>) -> Result<judge::Judged, NoReading> {
    judge::judge("eth_feeHistory", &send::tip_params(0x40), answers.into_iter().map(|(n, a)| (n.to_string(), a)).collect())
}

fn tip(answers: Vec<(&str, Result<W, Trouble>)>) -> Option<u64> {
    judged(answers).ok().and_then(|j| send::tip_of(&j.value))
}

const GWEI: u64 = 1_000_000_000;

#[test]
fn the_fee_history_is_read_by_its_own_rule() {
    assert_eq!(judge::rule_of("eth_feeHistory", &send::tip_params(1)), Rule::Middle(Statistic::PaidTip));
    assert_eq!(judge::facts_of("eth_feeHistory", &send::tip_params(1)), Some(&judge::FEE_HISTORY_FACTS[..]));
    assert_eq!(judge::rule_of("eth_estimateGas", &Value::Arr(vec![])), Rule::Most(None));
}

#[test]
fn two_tables_that_differ_give_the_lower_number() {
    let a = table(&[30_000_000, 50_000_000, 40_000_000]);
    let b = table(&[700_000_000, 700_000_000, 700_000_000]);
    let j = judged(vec![("http://a", Ok(a.clone())), ("http://b", Ok(b.clone()))]).expect("a reading, not a disagreement");
    assert_eq!(send::tip_of(&j.value), Some(40_000_000));
    assert_eq!((j.sources.len(), j.single_source), (2, false), "both nodes are its sources");
    assert_eq!(tip(vec![("http://b", Ok(b)), ("http://a", Ok(a))]), Some(40_000_000));
    assert_eq!(send::Fees::of(Some(GWEI), Some(40_000_000)).priority, 40_000_000);
}

#[test]
fn several_tables_give_their_lower_median() {
    let n = |x: u64| Ok(table(&[x]));
    assert_eq!(tip(vec![("http://a", n(9)), ("http://b", n(1)), ("http://c", n(5))]), Some(5));
    assert_eq!(tip(vec![("http://a", n(9)), ("http://b", n(1)), ("http://c", n(5)), ("http://d", n(7))]), Some(5));
    assert_eq!(tip(vec![("http://a", n(2)), ("http://b", n(3)), ("http://c", n(u64::MAX))]), Some(3));
    assert_eq!(tip(vec![("http://a", n(4)), ("http://b", n(4))]), Some(4));
}

#[test]
fn a_table_that_gives_no_number_is_passed_over_in_its_question_alone() {
    let good = table(&[60_000_000]);
    let params = send::tip_params(0x40);
    for bad in ["null", "{\"oldestBlock\":\"0x2d\"}", "{\"reward\":[]}", "{\"reward\":[[\"12\"]]}", "{\"reward\":[[\"0x1ffffffffffffffff\"]]}", "{\"reward\":[\"0x1\"]}"] {
        let j = judged(vec![("http://a", Ok(w(bad))), ("http://b", Ok(good.clone()))]).unwrap_or_else(|e| panic!("{bad}: the other node's number: {e:?}"));
        assert_eq!(send::tip_of(&j.value), Some(60_000_000), "{bad}");
        assert!(j.single_source, "{bad}: one node gave the number");
        assert!(matches!(j.missing.as_slice(), [(n, Missing::Unreadable(_))] if n == "http://a"), "{bad}: {:?}", j.missing);
        assert_eq!(judge::unreadable("eth_feeHistory", &params, &w(bad)), None, "{bad}: not passed over for the rest of the read");
    }
    assert_eq!(tip(vec![("http://a", Err(Trouble::Transport(zikaron_anchor::rpc::Broke::Short))), ("http://b", Ok(good))]), Some(60_000_000));
}

#[test]
fn no_number_from_any_node_is_no_reading() {
    match judged(vec![("http://a", Ok(w("null"))), ("http://b", Ok(w("{\"reward\":[]}")))]) {
        Err(NoReading::NoneAnswered(m)) => assert_eq!(m.len(), 2, "{m:?}"),
        other => panic!("no reading: {other:?}"),
    }
    assert_eq!(send::Fees::of(Some(GWEI), None).priority, send::PRIORITY_FEE);
}

#[test]
fn the_lower_median_is_still_held_at_the_ceiling() {
    let t = tip(vec![("http://a", Ok(table(&[3 * GWEI]))), ("http://b", Ok(table(&[5 * GWEI])))]);
    assert_eq!(t, Some(3 * GWEI));
    assert_eq!(send::Fees::of(Some(GWEI), t).priority, send::PRIORITY_FEE);
}
