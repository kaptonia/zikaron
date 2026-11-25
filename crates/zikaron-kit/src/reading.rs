use crate::tokens::{self as t, Key};
use zikaron::tokens::EntryType;
use zikaron::audit::Outcome;
use zikaron::entry::Entry;
use zikaron::json::Value;
use zikaron::trace;

pub struct Lines {
    ids: Vec<String>,
    at: std::collections::HashMap<String, usize>,
}

impl Lines {
    pub fn of(ledger: &[Entry]) -> Lines {
        let ids: Vec<String> = ledger.iter().map(|e| e.id_hex()).collect();
        let mut at = std::collections::HashMap::with_capacity(ids.len());
        for (i, id) in ids.iter().enumerate() {
            at.insert(id.clone(), i);
        }
        Lines { ids, at }
    }
    pub fn id(&self, i: usize) -> &str {
        &self.ids[i]
    }
    pub fn position(&self, id: &str) -> Option<usize> {
        self.at.get(id).copied()
    }
}

pub fn reachable_at(ledger: &[Entry], lines: &Lines, from: usize, target: usize) -> bool {
    let mut cur = from;
    let mut walked = 0usize;
    loop {
        if cur == target {
            return true;
        }
        walked += 1;
        if walked > ledger.len() {
            return false;
        }
        let prev = match &ledger[cur].prev {
            Some(p) => p.as_str(),
            None => return false,
        };
        match lines.position(prev) {
            Some(next) => cur = next,
            None => return false,
        }
    }
}

pub fn reachable(ledger: &[Entry], from: &Entry, target_id: &str) -> bool {
    if from.id_hex() == target_id {
        return true;
    }
    let lines = Lines::of(ledger);
    let tgt = match lines.position(target_id) {
        Some(t) => t,
        None => return false,
    };
    let start = match &from.prev {
        Some(p) => match lines.position(p) {
            Some(i) => i,
            None => return false,
        },
        None => return false,
    };
    reachable_at(ledger, &lines, start, tgt)
}

pub fn bounds(o: &Outcome, lines: &Lines) -> Vec<Option<u64>> {
    let n = o.ledger.len();
    let mut best: Vec<Option<u64>> = vec![None; n];
    for a in &o.counted {
        let mut cur = match lines.position(&a.hash) {
            Some(p) => p,
            None => continue,
        };
        let mut steps = 0usize;
        loop {
            match best[cur] {
                Some(x) if x <= a.block_timestamp => break,
                _ => best[cur] = Some(a.block_timestamp),
            }
            steps += 1;
            if steps > n {
                break;
            }
            let prev = match &o.ledger[cur].prev {
                Some(p) => p.as_str(),
                None => break,
            };
            match lines.position(prev) {
                Some(next) => cur = next,
                None => break,
            }
        }
    }
    best
}

fn reaches(o: &Outcome, lines: &Lines, target: usize) -> Vec<bool> {
    let n = o.ledger.len();
    let mut memo: Vec<Option<bool>> = vec![None; n];
    for i in 0..n {
        if memo[i].is_some() {
            continue;
        }
        let mut path: Vec<usize> = Vec::new();
        let mut cur = i;
        let ans;
        loop {
            if cur == target {
                ans = true;
                break;
            }
            if let Some(m) = memo[cur] {
                ans = m;
                break;
            }
            if path.len() > n {
                ans = false;
                break;
            }
            path.push(cur);
            let prev = match &o.ledger[cur].prev {
                Some(p) => p.as_str(),
                None => {
                    ans = false;
                    break;
                }
            };
            match lines.position(prev) {
                Some(next) => cur = next,
                None => {
                    ans = false;
                    break;
                }
            }
        }
        if memo[cur].is_none() {
            memo[cur] = Some(ans);
        }
        for p in path {
            memo[p] = Some(ans);
        }
    }
    memo.into_iter().map(|x| x.unwrap_or(false)).collect()
}

pub fn bound_of(o: &Outcome, e: &Entry) -> Option<u64> {
    let lines = Lines::of(&o.ledger);
    let pos = lines.position(&e.id_hex())?;
    bounds(o, &lines)[pos]
}

pub fn is_anchored(o: &Outcome, e: &Entry) -> bool {
    bound_of(o, e).is_some()
}

pub fn unproven_reaches(o: &Outcome, e: &Entry) -> bool {
    let lines = Lines::of(&o.ledger);
    let target = match lines.position(&e.id_hex()) {
        Some(p) => p,
        None => return false,
    };
    o.unproven
        .iter()
        .filter_map(|a| lines.position(&a.hash))
        .any(|from| reachable_at(&o.ledger, &lines, from, target))
}

fn obj(members: Vec<(Key, Value)>) -> Value {
    Value::Obj(members.into_iter().map(|(k, v)| (k.as_str().to_string(), v)).collect())
}

fn pick<'a>(h: &[&'a Entry], largest: bool) -> &'a Entry {
    let mut best = h[0];
    for e in h.iter().skip(1) {
        let better = if largest {
            e.seq > best.seq || (e.seq == best.seq && e.id < best.id)
        } else {
            e.seq < best.seq || (e.seq == best.seq && e.id < best.id)
        };
        if better {
            best = e;
        }
    }
    best
}

pub fn depth(outcome: Option<&Outcome>, work: &str) -> Value {
    trace::mark(t::K2);
    let o = match outcome {
        Some(o) => o,
        None => return obj(vec![(Key::Valid, Value::Bool(false))]),
    };
    let h: Vec<&Entry> = o
        .ledger
        .iter()
        .filter(|e| {
            e.kind == EntryType::History
                && e.body.member("content").and_then(|c| c.as_str()) == Some(work)
        })
        .collect();

    if h.is_empty() {
        return obj(vec![
            (Key::Valid, Value::Bool(true)),
            (Key::Label, Value::Str(o.label.as_str().to_string())),
            (Key::Found, Value::Bool(false)),
            (Key::Earliest, Value::Null),
            (Key::Deepest, Value::Int(0)),
            (
                Key::Continuity,
                obj(vec![(Key::Anchored, Value::Int(0)), (Key::Span, Value::Int(0))]),
            ),
        ]);
    }

    let lines = Lines::of(&o.ledger);
    let bound = bounds(o, &lines);
    let mut earliest: Option<u64> = None;
    let mut deepest: u64 = 0;
    for e in &h {
        let pos = match lines.position(&e.id_hex()) {
            Some(p) => p,
            None => continue,
        };
        if let Some(b) = bound[pos] {
            deepest += 1;
            earliest = Some(match earliest {
                Some(x) if x <= b => x,
                _ => b,
            });
        }
    }

    let h0 = pick(&h, false);
    let hmax = pick(&h, true);
    let span = hmax.seq - h0.seq + 1;
    let counted: std::collections::HashSet<&str> =
        o.counted.iter().map(|a| a.hash.as_str()).collect();
    let h0_pos = lines.position(&h0.id_hex());
    let mut seqs: std::collections::HashSet<u64> = std::collections::HashSet::new();
    if let Some(h0_pos) = h0_pos {
        let to_h0 = reaches(o, &lines, h0_pos);
        for (i, e) in o.ledger.iter().enumerate() {
            if e.seq < h0.seq || e.seq > hmax.seq || seqs.contains(&e.seq) {
                continue;
            }
            if counted.contains(lines.id(i)) && to_h0[i] {
                seqs.insert(e.seq);
            }
        }
    }

    obj(vec![
        (Key::Valid, Value::Bool(true)),
        (Key::Label, Value::Str(o.label.as_str().to_string())),
        (Key::Found, Value::Bool(true)),
        (
            Key::Earliest,
            match earliest {
                Some(x) => Value::Int(x),
                None => Value::Null,
            },
        ),
        (Key::Deepest, Value::Int(deepest)),
        (
            Key::Continuity,
            obj(vec![
                (Key::Anchored, Value::Int(seqs.len() as u64)),
                (Key::Span, Value::Int(span)),
            ]),
        ),
    ])
}
