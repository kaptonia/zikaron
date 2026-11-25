use crate::End;
use zikaron_seal::queue::Queue;

static AMEND: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn amend<E: End, T>(end: &E, f: impl FnOnce(&mut Queue) -> T) -> Result<(T, Queue), E::Fail> {
    let _g = AMEND.lock().unwrap_or_else(|e| e.into_inner());
    let mut q = end.read_queue()?;
    let out = f(&mut q);
    end.write_queue(&q)?;
    Ok((out, q))
}

pub fn read<E: End>(end: &E) -> Result<Queue, E::Fail> {
    let _g = AMEND.lock().unwrap_or_else(|e| e.into_inner());
    end.read_queue()
}

pub fn settle<E: End>(end: &E, ids: &[String], anchored: bool, at: Option<&Inclusion>) -> Result<(usize, Queue), E::Fail> {
    if !anchored {
        return Ok((0, read(end)?));
    }
    amend(end, |q| match at {
        Some(i) => q.included_out(ids, &i.tx, i.chain, i.block),
        None => q.anchored_out(ids),
    })
}

pub fn follow<E: End>(end: &E, on_chain: &[(String, u64)], owed: &[(String, bool, Option<u64>)], others: &[u64], at: u64) -> Result<Queue, E::Fail> {
    amend(end, |q| {
        let mut chains: Vec<u64> = on_chain.iter().map(|(_, c)| *c).collect();
        chains.sort_unstable();
        chains.dedup();
        for c in chains {
            let ids: Vec<String> = on_chain.iter().filter(|(_, x)| *x == c).map(|(id, _)| id.clone()).collect();
            q.seen_out_on(&ids, c);
        }
        for (id, alone, carried_on) in owed {
            let _ = q.push_placed(id, *alone, *carried_on, others, at);
        }
    })
    .map(|(_, q)| q)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inclusion {
    pub tx: String,
    pub chain: u64,
    pub block: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Owed {
    pub entries: Vec<String>,
    pub unanchored: Vec<String>,
    pub complete: bool,
}

pub fn whole(label: &str, asked: usize, unanswered: usize) -> bool {
    use zikaron::v2::tokens::Label;
    (label == Label::Complete.as_str() || label == Label::Gaps.as_str()) && asked > 0 && unanswered == 0
}

pub fn uncovered(q: &Queue, pile: &[Vec<u8>], published: &[String]) -> Vec<String> {
    use zikaron_glue::book::{self, Kind};
    let entries: Vec<book::Entry> = pile.iter().filter_map(|b| book::check(b).ok()).collect();
    let noticed: Vec<String> = entries.iter().filter(|e| e.kind == Kind::Grant && zikaron_glue::notices::carries(&e.bytes)).map(|e| e.id_hex().to_ascii_lowercase()).collect();
    let by_notice = |e: &book::Entry| e.kind == Kind::Revocation && e.body.member("grant").and_then(|v| v.as_str()).is_some_and(|g| noticed.contains(&g.to_ascii_lowercase()));
    let (_, withdrawn) = zikaron_glue::retraction::read_items(pile);
    let mut local: Vec<String> = Vec::new();
    for (subject, (_, retraction)) in &withdrawn.deleted {
        let s = subject.to_ascii_lowercase();
        if !q.published(&s, published) {
            local.push(s);
            local.push(retraction.to_ascii_lowercase());
        }
    }
    entries
        .iter()
        .filter(|e| e.kind != Kind::Face && !by_notice(e))
        .map(|e| e.id_hex().to_ascii_lowercase())
        .filter(|id| !q.has(id) && !q.anchored.iter().any(|(x, _)| x == id) && !q.passed.contains(id) && !local.contains(id))
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn caught_up<E: End>(end: &E, chain: u64, others: &[u64], at: u64, published: &[String], read: impl FnOnce(&[Vec<u8>]) -> Option<Owed>) -> Result<Queue, E::Fail> {
    let q = self::read(end)?;
    let pile = end.pile()?;
    let owe = uncovered(&q, &pile, published);
    if owe.is_empty() {
        return Ok(q);
    }
    let Some(owed) = read(&pile) else { return Ok(q) };
    let places = zikaron_seal::queue::Place::of_pile(&pile);
    amend(end, |q| {
        for id in &owe {
            if owed.unanchored.contains(id) {
                let p = places.get(id).copied();
                let _ = q.push_placed(id, p.is_some_and(|p| p.alone), p.and_then(|p| p.carried_on), others, at);
            } else if owed.entries.contains(id) {
                if !q.anchored.iter().any(|(x, c)| x == id && *c == chain) {
                    q.anchored.push((id.clone(), chain));
                }
            } else if owed.complete && !q.passed.contains(id) {
                q.passed.push(id.clone());
            }
        }
    })
    .map(|(_, q)| q)
}

pub fn owed_on(eps: &[crate::node::Endpoint], chain: u64, registry: &[u8; 20], from_block: u64, pile: &[Vec<u8>], facts: &dyn crate::Facts) -> Option<Owed> {
    let (head, _) = crate::node::head_block(eps, chain).ok()?;
    let senders = zikaron_glue::basis::senders_of(pile);
    let regs = zikaron_glue::notices::registries_of(chain, &zikaron::hexfmt::encode(registry));
    let window = zikaron_glue::basis::Window { chain, registries: &regs, from_block, to_block: head.max(from_block), senders: &senders };
    let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&window));
    let crate::agree::Scan::Basis(got) = crate::agree::scan(eps, &basis, facts).ok()? else { return None };
    let read = zikaron_glue::book::read(pile, &got.fragment).ok()?;
    let label = read.label.as_str();
    if !whole(label, got.asked, got.unanswered.len()) {
        return None;
    }
    Some(Owed { entries: read.entries.iter().map(|e| e.id_hex().to_ascii_lowercase()).collect(), unanchored: zikaron_glue::book::unanchored_in(&read, chain), complete: label == zikaron::v2::tokens::Label::Complete.as_str() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Code, Trouble};
    use zikaron::json::Value;
    use zikaron_glue::book::{self, Law, Sealed};

    const KEY: [u8; 32] = [0x3c; 32];

    struct Pile(std::cell::RefCell<Queue>, Vec<Vec<u8>>);

    impl End for Pile {
        type Fail = Trouble;
        fn code_of(f: &Trouble) -> Option<String> {
            Some(f.code.as_str().to_string())
        }
        fn read_queue(&self) -> Result<Queue, Trouble> {
            Ok(self.0.borrow().clone())
        }
        fn write_queue(&self, q: &Queue) -> Result<(), Trouble> {
            *self.0.borrow_mut() = q.clone();
            Ok(())
        }
        fn pile(&self) -> Result<Vec<Vec<u8>>, Trouble> {
            Ok(self.1.clone())
        }
        fn behind(&self, lacks: &[String]) -> Trouble {
            Trouble::text(Code::NewerElsewhere, lacks.len().to_string())
        }
        fn unread(&self, f: Trouble) -> Trouble {
            f
        }
        fn mark(&self) -> Result<Option<zikaron_seal::unfetched::State>, Trouble> {
            Ok(None)
        }
        fn lift(&self) -> Result<(), Trouble> {
            Ok(())
        }
        fn machine(&self) -> Result<std::path::PathBuf, Trouble> {
            Ok(std::path::PathBuf::new())
        }
    }

    fn entry(typ: &str, seq: u64, prev: Option<&[u8]>, body: Value) -> Vec<u8> {
        let author = zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(&KEY).expect("key"));
        let prev = prev.map(|p| zikaron::hexfmt::encode(&book::entry_id(p)));
        let sign = |pre: &[u8], domain: &str| -> Result<String, ()> {
            let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
            let (r, s, v) = zikaron::cryptox::sign_digest(&KEY, &digest).ok_or(())?;
            let mut sig = r.to_vec();
            sig.extend_from_slice(&s);
            sig.push(v);
            Ok(zikaron::hexfmt::encode(&sig))
        };
        match book::seal(Law::Two, &author, typ, seq, prev.as_deref(), body, sign) {
            Sealed::Done(e) => e.bytes,
            _ => panic!("sealed {typ}"),
        }
    }

    fn note(n: u8) -> Value {
        let mode = Value::Obj(vec![("mark".into(), Value::Str("hand".into())), ("toolchain".into(), Value::Str(zikaron::hexfmt::encode(&[0x70; 32])))]);
        Value::Obj(vec![("content".into(), Value::Str(zikaron::hexfmt::encode(&[n; 32]))), ("mode".into(), mode)])
    }

    fn id(b: &[u8]) -> String {
        zikaron::hexfmt::encode(&book::entry_id(b)).to_ascii_lowercase()
    }

    #[test]
    fn the_queue_follows_only_a_reading_that_is_whole_and_keeps_as_read_only_from_a_complete_one() {
        let g = entry("genesis", 0, None, Value::Obj(vec![("statement_md".into(), Value::Str("works".into()))]));
        let h1 = entry("history", 1, Some(&g), note(1));
        let h2 = entry("history", 2, Some(&h1), note(2));
        let pile = vec![g.clone(), h1.clone(), h2.clone()];
        let fresh = || Pile(std::cell::RefCell::new(Queue { chain: 7, ..Queue::default() }), pile.clone());
        let owed = |complete| Owed { entries: vec![id(&g), id(&h1)], unanchored: vec![id(&h1)], complete };
        let end = fresh();
        let q = caught_up(&end, 7, &[], 1, &[], |_| None).expect("queue");
        assert!(q.items.is_empty() && q.anchored.is_empty() && q.passed.is_empty(), "a reading not whole writes nothing");
        assert_eq!(uncovered(&q, &pile, &[]).len(), 3, "and leaves every entry uncovered");
        let end = fresh();
        let q = caught_up(&end, 7, &[], 1, &[], |_| Some(owed(false))).expect("queue");
        assert!(q.has(&id(&h1)), "owed: queued");
        assert!(q.anchored.iter().any(|(x, c)| *x == id(&g) && *c == 7), "on chain: recorded");
        assert!(q.passed.is_empty(), "short of complete: nothing kept as read");
        assert_eq!(uncovered(&q, &pile, &[]), vec![id(&h2)], "read again next time");
        let end = fresh();
        let q = caught_up(&end, 7, &[], 1, &[], |_| Some(owed(true))).expect("queue");
        assert_eq!(q.passed, vec![id(&h2)], "complete: kept as read");
        assert!(uncovered(&q, &pile, &[]).is_empty());
    }
}
