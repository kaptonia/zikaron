//! This product's MCP tool table (`zikaron_cli::mcptools`) names only verbs and flags of the verb table, so a renamed
//! flag cannot quietly stop being withheld or checked as a path, and every tool says its one sentence.

use zikaron_cli::mcptools::TABLE as T;
use zikaron_cli::verbs;

fn row(verb: &str, homed: bool) -> Vec<&'static str> {
    verbs::accepts(verb, homed).unwrap_or_else(|| panic!("{verb} is a verb"))
}

fn homed(verb: &str) -> bool {
    verbs::HOME.iter().any(|(v, _)| *v == verb)
}

#[test]
fn the_tool_table_names_only_verbs_and_flags_of_the_verb_table() {
    for v in T.not_writes {
        assert!(homed(v), "{v} is a row of verbs::HOME");
    }
    let mut tools: Vec<&str> = verbs::HOME.iter().map(|(v, _)| *v).filter(|v| !T.not_writes.contains(v)).collect();
    for &(verb, withheld) in T.reads {
        let flags = row(verb, false);
        for f in withheld {
            assert!(flags.contains(f), "{verb} withholds --{f}, a flag of its row");
        }
        tools.push(verb);
    }
    for v in T.writes_too.iter().chain(T.home_optional).chain(T.sends) {
        assert!(tools.contains(v), "{v} is a tool");
    }
    for v in T.writes_too {
        assert!(T.reads.iter().any(|(r, _)| r == v) && homed(v), "{v} is a read tool the desktop does too");
    }
    for &(verb, flag) in T.as_home {
        assert!(T.reads.iter().any(|(r, _)| *r == verb) && row(verb, false).contains(&flag), "{verb} --{flag}");
    }
    let flags_of = |v: &str| if homed(v) && !T.reads.iter().any(|(r, _)| *r == v) { row(v, true) } else { row(v, false) };
    for &(verb, flag) in T.paths {
        if verb.is_empty() {
            assert!(tools.iter().any(|v| flags_of(v).contains(&flag)), "--{flag} is some tool's");
        } else {
            assert!(tools.contains(&verb) && flags_of(verb).contains(&flag), "{verb} --{flag}");
        }
    }
    for f in T.joined {
        assert!(T.paths.iter().any(|(_, p)| p == f), "--{f} is a path");
    }
    for v in &tools {
        assert!(!(T.told)(v).is_empty(), "{v} says its sentence");
    }
}
