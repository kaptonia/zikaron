//! Merkle-Patricia tries: build, prove, verify (no third-party chain crates).
//!
//! A §9.7 kit pins a transaction and its receipt to two roots of the header, and nodes serve only account
//! proofs. So this layer builds those two tries from every transaction (or receipt) of the block, checks the
//! roots against the header and takes the path of one item. A root mismatch stops: the block at hand is not
//! the one it claims.
//!
//! Keys are the RLP of the index within the block; values are the item's full bytes.

use crate::rlp;
use zikaron::cryptox::keccak256;

/// An encoded node: shorter than 32 bytes is inlined, otherwise referenced by hash.
fn reference(encoded: &[u8]) -> Vec<u8> {
    if encoded.len() < 32 {
        encoded.to_vec()
    } else {
        rlp::bytes(&keccak256(encoded))
    }
}

fn nibbles(key: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(key.len() * 2);
    for b in key {
        out.push(b >> 4);
        out.push(b & 0xf);
    }
    out
}

/// Hex-prefix path encoding (shared by leaf and extension).
fn hex_prefix(path: &[u8], leaf: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(path.len() / 2 + 1);
    let odd = path.len() % 2 == 1;
    let flag = if leaf { 2 } else { 0 } + if odd { 1 } else { 0 };
    if odd {
        out.push((flag << 4) | path[0]);
        for c in path[1..].chunks(2) {
            out.push((c[0] << 4) | c[1]);
        }
    } else {
        out.push(flag << 4);
        for c in path.chunks(2) {
            out.push((c[0] << 4) | c[1]);
        }
    }
    out
}

/// A trie built by index.
pub struct Trie {
    entries: Vec<(Vec<u8>, Vec<u8>)>,
}

/// The answer of one proof check. "No such key" is an answer, not a failure: an address absent from the state
/// trie is exactly §9.3's "no code", and reading it as a broken proof would turn a valid verdict into an
/// unmeasurable one.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Answer {
    Value(Vec<u8>),
    Absent,
}

impl Trie {
    /// Build from index to bytes: keys are the RLP of the index.
    pub fn indexed(values: &[Vec<u8>]) -> Trie {
        let entries = values
            .iter()
            .enumerate()
            .map(|(i, v)| (rlp::quantity(i as u64), v.clone()))
            .collect();
        Trie { entries }
    }

    fn items(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.entries.iter().map(|(k, v)| (nibbles(k), v.clone())).collect()
    }

    /// The root.
    pub fn root(&self) -> [u8; 32] {
        match self.build(&self.items(), 0) {
            None => keccak256(&rlp::bytes(&[])),
            Some(n) => keccak256(&n),
        }
    }

    /// The proof of a key: node encodings from the root down (inlined nodes live inside their parent).
    pub fn proof(&self, key: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        self.collect(&self.items(), 0, &nibbles(key), &mut out);
        out
    }

    fn collect(&self, items: &[(Vec<u8>, Vec<u8>)], depth: usize, path: &[u8], out: &mut Vec<Vec<u8>>) {
        let Some(node) = self.build(items, depth) else { return };
        // Nodes shorter than 32 bytes are inlined in their parent, except the root: it has no parent and the
        // verifier fetches it by hash. Without it, a proof built here would fail its own verification.
        if node.len() >= 32 || out.is_empty() {
            out.push(node);
        }
        if items.len() == 1 {
            return;
        }
        let common = self.common_prefix(items, depth);
        if common > depth {
            self.collect(items, common, path, out);
            return;
        }
        if path.len() <= depth {
            return;
        }
        let slot = path[depth];
        let mine: Vec<(Vec<u8>, Vec<u8>)> = items
            .iter()
            .filter(|(k, _)| k.len() > depth && k[depth] == slot)
            .cloned()
            .collect();
        if !mine.is_empty() {
            self.collect(&mine, depth + 1, path, out);
        }
    }

    fn common_prefix(&self, items: &[(Vec<u8>, Vec<u8>)], depth: usize) -> usize {
        let first = &items[0].0;
        let mut common = depth;
        'outer: while common < first.len() {
            let c = first[common];
            for (k, _) in items {
                if k.len() <= common || k[common] != c {
                    break 'outer;
                }
            }
            common += 1;
        }
        common
    }

    fn build(&self, items: &[(Vec<u8>, Vec<u8>)], depth: usize) -> Option<Vec<u8>> {
        if items.is_empty() {
            return None;
        }
        if items.len() == 1 {
            let (k, v) = &items[0];
            return Some(rlp::list(&[rlp::bytes(&hex_prefix(&k[depth..], true)), rlp::bytes(v)]));
        }
        let common = self.common_prefix(items, depth);
        if common > depth {
            let inner = self.build(items, common)?;
            return Some(rlp::list(&[
                rlp::bytes(&hex_prefix(&items[0].0[depth..common], false)),
                reference(&inner),
            ]));
        }
        let mut children: Vec<Vec<u8>> = Vec::with_capacity(17);
        for slot in 0..16u8 {
            let mine: Vec<(Vec<u8>, Vec<u8>)> = items
                .iter()
                .filter(|(k, _)| k.len() > depth && k[depth] == slot)
                .cloned()
                .collect();
            match self.build(&mine, depth + 1) {
                None => children.push(rlp::bytes(&[])),
                Some(n) => children.push(reference(&n)),
            }
        }
        let here = items.iter().find(|(k, _)| k.len() == depth);
        children.push(match here {
            Some((_, v)) => rlp::bytes(v),
            None => rlp::bytes(&[]),
        });
        Some(rlp::list(&children))
    }
}

/// Verify a proof from the root down. Each node must hash to what the level above points to, or the path is
/// made up.
pub fn verify(root: &[u8; 32], key: &[u8], proof: &[Vec<u8>]) -> Option<Answer> {
    let mut by_hash: std::collections::BTreeMap<[u8; 32], Vec<u8>> = std::collections::BTreeMap::new();
    for n in proof {
        by_hash.insert(keccak256(n), n.clone());
    }
    let path = nibbles(key);
    // Empty trie: the root is the hash of the empty string; no key is present.
    if *root == keccak256(&rlp::bytes(&[])) {
        return Some(Answer::Absent);
    }
    let first = by_hash.get(root)?.clone();
    step(&by_hash, &rlp::decode_all(&first)?, &path, 0)
}

fn step(
    nodes: &std::collections::BTreeMap<[u8; 32], Vec<u8>>,
    item: &rlp::Item,
    path: &[u8],
    at: usize,
) -> Option<Answer> {
    let l = item.list()?;
    match l.len() {
        17 => {
            if at == path.len() {
                let v = l[16].bytes()?;
                return Some(if v.is_empty() { Answer::Absent } else { Answer::Value(v.to_vec()) });
            }
            follow(nodes, &l[path[at] as usize], path, at + 1)
        }
        2 => {
            let (leaf, seg) = decode_hex_prefix(l[0].bytes()?)?;
            if path.len() < at + seg.len() || path[at..at + seg.len()] != seg[..] {
                // The path forks here: the key is not in the trie.
                return Some(Answer::Absent);
            }
            let at = at + seg.len();
            if leaf {
                if at != path.len() {
                    return Some(Answer::Absent);
                }
                return Some(Answer::Value(l[1].bytes()?.to_vec()));
            }
            follow(nodes, &l[1], path, at)
        }
        _ => None,
    }
}

fn follow(
    nodes: &std::collections::BTreeMap<[u8; 32], Vec<u8>>,
    child: &rlp::Item,
    path: &[u8],
    at: usize,
) -> Option<Answer> {
    match child {
        rlp::Item::List(_) => step(nodes, child, path, at),
        rlp::Item::Bytes(b) if b.is_empty() => Some(Answer::Absent),
        rlp::Item::Bytes(b) if b.len() == 32 => {
            let mut h = [0u8; 32];
            h.copy_from_slice(b);
            let n = nodes.get(&h)?;
            step(nodes, &rlp::decode_all(n)?, path, at)
        }
        _ => None,
    }
}

fn decode_hex_prefix(b: &[u8]) -> Option<(bool, Vec<u8>)> {
    let first = *b.first()?;
    let flag = first >> 4;
    let leaf = flag & 2 != 0;
    let odd = flag & 1 != 0;
    let mut out = Vec::new();
    if odd {
        out.push(first & 0xf);
    }
    for byte in &b[1..] {
        out.push(byte >> 4);
        out.push(byte & 0xf);
    }
    Some((leaf, out))
}
