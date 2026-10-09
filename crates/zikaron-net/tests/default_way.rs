//! A process that sets no proxy setting follows the system proxy settings, read through the platform. Its own
//! test binary, which never sets a choice or a reader.

use zikaron_net::{parse, way_for, way_under, Choice, SystemProxies, SYSTEM_READ_CAP};

#[test]
fn a_process_that_sets_nothing_follows_the_system() {
    let t = parse("https://node.invalid:8545/").expect("address");
    let r = way_for(&t, SYSTEM_READ_CAP);
    assert_eq!(r.choice, Choice::System, "nothing chosen is following the system");
    // The route matches the platform's reading of this machine's settings.
    let platform = zikaron_os::system_proxies().map(|p| SystemProxies { https: p.https, http: p.http, socks: p.socks, exceptions: p.exceptions, auto_config: p.auto_config });
    assert_eq!(r, way_under(&t, &Choice::System, platform.as_ref()));
}

/// Where system settings are environment variables (unix other than macOS), a proxy set there is followed by
/// default. Runs as a child process so the environment is set before anything reads it.
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
