//! The MCP engine (`zikaron_cli::mcp`) under a table of its own: the tools are the verb table's rows as the table
//! hands them, nothing of another product's table; a number, a container or a member the law refuses is named where
//! it stands.

use zikaron::json::Value;
use zikaron_cli::mcp::{self, Table};

fn told(tool: &str) -> &'static str {
    if tool == "annotate" { "one" } else { "other" }
}

/// A table unlike the product's: two write tools, three read tools, `scan` given its home, `kit-verify` writing with
/// its home required, `annotate` sending.
static T: Table = Table {
    name: "engine-test",
    not_writes: &["init", "history", "grant", "revoke", "adopt", "attest", "retract", "anchor"],
    reads: &[("show", &["path"]), ("scan", &["endpoint", "fixture", "basis", "adoptions", "proxy"]), ("kit-verify", &["ledger", "key"])],
    writes_too: &["kit-verify"],
    home_optional: &[],
    as_home: &[("scan", "ledger")],
    paths: &[("", "home")],
    joined: &[],
    sends: &["annotate"],
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

fn hint(t: &Value, k: &str) -> Option<bool> {
    match t.member("annotations").and_then(|a| a.member(k)) {
        Some(Value::Bool(b)) => Some(*b),
        _ => None,
    }
}

/// The tools are the rows of `verbs::HOME` the table does not hold back, then the table's read tools, each with its
/// row's flags less the withheld; whether a tool writes, needs its home, sends, and what it says come from the table.
#[test]
fn the_tools_are_read_off_the_verb_table_and_the_table_handed_in() {
    let list = mcp::listed(&T);
    let Some(Value::Arr(all)) = list.member("tools") else { panic!("tools") };
    let names: Vec<&str> = all.iter().filter_map(|t| t.member("name").and_then(Value::as_str)).collect();
    assert_eq!(names, ["annotate", "kit-export", "show", "scan", "kit-verify"]);

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
    assert!(!required(show));

    let scan = tool(&list, "scan");
    assert!(params(scan).contains(&"home".to_string()) && !params(scan).contains(&"ledger".to_string()), "the flag given as the home is named home");

    let verify = tool(&list, "kit-verify");
    assert_eq!(hint(verify, "readOnlyHint"), Some(false), "a read tool that writes");
    assert!(required(verify), "its home is required unless the table lets it be left out");
    assert!(!params(verify).iter().any(|p| p == "key" || p == "ledger"));
}

/// Where a message the law's reader refuses stands: member names and places joined by `.`, the whole message `.`.
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
