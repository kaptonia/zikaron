use zikaron::json::Value;
use zikaron_cli::mcp::{self, Table};

fn told(tool: &str) -> &'static str {
    if tool == "annotate" { "one" } else { "other" }
}

static T: Table = Table {
    name: "engine-test",
    not_writes: &["init", "history", "grant", "revoke", "retract", "anchor"],
    reads: &[("show", &["path"]), ("scan", &["endpoint", "fixture", "basis", "proxy"]), ("kit-verify", &["ledger", "key"])],
    writes_too: &["kit-verify"],
    home_optional: &[],
    as_home: &[("scan", "ledger")],
    paths: &[("", "home")],
    joined: &[],
    sends: &["annotate"],
    door_reads: &[],
    alone_home: ("ledger", "ledger"),
    alone_more: &[],
    told,
};

fn tool<'a>(list: &'a Value, name: &str) -> &'a Value {
    let Some(Value::Arr(all)) = list.member("tools") else { panic!("tools") };
    all.iter().find(|t| t.member("name").and_then(Value::as_str) == Some(name)).unwrap_or_else(|| panic!("{name} listed"))
}

fn params(t: &Value) -> Vec<String> {
    match t.member("inputSchema").and_then(|s| s.member("properties")) {
        Some(Value::Obj(ms)) => ms.iter().map(|(k, _)| k.clone()).collect(),
        _ => panic!("properties"),
    }
}

fn required(t: &Value) -> bool {
    t.member("inputSchema").and_then(|s| s.member("required")).is_some()
}

fn required_names(t: &Value) -> Vec<String> {
    match t.member("inputSchema").and_then(|s| s.member("required")) {
        Some(Value::Arr(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

fn hint(t: &Value, k: &str) -> Option<bool> {
    match t.member("annotations").and_then(|a| a.member(k)) {
        Some(Value::Bool(b)) => Some(*b),
        _ => None,
    }
}

#[test]
fn the_tools_are_read_off_the_verb_table_and_the_table_handed_in() {
    let list = mcp::listed(&T);
    let Some(Value::Arr(all)) = list.member("tools") else { panic!("tools") };
    let names: Vec<&str> = all.iter().filter_map(|t| t.member("name").and_then(Value::as_str)).collect();
    assert_eq!(names, ["annotate", "kit-export", "grant-package", "clean", "face-table", "face-home", "face-account", "face-key", "face-name", "face-release", "face-reroot", "face-origin", "face-joined", "show", "scan", "kit-verify"]);

    let annotate = tool(&list, "annotate");
    assert_eq!(params(annotate), ["home", "subject", "note"]);
    assert!(required(annotate), "a write needs its home");
    assert_eq!((hint(annotate, "readOnlyHint"), hint(annotate, "destructiveHint"), hint(annotate, "openWorldHint")), (Some(false), Some(true), Some(true)));
    assert_eq!(annotate.member("description").and_then(Value::as_str), Some("one"));

    let export = tool(&list, "kit-export");
    assert_eq!(hint(export, "destructiveHint"), None, "only the table's senders send");
    let entry = export.member("inputSchema").and_then(|s| s.member("properties")).and_then(|p| p.member("entry"));
    assert_eq!(entry.and_then(|e| e.member("type")).and_then(Value::as_str), Some("array"), "a repeated flag is a list");

    let show = tool(&list, "show");
    assert_eq!(params(show), ["ledger", "entry"], "a withheld flag is no parameter");
    assert_eq!(hint(show, "readOnlyHint"), Some(true));
    assert_eq!(required_names(show), ["entry"]);
    assert!(show.member("inputSchema").and_then(|s| s.member("oneOf")).is_none());

    let scan = tool(&list, "scan");
    assert!(params(scan).contains(&"home".to_string()) && !params(scan).contains(&"ledger".to_string()), "the flag given as the home is named home");

    let verify = tool(&list, "kit-verify");
    assert_eq!(hint(verify, "readOnlyHint"), Some(false), "a read tool that writes");
    assert_eq!(required_names(verify), ["home", "kit"], "its home is required unless the table lets it be left out, and its kit always");
    assert!(!params(verify).iter().any(|p| p == "key" || p == "ledger"));
}

#[test]
fn a_refused_token_is_named_where_it_stands() {
    assert_eq!(mcp::fault_at(br#"{"params":{"arguments":{"n":-1}}}"#), "params.arguments.n");
    assert_eq!(mcp::fault_at(br#"{"a":[1,2,3.5]}"#), "a.2");
    assert_eq!(mcp::fault_at(br#"{"a":9007199254740992}"#), "a");
    assert_eq!(mcp::fault_at(br#"{"a":9007199254740991,"b":1,"b":2}"#), "b");
    assert_eq!(mcp::fault_at(br#"{"a":01}"#), "a");
    let deep = format!("{}{}", "[".repeat(129), "]".repeat(129));
    assert_eq!(mcp::fault_at(deep.as_bytes()), (0..128).map(|_| "0").collect::<Vec<_>>().join("."));
    assert_eq!(mcp::fault_at(br#"{"a":"-1.5e3","b":[0,7]}"#), ".");
}

#[test]
fn a_row_of_the_desktops_verbs_can_be_offered_as_a_read_through_the_desktop() {
    let reads = Table {
        name: "engine-door-reads",
        not_writes: T.not_writes,
        reads: T.reads,
        writes_too: T.writes_too,
        home_optional: T.home_optional,
        as_home: T.as_home,
        paths: T.paths,
        joined: T.joined,
        sends: T.sends,
        door_reads: &["kit-export"],
        alone_home: T.alone_home,
        alone_more: T.alone_more,
        told: T.told,
    };
    let list = mcp::listed(&reads);
    let export = tool(&list, "kit-export");
    assert_eq!(hint(export, "readOnlyHint"), Some(true), "offered as a read");
    assert_eq!(required_names(export).first().map(String::as_str), Some("home"), "its home still required");
    let home = std::env::temp_dir().join("h");
    let given = vec![("home".to_string(), Value::Str(home.display().to_string())), ("out".to_string(), Value::Str(home.join("o").display().to_string()))];
    let through = mcp::argv_of(&reads, "kit-export", &given, false).expect("a tool").expect("an argv");
    assert_eq!(&through[..3], ["kit-export", "--home", &home.display().to_string()], "through the desktop, as a write goes");
    let alone = mcp::argv_of(&reads, "kit-export", &given, true).expect("a tool").expect("an argv");
    assert_eq!(&alone[..3], ["kit-export", "--ledger", &home.join("ledger").display().to_string()], "with the credential, as a write goes");
    assert_eq!(hint(tool(&mcp::listed(&T), "kit-export"), "readOnlyHint"), Some(false), "none named: it writes");
}

