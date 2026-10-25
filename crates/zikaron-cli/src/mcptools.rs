//! This product's MCP tools (CLI-SCHEMA §13): which verbs are tools and how their parameters go, handed to the
//! engine (`crate::mcp`) as one table.

use crate::mcp::Table;
use crate::out;

pub static TABLE: Table = Table {
    name: "zikaron",
    not_writes: &NOT_WRITES,
    reads: &READS,
    writes_too: &WRITES_TOO,
    home_optional: &HOME_OPTIONAL,
    as_home: &AS_HOME,
    paths: &PATHS,
    joined: &JOINED,
    sends: &SENDS,
    told,
};

/// The verbs the desktop does under `--home` (`verbs::HOME`) that are no write tool: `init` (the home has its genesis
/// already) and `attest` (it asks for the passcode, on the desktop alone). Every other row of `HOME` is a write tool.
const NOT_WRITES: [&str; 2] = ["init", "attest"];

/// The read tools, each with the flags of its row it does not take.
const READS: [(&str, &[&str]); 9] = [
    ("show", &[]),
    ("audit", &["out"]),
    ("scan", &["endpoint", "fixture", "basis", "adoptions", "proxy"]),
    ("check-grant", &[]),
    ("chain-check", &[]),
    ("depth", &[]),
    ("badge", &["encode"]),
    ("kit-verify", &["ledger", "key"]),
    ("contract", &[]),
];

/// The read tools that write: `kit-verify` takes the originals out into a folder (`originals-to`). A verb with a read
/// tool (`kit-verify`) is offered once, as that tool, though the desktop does it under `--home` too.
const WRITES_TOO: [&str; 1] = ["kit-verify"];

/// The write tools whose `home` may be left out: `kit-verify` verifies a plain package, and takes its originals out,
/// with no home (a package addressed to one key is opened through a home: the credential's, or the desktop's).
const HOME_OPTIONAL: [&str; 1] = ["kit-verify"];

/// A read tool's flag given as the home: `scan` reads with the home's chain settings and nodes (`--ledger
/// <home>/ledger`).
const AS_HOME: [(&str, &str); 1] = [("scan", "ledger")];

/// The parameters whose values are paths (any tool when the tool is empty).
const PATHS: [(&str, &str); 13] = [
    ("", "home"),
    ("", "ledger"),
    ("", "input"),
    ("", "fragment"),
    ("history", "file"),
    ("adopt", "anchors"),
    ("kit-export", "out"),
    ("kit-verify", "kit"),
    ("kit-verify", "originals-to"),
    ("show", "path"),
    ("check-grant", "grant"),
    ("chain-check", "hop"),
    ("badge", "decode"),
];

/// A `hop` is one path or two joined by `=`.
const JOINED: [&str; 1] = ["hop"];

/// `anchor` sends this home's queue, spending this identity's gas when sending is automatic.
const SENDS: [&str; 1] = ["anchor"];

/// Each tool's one sentence, a closed constant in the system's language (`out::say`); never anything from a ledger.
fn told(tool: &str) -> &'static str {
    let (zh, en) = match tool {
        "history" => ("为一个文件在本家账本里记一条存证", "Record a file in this home's ledger"),
        "grant" => ("签发一份授权", "Issue a grant"),
        "revoke" => ("撤销一份授权", "Revoke a grant"),
        "adopt" => ("收编已在链上的存证", "Adopt anchors already on chain"),
        "annotate" => ("给一条记录添加注记", "Annotate a record"),
        "retract" => ("删除一条记录(写一条撤回)", "Retract a record (write a retraction)"),
        "kit-export" => ("把记录包导出到给定位置;给了接收方即加密给它", "Export a record kit to the given place; given a recipient, encrypt it to them"),
        "anchor" => (
            "上链:发送本家的待上链队列;自动发送时花这个身份的 gas",
            "Anchor: send this home's queue; with sending automatic it spends this identity's gas",
        ),
        "show" => ("读一条记录", "Show one entry"),
        "audit" => ("核验一本账本", "Audit a ledger"),
        "scan" => ("读链上的存证,用本家的链设置", "Read anchors on chain with this home's chain settings"),
        "check-grant" => ("核验一份授权", "Check a grant"),
        "chain-check" => ("核验一条转授权链", "Check a chain of grants"),
        "depth" => ("读一条记录的上链深度", "Read an entry's anchoring depth"),
        "badge" => ("解读一份授权码", "Decode a grant code"),
        "kit-verify" => (
            "核验一份记录包,加密的经本家打开;给了位置即把原件取到那里",
            "Verify a record kit, an encrypted one opened through this home; given a place, take its originals out there",
        ),
        "contract" => ("读命令行的契约:动词、旗与退出码", "Read the command line's contract: verbs, flags and exit codes"),
        _ => ("", ""),
    };
    out::say(zh, en)
}
