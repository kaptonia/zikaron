use crate::input::{self, Accession, Anchor, Input, NoTable, Verdict};
use crate::shapes::{self, Facet, Shape};
use crate::{consent, marker};
use std::collections::{BTreeMap, BTreeSet};
use zikaron::json::Value;
use zikaron::v2::entry::{self, Entry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Misread {
    Shape,
    Root,
    Author,
    Signature,
    Subject,
    Unseated,
}

impl Misread {
    pub fn as_str(self) -> &'static str {
        match self {
            Misread::Shape => "SHAPE",
            Misread::Root => "ROOT",
            Misread::Author => "AUTHOR",
            Misread::Signature => "SIGNATURE",
            Misread::Subject => "SUBJECT",
            Misread::Unseated => "UNSEATED",
        }
    }
}

pub const NO_TABLE: &str = "NO_TABLE";

type Order = (u64, u64, u64, String);

fn order(a: &Anchor) -> Order {
    (a.time, a.chain, a.block, a.tx.clone())
}

struct FaceEntry {
    entry: Entry,
    shape: Option<Shape>,
}

impl FaceEntry {
    fn text(&self, k: &str) -> &str {
        shapes::text(&self.entry.body, k)
    }
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn obj(ms: Vec<(&str, Value)>) -> Value {
    Value::Obj(ms.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn moment_ref(a: &Anchor) -> Value {
    obj(vec![
        ("chainId", Value::Int(a.chain)),
        ("blockNumber", Value::Int(a.block)),
        ("blockTimestamp", Value::Int(a.time)),
        ("tx", s(&a.tx)),
    ])
}

fn moment_of<'a>(seated: &BTreeMap<String, Vec<&'a Anchor>>, id: &str) -> &'a Anchor {
    seated[id].iter().copied().min_by_key(|a| order(a)).expect("a seated entry has a record")
}

struct SpanOf<'a> {
    since: &'a Anchor,
    entry: String,
    release: Option<(String, &'a Anchor)>,
}

pub fn table_of(raw: &[u8]) -> Result<Value, NoTable> {
    input::load(raw).map(|i| table(&i))
}

pub fn table(inp: &Input) -> Value {
    let r = inp.root.as_str();

    let mut entries: BTreeSet<String> = BTreeSet::new();
    let mut faces: BTreeMap<String, FaceEntry> = BTreeMap::new();
    for b in &inp.pile {
        let Ok(e) = entry::check(b) else { continue };
        entries.insert(e.id_hex.clone());
        if e.entry_type == crate::ENTRY_TYPE {
            let shape = shapes::shape_of(&e.body);
            faces.insert(e.id_hex.clone(), FaceEntry { entry: e, shape });
        }
    }

    let reads = |f: &FaceEntry| -> bool {
        match f.shape {
            None => false,
            Some(_) if f.text("root") != r => false,
            Some(Shape::Key) => consent::holds(&f.entry.body),
            Some(_) => true,
        }
    };

    let mut steps: BTreeMap<(u64, u64, u64), (Vec<&Anchor>, Vec<&Accession>)> = BTreeMap::new();
    for a in &inp.anchors {
        steps.entry((a.time, a.chain, a.block)).or_default().0.push(a);
    }
    for x in &inp.accessions {
        steps.entry((x.time, x.chain, x.block)).or_default().1.push(x);
    }

    let mut office = r.to_string();
    let mut home: Option<u64> = None;
    let mut head: BTreeMap<u64, String> = BTreeMap::new();
    let mut line: Vec<(String, Option<&Accession>)> = vec![(r.to_string(), None)];
    let mut keys: BTreeSet<String> = BTreeSet::from([r.to_string()]);
    let mut seated: BTreeMap<String, Vec<&Anchor>> = BTreeMap::new();
    let mut openings: Vec<(&Anchor, String)> = Vec::new();
    let mut unread: BTreeSet<&str> = BTreeSet::new();
    let mut unproven: u64 = 0;
    let mut sealed = false;
    for (&(_, chain, _), (anchors, accessions)) in &mut steps {
        let mut opens = false;
        let mut closes = false;
        for a in anchors.iter().filter(|_| !sealed) {
            if a.verdict == Verdict::Unproven && a.sender == office {
                unproven += 1;
            }
            if a.verdict != Verdict::Counted || a.sender != office {
                continue;
            }
            if !entries.contains(&a.hash) {
                unread.insert(&a.hash);
                continue;
            }
            let Some(f) = faces.get(&a.hash) else { continue };
            let Some(shape) = f.shape else { continue };
            if f.entry.author != office {
                continue;
            }
            if shape == Shape::Home {
                let chain_named = f.entry.body.member("chain").and_then(Value::as_int);
                let head_here = head.get(&chain).map(String::as_str).unwrap_or(r);
                if chain_named != Some(chain) || head_here != office {
                    continue;
                }
                seated.entry(a.hash.clone()).or_default().push(a);
                if f.text("root") == r {
                    openings.push((a, a.hash.clone()));
                    opens = true;
                }
            } else if home == Some(chain) {
                seated.entry(a.hash.clone()).or_default().push(a);
                if shape == Shape::Reroot && f.text("root") == r {
                    closes = true;
                }
            }
        }
        if opens {
            home = Some(chain);
        }
        accessions.sort_by_key(|x| (x.tx_index, x.log_index));
        for x in accessions.iter() {
            if x.root != r {
                continue;
            }
            if x.holder == office && home.is_none_or(|h| h == chain) {
                office = x.to.clone();
                line.push((x.to.clone(), Some(x)));
                keys.insert(x.to.clone());
            }
            head.insert(chain, x.to.clone());
        }
        if closes {
            sealed = true;
        }
    }

    let moment = |id: &str| moment_of(&seated, id);

    let stands_as = |f: &FaceEntry, id: &str| reads(f) && seated.contains_key(id);
    let bindings: BTreeSet<String> = faces
        .iter()
        .filter(|(id, f)| matches!(f.shape, Some(Shape::Account | Shape::Key | Shape::Persona)) && stands_as(f, id))
        .map(|(id, _)| id.clone())
        .collect();
    let homes_standing: BTreeSet<String> = faces
        .iter()
        .filter(|(id, f)| f.shape == Some(Shape::Home) && stands_as(f, id))
        .map(|(id, _)| id.clone())
        .collect();
    let moves_standing: BTreeSet<String> = faces
        .iter()
        .filter(|(id, f)| matches!(f.shape, Some(Shape::Reroot | Shape::Origin)) && stands_as(f, id))
        .map(|(id, _)| id.clone())
        .collect();
    let releases: BTreeSet<String> = faces
        .iter()
        .filter(|(id, f)| f.shape == Some(Shape::Release) && stands_as(f, id) && bindings.contains(f.text("subject")))
        .map(|(id, _)| id.clone())
        .collect();
    let stands = |id: &str| bindings.contains(id) || homes_standing.contains(id) || releases.contains(id) || moves_standing.contains(id);

    let first = |shape: Shape, member: &str| -> Value {
        let chosen = moves_standing.iter().filter(|id| faces[id.as_str()].shape == Some(shape)).min_by(|a, b| (order(moment(a)), a.as_str()).cmp(&(order(moment(b)), b.as_str())));
        match chosen {
            None => Value::Null,
            Some(id) => obj(vec![("entry", s(id)), (member, s(faces[id.as_str()].text(member))), ("since", moment_ref(moment(id)))]),
        }
    };
    let reroot = first(Shape::Reroot, "to");
    let origin = first(Shape::Origin, "from");

    let mut ending: BTreeMap<String, String> = BTreeMap::new();
    for q in &releases {
        let b = faces[q].text("subject").to_string();
        let better = match ending.get(&b) {
            None => true,
            Some(was) => (order(moment(q)), q.as_str()) < (order(moment(was)), was.as_str()),
        };
        if better {
            ending.insert(b, q.clone());
        }
    }

    let mut rows: BTreeMap<Facet, Vec<SpanOf>> = BTreeMap::new();
    for b in &bindings {
        let f = &faces[b];
        let since = moment(b);
        let release = ending.get(b).map(|q| {
            let until = moment(q);
            let until = if order(until) < order(since) { since } else { until };
            (q.clone(), until)
        });
        let facet = f.shape.and_then(|sh| shapes::facet_of(sh, &f.entry.body)).expect("a binding names a facet");
        rows.entry(facet).or_default().push(SpanOf { since, entry: b.clone(), release });
    }
    for spans in rows.values_mut() {
        spans.sort_by(|x, y| (order(x.since), &x.entry).cmp(&(order(y.since), &y.entry)));
    }

    let mut ruling: Option<(Order, &str, &str)> = None;
    for (facet, spans) in &rows {
        let Facet::Persona { name } = facet else { continue };
        for sp in spans.iter().filter(|sp| sp.release.is_none()) {
            let k = order(sp.since);
            if ruling.as_ref().is_none_or(|(o, e, _)| (&k, sp.entry.as_str()) > (o, *e)) {
                ruling = Some((k, sp.entry.as_str(), name.as_str()));
            }
        }
    }
    let persona = match ruling {
        None => Value::Null,
        Some((_, e, name)) => obj(vec![("entry", s(e)), ("name_md", s(name))]),
    };

    let faces_out: Vec<Value> = rows
        .iter()
        .map(|(facet, spans)| {
            let state = if spans.iter().any(|sp| sp.release.is_none()) { "bound" } else { "released" };
            let spans_out = Value::Arr(
                spans
                    .iter()
                    .map(|sp| {
                        let (release, until) = match &sp.release {
                            None => (Value::Null, Value::Null),
                            Some((q, until)) => (s(q), moment_ref(until)),
                        };
                        obj(vec![
                            ("entry", s(&sp.entry)),
                            ("since", moment_ref(sp.since)),
                            ("release", release),
                            ("until", until),
                        ])
                    })
                    .collect(),
            );
            let mut ms = match facet {
                Facet::Account { platform, handle } => {
                    vec![("kind", s("account")), ("platform", s(platform)), ("handle", s(handle))]
                }
                Facet::Key { address, role } => vec![("kind", s("key")), ("address", s(address)), ("role", s(role))],
                Facet::Persona { name } => vec![("kind", s("persona")), ("name_md", s(name))],
            };
            ms.push(("state", s(state)));
            ms.push(("spans", spans_out));
            obj(ms)
        })
        .collect();

    let reason = |f: &FaceEntry| -> Misread {
        match f.shape {
            None => Misread::Shape,
            Some(_) if f.text("root") != r => Misread::Root,
            Some(_) if !keys.contains(&f.entry.author) => Misread::Author,
            Some(Shape::Key) if !consent::holds(&f.entry.body) => Misread::Signature,
            Some(Shape::Release) if !bindings.contains(f.text("subject")) => Misread::Subject,
            Some(_) => Misread::Unseated,
        }
    };
    let listed: BTreeSet<&str> = inp.anchors.iter().filter(|a| keys.contains(&a.sender)).map(|a| a.hash.as_str()).collect();
    let misread: Vec<Value> = faces
        .iter()
        .filter(|(id, _)| !stands(id) && listed.contains(id.as_str()))
        .map(|(id, f)| obj(vec![("entry", s(id)), ("reason", s(reason(f).as_str()))]))
        .collect();

    let lineage: Vec<Value> = line
        .iter()
        .map(|(key, x)| {
            let accession = match x {
                None => Value::Null,
                Some(x) => obj(vec![
                    ("chainId", Value::Int(x.chain)),
                    ("blockNumber", Value::Int(x.block)),
                    ("blockTimestamp", Value::Int(x.time)),
                    ("txIndex", Value::Int(x.tx_index)),
                    ("logIndex", Value::Int(x.log_index)),
                    ("tx", s(&x.tx)),
                    ("holder", s(&x.holder)),
                ]),
            };
            obj(vec![("accession", accession), ("key", s(key))])
        })
        .collect();
    openings.sort_by(|(a, i), (b, j)| (order(a), i).cmp(&(order(b), j)));
    let homes: Vec<Value> = openings
        .iter()
        .map(|(a, i)| {
            obj(vec![
                ("entry", s(i)),
                ("chainId", Value::Int(a.chain)),
                ("blockNumber", Value::Int(a.block)),
                ("blockTimestamp", Value::Int(a.time)),
                ("tx", s(&a.tx)),
                ("sender", s(&a.sender)),
            ])
        })
        .collect();
    obj(vec![
        ("root", s(r)),
        ("marker", s(&marker(r))),
        ("basis", inp.chains.clone()),
        ("home", home.map(Value::Int).unwrap_or(Value::Null)),
        ("office", s(&office)),
        ("persona", persona),
        ("reroot", reroot),
        ("origin", origin),
        ("lineage", Value::Arr(lineage)),
        ("homes", Value::Arr(homes)),
        ("faces", Value::Arr(faces_out)),
        ("misread", Value::Arr(misread)),
        ("unread", Value::Int(unread.len() as u64)),
        ("unproven", Value::Int(unproven)),
    ])
}

pub fn joined(first: &Value, second: &Value) -> bool {
    let root = |t: &Value| t.member("root").and_then(Value::as_str).map(str::to_string);
    let named = |t: &Value, m: &str, k: &str| t.member(m).and_then(|x| x.member(k)).and_then(Value::as_str).map(str::to_string);
    let (Some(r1), Some(r2)) = (root(first), root(second)) else { return false };
    named(first, "reroot", "to").as_deref() == Some(r2.as_str()) && named(second, "origin", "from").as_deref() == Some(r1.as_str())
}

pub fn joined_of(raw: &[u8]) -> Result<bool, NoTable> {
    let (a, b) = input::load_pair(raw)?;
    Ok(joined(&table(&a), &table(&b)))
}
