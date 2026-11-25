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

#[test]
fn a_parameter_says_it_takes_a_path_exactly_where_a_relative_path_is_refused() {
    use zikaron::json::Value;
    let list = zikaron_cli::mcp::listed(&T);
    let tools = list.member("tools").and_then(Value::as_arr).expect("tools");
    let mut described = 0;
    for tool in tools {
        let name = tool.member("name").and_then(Value::as_str).expect("a name");
        let Some(Value::Obj(props)) = tool.member("inputSchema").and_then(|s| s.member("properties")) else { panic!("properties of {name}") };
        for (param, schema) in props {
            let given = if schema.member("type").and_then(Value::as_str) == Some("array") { Value::Arr(vec![Value::Str("relative".into())]) } else { Value::Str("relative".into()) };
            let mut call = vec![(param.clone(), given)];
            let home_needed = tool.member("inputSchema").and_then(|s| s.member("required")).and_then(Value::as_arr).is_some_and(|r| r.iter().any(|x| x.as_str() == Some("home")));
            if param != "home" && home_needed {
                call.insert(0, ("home".to_string(), Value::Str(std::env::temp_dir().join("h").display().to_string())));
            }
            let refused = matches!(zikaron_cli::mcp::argv_of(&T, name, &call, false), Some(Err(e)) if e.contains(zikaron_cli::out::Said::McpAbsolute.text()));
            let said = schema.member("description").and_then(Value::as_str);
            assert_eq!(said.is_some(), refused, "{name} {param}: {schema:?}");
            described += usize::from(said.is_some());
            let mut kept: Vec<&String> = match schema {
                Value::Obj(m) => m.iter().map(|(k, _)| k).filter(|k| k.as_str() != "description").collect(),
                _ => Vec::new(),
            };
            kept.sort();
            assert!(kept == ["type"] || kept == ["items", "type"], "{name} {param}: the type is as before");
        }
    }
    assert!(described > 0, "some parameter takes a path");
}
