//! Reading the system proxy settings is bounded: a reader that hangs or fails leaves a new connection going as
//! if there were no proxy, within the cap and the exchange deadline, and reports it; display never waits; one
//! read at a time, with no lock held while reading. Its own test binary, since the reader is process-wide.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use zikaron_net::{parse, post_json, read_system_with, way_for, way_shown, Ask, Fail, Limits, SystemProxies, Way, SYSTEM_FRESH, SYSTEM_READ_CAP};

static READS: AtomicUsize = AtomicUsize::new(0);

fn silent() -> Option<SystemProxies> {
    READS.fetch_add(1, Ordering::SeqCst);
    std::thread::sleep(Duration::from_secs(30));
    None
}

fn unreadable() -> Option<SystemProxies> {
    READS.fetch_add(1, Ordering::SeqCst);
    None
}

fn panicking() -> Option<SystemProxies> {
    READS.fetch_add(1, Ordering::SeqCst);
    panic!("the reader broke")
}

fn slow_socks() -> Option<SystemProxies> {
    READS.fetch_add(1, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(300));
    Some(SystemProxies { socks: Some(("127.0.0.1".into(), 1)), ..Default::default() })
}

fn t() -> zikaron_net::Target {
    parse("https://node.invalid/").expect("address")
}

#[test]
fn reading_the_system_is_bounded_and_falls_to_no_proxy() {
    // A reader that never answers: the connection waits only up to the cap, goes direct, and reports why.
    read_system_with(silent);
    let began = Instant::now();
    let r = way_for(&t(), Duration::from_millis(200));
    let took = began.elapsed();
    assert_eq!((r.way, r.system_unread), (Way::Direct, true));
    assert!(took >= Duration::from_millis(200) && took < Duration::from_millis(900), "{took:?}");
    // Display never waits, even with a read in flight; a second connection joins the read in flight.
    let began = Instant::now();
    let shown = way_shown(&t());
    assert!(began.elapsed() < Duration::from_millis(100) && shown.system_unread);
    let _ = way_for(&t(), Duration::from_millis(50));
    assert_eq!(READS.load(Ordering::SeqCst), 1, "one reading at a time");
    // A deadline shorter than the cap also bounds the wait.
    let limits = Limits { deadline: Duration::from_millis(300), max_answer: 0 };
    let began = Instant::now();
    let answered = post_json(&t(), b"{}", &limits, Ask::Read);
    assert!(began.elapsed() < SYSTEM_READ_CAP, "the deadline bounds the wait: {:?}", began.elapsed());
    assert!(matches!(answered, Err(Fail::Name(_)) | Err(Fail::Late(_))), "{:?}", answered.err());

    // A reader that fails: direct at once, and reported.
    read_system_with(unreadable);
    let began = Instant::now();
    let r = way_for(&t(), SYSTEM_READ_CAP);
    assert!(began.elapsed() < Duration::from_millis(500));
    assert_eq!((r.way, r.system_unread), (Way::Direct, true));
    // A reader that panics: the same, and later reads still happen.
    read_system_with(panicking);
    let before = READS.load(Ordering::SeqCst);
    assert!(way_for(&t(), SYSTEM_READ_CAP).system_unread);
    assert!(way_for(&t(), SYSTEM_READ_CAP).system_unread);
    assert_eq!(READS.load(Ordering::SeqCst), before + 2, "a broken reading does not stop the next");

    // A slow reader within the cap: waited for and followed.
    read_system_with(slow_socks);
    let r = way_for(&t(), SYSTEM_READ_CAP);
    assert!(matches!(&r.way, Way::Through(p) if p.spelled() == "socks5://127.0.0.1:1") && !r.system_unread);
    // Fresh for SYSTEM_FRESH: not read again within it.
    let before = READS.load(Ordering::SeqCst);
    let _ = way_for(&t(), SYSTEM_READ_CAP);
    assert_eq!(READS.load(Ordering::SeqCst), before);
    // After it, read again; display uses the last read meanwhile.
    std::thread::sleep(SYSTEM_FRESH + Duration::from_millis(50));
    let began = Instant::now();
    let shown = way_shown(&t());
    assert!(began.elapsed() < Duration::from_millis(100));
    assert!(matches!(&shown.way, Way::Through(_)) && !shown.system_unread, "the last reading is shown while a fresh one is taken");
    let _ = way_for(&t(), SYSTEM_READ_CAP);
    assert_eq!(READS.load(Ordering::SeqCst), before + 1);

    // Shutdown while a connection waits for the system settings: it stops waiting at once.
    read_system_with(silent);
    let asking = std::thread::spawn(|| {
        let began = Instant::now();
        let r = post_json(&t(), b"{}", &Limits { deadline: Duration::from_secs(20), max_answer: 0 }, Ask::Read);
        (r.err(), began.elapsed())
    });
    std::thread::sleep(Duration::from_millis(100));
    zikaron_net::close_down();
    let (said, took) = asking.join().expect("asking");
    assert_eq!(said, Some(Fail::Closed));
    assert!(took < Duration::from_millis(1000), "{took:?}");
    zikaron_net::open_up();
}
