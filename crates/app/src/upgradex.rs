use zikaron::json::Value;
use zikaron_glue::book::{self, Form, Kind, Law};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stage {
    Nothing,
    Handed,
    Anchor { owed: usize },
    Close,
    Closing { id: String },
    Genesis,
    Continuing { id: String },
    Done,
}

impl Stage {
    pub fn step(&self) -> Option<usize> {
        match self {
            Stage::Nothing | Stage::Handed => None,
            Stage::Anchor { .. } => Some(1),
            Stage::Close | Stage::Closing { .. } => Some(2),
            Stage::Genesis | Stage::Continuing { .. } => Some(3),
            Stage::Done => Some(4),
        }
    }

    pub fn pending(&self) -> bool {
        !matches!(self, Stage::Nothing | Stage::Done)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Stage::Nothing => "nothing",
            Stage::Handed => "handed",
            Stage::Anchor { .. } => "anchor",
            Stage::Close => "close",
            Stage::Closing { .. } => "closing",
            Stage::Genesis => "continue",
            Stage::Continuing { .. } => "continuing",
            Stage::Done => "done",
        }
    }
}

fn on_chain(read: &book::Ledger, queue: &crate::queue::Queue) -> crate::chainx::OnChain {
    crate::chainx::OnChain::of(read.anchored_ids().into_iter().map(|id| (None, id)), queue)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leg {
    Unsent,
    Out,
}

pub fn leg(stage: &Stage, queue: &crate::queue::Queue) -> Option<Leg> {
    let (Stage::Closing { id } | Stage::Continuing { id }) = stage else { return None };
    let rows: Vec<&crate::queue::Queued> = queue.items.iter().filter(|q| q.id.eq_ignore_ascii_case(id)).collect();
    let out = !rows.is_empty() && rows.iter().all(|q| matches!(q.step, crate::queue::Step::Submitted { .. } | crate::queue::Step::Resent { .. }));
    Some(if out { Leg::Out } else { Leg::Unsent })
}

pub fn sends_from(stage: &Stage, chains: usize, leg: Option<Leg>) -> Vec<(usize, usize)> {
    let out = leg == Some(Leg::Out);
    match stage {
        Stage::Anchor { owed } => vec![(*owed, 1), (1, chains), (1, 1)],
        Stage::Close => vec![(1, chains), (1, 1)],
        Stage::Closing { .. } if out => vec![(1, 1)],
        Stage::Closing { .. } => vec![(1, chains), (1, 1)],
        Stage::Genesis => vec![(1, 1)],
        Stage::Continuing { .. } if out => Vec::new(),
        Stage::Continuing { .. } => vec![(1, 1)],
        Stage::Nothing | Stage::Handed | Stage::Done => Vec::new(),
    }
}

pub fn known(read: bool, leg: Option<Leg>) -> bool {
    read || leg == Some(Leg::Unsent)
}

pub fn cost_now(stage: &Stage, known: bool, leg: Option<Leg>, cost: Option<&(Stage, usize, u128)>) -> Option<(usize, u128)> {
    if leg == Some(Leg::Out) && matches!(stage, Stage::Continuing { .. }) {
        return Some((0, 0));
    }
    cost.filter(|(s, _, _)| known && s == stage).map(|(_, txs, wei)| (*txs, *wei))
}

pub fn leg_words(leg: Leg, early: bool) -> crate::lang::Key {
    match (leg, early) {
        (_, true) => crate::lang::Key::UpWaitBlock,
        (Leg::Unsent, false) => crate::lang::Key::UpUnsent,
        (Leg::Out, false) => crate::lang::Key::UpWaiting,
    }
}

pub fn stage(items: &[Vec<u8>], fragment: Option<&Value>, me: Option<&str>, queue: &crate::queue::Queue) -> Stage {
    if !items.iter().any(|b| book::check(b).is_ok_and(|e| e.law == Law::One)) {
        return Stage::Nothing;
    }
    let empty = book::empty_fragment();
    let Ok(read) = book::read(items, fragment.unwrap_or(&empty)) else { return Stage::Nothing };
    match read.form {
        Form::New => return Stage::Nothing,
        Form::Continued => {
            let on = on_chain(&read, queue);
            let genesis = read.entries[read.old_len..].iter().find(|e| e.seq == 0 && e.kind == Kind::Genesis).map(|e| e.id_hex());
            return match genesis {
                Some(id) if !on.anywhere(&id) => Stage::Continuing { id },
                _ => Stage::Done,
            };
        }
        Form::Old | Form::Pending => {}
    }
    let Some(me) = me else { return Stage::Nothing };
    let in_office = read.entries[..read.old_len].is_empty() || office_of_old(&read).is_some_and(|o| o.eq_ignore_ascii_case(me));
    if read.handed_off_old(me) || !in_office {
        return Stage::Handed;
    }
    let on = on_chain(&read, queue);
    let on_chain = |id: &str| on.anywhere(id);
    let closing = read.closing_by(me).map(|e| e.id_hex());
    let owed = read.entries[..read.old_len].iter().filter(|e| Some(e.id_hex()) != closing && !on_chain(&e.id_hex())).count();
    if owed > 0 {
        return Stage::Anchor { owed };
    }
    let Some(closing) = closing else { return Stage::Close };
    if !on_chain(&closing) {
        return Stage::Closing { id: closing };
    }
    match genesis_of(items, &read.root, &closing) {
        None => Stage::Genesis,
        Some(id) => Stage::Continuing { id },
    }
}

fn office_of_old(read: &book::Ledger) -> Option<String> {
    let old = &read.entries[..read.old_len];
    let mut at = old.iter().find(|e| e.seq == 0 && e.kind == Kind::Genesis)?.author.clone();
    let mut successions: Vec<&book::Entry> = old.iter().filter(|e| e.kind == Kind::Succession).collect();
    successions.sort_by_key(|e| e.seq);
    for s in successions {
        if s.author.eq_ignore_ascii_case(&at) {
            if let Some(to) = s.to() {
                at = to.to_string();
            }
        }
    }
    Some(at)
}

pub fn genesis_of(items: &[Vec<u8>], root: &str, closing: &str) -> Option<String> {
    items
        .iter()
        .filter_map(|b| book::check(b).ok())
        .filter(|e| e.law == Law::Two && e.seq == 0 && e.kind == Kind::Genesis)
        .find(|e| e.continues().is_some_and(|(r, c)| r.eq_ignore_ascii_case(root) && c.eq_ignore_ascii_case(closing)))
        .map(|e| e.id_hex())
}

pub fn owed(items: &[Vec<u8>], fragment: Option<&Value>, me: &str, queue: &crate::queue::Queue) -> Vec<String> {
    let empty = book::empty_fragment();
    let Ok(read) = book::read(items, fragment.unwrap_or(&empty)) else { return Vec::new() };
    let on = on_chain(&read, queue);
    let closing = read.closing_by(me).map(|e| e.id_hex());
    read.entries[..read.old_len]
        .iter()
        .map(|e| e.id_hex())
        .filter(|id| Some(id) != closing.as_ref() && !on.anywhere(id))
        .collect()
}

pub const WALKED: [crate::task::Kind; 3] = [crate::task::Kind::Gas, crate::task::Kind::Anchor, crate::task::Kind::Audit];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_an_upgrade_still_sends_is_its_stages_own() {
        let id = || "0xab".to_string();
        let count = |s: &Stage, leg: Option<Leg>| sends_from(s, 2, leg).iter().map(|(_, n)| n).sum::<usize>();
        assert_eq!(sends_from(&Stage::Anchor { owed: 4 }, 2, None), [(4, 1), (1, 2), (1, 1)]);
        assert_eq!(count(&Stage::Anchor { owed: 4 }, None), 4);
        assert_eq!(count(&Stage::Close, None), 3);
        assert_eq!(count(&Stage::Closing { id: id() }, Some(Leg::Unsent)), 3, "the closing succession not sent: it and the genesis");
        assert_eq!(count(&Stage::Closing { id: id() }, Some(Leg::Out)), 1, "the closing succession sent: only the genesis");
        assert_eq!(count(&Stage::Genesis, None), 1);
        assert_eq!(count(&Stage::Continuing { id: id() }, Some(Leg::Unsent)), 1, "the genesis not sent: still to send");
        assert_eq!(count(&Stage::Continuing { id: id() }, Some(Leg::Out)), 0, "the genesis sent: nothing more");
        for s in [Stage::Done, Stage::Handed, Stage::Nothing] {
            assert_eq!(count(&s, None), 0, "{s:?}");
        }
    }

    #[test]
    fn the_card_says_the_cost_of_its_own_stage_alone() {
        let stale = (Stage::Anchor { owed: 4 }, 3, 63);
        let now = Stage::Continuing { id: "0xab".into() };
        assert_eq!(cost_now(&now, true, Some(Leg::Unsent), Some(&stale)), None, "another stage's estimate");
        assert_eq!(cost_now(&stale.0, known(false, None), None, Some(&stale)), None, "not read on chain");
        assert_eq!(cost_now(&now, true, Some(Leg::Unsent), None), None);
        let own = (now.clone(), 1, 9);
        assert_eq!(cost_now(&now, known(false, Some(Leg::Unsent)), Some(Leg::Unsent), Some(&own)), Some((1, 9)), "not sent: known from the queue, before the chain is read");
        assert_eq!(cost_now(&now, known(false, Some(Leg::Out)), Some(Leg::Out), None), Some((0, 0)), "sent and waiting: nothing more");
        assert!(!known(false, Some(Leg::Out)) && known(true, Some(Leg::Out)));
    }

    #[test]
    fn a_written_steps_place_is_the_queues() {
        use crate::queue::{Queue, Queued, Step};
        let id = "0xab".to_string();
        let at = |step: Step| Queue { chain: 7, items: vec![Queued { id: id.clone(), at: 0, step, chain: 7 }], ..Default::default() };
        let continuing = Stage::Continuing { id: id.clone() };
        assert_eq!(leg(&continuing, &Queue::default()), Some(Leg::Unsent), "in no row");
        assert_eq!(leg(&continuing, &at(Step::Queued)), Some(Leg::Unsent));
        assert_eq!(leg(&continuing, &at(Step::Refused { said: "x".into() })), Some(Leg::Unsent));
        assert_eq!(leg(&continuing, &at(Step::Submitted { tx: "0x1".into(), chain: 7, nonce: Some(6) })), Some(Leg::Out));
        assert_eq!(leg(&Stage::Closing { id: id.clone() }, &at(Step::Resent { txs: vec!["0x1".into()], chain: 7, nonce: Some(6) })), Some(Leg::Out));
        assert_eq!(leg(&Stage::Genesis, &at(Step::Queued)), None);
        assert_eq!(leg_words(Leg::Unsent, false), crate::lang::Key::UpUnsent);
        assert_eq!(leg_words(Leg::Out, false), crate::lang::Key::UpWaiting);
        assert_eq!(leg_words(Leg::Unsent, true), crate::lang::Key::UpWaitBlock);
    }

    fn genesis_one(key: &[u8; 32]) -> (Vec<u8>, String) {
        let addr = zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(key).expect("an address"));
        let sealed = book::seal::<()>(Law::One, &addr, "genesis", 0, None, Value::Obj(vec![("statement_md".into(), Value::Str("开端".into()))]), |pre, domain| {
            let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
            let (r, s, v) = zikaron::cryptox::sign_digest(key, &digest).ok_or(())?;
            let mut raw = r.to_vec();
            raw.extend_from_slice(&s);
            raw.push(v);
            Ok(zikaron::hexfmt::encode(&raw))
        });
        match sealed {
            book::Sealed::Done(e) => (e.bytes, addr),
            _ => panic!("sealed"),
        }
    }

    #[test]
    fn an_entry_with_this_homes_receipt_is_not_owed_again() {
        let (g, me) = genesis_one(&[0x21; 32]);
        let id = zikaron::hexfmt::encode(&book::entry_id(&g));
        let items = vec![g];
        let empty = crate::queue::Queue { chain: 7, ..Default::default() };
        assert_eq!(stage(&items, None, Some(&me), &empty), Stage::Anchor { owed: 1 });
        assert_eq!(owed(&items, None, &me, &empty), vec![id.clone()]);
        let mut held = empty.clone();
        held.blocks.push(crate::queue::Block { id: id.to_ascii_uppercase().replace("0X", "0x"), tx: id.clone(), chain: 7, block: 9 });
        assert_eq!(stage(&items, None, Some(&me), &held), Stage::Close);
        assert!(owed(&items, None, &me, &held).is_empty());
        let mut recorded = empty.clone();
        recorded.anchored.push((id.clone(), 7));
        assert_eq!(stage(&items, None, Some(&me), &recorded), Stage::Close);
    }
}
