use crate::node::{self, Endpoint};
use crate::{Code, Facts, Trouble, Why};
use zikaron::json::Value;
use zikaron_glue::face::{Basis, Table};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lane {
    pub chain: u64,
    pub registry: [u8; 20],
    pub from_block: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Left {
    NoBook,
    BookWrong,
    BookUnread(Trouble),
    HeadUnread(Trouble),
    NoBlockBy,
    Unread(Trouble),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Built {
    pub input: Value,
    pub table: Table,
    pub moment: u64,
    pub ends: Vec<(u64, u64)>,
    pub left: Vec<(u64, Left)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NotRead {
    NoChain(Vec<(u64, Left)>),
    NoTable(Vec<(u64, Left)>),
}

impl Left {
    pub fn word(&self) -> String {
        match self {
            Left::NoBook => "NO_BOOK".to_string(),
            Left::BookWrong => "BOOK_WRONG".to_string(),
            Left::BookUnread(t) => format!("BOOK_UNREAD {}", crate::said(&t.why)),
            Left::HeadUnread(t) => format!("HEAD_UNREAD {}", crate::said(&t.why)),
            Left::NoBlockBy => "NO_BLOCK_BY".to_string(),
            Left::Unread(t) => format!("UNREAD {}", crate::said(&t.why)),
        }
    }
}

pub fn omitted(left: &[(u64, Left)]) -> Vec<String> {
    left.iter().map(|(c, l)| format!("{c} {}", l.word())).collect()
}

pub const NO_ROOT: &str = "NO_ROOT";

impl NotRead {
    pub fn said(&self) -> String {
        let (word, left) = match self {
            NotRead::NoChain(l) => ("NO_CHAIN", l),
            NotRead::NoTable(l) => ("NO_TABLE", l),
        };
        std::iter::once(word.to_string()).chain(omitted(left)).collect::<Vec<_>>().join("; ")
    }
}

fn opened(eps: &[Endpoint], chain: u64) -> Vec<Box<dyn zikaron_anchor::rpc::Endpoint + Send>> {
    eps.iter().filter(|e| e.chain == chain).filter_map(|e| node::endpoint_at(e.url.for_transport())).collect()
}

fn refused(r: zikaron_anchor::scan::Refusal) -> Trouble {
    Trouble::new(Code::ScanRefused, Why::Scan { unanswered: Vec::new(), refused: vec![r] })
}

fn last_by(eps: &[Endpoint], chain: u64, head: u64, moment: u64) -> Result<Option<u64>, Trouble> {
    last_by_with(head, moment, |n| node::block_time(eps, chain, n))
}

fn last_by_with<E>(head: u64, moment: u64, mut time: impl FnMut(u64) -> Result<u64, E>) -> Result<Option<u64>, E> {
    let (mut lo, mut hi) = (None, head);
    let mut step = 1u64;
    while lo.is_none() {
        let probe = hi.saturating_sub(step);
        if time(probe)? <= moment {
            lo = Some(probe);
        } else if probe == 0 {
            return Ok(None);
        } else {
            hi = probe;
            step = step.saturating_mul(2);
        }
    }
    let mut lo = lo.unwrap_or(0);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if time(mid)? <= moment {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(Some(lo))
}

pub fn accessions(eps: &[Endpoint], b: &Basis, root: &[u8; 20]) -> Result<Vec<Value>, Trouble> {
    let mut answers: Vec<Vec<Value>> = Vec::new();
    let mut first: Option<Trouble> = None;
    let nodes = opened(eps, b.chain);
    let places: Vec<String> = nodes.iter().map(|ep| ep.place()).collect();
    let places_all = places.clone();
    let (chain, book, from, to, root) = (b.chain, b.book, b.book_from_block, b.to_block, *root);
    type Got = Option<Result<Vec<Value>, zikaron_anchor::scan::Refusal>>;
    let ask = |nodes: Vec<Box<dyn zikaron_anchor::rpc::Endpoint + Send>>, at: Vec<String>| -> Vec<Option<Got>> {
        zikaron_anchor::endpoints::until_enough(
            nodes,
            move |mut ep| Some(zikaron_anchor::scan::accessions_under(ep.as_mut(), chain, &book, &root, from, to)),
            || None,
            move |slots: &[Option<Got>]| {
                let read: Vec<(&str, Vec<u8>)> = at.iter().zip(slots).filter_map(|(p, g)| match g {
                    Some(Some(Ok(xs))) => Some((p.as_str(), zikaron::json::canon_bytes(&Value::Arr(xs.clone())))),
                    _ => None,
                }).collect();
                zikaron_anchor::judge::two_places_alike(&read) || zikaron_anchor::action::read_while_cut(read.iter().map(|(p, _)| *p))
            },
        )
    };
    let mut read = ask(nodes, places.clone());
    zikaron_anchor::action::cut_and_ask(
        &mut read,
        |i, _| vec![places[i].clone()],
        |g| matches!(g, Some(Some(Ok(_)))),
        |g| matches!(g, Some(Some(Err(r))) if r.cut()),
        |idx, _| {
            let again: Vec<Box<dyn zikaron_anchor::rpc::Endpoint + Send>> = opened(eps, chain).into_iter().enumerate().filter(|(i, _)| idx.contains(i)).map(|(_, ep)| ep).collect();
            ask(again, idx.iter().map(|i| places[*i].clone()).collect())
        },
    );
    let given: Vec<(String, Vec<Value>)> = places_all.iter().zip(&read).filter_map(|(p, g)| match g {
        Some(Some(Ok(xs))) => Some((p.clone(), xs.clone())),
        _ => None,
    }).collect();
    let bytes: Vec<(&str, Vec<u8>)> = given.iter().map(|(p, xs)| (p.as_str(), zikaron::json::canon_bytes(&Value::Arr(xs.clone())))).collect();
    if let Some(i) = zikaron_anchor::judge::alike_among(&bytes) {
        return Ok(given[i].1.clone());
    }
    let read: Vec<Got> = read.into_iter().flatten().collect();
    for r in read.into_iter().flatten() {
        match r {
            Ok(xs) => answers.push(xs),
            Err(r) => {
                first.get_or_insert(refused(r));
            }
        }
    }
    let Some(one) = answers.first() else { return Err(first.unwrap_or_else(|| Trouble::new(Code::NoEndpoint, Why::NoNodes))) };
    if answers.iter().any(|a| a != one) {
        return Err(Trouble::new(Code::Disagree, Why::Differ { count: answers.len(), sources: format!("accessions chain {}", b.chain) }));
    }
    Ok(one.clone())
}

fn anchors(eps: &[Endpoint], b: &Basis, keys: &[String], facts: &dyn Facts) -> Result<Vec<Value>, Trouble> {
    let registries: Vec<String> = b.registries.iter().map(|r| zikaron::hexfmt::encode(r)).collect();
    let basis = zikaron_glue::basis::basis_of(&zikaron_glue::basis::Window { chain: b.chain, registries: &registries, from_block: b.from_block, to_block: b.to_block, senders: keys });
    let mine: Vec<Endpoint> = eps.iter().filter(|e| e.chain == b.chain).cloned().collect();
    match crate::agree::scan(&mine, &zikaron::json::canon_bytes(&basis), facts)? {
        crate::agree::Scan::Basis(a) => {
            let read = a.fragment.member("basis").and_then(|x| x.member("chains")).and_then(Value::as_arr).is_some_and(|c| !c.is_empty());
            if !read {
                return Err(Trouble::new(Code::ScanRefused, Why::Scan { unanswered: a.unanswered, refused: Vec::new() }));
            }
            Ok(a.fragment.member("anchors").and_then(Value::as_arr).cloned().unwrap_or_default())
        }
        crate::agree::Scan::NoLabel { .. } => Err(Trouble::new(Code::AuditInput, Why::Text(format!("basis chain {}", b.chain)))),
    }
}

fn h32(v: &Value, k: &str) -> Option<[u8; 32]> {
    zikaron::hexfmt::decode(v.member(k)?.as_str()?)?.try_into().ok()
}

fn carried(eps: &[Endpoint], chain: u64, recs: &[Value], have: &std::collections::BTreeSet<[u8; 32]>) -> Result<Vec<Vec<u8>>, Trouble> {
    let mut txs: Vec<[u8; 32]> = recs.iter().filter(|a| h32(a, "hash").is_some_and(|h| !have.contains(&h))).filter_map(|a| h32(a, "tx")).collect();
    txs.sort();
    txs.dedup();
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut found: Vec<((u64, [u8; 32]), Option<Vec<u8>>)> = Vec::new();
    let mut nodes = opened(eps, chain);
    for tx in txs {
        let kept = zikaron_anchor::settled::carried_kept(chain, &tx);
        let entry = match kept {
            Some(e) => e,
            None => {
                let mut got = None;
                let mut first = None;
                for ep in nodes.iter_mut() {
                    match zikaron_anchor::scan::tx_proved(ep.as_mut(), chain, &tx) {
                        Ok(Some(t)) => {
                            got = Some(t);
                            break;
                        }
                        Ok(None) => {}
                        Err(r) => {
                            first.get_or_insert(refused(r));
                        }
                    }
                }
                let Some(t) = got else {
                    return Err(first.unwrap_or_else(|| Trouble::new(Code::ChainShape, Why::Text(format!("tx {}", zikaron::hexfmt::encode(&tx))))));
                };
                let e = zikaron_glue::face::carried(&t.input).map(<[u8]>::to_vec);
                found.push(((chain, tx), e.clone()));
                e
            }
        };
        out.extend(entry);
    }
    zikaron_anchor::settled::keep_carried(&found);
    Ok(out)
}

pub fn input(eps: &[Endpoint], root: &[u8; 20], lanes: &[Lane], pile: &[Vec<u8>], facts: &dyn Facts) -> Result<Built, NotRead> {
    let mut given: Vec<Lane> = Vec::new();
    for l in lanes {
        if !given.iter().any(|g| g.chain == l.chain) {
            given.push(*l);
        }
    }
    given.sort_by_key(|l| l.chain);
    let mut dropped: Vec<(u64, Left)> = Vec::new();
    loop {
        let with = |mut left: Vec<(u64, Left)>, dropped: &[(u64, Left)]| {
            left.extend(dropped.iter().cloned());
            left.sort_by_key(|(c, _)| *c);
            left
        };
        match once(eps, root, &given, pile, facts) {
            Pass::Done(Ok(mut b)) => {
                b.left = with(std::mem::take(&mut b.left), &dropped);
                return Ok(b);
            }
            Pass::Done(Err(NotRead::NoChain(l))) => return Err(NotRead::NoChain(with(l, &dropped))),
            Pass::Done(Err(NotRead::NoTable(l))) => return Err(NotRead::NoTable(with(l, &dropped))),
            Pass::Again(late) => {
                given.retain(|g| !late.iter().any(|(c, _)| *c == g.chain));
                dropped.extend(late);
            }
        }
    }
}

enum Pass {
    Done(Result<Built, NotRead>),
    Again(Vec<(u64, Left)>),
}

fn once(eps: &[Endpoint], root: &[u8; 20], given: &[Lane], pile: &[Vec<u8>], facts: &dyn Facts) -> Pass {
    let mut left: Vec<(u64, Left)> = Vec::new();

    let mut heads: Vec<(Lane, [u8; 20], u64, u64)> = Vec::new();
    for l in given.iter().copied() {
        let book = match crate::book::state(eps, l.chain) {
            Ok(crate::book::State::At(a)) => a,
            Ok(crate::book::State::Off) => {
                left.push((l.chain, Left::NoBook));
                continue;
            }
            Ok(crate::book::State::Wrong) => {
                left.push((l.chain, Left::BookWrong));
                continue;
            }
            Err(t) => {
                left.push((l.chain, Left::BookUnread(t)));
                continue;
            }
        };
        match node::head_time(eps, l.chain) {
            Ok((time, number, _)) => heads.push((l, book, number, time)),
            Err(t) => left.push((l.chain, Left::HeadUnread(t))),
        }
    }
    let Some(moment) = zikaron_glue::face::moment(&heads.iter().map(|h| h.3).collect::<Vec<_>>()) else {
        left.extend(heads.iter().map(|h| (h.0.chain, Left::NoBlockBy)));
        left.sort_by_key(|(c, _)| *c);
        return Pass::Done(Err(NotRead::NoChain(left)));
    };
    let head_of: Vec<(u64, u64)> = heads.iter().map(|h| (h.0.chain, h.3)).collect();
    let early = left.len();

    let mut basis: Vec<Basis> = Vec::new();
    for (l, book, head, _) in heads {
        match last_by(eps, l.chain, head, moment) {
            Ok(Some(end)) => {
                let from = zikaron_anchor::revoke::row(l.chain).map_or(l.from_block, |r| r.from_block.min(l.from_block));
                let book_from = zikaron_anchor::book::from_block(l.chain).unwrap_or(0);
                basis.push(Basis {
                    chain: l.chain,
                    from_block: from.min(end),
                    to_block: end,
                    registries: zikaron_anchor::revoke::registries(l.chain, &[l.registry]),
                    book,
                    book_from_block: book_from.min(end),
                });
            }
            Ok(None) => left.push((l.chain, Left::NoBlockBy)),
            Err(t) => left.push((l.chain, Left::Unread(t))),
        }
    }

    let mut accs: Vec<(u64, Vec<Value>)> = Vec::new();
    basis.retain(|b| match accessions(eps, b, root) {
        Ok(xs) => {
            accs.push((b.chain, xs));
            true
        }
        Err(t) => {
            left.push((b.chain, Left::Unread(t)));
            false
        }
    });
    let all: Vec<Value> = accs.iter().flat_map(|(_, xs)| xs.iter().cloned()).collect();
    let keys: Vec<String> = zikaron_glue::face::keys_in_reach(root, &all).iter().map(|k| zikaron::hexfmt::encode(k)).collect();

    let have: std::collections::BTreeSet<[u8; 32]> = pile.iter().map(|b| zikaron::cryptox::sha256(b)).collect();
    let mut recs: Vec<Value> = Vec::new();
    let mut more: Vec<Vec<u8>> = Vec::new();
    basis.retain(|b| match anchors(eps, b, &keys, facts).and_then(|a| carried(eps, b.chain, &a, &have).map(|c| (a, c))) {
        Ok((a, c)) => {
            recs.extend(a);
            more.extend(c);
            true
        }
        Err(t) => {
            left.push((b.chain, Left::Unread(t)));
            false
        }
    });
    let late: Vec<(u64, Left)> = left[early..].to_vec();
    let standing = head_of.iter().filter(|(c, _)| basis.iter().any(|b| b.chain == *c)).map(|(_, t)| *t);
    if !late.is_empty() && zikaron_glue::face::moment(&standing.collect::<Vec<_>>()).is_some_and(|m| m > moment) {
        return Pass::Again(late);
    }
    left.sort_by_key(|(c, _)| *c);
    if basis.is_empty() {
        return Pass::Done(Err(NotRead::NoChain(left)));
    }
    let kept: Vec<u64> = basis.iter().map(|b| b.chain).collect();
    let accessions: Vec<Value> = accs.into_iter().filter(|(c, _)| kept.contains(c)).flat_map(|(_, xs)| xs).collect();
    let mut pile = pile.to_vec();
    pile.extend(more);
    let input = zikaron_glue::face::input(root, &basis, &recs, &accessions, &pile);
    let table = match zikaron_glue::face::read(&input) {
        Ok(t) => t,
        Err(_) => return Pass::Done(Err(NotRead::NoTable(left))),
    };
    Pass::Done(Ok(Built { input, table, moment, ends: basis.iter().map(|b| (b.chain, b.to_block)).collect(), left }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chain_ends_at_its_last_block_at_or_before_the_moment() {
        let time = |n: u64| -> Result<u64, ()> { Ok(100 + n / 2) };
        assert_eq!(last_by_with(1_000, 150, time), Ok(Some(101)), "blocks 100 and 101 are second 150; 102 is later");
        assert_eq!(last_by_with(1_000, 599, time), Ok(Some(999)), "the head's own second is later: the block before it");
        assert_eq!(last_by_with(1_000, 100, time), Ok(Some(1)));
        assert_eq!(last_by_with(1_000, 99, time), Ok(None), "every block is later");
        let mut asked = 0usize;
        let counted = |n: u64| -> Result<u64, ()> {
            asked += 1;
            Ok(100 + n / 2)
        };
        assert_eq!(last_by_with(20_000_000, 10_000_099, counted), Ok(Some(19_999_999)));
        assert!(asked <= 4, "a moment just before the head takes a few questions, not one per block: {asked}");
        assert_eq!(last_by_with(5, 200, |_| Err::<u64, &str>("unread")), Err("unread"), "a time not read is no end");
    }

    #[test]
    fn a_chain_with_no_book_is_left_out_and_none_left_is_no_input() {
        let got = input(&[], &[0x11; 20], &[Lane { chain: 31_337, registry: [0x22; 20], from_block: 0 }], &[], &crate::Bare);
        assert_eq!(got, Err(NotRead::NoChain(vec![(31_337, Left::NoBook)])));
    }
}
