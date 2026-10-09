//! The six grant checks (kit law §10.2, §10.3), the ledger link (§10.4) and the chain check (§10.5). Audit
//! and acceptance go through the public API of `zikaron`; this crate judges no syntax of its own.

use crate::badge;
use crate::reading;
use crate::tokens::{self as t, ChainToken, Check, CheckVerdict, FailKind, Key, Reason, State};
use zikaron::tokens::{EntryType, FindingName, Label};
use zikaron::audit::{self, Outcome};
use zikaron::entry::{self, Entry};
use zikaron::hexfmt;
use zikaron::json::Value;
use zikaron::trace;

/// Builds every result object; keys come from [`Key`].
fn obj(members: Vec<(Key, Value)>) -> Value {
    Value::Obj(members.into_iter().map(|(k, v)| (k.as_str().to_string(), v)).collect())
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

/// What one run of the six checks yields: the §10.3 result object plus what the chain check needs.
pub struct Checked {
    pub value: Value,
    pub verdict: CheckVerdict,
    /// Whether check 1 passed (it decides an undetermined link in the chain check).
    pub check1_pass: bool,
    /// The grant entry when check 1 passed.
    pub grant: Option<Entry>,
}

/// Kit law §10.1: the audit runs on `I'`, the input with `g` added to the pile of `I`.
pub fn with_grant_in_pile(input: &Value, g: &[u8]) -> Value {
    match input {
        Value::Obj(ms) => {
            let mut out = ms.clone();
            for m in out.iter_mut() {
                if m.0 == "pile" {
                    if let Value::Arr(items) = &m.1 {
                        let mut items = items.clone();
                        items.push(s(&hexfmt::encode(g)));
                        m.1 = Value::Arr(items);
                    }
                }
            }
            Value::Obj(out)
        }
        other => other.clone(),
    }
}

fn window_of(g: &Entry) -> Option<(u64, u64)> {
    let w = g.body.member("window")?;
    let from = w.member("from")?.as_int()?;
    let to = w.member("to")?.as_int()?;
    Some((from, to))
}

/// Coverage in check 4 of kit law §10.2: `chains` is non-empty, and every chains object has non-empty
/// registries and senders containing every key of the ledger's whole-set lineage.
fn covering(o: &Outcome) -> bool {
    let chains = match o.basis.member("chains").and_then(|x| x.as_arr()) {
        Some(c) => c,
        None => return false,
    };
    if chains.is_empty() {
        return false;
    }
    for c in chains {
        let regs = match c.member("registries").and_then(|x| x.as_arr()) {
            Some(r) => r,
            None => return false,
        };
        if regs.is_empty() {
            return false;
        }
        let senders: Vec<String> = match c.member("senders").and_then(|x| x.as_arr()) {
            Some(x) => x.iter().filter_map(|v| v.as_str().map(|y| y.to_string())).collect(),
            None => return false,
        };
        if !o.lineage.iter().all(|k| senders.contains(k)) {
            return false;
        }
    }
    true
}

fn findings_on(o: &Outcome, id: &str, name: FindingName) -> bool {
    o.findings.iter().any(|f| f.entry_id == id && f.name == name)
}

/// Kit law §10.2: the six checks in the written order; §10.3: the result object.
pub fn grant_check(g: &[u8], input: Option<&Value>, now: Option<u64>) -> Checked {
    let outcome: Option<Outcome> = match input {
        Some(v) => audit::audit_full(&with_grant_in_pile(v, g)),
        None => None,
    };
    grant_check_with(g, outcome.as_ref(), now)
}

/// The same six checks on an existing audit outcome, so one chain-check hop audits once.
pub fn grant_check_with(g: &[u8], outcome: Option<&Outcome>, now: Option<u64>) -> Checked {
    trace::mark(t::K2);
    let mut state: [State; 6] = [State::Unknown; 6];
    let mut reason1: Option<Reason> = None;

    // Check 1: BAD_SIG.
    let accepted = entry::check(g);
    let grant: Option<Entry> = match &accepted {
        Ok(e) if e.kind == EntryType::Grant => Some(e.clone()),
        _ => None,
    };

    match &accepted {
        Err(tok) => {
            state[0] = State::Fail;
            reason1 = Some(Reason::Parent(*tok));
        }
        Ok(e) if e.kind != EntryType::Grant => {
            state[0] = State::Fail;
            reason1 = Some(Reason::NotAGrant);
        }
        Ok(_) => state[0] = State::Pass,
    }

    if state[0] == State::Pass {
        let g_entry = grant.as_ref().unwrap();
        let g_id = g_entry.id_hex();

        // Check 2: BROKEN_LEDGER.
        match outcome {
            None => state[1] = State::Unknown,
            Some(o) => {
                state[1] = if o.label == Label::BrokenChain {
                    State::Fail
                } else {
                    State::Pass
                }
            }
        }

        // Check 3: NOT_IN_LEDGER. With check 2 FAIL, checks 3, 4 and 6 are unknown.
        if let (Some(o), true) = (outcome, state[1] == State::Pass) {
            let in_ledger = o.ledger.iter().any(|e| e.id_hex() == g_id);
            state[2] = if !in_ledger {
                State::Fail
            } else if findings_on(o, &g_id, FindingName::AuthorityMismatch) {
                State::Unknown
            } else {
                State::Pass
            };
        }

        // Check 4: UNANCHORED. Anchored is PASS; a non-covering basis is unknown; a retained UNPROVEN record
        // whose hash is the entry_id of a ledger entry from which g is reachable (§8.2 with `counted` read as
        // UNPROVEN) is unknown, since that anchor is undecided; only a COMPLETE report FAILs; anything else is
        // unknown.
        if let (Some(o), true, true) = (outcome, state[1] == State::Pass, state[2] != State::Fail) {
            let e = o.ledger.iter().find(|e| e.id_hex() == g_id);
            state[3] = match e {
                Some(e) if reading::is_anchored(o, e) => State::Pass,
                Some(e) => {
                    if !covering(o) {
                        State::Unknown
                    } else if reading::unproven_reaches(o, e) {
                        State::Unknown
                    } else if o.label == Label::Complete {
                        State::Fail
                    } else {
                        State::Unknown
                    }
                }
                None => State::Unknown,
            };
        }

        // Check 5: EXPIRED. Reads only now, not the ledger.
        state[4] = match window_of(g_entry) {
            None => State::Pass,
            Some((from, to)) => match now {
                None => State::Unknown,
                Some(n) => {
                    if n < from || n > to {
                        State::Fail
                    } else {
                        State::Pass
                    }
                }
            },
        };

        // Check 6: REVOKED.
        if let (Some(o), true, true) = (outcome, state[1] == State::Pass, state[2] != State::Fail) {
            let revoked = o.ledger.iter().any(|e| {
                e.kind == EntryType::Revocation
                    && e.body.member("grant").and_then(|x| x.as_str()) == Some(g_id.as_str())
                    && !findings_on(o, &e.id_hex(), FindingName::AuthorityMismatch)
            });
            state[5] = if revoked {
                State::Fail
            } else if o.label == Label::Complete {
                State::Pass
            } else {
                State::Unknown
            };
        }
    }

    let tokens = Check::ALL;
    let checks: Vec<Value> = (0..6)
        .map(|i| {
            obj(vec![
                (Key::N, Value::Int(i as u64 + 1)),
                (Key::Token, s(tokens[i].as_str())),
                (Key::State, s(state[i].as_str())),
                (
                    Key::Reason,
                    match (i, reason1) {
                        (0, Some(r)) if state[0] == State::Fail => s(r.as_str()),
                        _ => Value::Null,
                    },
                ),
            ])
        })
        .collect();
    let failed: Vec<Value> = (0..6)
        .filter(|i| state[*i] == State::Fail)
        .map(|i| s(tokens[i].as_str()))
        .collect();

    let verdict = if state.iter().all(|x| *x == State::Pass) {
        CheckVerdict::Green
    } else if state.iter().any(|x| *x == State::Fail) {
        CheckVerdict::Fail
    } else {
        CheckVerdict::Partial
    };

    let basis = match outcome {
        Some(o) => o.basis.clone(),
        None => Value::Null,
    };

    Checked {
        value: obj(vec![
            (Key::Verdict, s(verdict.as_str())),
            (Key::Basis, basis),
            (Key::Checks, Value::Arr(checks)),
            (Key::Failed, Value::Arr(failed)),
        ]),
        verdict,
        check1_pass: state[0] == State::Pass,
        grant,
    }
}

/// Kit law §10.4: the ledger link. No byte link is false; an invalid downstream input is unknown; a
/// downstream root other than the upstream grantee is false; otherwise true.
pub fn ledger_link(u: &Entry, d: &Entry, d_outcome: Option<&Outcome>) -> Option<bool> {
    if !badge::byte_link(u, d) {
        return Some(false);
    }
    let o = d_outcome?;
    let grantee = match u.body.member("grantee").and_then(|x| x.as_str()) {
        Some(x) => x.to_string(),
        None => return Some(false),
    };
    Some(o.root == grantee)
}

/// One chain-check hop: a grant and its optional audit input.
pub struct Hop<'a> {
    pub grant: &'a [u8],
    pub input: Option<Value>,
}

/// Kit law §10.5: the check of a whole chain.
pub fn chain_check(hops: &[Hop], now: Option<u64>) -> Value {
    trace::mark(t::K2);
    if hops.is_empty() {
        return obj(vec![
            (Key::Verdict, s(CheckVerdict::Fail.as_str())),
            (Key::Hops, Value::Arr(vec![])),
            (Key::Links, Value::Arr(vec![])),
            (Key::Token, s(ChainToken::Empty.as_str())),
            (
                Key::Failing,
                obj(vec![(Key::Kind, s(FailKind::Empty.as_str())), (Key::Index, Value::Int(0))]),
            ),
        ]);
    }

    let mut results: Vec<Checked> = Vec::with_capacity(hops.len());
    let mut outcomes: Vec<Option<Outcome>> = Vec::with_capacity(hops.len());
    for h in hops {
        let outcome = match &h.input {
            Some(v) => audit::audit_full(&with_grant_in_pile(v, h.grant)),
            None => None,
        };
        results.push(grant_check_with(h.grant, outcome.as_ref(), now));
        outcomes.push(outcome);
    }

    let mut links: Vec<Option<bool>> = Vec::with_capacity(hops.len().saturating_sub(1));
    for k in 1..hops.len() {
        let undecided = !results[k - 1].check1_pass || !results[k].check1_pass;
        links.push(if undecided {
            None
        } else {
            ledger_link(
                results[k - 1].grant.as_ref().unwrap(),
                results[k].grant.as_ref().unwrap(),
                outcomes[k].as_ref(),
            )
        });
    }

    // The failing point: the first that applies, in order.
    let mut failing: Option<(FailKind, u64)> = None;
    let mut token: Option<ChainToken> = None;
    if results[0].check1_pass
        && results[0]
            .grant
            .as_ref()
            .map(|g| g.body.member("upstream").is_some())
            .unwrap_or(false)
    {
        failing = Some((FailKind::Incomplete, 0));
        token = Some(ChainToken::Incomplete);
    }
    if failing.is_none() {
        if let Some(k) = links.iter().position(|l| *l == Some(false)) {
            failing = Some((FailKind::Link, k as u64 + 1));
            token = Some(ChainToken::Link);
        }
    }
    if failing.is_none() {
        if let Some(k) = results.iter().position(|r| r.verdict == CheckVerdict::Fail) {
            failing = Some((FailKind::Hop, k as u64));
            token = None;
        }
    }

    let verdict = if failing.is_some() {
        CheckVerdict::Fail
    } else if results.iter().all(|r| r.verdict == CheckVerdict::Green) && links.iter().all(|l| *l == Some(true))
    {
        CheckVerdict::Green
    } else {
        CheckVerdict::Partial
    };

    obj(vec![
        (Key::Verdict, s(verdict.as_str())),
        (
            Key::Hops,
            Value::Arr(results.iter().map(|r| r.value.clone()).collect()),
        ),
        (
            Key::Links,
            Value::Arr(
                links
                    .iter()
                    .map(|l| match l {
                        Some(b) => Value::Bool(*b),
                        None => Value::Null,
                    })
                    .collect(),
            ),
        ),
        (
            Key::Token,
            match token {
                Some(x) => s(x.as_str()),
                None => Value::Null,
            },
        ),
        (
            Key::Failing,
            match failing {
                Some((kind, index)) => obj(vec![(Key::Kind, s(kind.as_str())), (Key::Index, Value::Int(index))]),
                None => Value::Null,
            },
        ),
    ])
}
