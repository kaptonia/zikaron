//! An entropy error names the operation and source before the OS message (`entropy <source>: …`) and keeps the
//! OS error kind; no weaker source is substituted. Its own test binary, since the simulated failure is
//! process-wide and no other test may read entropy alongside it.

/// A simulated unreadable source gives `entropy <ENTROPY_SOURCE>: …` and no bytes.
#[test]
fn an_entropy_error_names_its_interface_and_source() {
    zikaron_os::pretend_entropy_fails(true);
    let one = zikaron_os::random(32);
    let mut buf = [0u8; 16];
    let two = zikaron_os::fill_random(&mut buf);
    zikaron_os::pretend_entropy_fails(false);
    for e in [one.err().expect("refused, never a weaker source"), two.err().expect("refused")] {
        let said = e.to_string();
        assert!(said.starts_with(&format!("entropy {}: ", zikaron_os::ENTROPY_SOURCE)), "{said}");
        assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied, "the kind kept");
    }
    assert_eq!(buf, [0u8; 16], "nothing filled");
    assert_eq!(zikaron_os::random(32).expect("read again once not pretended").len(), 32);
}
