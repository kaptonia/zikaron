use crate::rlp;

enum Node {
    Leaf(Vec<u8>, Vec<u8>),
    Extension(Vec<u8>, Box<Node>),
    Branch(Vec<Option<Node>>, Option<Vec<u8>>),
}

pub fn index_key(i: u64) -> Vec<u8> {
    rlp::quantity(i)
}

fn nibbles(key: &[u8]) -> Vec<u8> {
    key.iter().flat_map(|b| [b >> 4, b & 0x0f]).collect()
}

fn build(items: &[(Vec<u8>, Vec<u8>)]) -> Node {
    if let [(path, value)] = items {
        return Node::Leaf(path.clone(), value.clone());
    }
    let first = &items[0].0;
    let last = &items[items.len() - 1].0;
    let common = first.iter().zip(last).take_while(|(a, b)| a == b).count();
    if common > 0 {
        let rest: Vec<(Vec<u8>, Vec<u8>)> = items.iter().map(|(p, v)| (p[common..].to_vec(), v.clone())).collect();
        return Node::Extension(first[..common].to_vec(), Box::new(build(&rest)));
    }
    let mut value = None;
    let mut children: Vec<Option<Node>> = (0..16).map(|_| None).collect();
    for (nib, child) in children.iter_mut().enumerate() {
        let under: Vec<(Vec<u8>, Vec<u8>)> = items.iter().filter(|(p, _)| p.first() == Some(&(nib as u8))).map(|(p, v)| (p[1..].to_vec(), v.clone())).collect();
        if !under.is_empty() {
            *child = Some(build(&under));
        }
    }
    if let Some((_, v)) = items.iter().find(|(p, _)| p.is_empty()) {
        value = Some(v.clone());
    }
    Node::Branch(children, value)
}

fn compact(path: &[u8], leaf: bool) -> Vec<u8> {
    let flag = if leaf { 2 } else { 0 } + (path.len() % 2) as u8;
    let mut out = Vec::with_capacity(path.len() / 2 + 1);
    let rest = if path.len() % 2 == 1 {
        out.push((flag << 4) | path[0]);
        &path[1..]
    } else {
        out.push(flag << 4);
        path
    };
    for pair in rest.chunks(2) {
        out.push((pair[0] << 4) | pair[1]);
    }
    out
}

fn encode(node: &Node) -> Vec<u8> {
    match node {
        Node::Leaf(path, value) => rlp::list(&[rlp::bytes(&compact(path, true)), rlp::bytes(value)]),
        Node::Extension(path, child) => rlp::list(&[rlp::bytes(&compact(path, false)), reference(child)]),
        Node::Branch(children, value) => {
            let mut items: Vec<Vec<u8>> = children.iter().map(|c| c.as_ref().map(reference).unwrap_or_else(|| rlp::bytes(&[]))).collect();
            items.push(rlp::bytes(value.as_deref().unwrap_or(&[])));
            rlp::list(&items)
        }
    }
}

fn reference(node: &Node) -> Vec<u8> {
    let enc = encode(node);
    if enc.len() < 32 {
        enc
    } else {
        rlp::bytes(&zikaron::cryptox::keccak256(&enc))
    }
}

pub fn root_and_proof(values: &[Vec<u8>], at: usize) -> Option<([u8; 32], Vec<Vec<u8>>)> {
    if at >= values.len() {
        return None;
    }
    let mut items: Vec<(Vec<u8>, Vec<u8>)> = values.iter().enumerate().map(|(i, v)| (nibbles(&index_key(i as u64)), v.clone())).collect();
    items.sort();
    let root = build(&items);
    let root_hash = zikaron::cryptox::keccak256(&encode(&root));
    let path = nibbles(&index_key(at as u64));
    let mut proof = vec![encode(&root)];
    let mut node = &root;
    let mut rest: &[u8] = &path;
    loop {
        let next = match node {
            Node::Leaf(..) => break,
            Node::Extension(p, child) => {
                rest = &rest[p.len()..];
                child.as_ref()
            }
            Node::Branch(children, _) => {
                let Some((first, more)) = rest.split_first() else { break };
                rest = more;
                children[*first as usize].as_ref()?
            }
        };
        let enc = encode(next);
        if enc.len() >= 32 {
            proof.push(enc);
        }
        node = next;
    }
    Some((root_hash, proof))
}

pub fn root_of(values: &[Vec<u8>]) -> [u8; 32] {
    match root_and_proof(values, 0) {
        Some((root, _)) => root,
        None => zikaron::cryptox::keccak256(&rlp::bytes(&[])),
    }
}

pub fn keyed_root_and_proof(pairs: &[(Vec<u8>, Vec<u8>)], key: &[u8]) -> Option<([u8; 32], Vec<Vec<u8>>)> {
    let mut items: Vec<(Vec<u8>, Vec<u8>)> = pairs.iter().map(|(k, v)| (nibbles(k), v.clone())).collect();
    items.sort();
    if items.is_empty() {
        return None;
    }
    let root = build(&items);
    let root_hash = zikaron::cryptox::keccak256(&encode(&root));
    let path = nibbles(key);
    let mut proof = vec![encode(&root)];
    let mut node = &root;
    let mut rest: &[u8] = &path;
    loop {
        let next = match node {
            Node::Leaf(..) => break,
            Node::Extension(p, child) => {
                if !rest.starts_with(p) {
                    break;
                }
                rest = &rest[p.len()..];
                child.as_ref()
            }
            Node::Branch(children, _) => {
                let Some((first, more)) = rest.split_first() else { break };
                rest = more;
                match children[*first as usize].as_ref() {
                    Some(c) => c,
                    None => break,
                }
            }
        };
        let enc = encode(next);
        if enc.len() >= 32 {
            proof.push(enc);
        }
        node = next;
    }
    Some((root_hash, proof))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_built_trie_proves_each_leaf_and_nothing_else() {
        assert_eq!(zikaron::hexfmt::encode(&root_of(&[])), "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421");
        for n in [1usize, 2, 17, 130] {
            let values: Vec<Vec<u8>> = (0..n).map(|i| format!("value number {i} of a block's trie").into_bytes()).collect();
            for at in [0, n / 2, n - 1] {
                let (root, proof) = root_and_proof(&values, at).expect("built");
                assert_eq!(crate::ahead::verify(&root, &index_key(at as u64), &proof), Ok(Some(values[at].clone())), "{n} leaves, at {at}");
                if n > 1 {
                    let other = (at + 1) % n;
                    assert_ne!(crate::ahead::verify(&root, &index_key(other as u64), &proof), Ok(Some(values[other].clone())), "another index is not proved by this proof");
                }
                let mut bad = proof.clone();
                let last = bad.len() - 1;
                let mid = bad[last].len() / 2;
                bad[last][mid] ^= 1;
                assert_ne!(crate::ahead::verify(&root, &index_key(at as u64), &bad), Ok(Some(values[at].clone())), "a node changed");
                let mut other_root = root;
                other_root[0] ^= 1;
                assert!(crate::ahead::verify(&other_root, &index_key(at as u64), &proof).is_err(), "another root");
            }
        }
    }
}
