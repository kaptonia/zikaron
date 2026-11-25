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
    door_reads: &DOOR_READS,
    alone_home: ALONE_HOME,
    alone_more: &[],
    told,
};

const NOT_WRITES: [&str; 1] = ["init"];

const READS: [(&str, &[&str]); 8] = [
    ("show", &[]),
    ("audit", &["out"]),
    ("scan", &["endpoint", "fixture", "basis", "proxy"]),
    ("check-grant", &["proxy", "rest-keys"]),
    ("chain-check", &["proxy", "rest-keys"]),
    ("badge", &["encode"]),
    ("kit-verify", &["ledger", "key"]),
    ("contract", &[]),
];

const DOOR_READS: [&str; 2] = ["face-table", "face-joined"];

const WRITES_TOO: [&str; 1] = ["kit-verify"];

const HOME_OPTIONAL: [&str; 1] = ["kit-verify"];

const AS_HOME: [(&str, &str); 1] = [("scan", "ledger")];

const PATHS: [(&str, &str); 15] = [
    ("", "home"),
    ("", "ledger"),
    ("", "input"),
    ("", "fragment"),
    ("", "book"),
    ("", "notices"),
    ("history", "file"),
    ("kit-export", "out"),
    ("grant-package", "out"),
    ("kit-verify", "kit"),
    ("kit-verify", "originals-to"),
    ("show", "path"),
    ("check-grant", "grant"),
    ("chain-check", "hop"),
    ("badge", "decode"),
];

const JOINED: [&str; 1] = ["hop"];

const ALONE_HOME: (&str, &str) = ("ledger", zikaron_seal::envelope::LEDGER_DIR);

const SENDS: [&str; 1] = ["anchor"];

fn told(tool: &str) -> &'static str {
    let (zh, en) = match tool {
        "history" => ("为一个文件在本家账本里记一条存证", "Record a file in this home's ledger"),
        "grant" => ("签发一份授权", "Issue a grant"),
        "revoke" => ("撤销一份授权", "Revoke a grant"),
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
        "badge" => ("解读一份授权码", "Decode a grant code"),
        "kit-verify" => (
            "核验一份记录包,加密的经本家打开;给了位置即把原件取到那里",
            "Verify a record kit, an encrypted one opened through this home; given a place, take its originals out there",
        ),
        "contract" => ("读命令行的契约:动词、旗与退出码", "Read the command line's contract: verbs, flags and exit codes"),
        "grant-package" => ("把一份授权导出为授权包;给了根地址即加密给它", "Export a grant as a grant package; given a root, encrypt it to its key"),
        "clean" => ("列出或清理本机未加密的导出件", "List or clear this machine's plain exports"),
        "face-table" => ("读本家身份的面表", "Read this home's face table"),
        "face-home" => ("立链或换世系链:写一条 home 面条目", "Open or move the home chain: write a home face entry"),
        "face-account" => ("关联一个平台账号", "Bind a platform account"),
        "face-key" => ("关联一个地址:同意由那个地址的钥签出并给出,或沿用账本里已有的", "Bind an address: its consent given as that address's key signed it, or the one the ledger holds"),
        "face-name" => ("设或改显示名", "Set or change the display name"),
        "face-release" => ("解除一条关联", "Release a binding"),
        "face-reroot" => ("换根:把面搬到另一个根,撤不回", "Move the faces to another root; it can't be undone"),
        "face-origin" => ("承根:写明面从哪个根搬来", "Name the root the faces came from"),
        "face-joined" => ("读两个根的面表是否相连", "Read whether two roots' face tables are joined"),
        _ => ("", ""),
    };
    out::say(zh, en)
}
