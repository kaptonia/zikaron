//! `zka`: a small driver binary for the anchoring crate.
//!
//! Each verb is one real library call; no decision is made here. Scan decisions live in `scan`, the basis
//! shape and the report belong to the core's `audit`, the endpoint rule lives in `endpoints`, proof kits in
//! `kit`.

use std::process::ExitCode;
use zikaron::json::{canon_bytes, Value};
use zikaron_anchor::wire::{self, Body, W};
use zikaron_anchor::rpc::Endpoint as _;
use zikaron_anchor::{endpoints, input, kit, rpc, scan, send};

fn misuse(m: &str) -> ! {
    eprintln!("zka: {m}");
    std::process::exit(2)
}

/// Print one answer with no trailing newline, as the core does. The byte shape comes from the core's
/// `canon_bytes` only.
fn say(v: &Value) -> ExitCode {
    say_bytes(&canon_bytes(v))
}

fn say_bytes(b: &[u8]) -> ExitCode {
    use std::io::Write;
    let mut out = std::io::stdout();
    if out.write_all(b).is_err() || out.flush().is_err() {
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

fn refused(code: &str, detail: &str) -> ExitCode {
    say(&Value::Obj(vec![
        ("detail".into(), Value::Str(detail.into())),
        ("ok".into(), Value::Bool(false)),
        ("reason".into(), Value::Str(code.into())),
    ]))
}

fn read_wire(path: &str) -> (Vec<u8>, W) {
    let b = std::fs::read(path).unwrap_or_else(|e| misuse(&format!("读不出 {path}:{e}")));
    let v = wire::parse(&b).unwrap_or_else(|| misuse(&format!("{path} 不是 JSON")));
    (b, v)
}

fn adoptions_of(v: &W) -> Vec<(u64, [u8; 32])> {
    let mut out = Vec::new();
    for e in v.member("adoptions").and_then(|x| x.as_arr()).unwrap_or(&[]) {
        let (Some(c), Some(t)) = (e.member("chainId").and_then(|x| x.as_u64()), e.member("tx").and_then(|x| x.as_str())) else {
            misuse("导入元素要 {chainId, tx}")
        };
        out.push((c, h32(t)));
    }
    out
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

/// `--flag value` options; a flag may repeat.
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

/// `--endpoint <chain>=<url>`, several per chain allowed (the endpoint rule).
fn endpoint_specs(f: &Flags) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    for spec in f.many("endpoint") {
        let (c, url) = spec.split_once('=').unwrap_or_else(|| misuse("--endpoint 的形是 <链号>=<url>"));
        let id: u64 = c.parse().unwrap_or_else(|_| misuse("链号不是十进制整数"));
        out.push((id, url.to_string()));
    }
    if out.is_empty() {
        misuse("至少要一个 --endpoint <链号>=<url>");
    }
    out
}

/// How many rounds a scan runs: the largest endpoint count of any chain.
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

/// The fragment a recording replays to (or the core's no-label value). `scan` and `agree` share this one
/// replay so they cannot answer differently for the same recording.
fn replay_fragment(path: &str) -> Result<Value, (String, String)> {
    let (src, fx) = read_wire(path);
    let basis_bytes = match fx.member("basis") {
        Some(b) => b.raw(&src).to_vec(),
        None => b"null".to_vec(),
    };
    let adoptions = adoptions_of(&fx);
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
    match scan::run(&basis_bytes, &adoptions, &mut eps) {
        Err(r) => Err((r.code().into(), r.detail())),
        Ok(Err(no_label)) => Ok(no_label),
        Ok(Ok(s)) => Ok(scan::fragment(&s)),
    }
}

/// Which chains a recording names.
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
        // Replay one recording and print the scan's canonical fragment.
        "scan" => {
            let path = f.words.first().cloned().unwrap_or_else(|| misuse("usage: zka scan <fixture.json>"));
            match replay_fragment(&path) {
                Ok(v) => say(&v),
                Err((code, detail)) => refused(&code, &detail),
            }
        }
        // The offline side of the endpoint rule: several recordings as several endpoints.
        //
        // A recording is everything one endpoint said. Each gives a fragment for the same input; only full
        // agreement stands; a disagreement is named, never decided by majority. The same recording given
        // twice is one source: sources are told apart by path, as live endpoints are by url.
        "agree" => {
            let mut paths = f.many("recording");
            if paths.is_empty() {
                misuse("usage: zka agree --recording <a.json> --recording <b.json> ...");
            }
            let mut distinct: Vec<String> = paths
                .iter()
                .map(|p| std::fs::canonicalize(p).map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|_| p.clone()))
                .collect();
            distinct.sort();
            distinct.dedup();
            // How many distinct sources each chain has: fewer than two makes that chain single-source.
            let mut chains: Vec<u64> = distinct.iter().flat_map(|p| chains_in_recording(p)).collect();
            chains.sort_unstable();
            chains.dedup();
            let thin: Vec<u64> = chains
                .into_iter()
                .filter(|c| distinct.iter().filter(|p| chains_in_recording(p).contains(c)).count() < 2)
                .collect();
            let mut runs: Vec<(String, Value)> = Vec::new();
            for p in paths.drain(..) {
                match replay_fragment(&p) {
                    Ok(v) => runs.push((p, v)),
                    Err((code, detail)) => return refused(&code, &detail),
                }
            }
            match endpoints::agree_over(runs, thin) {
                Ok(reading) => say(&reading.value()),
                Err(d) => refused(
                    "E_ENDPOINTS_DISAGREE",
                    &format!("{} 份读数不一致:{}", d.fragments.len(), d.sources.join(" 对 ")),
                ),
            }
        }
        // Scan live endpoints (the endpoint rule: agreement across endpoints for green, single source
        // flagged).
        "scan-live" => {
            let (basis_src, _) = read_wire(&f.need("basis"));
            let adoptions = match f.one("adoptions") {
                Some(p) => adoptions_of(&read_wire(&p).1),
                None => Vec::new(),
            };
            let specs = endpoint_specs(&f);
            // Single source is counted per chain, by distinct endpoints, not by rounds: one endpoint agreeing
            // with itself across rounds corroborates nothing.
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
                    https.push((c, rpc::Http::new(&url).unwrap_or_else(|| misuse("端点只认 http://host:port"))));
                }
                let names: Vec<String> = https.iter().map(|(c, h)| format!("{c}={}", h.name())).collect();
                let mut eps: Vec<(u64, &mut dyn rpc::Endpoint)> =
                    https.iter_mut().map(|(c, h)| (*c, h as &mut dyn rpc::Endpoint)).collect();
                match scan::run(&basis_src, &adoptions, &mut eps) {
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
        // Record: write down every question and answer of a live scan in the fixture shape.
        "record" => {
            let (basis_src, _) = read_wire(&f.need("basis"));
            let adoptions_path = f.one("adoptions");
            let adoptions = match &adoptions_path {
                Some(p) => adoptions_of(&read_wire(p).1),
                None => Vec::new(),
            };
            let specs = endpoint_specs(&f);
            let out = f.need("out");
            let mut recorders: Vec<(u64, rpc::Recorder)> = Vec::new();
            for c in chains_of(&specs) {
                let url = nth_for(&specs, c, 0);
                let http = rpc::Http::new(&url).unwrap_or_else(|| misuse("端点只认 http://host:port"));
                recorders.push((c, rpc::Recorder::new(Box::new(http))));
            }
            let fragment = {
                let mut eps: Vec<(u64, &mut dyn rpc::Endpoint)> =
                    recorders.iter_mut().map(|(c, r)| (*c, r as &mut dyn rpc::Endpoint)).collect();
                match scan::run(&basis_src, &adoptions, &mut eps) {
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
            let adoptions_w = W::of(Body::Arr(
                adoptions
                    .iter()
                    .map(|(c, t)| {
                        W::of(Body::Obj(vec![
                            ("chainId".into(), W::of(Body::Num(c.to_string()))),
                            ("tx".into(), W::of(Body::Str(zikaron::hexfmt::encode(t)))),
                        ]))
                    })
                    .collect(),
            ));
            let fixture = W::of(Body::Obj(vec![
                ("adoptions".into(), adoptions_w),
                ("basis".into(), basis_w),
                ("expected".into(), W::of(Body::Str(String::from_utf8_lossy(&canon_bytes(&fragment)).into_owned()))),
                ("rpc".into(), W::of(Body::Obj(rpc_obj))),
            ]));
            std::fs::write(&out, format!("{}\n", wire::write(&fixture)))
                .unwrap_or_else(|e| misuse(&format!("写不下 {out}:{e}")));
            say(&Value::Obj(vec![("ok".into(), Value::Bool(true)), ("recorded".into(), Value::Str(out))]))
        }
        // The two anchoring forms.
        "anchor" => {
            let specs = endpoint_specs(&f);
            let (chain, url) = specs[0].clone();
            let mut ep = rpc::Http::new(&url).unwrap_or_else(|| misuse("端点只认 http://host:port"));
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
                "bare" => send::Form::Bare,
                _ => misuse("--form 只认 registry 或 bare"),
            };
            let registry = f.one("registry").map(|x| h20(&x));
            let hashes: Vec<[u8; 32]> = f.many("hash").iter().map(|x| h32(x)).collect();
            let calldata = f.one("calldata").map(|x| zikaron::hexfmt::decode(&x).unwrap_or_else(|| misuse("calldata 不是十六进制")));
            if hashes.is_empty() && calldata.is_none() {
                misuse("至少要一个 --hash,或一段 --calldata");
            }
            // The caller sets the wait: six seconds is shorter than one block on any real chain, and a fixed
            // number would set one block time for every chain. Default 90 seconds, `--wait-secs` to change.
            let wait = std::time::Duration::from_secs(
                f.one("wait-secs").and_then(|x| x.parse().ok()).unwrap_or(90),
            );
            match send::anchor(&mut ep, &key, chain, form, registry, &hashes, calldata, wait) {
                // Only "included with status 1" is anchored (§9.1); the other three states are named, and the
                // transaction hash is always printed because the bytes were broadcast.
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
                        send::Confirm::NotYet => {
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
        // Fragment plus three more fields make the audit input.
        "audit-input" => {
            let (_, read) = read_wire(&f.need("fragment"));
            // Accepts a multi-endpoint reading (the output of `scan-live`) or a bare fragment; the reading
            // wraps the fragment in `fragment`.
            let frag = read.member("fragment").cloned().unwrap_or(read);
            let frag = wire::to_core(&frag).unwrap_or_else(|| misuse("片段不在法 §3 的值域里"));
            let root = f.need("root");
            let pile = strings_of(f.one("pile"));
            let unavailable = strings_of(f.one("unavailable"));
            match input::assemble(&frag, &root, &pile, &unavailable) {
                Some(v) => say(&v),
                None => refused("E_FRAGMENT", "片段缺 anchors / basis / evidence 之一"),
            }
        }
        // Hand to the core for the report.
        //
        // The file's raw bytes go in unchanged: this layer does not parse or rewrite them, so the core reads
        // exactly what it should refuse. An unreadable path is misuse, as on the core's side.
        "audit" => {
            let path = f.words.first().cloned().unwrap_or_else(|| misuse("usage: zka audit <input.json>"));
            let bytes = std::fs::read(&path).unwrap_or_else(|e| misuse(&format!("读不出 {path}:{e}")));
            say_bytes(&input::audit(&bytes))
        }
        // §9.7 proof kit capture.
        "kit-capture" => {
            let specs = endpoint_specs(&f);
            let (chain, url) = specs[0].clone();
            let mut ep = rpc::Http::new(&url).unwrap_or_else(|| misuse("端点只认 http://host:port"));
            let txh = h32(&f.need("tx"));
            let hash = h32(&f.need("hash"));
            let emitter = f.one("emitter").map(|x| h20(&x));
            match kit::capture(&mut ep, chain, &txh, &hash, emitter) {
                Ok(v) => {
                    if let Some(out) = f.one("out") {
                        std::fs::write(&out, format!("{}\n", String::from_utf8_lossy(&canon_bytes(&v))))
                            .unwrap_or_else(|e| misuse(&format!("写不下 {out}:{e}")));
                    }
                    say(&v)
                }
                Err(e) => refused("E_KIT_CAPTURE", &format!("{e:?}")),
            }
        }
        // §9.7 offline kit verification (no node asked).
        "kit-verify" => {
            let path = f.words.first().cloned().unwrap_or_else(|| misuse("usage: zka kit-verify <kit.json>"));
            let (_, k) = read_wire(&path);
            match kit::verify(&k) {
                Ok(p) => {
                    // Emitter, form and block hash go out with the verdict: the recipient still has to run
                    // the §9.1 check of the emitter against their own registries and trust the block hash
                    // independently.
                    let mut fields = vec![
                        ("blockHash".to_string(), Value::Str(zikaron::hexfmt::encode(&p.block_hash))),
                        ("blockNumber".to_string(), Value::Int(p.block_number)),
                        ("blockTimestamp".to_string(), Value::Int(p.block_timestamp)),
                        ("chainId".to_string(), Value::Int(p.chain_id)),
                        ("form".to_string(), Value::Str(if p.registry_form { "registry".into() } else { "bare".into() })),
                        ("hash".to_string(), Value::Str(zikaron::hexfmt::encode(&p.hash))),
                        ("ok".to_string(), Value::Bool(true)),
                        ("sender".to_string(), Value::Str(zikaron::hexfmt::encode(&p.sender))),
                        ("tx".to_string(), Value::Str(zikaron::hexfmt::encode(&p.tx))),
                        ("verdict".to_string(), Value::Str(p.verdict.as_str().into())),
                    ];
                    if let Some(e) = p.emitter {
                        fields.push(("emitter".to_string(), Value::Str(zikaron::hexfmt::encode(&e))));
                    }
                    fields.sort_by(|a, b| a.0.cmp(&b.0));
                    say(&Value::Obj(fields))
                }
                Err(r) => refused(r.code(), &r.detail()),
            }
        }
        _ => misuse("usage: zka <scan|agree|scan-live|record|anchor|audit-input|audit|kit-capture|kit-verify> ..."),
    }
}
