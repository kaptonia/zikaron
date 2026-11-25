use std::time::{Duration, Instant};
use zikaron_net::{parse, post_json, read_system_with, Ask, Fail, Limits, SystemProxies, SYSTEM_READ_CAP};

fn silent() -> Option<SystemProxies> {
    std::thread::sleep(Duration::from_secs(30));
    None
}

#[test]
fn a_silent_system_reader_is_waited_on_for_the_cap_then_straight() {
    assert_eq!(SYSTEM_READ_CAP, Duration::from_secs(2));
    read_system_with(silent);
    let t = parse("https://node.invalid/").expect("address");
    let began = Instant::now();
    let said = post_json(&t, b"{}", &Limits { deadline: Duration::from_secs(5), max_answer: 0 }, Ask::Read).err();
    let took = began.elapsed();
    assert!(took >= SYSTEM_READ_CAP && took < Duration::from_secs(5), "the cap, not the deadline: took {took:?}");
    assert!(matches!(said, Some(Fail::Name(_)) | Some(Fail::Connect(_))), "straight: {said:?}");
}
