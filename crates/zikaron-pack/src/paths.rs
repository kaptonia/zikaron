//! Check whether a built binary contains a path from the build machine (release builds remap paths to neutral
//! names; a leftover would reveal the builder's home, checkout or tool caches). The paths searched for are this
//! machine's at check time: the home and current directories, Cargo's and rustup's homes, and on a CI runner
//! its workspace, temp directory and tool cache. Each is searched as UTF-8 and UTF-16 (Windows uses wide
//! strings).

/// Environment variables whose values are this machine's paths (in addition to the current directory).
pub const PATH_ENVS: [&str; 7] = ["HOME", "USERPROFILE", "CARGO_HOME", "RUSTUP_HOME", "GITHUB_WORKSPACE", "RUNNER_TEMP", "RUNNER_TOOL_CACHE"];

/// The paths to search for: each set value of [`PATH_ENVS`] and `cwd`, without trailing separator, at least
/// four characters (a shorter one such as `/` would match anywhere), deduplicated.
pub fn needles(env: impl Fn(&str) -> Option<String>, cwd: Option<String>) -> Vec<String> {
    let mut out: Vec<String> = PATH_ENVS.iter().filter_map(|k| env(k)).chain(cwd).map(|p| p.trim_end_matches(['/', '\\']).to_string()).filter(|p| p.chars().count() >= 4).collect();
    out.sort();
    out.dedup();
    out
}

/// Where each needle occurs in `hay`: the needle, the byte offset, and `utf-8` or `utf-16`.
pub fn found(hay: &[u8], needles: &[String]) -> Vec<(String, usize, &'static str)> {
    let mut out = Vec::new();
    for n in needles {
        let wide: Vec<u8> = n.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
        for (form, needle) in [("utf-8", n.as_bytes().to_vec()), ("utf-16", wide)] {
            if needle.is_empty() || needle.len() > hay.len() {
                continue;
            }
            out.extend(hay.windows(needle.len()).enumerate().filter(|(_, w)| *w == needle.as_slice()).map(|(at, _)| (n.clone(), at, form)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needles: each set variable and the current directory, trailing separators dropped, short ones and
    /// duplicates skipped; matches in UTF-8 and UTF-16 are reported with offsets; a binary with none is clean.
    #[test]
    fn this_machines_paths_are_found_in_either_width() {
        let env = |k: &str| match k {
            "HOME" => Some("/home/runner/".to_string()),
            "GITHUB_WORKSPACE" => Some("/home/runner/work/z/z".to_string()),
            "RUNNER_TEMP" => Some("/".to_string()),
            "CARGO_HOME" => Some("/home/runner".to_string()),
            _ => None,
        };
        let n = needles(env, Some("/home/runner/work/z/z".to_string()));
        assert_eq!(n, vec!["/home/runner".to_string(), "/home/runner/work/z/z".to_string()]);
        let wide: Vec<u8> = "/home/runner".encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
        let hay = [b"xx/home/runner/work/z/z/src/main.rs\0".to_vec(), wide].concat();
        let hits = found(&hay, &n);
        assert!(hits.contains(&("/home/runner".to_string(), 2, "utf-8")));
        assert!(hits.contains(&("/home/runner/work/z/z".to_string(), 2, "utf-8")));
        assert!(hits.iter().any(|(p, _, form)| p == "/home/runner" && *form == "utf-16"));
        assert!(found(b"/zikaron/src/main.rs /cargo/registry", &n).is_empty(), "mapped names are not this machine's");
        assert!(found(b"", &n).is_empty());
    }
}
