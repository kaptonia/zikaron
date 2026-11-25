#[test]
fn the_engines_c_sources_leave_no_names_in_the_build() {
    let t = zikaron_net::parse("https://node.invalid/").expect("address");
    std::hint::black_box(zikaron_net::way_for(&t, std::time::Duration::ZERO));
    let me = std::fs::read(std::env::current_exe().expect("this test binary")).expect("its bytes");
    for backwards in ["c.sjkciuq", "c.pxegerbil", "c.edocinubil", "c.aotd"] {
        let name: String = backwards.chars().rev().collect();
        let wide: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        for needle in [name.as_bytes().to_vec(), wide] {
            assert!(!me.windows(needle.len()).any(|w| w == needle.as_slice()), "{name} is named in the build");
        }
    }
}
