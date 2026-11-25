#![forbid(unsafe_code)]

use std::process::ExitCode;
use zikaron::json::{canon_bytes, Value};
use zikaron_anchor::wire::{self, Body, W};
use zikaron_anchor::rpc::Endpoint as _;
use zikaron_anchor::{endpoints, input, rpc, scan, send};

fn misuse(m: &str) -> ! {
    eprintln!("zka: {m}");
    std::process::exit(2)
}

fn say(v: &Value) -> ExitCode {
    say_bytes(&canon_bytes(v))
}

fn say_bytes(b: &[u8]) -> ExitCode {
    say_bytes_as(b, 0)
}

fn say_bytes_as(b: &[u8], code: u8) -> ExitCode {
    use std::io::Write;
    let mut out = std::io::stdout();
    if out.write_all(b).is_err() || out.flush().is_err() {
        return ExitCode::from(2);
    }
    ExitCode::from(code)
}

const PARTIAL: u8 = 3;
const DENIED: u8 = 1;
const REFUSED: u8 = 4;

fn refused_as(code: &str, detail: &str, exit: u8) -> ExitCode {
    say_bytes_as(
        &canon_bytes(&Value::Obj(vec![
            ("detail".into(), Value::Str(detail.into())),
            ("ok".into(), Value::Bool(false)),
            ("reason".into(), Value::Str(code.into())),
        ])),
        exit,
    )
}

fn refused(code: &str, detail: &str) -> ExitCode {
    refused_as(code, detail, 0)
}

fn read_wire(path: &str) -> (Vec<u8>, W) {
    let b = std::fs::read(path).unwrap_or_else(|e| misuse(&format!("读不出 {path}:{e}")));
    let v = wire::parse(&b).unwrap_or_else(|| misuse(&format!("{path} 不是 JSON")));
    (b, v)
}

fn h32(x: &str) -> [u8; 32] {
    let b = zikaron::hexfmt::decode(x).unwrap_or_else(|| misuse(&format!("{x} 不是十六进制")));
    if b.len() != 32 {
        misuse(&format!("{x} 不是三十二字节"));
    }
    let mut h = [0u8; 32];
    h.copy_from_slice(&b);
    h
}

fn h20(x: &str) -> [u8; 20] {
    let b = zikaron::hexfmt::decode(x).unwrap_or_else(|| misuse(&format!("{x} 不是十六进制")));
    if b.len() != 20 {
        misuse(&format!("{x} 不是二十字节"));
    }
    let mut h = [0u8; 20];
    h.copy_from_slice(&b);
    h
}

struct Flags {
    pairs: Vec<(String, String)>,
    words: Vec<String>,
}

impl Flags {
    fn parse(argv: &[String]) -> Flags {
        let mut f = Flags { pairs: Vec::new(), words: Vec::new() };
        let mut i = 0;
        while i < argv.len() {
            let a = &argv[i];
            if let Some(name) = a.strip_prefix("--") {
                i += 1;
                let v = argv.get(i).cloned().unwrap_or_else(|| misuse(&format!("--{name} 后面缺值")));
                f.pairs.push((name.to_string(), v));
            } else {
                f.words.push(a.clone());
            }
            i += 1;
        }
        f
    }
    fn one(&self, name: &str) -> Option<String> {
        self.pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    }
    fn need(&self, name: &str) -> String {
        self.one(name).unwrap_or_else(|| misuse(&format!("缺 --{name}")))
    }
    fn many(&self, name: &str) -> Vec<String> {
        self.pairs.iter().filter(|(k, _)| k == name).map(|(_, v)| v.clone()).collect()
    }
}

fn endpoint_specs(f: &Flags) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    for spec in f.many("endpoint") {
        match zikaron_anchor::rpc::endpoint_spec(&spec) {
            Ok(x) => out.push(x),
            Err(zikaron_anchor::rpc::NotAnEndpoint::ChainNotInt) => misuse("链号不是十进制整数"),
            Err(_) => misuse("--endpoint 的形是 <链号>=<url>"),
        }
    }
    if out.is_empty() {
        misuse("至少要一个 --endpoint <链号>=<url>");
    }
    out
}

fn rounds(specs: &[(u64, String)]) -> usize {
    let mut n = 1;
    for (c, _) in specs {
        n = n.max(specs.iter().filter(|(x, _)| x == c).count());
    }
    n
}

fn nth_for(specs: &[(u64, String)], chain: u64, k: usize) -> String {
    let mine: Vec<&String> = specs.iter().filter(|(c, _)| *c == chain).map(|(_, u)| u).collect();
    mine[k.min(mine.len() - 1)].clone()
}

fn chains_of(specs: &[(u64, String)]) -> Vec<u64> {
    let mut c: Vec<u64> = specs.iter().map(|(x, _)| *x).collect();
    c.sort_unstable();
    c.dedup();
    c
}

fn strings_of(path: Option<String>) -> Vec<String> {
    let Some(p) = path else { return Vec::new() };
    let (_, v) = read_wire(&p);
    v.as_arr()
        .unwrap_or_else(|| misuse(&format!("{p} 不是一个数组")))
        .iter()
        .map(|x| x.as_str().unwrap_or_else(|| misuse("数组里要的是字符串")).to_string())
        .collect()
}

fn replay_fragment(path: &str) -> Result<Value, (String, String)> {
    let (src, fx) = read_wire(path);
    let basis_bytes = match fx.member("basis") {
        Some(b) => b.raw(&src).to_vec(),
        None => b"null".to_vec(),
    };
    let recorded = fx.member("rpc").cloned().unwrap_or(W::of(Body::Obj(Vec::new())));
    let Body::Obj(chains) = &recorded.body else { misuse("录制的 rpc 不是对象") };
    let mut replays: Vec<(u64, rpc::Replay)> = Vec::new();
    for (cid, exchanges) in chains {
        let id: u64 = cid.parse().unwrap_or_else(|_| misuse("录制的链号不是十进制整数"));
        let ex = exchanges.as_arr().unwrap_or(&[]).to_vec();
        match rpc::Replay::new(format!("replay:{id}"), &ex) {
            Ok(r) => replays.push((id, r)),
            Err(e) => return Err(("E_RECORDING".into(), format!("{e:?}"))),
        }
    }
    let mut eps: Vec<(u64, &mut dyn rpc::Endpoint)> =
        replays.iter_mut().map(|(c, r)| (*c, r as &mut dyn rpc::Endpoint)).collect();
    match scan::run(&basis_bytes, &mut eps) {
        Err(r) => Err((r.code().into(), r.detail())),
        Ok(Err(no_label)) => Ok(no_label),
        Ok(Ok(s)) => Ok(scan::fragment(&s)),
    }
}

fn recorded_conclusion(path: &str) -> Option<Value> {
    let (_, fx) = read_wire(path);
    let text = fx.member("expected").and_then(|x| x.as_str())?.to_string();
    if text.is_empty() {
        return None;
    }
    zikaron::json::parse(text.as_bytes()).ok()
}

fn agree_recordings(paths: &[String]) -> Result<endpoints::Reading, (String, String)> {
    let mut distinct: Vec<String> = paths
        .iter()
        .map(|p| std::fs::canonicalize(p).map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|_| p.clone()))
        .collect();
    distinct.sort();
    distinct.dedup();
    let mut chains: Vec<u64> = distinct.iter().flat_map(|p| chains_in_recording(p)).collect();
    chains.sort_unstable();
    chains.dedup();
    let thin: Vec<u64> = chains
        .into_iter()
        .filter(|c| distinct.iter().filter(|p| chains_in_recording(p).contains(c)).count() < 2)
        .collect();
    let mut runs: Vec<(String, Value)> = Vec::new();
    for p in paths {
        runs.push((p.clone(), replay_fragment(p)?));
    }
    endpoints::agree_over(runs, thin).map_err(|d| {
        ("E_ENDPOINTS_DISAGREE".to_string(), format!("{} 份读数不一致:{}", d.fragments.len(), d.sources.join(" 对 ")))
    })
}

fn chains_in_recording(path: &str) -> Vec<u64> {
    let (_, fx) = read_wire(path);
    let mut out = Vec::new();
    if let Some(W { body: Body::Obj(chains), .. }) = fx.member("rpc") {
        for (cid, _) in chains {
            if let Ok(id) = cid.parse::<u64>() {
                out.push(id);
            }
        }
    }
    out
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let verb = argv.first().cloned().unwrap_or_default();
    let rest: Vec<String> = argv.into_iter().skip(1).collect();
    let f = Flags::parse(&rest);
    match verb.as_str() {
        "scan" => {
            let path = f.words.first().cloned().unwrap_or_else(|| misuse("usage: zka scan <fixture.json>"));
            match replay_fragment(&path) {
                Ok(v) => say(&v),
                Err((code, detail)) => refused(&code, &detail),
            }
        }
        "agree" => {
            let paths = f.many("recording");
            if paths.is_empty() {
                misuse("usage: zka agree --recording <a.json> --recording <b.json> ...");
            }
            match agree_recordings(&paths) {
                Ok(reading) => say(&reading.value()),
                Err((code, detail)) => refused(&code, &detail),
            }
        }
        "replay" => {
            let paths = f.words.clone();
            if paths.is_empty() {
                misuse("usage: zka replay <recording.json> [<recording.json> ...]");
            }
            let mut recorded: Vec<Value> = Vec::new();
            for p in &paths {
                match recorded_conclusion(p) {
                    Some(v) => recorded.push(v),
                    None => misuse(&format!("{p} 没有录下的结论(expected),用 zka scan 回放")),
                }
            }
            let now = if paths.len() == 1 { replay_fragment(&paths[0]) } else { agree_recordings(&paths).map(|r| r.fragment) };
            match now {
                Err((code, detail)) => refused_as(&code, &detail, REFUSED),
                Ok(v) if recorded.iter().all(|r| canon_bytes(r) == canon_bytes(&v)) => {
                    say(&Value::Obj(vec![("ok".into(), Value::Bool(true)), ("replayed".into(), v)]))
                }
                Ok(v) => say_bytes_as(
                    &canon_bytes(&Value::Obj(vec![
                        ("ok".into(), Value::Bool(false)),
                        ("reason".into(), Value::Str("E_REPLAY_DIFFERS".into())),
                        ("recorded".into(), recorded.swap_remove(0)),
                        ("replayed".into(), v),
                    ])),
                    PARTIAL,
                ),
            }
        }
        "agree-facts" => {
            let facts: Vec<String> = f.need("facts").split(',').filter(|x| !x.is_empty()).map(str::to_string).collect();
            let named: Vec<&str> = facts.iter().map(String::as_str).collect();
            let paths = f.many("answer");
            if paths.len() < 2 {
                misuse("usage: zka agree-facts --facts <a,b> --answer <one.json> --answer <two.json> ...");
            }
            let mut runs: Vec<(String, Value)> = Vec::new();
            for p in paths {
                let (_, w) = read_wire(&p);
                let v = wire::to_core(&w).unwrap_or_else(|| misuse(&format!("{p} 不是一枚读数")));
                runs.push((p, endpoints::project(&v, &named)));
            }
            match endpoints::agree(runs) {
                Ok(reading) => say(&reading.value()),
                Err(d) => refused(
                    "E_ENDPOINTS_DISAGREE",
                    &format!("{} 份读数不一致:{}", d.fragments.len(), d.sources.join(" 对 ")),
                ),
            }
        }
        "scan-live" => {
            let (basis_src, _) = read_wire(&f.need("basis"));
            let specs = endpoint_specs(&f);
            let thin: Vec<u64> = chains_of(&specs)
                .into_iter()
                .filter(|c| {
                    let mut urls: Vec<&String> = specs.iter().filter(|(x, _)| x == c).map(|(_, u)| u).collect();
                    urls.sort();
                    urls.dedup();
                    urls.len() < 2
                })
                .collect();
            let mut runs: Vec<(String, Value)> = Vec::new();
            for k in 0..rounds(&specs) {
                let mut https: Vec<(u64, rpc::Http)> = Vec::new();
                for c in chains_of(&specs) {
                    let url = nth_for(&specs, c, k);
                    https.push((c, rpc::Http::new(&url).unwrap_or_else(|| misuse("端点只认 http:// 或 https:// 地址"))));
                }
                let names: Vec<String> = https.iter().map(|(c, h)| format!("{c}={}", h.name())).collect();
                let mut eps: Vec<(u64, &mut dyn rpc::Endpoint)> =
                    https.iter_mut().map(|(c, h)| (*c, h as &mut dyn rpc::Endpoint)).collect();
                match scan::run(&basis_src, &mut eps) {
                    Err(r) => return refused(r.code(), &r.detail()),
                    Ok(Err(no_label)) => return say(&no_label),
                    Ok(Ok(s)) => runs.push((names.join(","), scan::fragment(&s))),
                }
            }
            match endpoints::agree_over(runs, thin) {
                Ok(reading) => say(&reading.value()),
                Err(d) => refused(
                    "E_ENDPOINTS_DISAGREE",
                    &format!(
                        "{} 份读数不一致:{}",
                        d.fragments.len(),
                        d.sources.join(" 对 ")
                    ),
                ),
            }
        }
        "record" => {
            let (basis_src, _) = read_wire(&f.need("basis"));
            let specs = endpoint_specs(&f);
            let out = f.need("out");
            let mut recorders: Vec<(u64, rpc::Recorder)> = Vec::new();
            for c in chains_of(&specs) {
                let url = nth_for(&specs, c, 0);
                let http = rpc::Http::new(&url).unwrap_or_else(|| misuse("端点只认 http:// 或 https:// 地址"));
                recorders.push((c, rpc::Recorder::new(Box::new(http))));
            }
            let fragment = {
                let mut eps: Vec<(u64, &mut dyn rpc::Endpoint)> =
                    recorders.iter_mut().map(|(c, r)| (*c, r as &mut dyn rpc::Endpoint)).collect();
                match scan::run(&basis_src, &mut eps) {
                    Err(r) => return refused(r.code(), &r.detail()),
                    Ok(Err(no_label)) => no_label,
                    Ok(Ok(s)) => scan::fragment(&s),
                }
            };
            let mut rpc_obj: Vec<(String, W)> = Vec::new();
            for (c, r) in recorders {
                rpc_obj.push((c.to_string(), W::of(Body::Arr(r.exchanges))));
            }
            let basis_w = wire::parse(&basis_src).unwrap_or_else(|| misuse("基底不是 JSON"));
            let fixture = W::of(Body::Obj(vec![
                ("basis".into(), basis_w),
                ("expected".into(), W::of(Body::Str(String::from_utf8_lossy(&canon_bytes(&fragment)).into_owned()))),
                ("rpc".into(), W::of(Body::Obj(rpc_obj))),
            ]));
            std::fs::write(&out, format!("{}\n", wire::write(&fixture)))
                .unwrap_or_else(|e| misuse(&format!("写不下 {out}:{e}")));
            say(&Value::Obj(vec![("ok".into(), Value::Bool(true)), ("recorded".into(), Value::Str(out))]))
        }
        "notices-record" => {
            let specs = endpoint_specs(&f);
            let (chain, url) = specs[0].clone();
            let venue = zikaron_anchor::revoke::venue(chain).unwrap_or_else(|| misuse("这条链没有部署表所载的撤销合约与换钥簿"));
            let out = f.need("out");
            let mut head = rpc::Http::new(&url).unwrap_or_else(|| misuse("端点只认 http:// 或 https:// 地址"));
            let to = match f.one("to") {
                Some(t) => t.parse::<u64>().unwrap_or_else(|_| misuse("--to 不是十进制整数")),
                None => head.call("eth_blockNumber", &Value::Arr(Vec::new())).ok().and_then(|w| w.as_str().and_then(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok())).unwrap_or_else(|| misuse("读不出链头")),
            };
            let mut rec = rpc::Recorder::new(Box::new(rpc::Http::new(&url).unwrap_or_else(|| misuse("端点只认 http:// 或 https:// 地址"))));
            let n = match scan::notice_input(&mut rec, &venue, to) {
                Ok(v) => v,
                Err(r) => return refused(r.code(), &r.detail()),
            };
            let fixture = W::of(Body::Obj(vec![
                ("chain".into(), W::of(Body::Str(chain.to_string()))),
                ("expected".into(), W::of(Body::Str(String::from_utf8_lossy(&canon_bytes(&n)).into_owned()))),
                ("rpc".into(), W::of(Body::Arr(rec.exchanges))),
                ("to".into(), W::of(Body::Str(to.to_string()))),
            ]));
            std::fs::write(&out, format!("{}\n", wire::write(&fixture))).unwrap_or_else(|e| misuse(&format!("写不下 {out}:{e}")));
            say(&Value::Obj(vec![("ok".into(), Value::Bool(true)), ("recorded".into(), Value::Str(out))]))
        }
        "notices" => {
            let path = f.words.first().cloned().unwrap_or_else(|| misuse("usage: zka notices <recording.json>"));
            let (_, fx) = read_wire(&path);
            let chain: u64 = fx.member("chain").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or_else(|| misuse("录制没有链号"));
            let to: u64 = fx.member("to").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or_else(|| misuse("录制没有止块"));
            let recorded = recorded_conclusion(&path).unwrap_or_else(|| misuse("录制没有录下的结论(expected)"));
            let venue = zikaron_anchor::revoke::venue(chain).unwrap_or_else(|| misuse("这条链没有部署表所载的撤销合约与换钥簿"));
            let ex = fx.member("rpc").and_then(|x| x.as_arr()).unwrap_or(&[]).to_vec();
            let mut replay = match rpc::Replay::new(format!("replay:{chain}"), &ex) {
                Ok(r) => r,
                Err(e) => return refused_as("E_RECORDING", &format!("{e:?}"), REFUSED),
            };
            match scan::notice_input(&mut replay, &venue, to) {
                Err(r) => refused_as(r.code(), &r.detail(), REFUSED),
                Ok(v) if canon_bytes(&v) == canon_bytes(&recorded) => say(&Value::Obj(vec![("ok".into(), Value::Bool(true)), ("replayed".into(), v)])),
                Ok(v) => say_bytes_as(
                    &canon_bytes(&Value::Obj(vec![("ok".into(), Value::Bool(false)), ("reason".into(), Value::Str("E_REPLAY_DIFFERS".into())), ("recorded".into(), recorded), ("replayed".into(), v)])),
                    PARTIAL,
                ),
            }
        }
        "anchor" => {
            let specs = endpoint_specs(&f);
            let (chain, url) = specs[0].clone();
            let mut ep = rpc::Http::new(&url).unwrap_or_else(|| misuse("端点只认 http:// 或 https:// 地址"));
            let key = {
                let k = f.need("key");
                let b = zikaron::hexfmt::decode(&k).unwrap_or_else(|| misuse("私钥不是十六进制"));
                if b.len() != 32 {
                    misuse("私钥不是三十二字节");
                }
                let mut x = [0u8; 32];
                x.copy_from_slice(&b);
                x
            };
            let form = match f.need("form").as_str() {
                "registry" => send::Form::Registry,
                _ => misuse("--form 只认 registry"),
            };
            let registry = f.one("registry").map(|x| h20(&x));
            let hashes: Vec<[u8; 32]> = f.many("hash").iter().map(|x| h32(x)).collect();
            let calldata = f.one("calldata").map(|x| zikaron::hexfmt::decode(&x).unwrap_or_else(|| misuse("calldata 不是十六进制")));
            if hashes.is_empty() && calldata.is_none() {
                misuse("至少要一个 --hash,或一段 --calldata");
            }
            let wait = std::time::Duration::from_secs(
                f.one("wait-secs").and_then(|x| x.parse().ok()).unwrap_or(90),
            );
            match send::anchor(&mut ep, &key, chain, form, registry, &hashes, calldata, wait) {
                Ok(s) if s.anchored() => {
                    let bn = match s.confirm {
                        send::Confirm::Included { block_number, .. } => Value::Int(block_number),
                        _ => Value::Null,
                    };
                    say(&Value::Obj(vec![
                        ("blockNumber".into(), bn),
                        ("ok".into(), Value::Bool(true)),
                        ("tx".into(), Value::Str(zikaron::hexfmt::encode(&s.tx))),
                    ]))
                }
                Ok(s) => {
                    let mut fields = vec![
                        ("ok".to_string(), Value::Bool(false)),
                        ("tx".to_string(), Value::Str(zikaron::hexfmt::encode(&s.tx))),
                    ];
                    match &s.confirm {
                        send::Confirm::Included { status, .. } => {
                            fields.push(("reason".into(), Value::Str("E_TX_STATUS".into())));
                            fields.push(("status".into(), Value::Int(*status)));
                        }
                        send::Confirm::NotYet | send::Confirm::Split { .. } => {
                            fields.push(("reason".into(), Value::Str("E_TX_NOT_YET".into())));
                            fields.push(("waitedSeconds".into(), Value::Int(wait.as_secs())));
                        }
                        send::Confirm::Unreachable(why) => {
                            fields.push(("detail".into(), Value::Str(why.clone())));
                            fields.push(("reason".into(), Value::Str("E_TX_UNCONFIRMED".into())));
                        }
                    }
                    fields.sort_by(|a, b| a.0.cmp(&b.0));
                    say(&Value::Obj(fields))
                }
                Err(e) => refused("E_SEND", &format!("{e:?}")),
            }
        }
        "audit-input" => {
            let (_, read) = read_wire(&f.need("fragment"));
            let frag = read.member("fragment").cloned().unwrap_or(read);
            let frag = wire::to_core(&frag).unwrap_or_else(|| misuse("片段不在法 §3 的值域里"));
            let root = f.need("root");
            let pile = strings_of(f.one("pile"));
            match input::assemble(&frag, &root, &pile) {
                Some(v) => say(&v),
                None => refused("E_FRAGMENT", "片段缺 anchors / basis 之一"),
            }
        }
        "audit" => {
            let path = f.words.first().cloned().unwrap_or_else(|| misuse("usage: zka audit <input.json>"));
            let bytes = std::fs::read(&path).unwrap_or_else(|e| misuse(&format!("读不出 {path}:{e}")));
            let report = input::audit(&bytes);
            let label = zikaron::json::parse(&report).ok().and_then(|v| v.member("label").and_then(|l| l.as_str()).map(str::to_string));
            use zikaron::v2::tokens::Label;
            let exit = match label.as_deref().and_then(Label::parse) {
                Some(Label::Complete) => 0,
                Some(Label::Gaps | Label::Unproven) => PARTIAL,
                _ => DENIED,
            };
            say_bytes_as(&report, exit)
        }
        _ => misuse("usage: zka <scan|agree|replay|scan-live|record|anchor|audit-input|audit> ..."),
    }
}
