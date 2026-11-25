use app::action::{apply, apply_settled, Action, Applied};
use app::queue::Kept as _;
use std::path::{Path, PathBuf};

const ROLE: &str = "ZK_COPIED_ROLE";
const MACHINE: &str = "ZK_COPIED_MACHINE";
const NODE: &str = "ZK_COPIED_NODE";
const PIN: &str = "27618394";

fn scratch(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("zk-copied-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("a scratch place");
    p
}

fn tree(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(at: &Path, base: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries: Vec<_> = std::fs::read_dir(at).expect("a folder").flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out);
            } else {
                out.push((p.strip_prefix(base).expect("below").to_path_buf(), std::fs::read(&p).expect("a file")));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("the copy's folder");
    for e in std::fs::read_dir(from).expect("a folder").flatten() {
        let (p, q) = (e.path(), to.join(e.file_name()));
        if p.is_dir() {
            copy_tree(&p, &q);
        } else {
            std::fs::copy(&p, &q).expect("a file copied");
        }
    }
}

fn child(test: &str, role: &str, machine: &Path, node: Option<&str>) -> Vec<String> {
    let me = std::env::current_exe().expect("this test binary");
    let mut cmd = std::process::Command::new(me);
    cmd.args([test, "--exact", "--nocapture", "--test-threads=1"]).env(ROLE, role).env(MACHINE, machine);
    if let Some(n) = node {
        cmd.env(NODE, n);
    }
    let out = cmd.output().expect("the child runs");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success() && text.contains("1 passed"), "{role}: {}\n{text}\n{}", out.status, String::from_utf8_lossy(&out.stderr));
    text.lines().filter_map(|l| l.split_once("SAID ").map(|(_, said)| said.to_string())).collect()
}

fn opened(lay: bool) -> app::shell::Shell {
    let machine = PathBuf::from(std::env::var(MACHINE).expect("a machine directory"));
    app::places::set(app::places::Places { key_account: app::places::ACCOUNT.to_string(), machine_dir: Some(machine), user_home: None });
    if lay {
        app::keybox::set_pin(PIN).expect("a vault laid");
    } else {
        app::keybox::unlock(PIN).expect("the vault opens");
    }
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let _ = shell.refresh_anchor();
    shell
}

fn lay(shell: &mut app::shell::Shell) {
    let machine = app::home::machine_dir().expect("the machine directory");
    let chosen = machine.join(app::home::DEFAULT_HOME);
    std::fs::create_dir_all(&chosen).expect("the chosen home");
    app::home::write_pointer(&chosen).expect("the chosen home recorded");
    let s = app::key::generate().expect("a fresh key");
    app::register::change(|reg| app::identity::add_existing(reg, &s, app::roles::Role::Author, false, "")).expect("an identity");
    let _ = shell.refresh_anchor();
    let _ = app::action::boot_home(shell);
    assert!(shell.home.is_some(), "the seat's home opened: {:?}", shell.faults);
    let _ = apply_settled(shell, Action::Genesis { statement: "copied".into() });
}

fn said_home(shell: &mut app::shell::Shell) {
    if shell.home.is_none() {
        let _ = app::action::boot_home(shell);
    }
    let h = shell.home.as_ref().expect("the seat's home opened");
    let n = h.ledger().and_then(|l| l.survey()).map(|s| s.items.len()).expect("the ledger reads");
    println!("SAID home={} entries={n}", h.root().display());
}

fn write_one(shell: &mut app::shell::Shell, tag: &str) -> Applied {
    if shell.home.is_none() {
        let _ = app::action::boot_home(shell);
    }
    let probe = std::env::temp_dir().join(format!("zk-copied-probe-{tag}-{}.txt", std::process::id()));
    std::fs::write(&probe, tag.as_bytes()).expect("a file");
    let _ = apply_settled(shell, Action::TakeContent { source: app::anchorx::Source::File, path: probe.display().to_string() });
    let got = apply_settled(shell, Action::RecordWork { note_md: String::new(), files: Vec::new(), for_: None });
    let _ = std::fs::remove_file(&probe);
    got
}

#[test]
fn a_machine_folder_copied_elsewhere_reads_and_writes_its_own_homes() {
    const TEST: &str = "a_machine_folder_copied_elsewhere_reads_and_writes_its_own_homes";
    match std::env::var(ROLE).as_deref() {
        Ok("lay") => return lay(&mut opened(true)),
        Ok("write") => {
            let mut shell = opened(false);
            let got = write_one(&mut shell, "on the copy");
            assert!(matches!(got, Applied::Recorded { .. }), "the entry written: {got:?} {:?}", shell.faults);
            return said_home(&mut shell);
        }
        Ok(_) => return said_home(&mut opened(false)),
        Err(_) => {}
    }
    let work = scratch("offline");
    let (orig, copy) = (work.join("one").join("machine"), work.join("two").join("machine"));
    child(TEST, "lay", &orig, None);
    copy_tree(&orig, &copy);
    let before = tree(&orig);
    let wrote = child(TEST, "write", &copy, None);
    assert_eq!(tree(&orig), before, "the original not changed by a byte: the copy's write landed in the copy");
    let (on_copy, on_orig) = (wrote.join(" "), child(TEST, "read", &orig, None).join(" "));
    assert!(on_copy.starts_with(&format!("home={}", copy.display())) && on_copy.ends_with("entries=2"), "the copy reads its own ledger: {on_copy}");
    assert!(on_orig.starts_with(&format!("home={}", orig.display())) && on_orig.ends_with("entries=1"), "the original as it stood: {on_orig}");
    let _ = std::fs::remove_dir_all(&work);
}

const CHAIN: u64 = 31337;
const REGISTRY: &str = "ZK_COPIED_REGISTRY";
const BANKER: &str = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";

fn rpc(url: &str, method: &str, params: &str) -> String {
    use std::io::{Read, Write};
    let at = url.trim_start_matches("http://");
    let body = format!("{{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"{method}\",\"params\":{params}}}");
    let mut s = std::net::TcpStream::connect(at).expect("the local chain answers");
    s.write_all(format!("POST / HTTP/1.1\r\nHost: {at}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).expect("asked");
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    let json = out.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    let v = zikaron::json::parse(json.as_bytes()).unwrap_or_else(|_| panic!("{method}: {json}"));
    let r = v.member("result").unwrap_or_else(|| panic!("{method}: {json}"));
    String::from_utf8_lossy(&zikaron::json::canon_bytes(r)).trim_matches('"').to_string()
}

struct Anvil(std::process::Child, String);

impl Drop for Anvil {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn anvil() -> Option<Anvil> {
    let on_path = |tool: &str| std::process::Command::new(tool).arg("--version").output().is_ok_and(|o| o.status.success());
    if !on_path("anvil") || !on_path("forge") {
        return None;
    }
    let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).expect("a free port").port();
    let child = std::process::Command::new("anvil").args(["--port", &port.to_string(), "--chain-id", &CHAIN.to_string(), "--silent"]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().expect("anvil starts");
    let a = Anvil(child, format!("http://127.0.0.1:{port}"));
    for _ in 0..400 {
        if std::net::TcpStream::connect(a.1.trim_start_matches("http://")).is_ok() {
            return Some(a);
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("anvil never listened");
}

fn deploy_registry(url: &str, work: &Path) -> String {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("base").join("zikaron-core").join("contracts");
    let stage = work.join("registry-build");
    std::fs::create_dir_all(&stage).expect("a build place");
    std::fs::copy(src.join("foundry.toml"), stage.join("foundry.toml")).expect("the settings");
    copy_tree(&src.join("src"), &stage.join("src"));
    let built = std::process::Command::new("forge").args(["build", "--offline", "--skip", "test"]).current_dir(&stage).output().expect("forge runs");
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let art = std::fs::read(stage.join("out").join("ZikaronRegistry.sol").join("ZikaronRegistry.json")).expect("the artifact");
    let v = zikaron::json::parse(&art).expect("json");
    let code = v.member("bytecode").and_then(|b| b.member("object")).and_then(zikaron::json::Value::as_str).expect("the creation code").to_string();
    let tx = rpc(url, "eth_sendTransaction", &format!("[{{\"from\":\"{BANKER}\",\"data\":\"{code}\"}}]"));
    let mut said = String::from("null");
    for _ in 0..400 {
        said = rpc(url, "eth_getTransactionReceipt", &format!("[\"{tx}\"]"));
        if said != "null" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let receipt = zikaron::json::parse(said.as_bytes()).expect("a receipt");
    receipt.member("contractAddress").and_then(zikaron::json::Value::as_str).unwrap_or_else(|| panic!("deployed: {tx} {said}")).to_string()
}

fn landed(shell: &mut app::shell::Shell, kind: app::task::Kind) -> Vec<app::task::Outcome> {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let mut got = Vec::new();
    while std::time::Instant::now() < until {
        got.extend(shell.drain().into_iter().filter(|o| o.kind == kind));
        if !got.is_empty() && !shell.tasks.in_flight(kind) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    got
}

fn on_chain(shell: &mut app::shell::Shell) {
    let nodes = std::env::var(NODE).expect("nodes");
    let specs = nodes.split(' ').map(|u| format!("{CHAIN}={u}")).collect::<Vec<_>>().join(" ");
    assert!(matches!(apply(shell, Action::SetEndpoints { specs }), Applied::Endpoints(2)));
    if shell.tasks.in_flight(app::task::Kind::Chain) {
        landed(shell, app::task::Kind::Chain);
    }
    let registry = std::env::var(REGISTRY).expect("the registry");
    match apply(shell, Action::SetBasis { chain: CHAIN.to_string(), registry, from_block: "0".into() }) {
        Applied::Started(app::task::Kind::Basis) => {
            landed(shell, app::task::Kind::Basis);
        }
        Applied::Basis { .. } => {}
        other => panic!("the basis: {other:?} {:?}", shell.faults),
    }
    assert!(shell.settings.registry.is_some(), "the basis kept: {:?}", shell.faults);
}

fn anchor_all(shell: &mut app::shell::Shell) {
    let n = shell.queue.len();
    assert!(n > 0, "something queued");
    answers_gas(apply_settled(shell, Action::EstimateGas { count: n }), shell);
    assert!(matches!(apply(shell, Action::SendBatch { count: n }), Applied::Started(app::task::Kind::Anchor)), "{:?}", shell.faults);
    landed(shell, app::task::Kind::Anchor);
    let h = shell.home.as_ref().expect("a home");
    let left = app::queue::Queue::read(h).map(|q| q.len()).unwrap_or(usize::MAX);
    assert!(matches!(shell.sent, Some(app::task::Done::Anchored { confirmed: true, .. })) && left == 0, "anchored: {:?} left {left} {:?}", shell.sent, shell.faults);
    println!("SAID anchored={n}");
}

fn answers_gas(a: Applied, shell: &app::shell::Shell) {
    assert!(matches!(a, Applied::Gas { .. }), "estimated: {a:?} {:?}", shell.faults);
}

fn export(shell: &mut app::shell::Shell, out: &Path) {
    let lacks = app::exitgate::ask_of(shell).and_then(|ask| app::exitgate::read(&ask)).map(|r| r.missing.join(",")).unwrap_or_else(|f| format!("unread {}", f.said()));
    let asked = apply(shell, Action::ExportKit { from: String::new(), to: String::new(), ids: String::new(), attach: String::new(), note: "copied".into(), out: out.display().to_string(), form: "folder".into(), recipient: String::new(), recipient_key: String::new(), originals: "none".into(), companion: true });
    let said = match asked {
        Applied::Started(app::task::Kind::Kit) => match landed(shell, app::task::Kind::Kit).pop().map(|o| o.result) {
            Some(Ok(_)) => "ok".to_string(),
            Some(Err(f)) => f.which().map(|k| k.as_str().to_string()).unwrap_or_else(|| f.said().to_string()),
            None => "never landed".to_string(),
        },
        other => format!("{other:?}"),
    };
    println!("SAID export={said} lacks={lacks}");
}

#[test]
fn a_machine_folder_copied_before_an_anchor_is_refused_behind_the_chain() {
    const TEST: &str = "a_machine_folder_copied_before_an_anchor_is_refused_behind_the_chain";
    match std::env::var(ROLE).as_deref() {
        Ok("lay") => {
            let mut shell = opened(true);
            lay(&mut shell);
            on_chain(&mut shell);
            return println!("SAID anchor={}", shell.anchor.map(|a| a.hex()).expect("a seated key"));
        }
        Ok("anchor") => {
            let mut shell = opened(false);
            let _ = app::action::boot_home(&mut shell);
            return anchor_all(&mut shell);
        }
        Ok("write and anchor") => {
            let mut shell = opened(false);
            let got = write_one(&mut shell, "the next entry");
            let Applied::Recorded { id, .. } = got else { panic!("written: {got:?} {:?}", shell.faults) };
            anchor_all(&mut shell);
            return println!("SAID entry={}", id.trim_start_matches("0x").to_ascii_lowercase());
        }
        Ok(role) if role.starts_with("export ") => {
            let mut shell = opened(false);
            let _ = app::action::boot_home(&mut shell);
            return export(&mut shell, Path::new(role.trim_start_matches("export ")));
        }
        Ok("write") => {
            let mut shell = opened(false);
            let got = write_one(&mut shell, "on the copy");
            let said = match got {
                Applied::Trouble(f) => f.which().map(|k| k.as_str().to_string()).unwrap_or_default(),
                other => format!("{other:?}"),
            };
            return println!("SAID write={said}");
        }
        Ok(_) => return said_home(&mut opened(false)),
        Err(_) => {}
    }
    let Some(chain) = anvil() else {
        eprintln!("anvil or forge is not on this machine: the local-chain check is not run");
        return;
    };
    let work = scratch("behind");
    let registry = deploy_registry(&chain.1, &work);
    let nodes = format!("{} {}", chain.1, chain.1.replace("127.0.0.1", "localhost"));
    let run = |role: &str, machine: &Path| -> Vec<String> {
        let me = std::env::current_exe().expect("this test binary");
        let out = std::process::Command::new(me).args([TEST, "--exact", "--nocapture", "--test-threads=1"]).env(ROLE, role).env(MACHINE, machine).env(NODE, &nodes).env(REGISTRY, &registry).output().expect("the child runs");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(out.status.success() && text.contains("1 passed"), "{role}: {}\n{text}\n{}", out.status, String::from_utf8_lossy(&out.stderr));
        text.lines().filter_map(|l| l.split_once("SAID ").map(|(_, said)| said.to_string())).collect()
    };
    let (orig, copy) = (work.join("one").join("machine"), work.join("two").join("machine"));
    let key = run("lay", &orig).join("");
    rpc(&chain.1, "anvil_setBalance", &format!("[\"{}\",\"0x8ac7230489e80000\"]", key.trim_start_matches("anchor=")));
    run("anchor", &orig);
    copy_tree(&orig, &copy);
    let entry = run("write and anchor", &orig).into_iter().find_map(|l| l.strip_prefix("entry=").map(str::to_string)).expect("the next entry");
    let before = tree(&orig);
    let stale_kit = work.join("stale-kit");
    let said = run(&format!("export {}", stale_kit.display()), &copy).join(" ");
    assert!(said.starts_with("export=NEWER_ELSEWHERE"), "the copy behind is refused at its exit: {said}");
    assert!(said.ends_with(&format!("lacks={entry}")), "the gate names the entry the copy lacks: {said}");
    assert!(!stale_kit.exists(), "no kit written");
    let said = run("write", &copy).join(" ");
    assert!(said.starts_with("write=") && said != "write=" && !said.contains("Recorded"), "a write on the copy is refused: {said}");
    assert_eq!(tree(&orig), before, "nothing the copy did changed the original by a byte");
    let fresh_kit = work.join("kit");
    let said = run(&format!("export {}", fresh_kit.display()), &orig).join(" ");
    assert_eq!(said, "export=ok lacks=", "the original exports as ever");
    drop(chain);
    let _ = std::fs::remove_dir_all(&work);
}
