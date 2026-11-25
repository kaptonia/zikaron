use crate::fault::{Fault, Known};
use zikaron::json::Value;

pub struct Book {
    pub who: String,
    pub anchors: usize,
    pub asked: usize,
    pub entries: usize,
    pub label: String,
    pub timeline: Vec<crate::ledgerx::Row>,
    pub grants: Vec<crate::grantx::Row>,
    pub latest: Option<u64>,
}

impl Book {
    pub fn only_anchors(&self) -> bool {
        self.entries == 0
    }
}

pub fn who(typed: &str) -> Result<crate::key::Address, Fault> {
    crate::key::Address::parse(typed)
        .ok_or_else(|| Fault::known(Known::AddressShape, typed.trim().to_string()))
}

pub fn bytes_from(dir: &std::path::Path) -> Result<Vec<Vec<u8>>, Fault> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(listing) = std::fs::read_dir(&at) else { continue };
        let mut names: Vec<std::path::PathBuf> =
            listing.filter_map(|e| e.ok().map(|e| e.path())).collect();
        names.sort();
        for p in names {
            let Ok(md) = std::fs::symlink_metadata(&p) else { continue };
            if md.is_dir() {
                stack.push(p);
                continue;
            }
            if !md.is_file() {
                continue;
            }
            let name = p.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            if zikaron_store::layout::parse_entry_file(&name).is_none() {
                continue;
            }
            if let Ok(b) = std::fs::read(&p) {
                if zikaron_glue::book::check(&b).is_ok() {
                    out.push(b);
                }
            }
        }
    }
    if out.is_empty() {
        return Err(Fault::known(Known::NoBytes, dir.display().to_string()));
    }
    Ok(out)
}

pub fn basis_for(
    ground: &crate::auditx::Ground,
    who: &crate::key::Address,
    bytes: &[Vec<u8>],
) -> crate::auditx::Ground {
    let mut g = ground.clone();
    let line = crate::auditx::senders_of(bytes);
    g.senders = if line.is_empty() { vec![who.hex()] } else { line };
    g
}

pub fn basis_with_book(ground: &crate::auditx::Ground, eps: &[crate::chainx::Endpoint], who: &crate::key::Address, bytes: &[Vec<u8>]) -> crate::auditx::Ground {
    let mut g = basis_for(ground, who, bytes);
    g.senders = senders_with_book(&who.hex(), g.senders, &book_offer(Some((eps, ground)), &[]));
    g
}

pub fn book_offer(main: Option<(&[crate::chainx::Endpoint], &crate::auditx::Ground)>, nets: &[crate::readnets::Net]) -> Vec<crate::chainx::RecipientNet> {
    let mut out: Vec<crate::chainx::RecipientNet> = Vec::new();
    if let Some((eps, g)) = main {
        out.push(crate::chainx::RecipientNet { chain: g.chain, registry: g.registry.0, from_block: g.from_block, nodes: eps.iter().filter(|e| e.chain == g.chain).map(|e| e.url.clone()).collect() });
    }
    out.extend(nets.iter().map(|n| crate::chainx::RecipientNet { chain: n.chain_id, registry: n.registry.0, from_block: n.from_block, nodes: n.nodes.iter().map(|u| crate::chainx::NodeAddr::new(u.as_str())).collect() }));
    out
}

pub fn book_key(root: &str, offer: &[crate::chainx::RecipientNet]) -> Option<String> {
    let r = crate::key::Address::parse(root)?;
    crate::chainx::book_current(&r.0, offer).map(|k| crate::key::Address(k).hex())
}

pub fn senders_with_book(root: &str, bytes_senders: Vec<String>, offer: &[crate::chainx::RecipientNet]) -> Vec<String> {
    with_key(bytes_senders, book_key(root, offer).as_deref())
}

pub fn with_key(senders: Vec<String>, key: Option<&str>) -> Vec<String> {
    let mut out = senders;
    if let Some(k) = key {
        if !out.iter().any(|x| x.eq_ignore_ascii_case(k)) {
            out.push(k.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

pub fn behind_book(copy: &[Vec<u8>], key: Option<&str>) -> bool {
    zikaron_glue::book::behind_book(copy, key)
}

pub fn copy_under_book(found: &mut crate::supplyx::Found, who: &crate::key::Address, offer: &[crate::chainx::RecipientNet]) {
    let behind = found.supply.as_ref().is_some_and(|s| behind_book(&s.items, book_key(&who.hex(), offer).as_deref()));
    if behind {
        if let Some(s) = found.supply.take() {
            found.misses.push(behind_miss(s.level, s.place));
        }
    }
}

fn behind_miss(level: crate::supplyx::Level, place: String) -> (crate::supplyx::Level, Fault) {
    (level, Fault::known(Known::NotThisLedger, place).worded(Some(crate::lang::Key::FaultWhatCopyBehindBook), Some(crate::lang::Key::FaultNextCopyBehindBook)))
}

#[allow(clippy::type_complexity)]
pub fn audit_read(
    eps: &[crate::chainx::Endpoint],
    ground: crate::auditx::Ground,
    who: &crate::key::Address,
    bytes: &[Vec<u8>],
    nets: &[crate::readnets::Net],
) -> Result<(crate::auditx::Ground, crate::auditx::Scanned1, Vec<crate::widex::Missed>, Book), Fault> {
    let (g, scanned, missed) = if nets.is_empty() {
        let (head, _) = crate::chainx::head_block(eps, ground.chain)?;
        let mut at_head = ground;
        at_head.to_block = head.max(at_head.from_block);
        let g = basis_with_book(&at_head, eps, who, bytes);
        let scanned = crate::auditx::scan_once(eps, &g)?;
        (g, scanned, Vec::new())
    } else {
        let (scanned, missed) = scan_wide(eps, &ground, who, bytes, nets)?;
        (ground, scanned, missed)
    };
    let mut book = read_scanned_with(&scanned, who, bytes, &withdrawals(&crate::readnets::all_endpoints(eps, nets), bytes))?;
    unread_where_missed(&mut book, &missed);
    Ok((g, scanned, missed, book))
}

pub fn unread_where_missed(book: &mut Book, missed: &[crate::widex::Missed]) {
    if missed.is_empty() {
        return;
    }
    for row in book.timeline.iter_mut().filter(|r| r.lamp == crate::ledgerx::Lamp::Landed) {
        row.lamp = crate::ledgerx::Lamp::ChainUnread;
    }
}

pub fn scan_wide(
    eps: &[crate::chainx::Endpoint],
    ground: &crate::auditx::Ground,
    who: &crate::key::Address,
    bytes: &[Vec<u8>],
    nets: &[crate::readnets::Net],
) -> Result<(crate::auditx::Scanned1, Vec<crate::widex::Missed>), Fault> {
    let mut g = basis_for(ground, who, bytes);
    g.senders = senders_with_book(&who.hex(), g.senders, &book_offer(Some((eps, ground)), nets));
    let w = crate::widex::scan(Some((eps, &g)), nets, &g.senders, crate::widex::Ask::First)?;
    Ok((crate::auditx::Scanned1 { anchors: w.anchors, asked: w.asked, fragment: w.fragment }, w.missed))
}

pub fn read_copy(who: &crate::key::Address, bytes: &[Vec<u8>]) -> Result<Book, Fault> {
    read_scanned(&crate::auditx::Scanned1 { anchors: 0, asked: 0, fragment: crate::auditx::empty_fragment() }, who, bytes)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Audited {
    pub rows: Vec<(String, crate::ledgerx::Lamp, Option<u64>)>,
    pub grants: Vec<(String, bool, Option<crate::grantx::Undecided>)>,
    pub latest: Option<u64>,
    pub missed: Vec<crate::widex::Missed>,
    pub diligence: Option<Seen>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seen {
    pub work: String,
    pub window: bool,
    pub clash: usize,
    pub undecided: usize,
    pub lineage: Vec<String>,
    pub successions: Vec<(u64, String, String)>,
}

fn lamp_word(l: crate::ledgerx::Lamp) -> &'static str {
    use crate::ledgerx::Lamp;
    match l {
        Lamp::Anchored => "anchored",
        Lamp::ChainUnread => "unread",
        _ => "notAnchored",
    }
}

fn lamp_of_word(w: &str) -> Option<crate::ledgerx::Lamp> {
    use crate::ledgerx::Lamp;
    match w {
        "anchored" => Some(Lamp::Anchored),
        "unread" => Some(Lamp::ChainUnread),
        "notAnchored" => Some(Lamp::Landed),
        _ => None,
    }
}

impl Audited {
    pub fn of_book(b: &Book, missed: &[crate::widex::Missed]) -> Audited {
        Audited {
            rows: b.timeline.iter().map(|r| (r.id.to_ascii_lowercase(), r.lamp, r.anchored_at)).collect(),
            grants: b.grants.iter().map(|g| (g.id.to_ascii_lowercase(), g.revoked, g.undecided.clone())).collect(),
            latest: b.latest,
            missed: missed.to_vec(),
            diligence: None,
        }
    }

    pub fn with_diligence(r: &crate::diligx::Read) -> Audited {
        let mut a = Audited {
            rows: r.timeline.iter().map(|x| (x.id.to_ascii_lowercase(), x.lamp, x.anchored_at)).collect(),
            grants: r.grants.iter().map(|g| (g.id.to_ascii_lowercase(), g.revoked, g.undecided.clone())).collect(),
            latest: r.latest,
            missed: r.missed.clone(),
            diligence: None,
        };
        a.diligence = Some(Seen {
            work: r.work.clone(),
            window: r.window.is_some(),
            clash: r.clash.len(),
            undecided: r.undecided.len(),
            lineage: r.lineage.clone(),
            successions: r.successions.iter().map(|x| (x.seq, x.kind.clone(), x.to.clone())).collect(),
        });
        a
    }

    pub fn row(&self, id: &str) -> Option<(crate::ledgerx::Lamp, Option<u64>)> {
        self.rows.iter().find(|(i, _, _)| i.eq_ignore_ascii_case(id)).map(|(_, l, t)| (*l, *t))
    }

    pub fn grant(&self, id: &str) -> Option<(bool, Option<crate::grantx::Undecided>)> {
        self.grants.iter().find(|(i, _, _)| i.eq_ignore_ascii_case(id)).map(|(_, r, u)| (*r, u.clone()))
    }

    pub fn to_value(&self) -> Value {
        let s = |x: &str| Value::Str(x.to_string());
        let opt = |n: Option<u64>| n.map(Value::Int).unwrap_or(Value::Null);
        let mut m = vec![
            ("grants".to_string(), Value::Arr(self.grants.iter().map(|(i, r, u)| Value::Arr(vec![s(i), Value::Bool(*r), u.as_ref().map(undecided_value).unwrap_or(Value::Null)])).collect())),
            ("latest".to_string(), opt(self.latest)),
            ("missed".to_string(), Value::Arr(self.missed.iter().map(missed_value).collect())),
            ("rows".to_string(), Value::Arr(self.rows.iter().map(|(i, l, t)| Value::Arr(vec![s(i), s(lamp_word(*l)), opt(*t)])).collect())),
        ];
        if let Some(d) = &self.diligence {
            m.insert(
                0,
                (
                    "diligence".to_string(),
                    Value::Obj(vec![
                        ("clash".to_string(), Value::Int(d.clash as u64)),
                        ("lineage".to_string(), Value::Arr(d.lineage.iter().map(|x| s(x)).collect())),
                        ("successions".to_string(), Value::Arr(d.successions.iter().map(|(q, k, t)| Value::Arr(vec![Value::Int(*q), s(k), s(t)])).collect())),
                        ("undecided".to_string(), Value::Int(d.undecided as u64)),
                        ("window".to_string(), Value::Bool(d.window)),
                        ("work".to_string(), s(&d.work)),
                    ]),
                ),
            );
        }
        Value::Obj(m)
    }

    pub fn of_value(v: &Value) -> Option<Audited> {
        let arr = |v: Option<&Value>| match v {
            Some(Value::Arr(a)) => Some(a.clone()),
            _ => None,
        };
        let str_of = |v: &Value| v.as_str().map(str::to_string);
        let int_of = |v: Option<&Value>| match v {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        };
        let rows = arr(v.member("rows"))?
            .iter()
            .map(|r| match r {
                Value::Arr(x) if x.len() == 3 => Some((str_of(&x[0])?, lamp_of_word(x[1].as_str()?)?, int_of(Some(&x[2])))),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let grants = arr(v.member("grants"))?
            .iter()
            .map(|g| match g {
                Value::Arr(x) if x.len() == 3 => Some((
                    str_of(&x[0])?,
                    matches!(x[1], Value::Bool(true)),
                    match &x[2] {
                        Value::Null => None,
                        u => Some(undecided_of(u)?),
                    },
                )),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let missed = arr(v.member("missed"))?.iter().map(missed_of).collect::<Option<Vec<_>>>()?;
        let diligence = match v.member("diligence") {
            Some(d) => Some(Seen {
                work: d.member("work").and_then(str_of)?,
                window: matches!(d.member("window"), Some(Value::Bool(true))),
                clash: int_of(d.member("clash"))? as usize,
                undecided: int_of(d.member("undecided"))? as usize,
                lineage: arr(d.member("lineage"))?.iter().filter_map(str_of).collect(),
                successions: arr(d.member("successions"))?
                    .iter()
                    .filter_map(|x| match x {
                        Value::Arr(y) if y.len() == 3 => Some((int_of(Some(&y[0]))?, str_of(&y[1])?, str_of(&y[2])?)),
                        _ => None,
                    })
                    .collect(),
            }),
            None => None,
        };
        Some(Audited { rows, grants, latest: int_of(v.member("latest")), missed, diligence })
    }
}

fn undecided_value(u: &crate::grantx::Undecided) -> Value {
    use crate::grantx::Undecided;
    let chain = u.chain().map(Value::Int).unwrap_or(Value::Null);
    let said = match u {
        Undecided::Unread(_, w) => Value::Str(w.clone()),
        _ => Value::Null,
    };
    Value::Obj(vec![("chain".to_string(), chain), ("code".to_string(), Value::Str(u.code().to_string())), ("said".to_string(), said)])
}

fn undecided_of(v: &Value) -> Option<crate::grantx::Undecided> {
    use crate::grantx::Undecided;
    let chain = match v.member("chain") {
        Some(Value::Int(n)) => Some(*n),
        Some(Value::Null) | None => None,
        _ => return None,
    };
    match v.member("code").and_then(|x| x.as_str())? {
        "unread" => Some(Undecided::Unread(chain?, v.member("said").and_then(|x| x.as_str())?.to_string())),
        "uncounted" => Some(Undecided::Uncounted(chain?)),
        "not-applying" => Some(Undecided::NotApplying(chain)),
        _ => None,
    }
}

fn missed_value(m: &crate::widex::Missed) -> Value {
    Value::Obj(vec![
        ("chainId".to_string(), Value::Int(m.chain_id)),
        ("fromBlock".to_string(), Value::Int(m.from_block)),
        ("name".to_string(), Value::Str(m.name.clone())),
        ("nodes".to_string(), Value::Int(m.nodes as u64)),
        ("reading".to_string(), Value::Str(m.reading.code().to_string())),
        ("registry".to_string(), Value::Str(m.registry.hex())),
    ])
}

fn missed_of(v: &Value) -> Option<crate::widex::Missed> {
    let int = |k: &str| match v.member(k) {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    };
    let reading = match v.member("reading").and_then(|x| x.as_str())? {
        "down" => crate::widex::Reading::Down,
        "fingerprint" => crate::widex::Reading::Fingerprint,
        _ => return None,
    };
    Some(crate::widex::Missed {
        chain_id: int("chainId")?,
        registry: crate::key::Address::parse(v.member("registry").and_then(|x| x.as_str())?)?,
        from_block: int("fromBlock")?,
        name: v.member("name").and_then(|x| x.as_str())?.to_string(),
        reading,
        nodes: int("nodes")? as usize,
    })
}

pub fn audit_key(what: &str, who: &str, bytes: &[Vec<u8>], ground: Option<&crate::auditx::Ground>, nets: &[crate::readnets::Net]) -> crate::sendx::behind::Key {
    let window = |chain: u64, registry: String, from: u64| Value::Obj(vec![("chainId".to_string(), Value::Int(chain)), ("fromBlock".to_string(), Value::Int(from)), ("registry".to_string(), Value::Str(registry))]);
    let mut chains: Vec<Value> = ground.map(|g| window(g.chain, g.registry.hex(), g.from_block)).into_iter().collect();
    chains.extend(nets.iter().map(|n| window(n.chain_id, n.registry.hex(), n.from_block)));
    let basis = zikaron::json::canon_bytes(&Value::Arr(chains));
    let pile = zikaron::json::canon_bytes(&Value::Arr(bytes.iter().map(|b| Value::Str(zikaron::hexfmt::encode(b))).collect()));
    crate::sendx::behind::Key::of(format!("{what}:{}", who.to_ascii_lowercase()).as_bytes(), &basis, &pile)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Withdrawal {
    Revoked,
    Standing,
    Undecided(crate::grantx::Undecided),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Withdrawals {
    per: Vec<([u8; 32], Withdrawal)>,
}

impl Withdrawals {
    pub fn of(&self, id: &[u8; 32], chain: Option<u64>) -> Withdrawal {
        self.per.iter().find(|(g, _)| g == id).map(|(_, w)| w.clone()).unwrap_or_else(|| match chain {
            Some(c) => Withdrawal::Undecided(crate::grantx::Undecided::Unread(c, format!("{} {c}", crate::lang::t(crate::lang::Key::NoticesUnread)))),
            None => Withdrawal::Undecided(crate::grantx::Undecided::NotApplying(None)),
        })
    }

    fn counted(&self, bytes: &[Vec<u8>]) -> Vec<crate::sendx::Notice> {
        bytes
            .iter()
            .filter_map(|b| {
                let id = zikaron::cryptox::sha256(b);
                let chain = zikaron_glue::notices::chain_of(b)?;
                self.per.iter().any(|(g, w)| *g == id && *w == Withdrawal::Revoked).then_some(crate::sendx::Notice { grant: id, chain, tx: [0u8; 32], block: 0 })
            })
            .collect()
    }
}

pub fn withdrawals(eps: &[crate::chainx::Endpoint], bytes: &[Vec<u8>]) -> Withdrawals {
    let grants: Vec<&[u8]> = bytes.iter().filter(|b| zikaron_glue::notices::carries(b)).map(Vec::as_slice).collect();
    if grants.is_empty() {
        return Withdrawals::default();
    }
    let notes = crate::noticex::for_grants(&grants, eps);
    let per = grants.iter().enumerate().map(|(i, g)| (zikaron::cryptox::sha256(g), withdrawal_of(g, notes.per.get(i).and_then(Option::as_ref), notes.said.get(i).and_then(Option::as_deref)))).collect();
    Withdrawals { per }
}

fn withdrawal_of(g: &[u8], n: Option<&Value>, said: Option<&str>) -> Withdrawal {
    use crate::grantx::Undecided;
    let chain = zikaron_glue::notices::chain_of(g);
    match (n, said, chain) {
        (Some(n), _, _) => match crate::sendx::judged(g, n) {
            crate::sendx::Back::Counted => Withdrawal::Revoked,
            crate::sendx::Back::NotSeen => Withdrawal::Standing,
            _ if zikaron_glue::notices::applies(g, n) => Withdrawal::Undecided(Undecided::Uncounted(chain.unwrap_or_default())),
            _ => Withdrawal::Undecided(Undecided::NotApplying(chain)),
        },
        (None, Some(words), Some(c)) => Withdrawal::Undecided(Undecided::Unread(c, words.to_string())),
        _ => Withdrawal::Undecided(Undecided::NotApplying(chain)),
    }
}

pub fn read_scanned(
    scanned: &crate::auditx::Scanned1,
    who: &crate::key::Address,
    bytes: &[Vec<u8>],
) -> Result<Book, Fault> {
    read_scanned_with(scanned, who, bytes, &Withdrawals::default())
}

fn not_whole(f: Fault) -> Fault {
    match f.which() {
        Some(Known::NoGenesis) => Fault::known(Known::BookNotWhole, f.tail().to_string()),
        _ => f,
    }
}

pub fn read_scanned_with(
    scanned: &crate::auditx::Scanned1,
    who: &crate::key::Address,
    bytes: &[Vec<u8>],
    withdrawals: &Withdrawals,
) -> Result<Book, Fault> {
    crate::trace::mark(crate::feature::Feature::D1);
    let (anchors, asked) = (scanned.anchors, scanned.asked);
    let latest = crate::vaultx::latest_anchor_of(&scanned.fragment);
    if bytes.is_empty() {
        return Ok(Book {
            who: who.hex(),
            anchors,
            asked,
            entries: 0,
            label: String::new(),
            timeline: Vec::new(),
            grants: Vec::new(),
            latest,
        });
    }
    let v = crate::auditx::ask_from(bytes, &scanned.fragment, Vec::new(), asked, true).map_err(not_whole)?;
    let line = crate::auditx::senders_of(bytes);
    if !line.iter().any(|a| a.eq_ignore_ascii_case(&who.hex())) {
        return Err(Fault::known(
            Known::NotHisBook,
            crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail202, &[&(who.hex()).to_string(), &(line.join(" ")).to_string()])),
        ));
    }
    let anchored = crate::ledgerx::anchored_of(&v.report);
    let of_ledger: Option<std::collections::BTreeSet<String>> = zikaron_glue::book::read(bytes, &scanned.fragment).ok().map(|l| l.entries.iter().map(|e| e.id_hex().to_ascii_lowercase()).collect());
    let in_ledger = |e: &zikaron_glue::book::Entry| of_ledger.as_ref().is_none_or(|s| s.contains(&e.id_hex().to_ascii_lowercase()));
    let mut timeline: Vec<crate::ledgerx::Row> = Vec::new();
    let mut grants: Vec<crate::grantx::Row> = Vec::new();
    let noticed = withdrawals.counted(bytes);
    let standing = crate::sendx::standing(bytes, &noticed);
    let mut revoked: Vec<String> = standing
        .iter()
        .filter_map(|b| zikaron_glue::book::check(b).ok())
        .filter(|e| e.kind == zikaron_glue::book::Kind::Revocation && in_ledger(e))
        .filter_map(|e| match &e.body {
            Value::Obj(m) => m.iter().find(|(k, _)| k == "grant").and_then(|(_, x)| match x {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            }),
            _ => None,
        })
        .collect();
    let mut undecided: Vec<(String, crate::grantx::Undecided)> = Vec::new();
    for b in bytes.iter().filter(|b| zikaron_glue::notices::carries(b)) {
        let id = zikaron::cryptox::sha256(b);
        let old = zikaron_glue::book::check(b).ok().and_then(|e| e.body.member("supersedes").and_then(Value::as_str).map(str::to_ascii_lowercase));
        match withdrawals.of(&id, zikaron_glue::notices::chain_of(b)) {
            Withdrawal::Revoked => {
                revoked.push(zikaron::hexfmt::encode(&id));
                revoked.extend(old);
            }
            Withdrawal::Standing => {}
            Withdrawal::Undecided(u) => {
                undecided.push((zikaron::hexfmt::encode(&id), u.clone()));
                undecided.extend(old.map(|o| (o, u)));
            }
        }
    }
    for b in bytes {
        let Ok(e) = zikaron_glue::book::check(b) else { continue };
        let id = zikaron::hexfmt::encode(&e.id);
        let tx = anchored.iter().find(|(h, _)| *h == id).map(|(_, t)| t.clone());
        timeline.push(crate::ledgerx::Row {
            law: e.law,
            seq: e.seq,
            kind: e.kind,
            summary: crate::ledgerx::summarize(e.kind, &e.body),
            work: crate::ledgerx::work_of(e.kind, &e.body),
            facts: crate::ledgerx::facts_of(&e),
            prev: e.prev.clone(),
            author: e.author.clone(),
            lamp: if tx.is_some() {
                crate::ledgerx::Lamp::Anchored
            } else {
                crate::ledgerx::Lamp::Landed
            },
            tx,
            bytes: b.len(),
            id: id.clone(),
            anchored_at: None,
        });
        if e.kind == zikaron_glue::book::Kind::Grant && in_ledger(&e) {
            let g = |k: &str| -> String {
                match &e.body {
                    Value::Obj(m) => m
                        .iter()
                        .find(|(n, _)| n == k)
                        .and_then(|(_, x)| match x {
                            Value::Str(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_default(),
                    _ => String::new(),
                }
            };
            grants.push(crate::grantx::Row {
                grantee: g("grantee"),
                work: g("work"),
                terms: g("terms"),
                window: crate::grantx::window_of(&e.body),
                exclusive: false,
                exclusive_from: crate::termsx::Exclusive::No,
                doc: None,
                doc_name: None,
                revoked: revoked.iter().any(|r| r.eq_ignore_ascii_case(&id)),
                unnoticed: false,
                member: e.body.member(zikaron_kit::v2::tokens::Key::Revocation.as_str()).is_some(),
                superseded: false,
                undecided: if revoked.iter().any(|r| r.eq_ignore_ascii_case(&id)) { None } else { undecided.iter().find(|(u, _)| u.eq_ignore_ascii_case(&id)).map(|(_, w)| w.clone()) },
                seq: e.seq,
                id,
            });
        }
    }
    timeline.sort_by(|a, b| (b.pos(), &a.id).cmp(&(a.pos(), &b.id)));
    crate::ledgerx::stamp(&mut timeline, bytes, &scanned.fragment);
    grants.sort_by(|a, b| (b.seq, &a.id).cmp(&(a.seq, &b.id)));
    Ok(Book {
        latest,
        who: who.hex(),
        anchors,
        asked,
        entries: bytes.len(),
        label: v.label,
        timeline,
        grants,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grantx::Undecided;

    #[test]
    fn a_copy_behind_the_book_says_so_in_its_own_words() {
        let (level, f) = behind_miss(crate::supplyx::Level::Kit, "/p".into());
        assert_eq!((level, f.which(), f.raw()), (crate::supplyx::Level::Kit, Some(Known::NotThisLedger), "NOT_THIS_LEDGER · /p".to_string()));
        assert_eq!(f.what_key(), Some(crate::lang::Key::FaultWhatCopyBehindBook));
        assert_eq!(f.then_key(), Some(crate::lang::Key::FaultNextCopyBehindBook));
        assert_ne!(f.human(), crate::lang::t(Known::NotThisLedger.what()));
    }
    use zikaron_glue::book::{seal, Law, Sealed};

    const KEY: [u8; 32] = [0x21; 32];
    const CHAIN: u64 = 11_155_111;

    fn hex20(b: &[u8; 20]) -> String {
        zikaron::hexfmt::encode(b)
    }

    fn author() -> String {
        hex20(&zikaron::cryptox::address_of_privkey(&KEY).expect("key"))
    }

    fn grant(contract: &str) -> Vec<u8> {
        let sign = |pre: &[u8], domain: &str| -> Result<String, ()> {
            let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
            let (r, s, v) = zikaron::cryptox::sign_digest(&KEY, &digest).ok_or(())?;
            let mut sig = r.to_vec();
            sig.extend_from_slice(&s);
            sig.push(v);
            Ok(zikaron::hexfmt::encode(&sig))
        };
        let body = Value::Obj(vec![
            ("grantee".into(), Value::Str(format!("0x{}", "44".repeat(20)))),
            ("revocation".into(), Value::Obj(vec![("at".into(), Value::Str(contract.into())), ("chain".into(), Value::Int(CHAIN))])),
            ("terms".into(), Value::Str(format!("0x{}", "71".repeat(32)))),
            ("work".into(), Value::Str(format!("0x{}", "72".repeat(32)))),
        ]);
        match seal(Law::Two, &author(), "grant", 1, Some(&format!("0x{}", "01".repeat(32))), body, sign) {
            Sealed::Done(e) => e.bytes,
            _ => panic!("sealed"),
        }
    }

    fn input(g: &[u8], contract: &str, rows: &[(String, &str)]) -> Value {
        let id = zikaron::hexfmt::encode(&zikaron::cryptox::sha256(g));
        let recs = rows
            .iter()
            .enumerate()
            .map(|(i, (sender, verdict))| {
                Value::Obj(vec![
                    ("blockNumber".into(), Value::Int(10 + i as u64)),
                    ("chainId".into(), Value::Int(CHAIN)),
                    ("grant".into(), Value::Str(id.clone())),
                    ("logIndex".into(), Value::Int(0)),
                    ("sender".into(), Value::Str(sender.clone())),
                    ("tx".into(), Value::Str(format!("0x{}", format!("{:02x}", i + 1).repeat(32)))),
                    ("txIndex".into(), Value::Int(0)),
                    ("verdict".into(), Value::Str((*verdict).into())),
                ])
            })
            .collect();
        let basis = Value::Obj(vec![
            ("book".into(), Value::Str(format!("0x{}", "b0".repeat(20)))),
            ("bookFromBlock".into(), Value::Int(0)),
            ("chainId".into(), Value::Int(CHAIN)),
            ("fromBlock".into(), Value::Int(0)),
            ("revocationContract".into(), Value::Str(contract.into())),
            ("toBlock".into(), Value::Int(100)),
        ]);
        Value::Obj(vec![("accessions".into(), Value::Arr(Vec::new())), ("basis".into(), basis), ("revocations".into(), Value::Arr(recs))])
    }

    #[test]
    fn another_grant_carrying_the_member_reads_revoked_standing_or_undecided_by_cause() {
        let at = format!("0x{}", "5a".repeat(20));
        let g = grant(&at);
        assert_eq!(withdrawal_of(&g, Some(&input(&g, &at, &[(author(), "counted")])), None), Withdrawal::Revoked);
        assert_eq!(withdrawal_of(&g, Some(&input(&g, &at, &[])), None), Withdrawal::Standing);
        assert_eq!(withdrawal_of(&g, Some(&input(&g, &at, &[(author(), "VOID")])), None), Withdrawal::Undecided(Undecided::Uncounted(CHAIN)));
        let other = format!("0x{}", "5b".repeat(20));
        assert_eq!(withdrawal_of(&g, Some(&input(&g, &other, &[(author(), "counted")])), None), Withdrawal::Undecided(Undecided::NotApplying(Some(CHAIN))));
        assert_eq!(withdrawal_of(&g, None, Some("Notices not read 11155111: x")), Withdrawal::Undecided(Undecided::Unread(CHAIN, "Notices not read 11155111: x".into())));
        let none = Withdrawals::default();
        assert!(matches!(none.of(&zikaron::cryptox::sha256(&g), Some(CHAIN)), Withdrawal::Undecided(Undecided::Unread(CHAIN, _))), "no input asked: undecided, never standing");
    }

    #[test]
    fn an_audits_networks_not_read_and_undecided_grants_are_read_back_with_why() {
        let net = |reading, nodes| crate::widex::Missed { chain_id: 10, registry: crate::key::Address([0x0a; 20]), from_block: 7, name: "OP".into(), reading, nodes };
        let grants = vec![
            ("0xa".to_string(), false, Some(Undecided::Unread(10, "Notices not read 10: x".into()))),
            ("0xb".to_string(), false, Some(Undecided::Uncounted(10))),
            ("0xc".to_string(), false, Some(Undecided::NotApplying(None))),
            ("0xd".to_string(), true, None),
        ];
        let a = Audited { grants, missed: vec![net(crate::widex::Reading::Down, 0), net(crate::widex::Reading::Fingerprint, 2)], ..Audited::default() };
        assert_eq!(Audited::of_value(&a.to_value()), Some(a.clone()));
        let mut bad = a.to_value();
        if let Value::Obj(m) = &mut bad {
            for (k, v) in m.iter_mut() {
                if k == "missed" {
                    *v = Value::Arr(vec![Value::Str("OP".into())]);
                }
            }
        }
        assert_eq!(Audited::of_value(&bad), None);
    }
}
