use crate::args::Args;
use crate::out::{self, Answer};
use crate::codes::Reason;
use crate::verbs::Need;

pub const PROGRAM: &str = "zikaron";

pub const WRITE: [&str; 5] = ["ledger", "key", "root", "seq", "prev"];

pub const KEYED_TOO: [&str; 1] = ["anchor"];

pub(crate) const ACCEPTS: [(&str, &[&str], &[&str], &[Need]); 32] = [
    ("keygen", &[], &[], &[]),
    ("init", &[], &["ledger", "key", "statement", "home"], &[Need::Flag("key"), Need::Flag("ledger")]),
    ("history", &WRITE, &["content", "mark", "toolchain", "note", "file", "home"], &[Need::Flag("ledger")]),
    ("grant", &WRITE, &["grantee", "work", "terms", "history", "window-from", "window-to", "scope", "upstream", "supersedes", "endpoint", "proxy", "rest-keys", "home"], &[Need::Flag("ledger")]),
    ("revoke", &WRITE, &["grant", "case", "endpoint", "proxy", "rest-keys", "home"], &[Need::Flag("ledger")]),
    ("annotate", &WRITE, &["subject", "note", "home"], &[Need::Flag("ledger")]),
    ("retract", &WRITE, &["subject", "note", "home"], &[Need::Flag("ledger")]),
    ("migrate", &[], &["ledger", "key", "statement", "effective", "home"], &[Need::Flag("ledger")]),
    ("anchor", &[], &["key", "endpoint", "form", "registry", "hash", "calldata", "wait-secs", "proxy", "rest-keys", "home", "ledger"], &[]),
    ("scan", &[], &["endpoint", "fixture", "basis", "proxy", "rest-keys", "ledger"], &[Need::AnyOf(&["ledger", "fixture", "endpoint"])]),
    ("audit", &[], &["ledger", "root", "input", "fragment", "out"], &[Need::AnyOf(&["ledger", "input"])]),
    ("check-grant", &[], &["ledger", "root", "input", "fragment", "now", "grant", "book", "notices", "endpoint", "proxy", "rest-keys"], &[Need::Flag("grant")]),
    ("chain-check", &[], &["ledger", "root", "hop", "now", "input", "fragment", "book", "notices", "endpoint", "proxy", "rest-keys"], &[]),
    ("badge", &[], &["encode", "decode"], &[Need::OneOf(&["encode", "decode"])]),
    ("kit-export", &[], &["ledger", "root", "note", "out", "entry", "file", "as", "home", "to", "to-key", "endpoint", "proxy", "rest-keys", "originals"], &[Need::Flag("ledger"), Need::Flag("out")]),
    ("kit-verify", &[], &["kit", "fragment", "originals-to", "home", "ledger", "key"], &[Need::Flag("kit")]),
    ("grant-package", &[], &["ledger", "home", "grant", "originals", "to", "out", "package", "originals-to", "key", "book", "fragment", "notices", "endpoint", "proxy", "rest-keys"], &[Need::OneOf(&["out", "package"])]),
    ("clean", &[], &["home", "ledger", "yes"], &[Need::OneOf(&["home", "ledger"])]),
    ("hand-over", &[], &["ledger", "out"], &[Need::Flag("ledger"), Need::Flag("out")]),
    ("take-over", &[], &["ledger", "handoff"], &[Need::Flag("ledger"), Need::Flag("handoff")]),
    ("show", &[], &["ledger", "entry", "path"], &[Need::OneOf(&["path", "entry"])]),
    ("contract", &[], &[], &[]),
    ("mcp", &[], &["rest-keys", "key-file", "endpoint", "proxy"], &[]),
    ("face-table", &[], &["root", "ledger", "home", "endpoint", "proxy", "rest-keys"], &[Need::OneOf(&["root", "ledger", "home"])]),
    ("face-home", &WRITE, &["chain", "endpoint", "proxy", "rest-keys", "home"], &[Need::Flag("ledger")]),
    ("face-account", &WRITE, &["platform", "handle", "note", "endpoint", "proxy", "rest-keys", "home"], &[Need::Flag("ledger")]),
    ("face-key", &WRITE, &["address", "role", "sig", "note", "endpoint", "proxy", "rest-keys", "home"], &[Need::Flag("ledger")]),
    ("face-name", &WRITE, &["name", "endpoint", "proxy", "rest-keys", "home"], &[Need::Flag("ledger")]),
    ("face-release", &WRITE, &["subject", "endpoint", "proxy", "rest-keys", "home"], &[Need::Flag("ledger")]),
    ("face-reroot", &WRITE, &["to", "endpoint", "proxy", "rest-keys", "home"], &[Need::Flag("ledger")]),
    ("face-origin", &WRITE, &["from", "endpoint", "proxy", "rest-keys", "home"], &[Need::Flag("ledger")]),
    ("face-joined", &[], &["root", "to", "ledger", "home", "endpoint", "proxy", "rest-keys"], &[Need::Flag("root"), Need::Flag("to")]),
];

pub const REPEATED: [(&str, &str); 29] = [
    ("anchor", "hash"),
    ("mcp", "endpoint"),
    ("grant", "endpoint"),
    ("revoke", "endpoint"),
    ("check-grant", "book"),
    ("chain-check", "book"),
    ("grant-package", "book"),
    ("check-grant", "notices"),
    ("chain-check", "notices"),
    ("grant-package", "notices"),
    ("check-grant", "endpoint"),
    ("chain-check", "endpoint"),
    ("grant-package", "endpoint"),
    ("scan", "fixture"),
    ("scan", "endpoint"),
    ("chain-check", "hop"),
    ("badge", "encode"),
    ("kit-export", "entry"),
    ("kit-export", "file"),
    ("kit-export", "endpoint"),
    ("face-table", "endpoint"),
    ("face-home", "endpoint"),
    ("face-account", "endpoint"),
    ("face-key", "endpoint"),
    ("face-name", "endpoint"),
    ("face-release", "endpoint"),
    ("face-reroot", "endpoint"),
    ("face-origin", "endpoint"),
    ("face-joined", "endpoint"),
];

pub const BY_HAND: [&str; 1] = ["migrate"];

pub(crate) const ALL_FLAGS: [&str; 29] = [
    "ledger", "key", "root", "seq", "prev", "statement", "content", "mark", "toolchain", "note",
    "grantee", "work", "terms", "history", "window-from", "window-to", "scope", "upstream",
    "grant", "case", "to", "effective",
    "subject", "endpoint", "form", "registry", "hash", "calldata", "wait-secs",
];

pub(crate) const MORE_FLAGS: [&str; 35] = [
    "fixture", "basis", "input", "fragment", "now", "hop", "entry",
    "path", "out", "file", "encode", "decode", "proxy", "rest-keys",
    "key-file", "home", "as", "kit", "handoff", "originals-to", "to-key", "originals",
    "package", "yes", "book", "notices", "supersedes",
    "chain", "platform", "handle", "address", "role", "sig", "name", "from",
];

pub(crate) fn dispatch(a: &Args) -> Answer {
    match a.verb() {
        "keygen" => crate::verbs::keygen(a),
        "init" => crate::verbs::init(a),
        "history" => crate::verbs::history(a),
        "grant" => crate::verbs::grant(a),
        "revoke" => crate::verbs::revoke(a),
        "annotate" => crate::verbs::annotate(a),
        "retract" => crate::verbs::retract(a),
        "migrate" => crate::verbs::migrate(a),
        "anchor" => crate::verbs::anchor(a),
        "scan" => crate::verbs::scan(a),
        "audit" => crate::verbs::audit_verb(a),
        "check-grant" => crate::verbs::check_grant(a),
        "chain-check" => crate::verbs::chain_check(a),
        "badge" => crate::verbs::badge_verb(a),
        "kit-export" => crate::verbs::kit_export(a),
        "kit-verify" => crate::verbs::kit_verify(a),
        "grant-package" => crate::grantpack::grant_package(a),
        "clean" => crate::grantpack::clean(a),
        "hand-over" => crate::handoff::hand_over(a),
        "take-over" => crate::handoff::take_over(a),
        "show" => crate::verbs::show(a),
        "contract" => crate::verbs::contract(a),
        "mcp" => crate::mcp::serve(a, &crate::mcptools::TABLE),
        "face-table" => crate::faceverbs::face_table(a),
        "face-home" => crate::faceverbs::face_home(a),
        "face-account" => crate::faceverbs::face_account(a),
        "face-key" => crate::faceverbs::face_key(a),
        "face-name" => crate::faceverbs::face_name(a),
        "face-release" => crate::faceverbs::face_release(a),
        "face-reroot" => crate::faceverbs::face_reroot(a),
        "face-origin" => crate::faceverbs::face_origin(a),
        "face-joined" => crate::faceverbs::face_joined(a),
        other => out::misuse(Reason::Args, out::Subject::Placed { at: 1, bytes: other.len() }, out::Said::NotAVerb),
    }
}
