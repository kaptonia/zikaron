use zikaron_net::{parse, way_for, way_shown, way_under, Auto, Choice, SystemProxies, SYSTEM_READ_CAP};

#[test]
fn a_process_that_sets_nothing_follows_the_system() {
    let t = parse("https://node.invalid:8545/").expect("address");
    let r = way_for(&t, SYSTEM_READ_CAP);
    assert_eq!(r.choice, Choice::System, "nothing chosen is following the system");
    let platform = zikaron_os::system_proxies().map(|p| SystemProxies::from_platform(&p));
    match platform.as_ref() {
        Some(s) if s.automatic() => {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
            let mut r = r;
            while r != way_shown(&t) && std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(100));
                r = way_for(&t, SYSTEM_READ_CAP);
            }
            assert_eq!(r, way_shown(&t));
        }
        p => assert_eq!(r, way_under(&t, &Choice::System, p, &Auto::None)),
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn the_environments_proxy_is_followed_with_nothing_chosen() {
    if std::env::var_os("ZKN_DEFAULT_WAY_CHILD").is_none() {
        let me = std::env::current_exe().expect("this test binary");
        let out = std::process::Command::new(me)
            .args(["--exact", "the_environments_proxy_is_followed_with_nothing_chosen", "--nocapture"])
            .env("ZKN_DEFAULT_WAY_CHILD", "1")
            .env("https_proxy", "http://127.0.0.1:7890")
            .env_remove("no_proxy")
            .env_remove("NO_PROXY")
            .output()
            .expect("the child runs");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
        return;
    }
    let t = parse("https://node.invalid:8545/").expect("address");
    match way_for(&t, SYSTEM_READ_CAP).way {
        zikaron_net::Way::Through(p) => assert_eq!(p.spelled(), "http://127.0.0.1:7890"),
        other => panic!("{other:?}"),
    }
}
