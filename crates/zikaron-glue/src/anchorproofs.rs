use std::path::{Path, PathBuf};
use zikaron::json::Value;
use zikaron_anchor::proofs::AnchorProof;

pub const FORM: &str = "zikaron.anchor-proofs/1";

pub const BESIDE: &str = ".proofs";

pub fn beside(package: &Path) -> PathBuf {
    let name = package.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    package.with_file_name(format!("{name}{BESIDE}"))
}

pub fn link_beside(link: &str) -> String {
    format!("{}{BESIDE}", link.trim_end_matches('/'))
}

pub fn file_of(hash: &[u8; 32]) -> String {
    format!("{}.json", zikaron::hexfmt::encode(hash))
}

pub fn link_of(link: &str, hash: &[u8; 32]) -> String {
    format!("{}/{}", link_beside(link), file_of(hash))
}

pub fn bytes_of(root: &[u8; 20], proofs: &[AnchorProof]) -> Vec<u8> {
    let mut rows: Vec<&AnchorProof> = proofs.iter().collect();
    rows.sort_by_key(|p| (p.chain_id, p.tx_hash(), p.hash));
    rows.dedup_by_key(|p| (p.chain_id, p.tx_hash(), p.hash));
    let v = Value::Obj(vec![
        ("form".into(), Value::Str(FORM.into())),
        ("proofs".into(), Value::Arr(rows.into_iter().map(AnchorProof::to_value).collect())),
        ("root".into(), Value::Str(zikaron::hexfmt::encode(root))),
    ]);
    zikaron::json::canon_bytes(&v)
}

pub fn read(bytes: &[u8]) -> Option<([u8; 20], Vec<AnchorProof>)> {
    let v = zikaron::json::parse(bytes).ok()?;
    let Value::Obj(ms) = &v else { return None };
    let names: Vec<&str> = ms.iter().map(|(n, _)| n.as_str()).collect();
    if names != ["form", "proofs", "root"] || v.member("form").and_then(Value::as_str) != Some(FORM) {
        return None;
    }
    let root: [u8; 20] = v.member("root").and_then(Value::as_str).and_then(zikaron::hexfmt::decode)?.try_into().ok()?;
    let rows = match v.member("proofs")? {
        Value::Arr(rows) => rows.iter().filter_map(AnchorProof::of_value).collect(),
        _ => return None,
    };
    Some((root, rows))
}

pub fn of_hash(bytes: &[u8], hash: &[u8; 32], root: Option<&[u8; 20]>) -> Option<Vec<AnchorProof>> {
    match read(bytes) {
        Some((r, rows)) if root.is_none_or(|want| *want == r) && rows.iter().all(|p| p.hash == *hash) => Some(rows),
        _ => None,
    }
}

pub fn beside_of(package: &Path, hash: &[u8; 32], root: Option<&[u8; 20]>) -> Option<Vec<AnchorProof>> {
    of_hash(&std::fs::read(beside(package).join(file_of(hash))).ok()?, hash, root)
}

pub fn all_beside(package: &Path) -> Vec<AnchorProof> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(beside(package)).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(hash) = name.strip_suffix(".json").and_then(zikaron::hexfmt::decode).and_then(|b| <[u8; 32]>::try_from(b).ok()) else { continue };
        if file_of(&hash) != name {
            continue;
        }
        if let Some(rows) = std::fs::read(e.path()).ok().and_then(|b| of_hash(&b, &hash, None)) {
            out.extend(rows);
        }
    }
    out.sort_by_key(|p| (p.chain_id, p.tx_hash(), p.hash));
    out
}

pub fn offer_beside(package: &Path) {
    let package = package.to_path_buf();
    zikaron_anchor::ahead::offer_by_hash(move |hash| beside_of(&package, hash, None));
}

pub fn write_beside(package: &Path, root: &[u8; 20], proofs: &[AnchorProof]) -> Result<(), crate::landing::Trouble> {
    let dir = beside(package);
    if dir.is_file() {
        let _ = std::fs::remove_file(&dir);
    }
    crate::landing::mkdir(&dir)?;
    let mut by_hash: std::collections::BTreeMap<[u8; 32], Vec<AnchorProof>> = std::collections::BTreeMap::new();
    for p in proofs {
        by_hash.entry(p.hash).or_default().push(p.clone());
    }
    for (hash, rows) in by_hash {
        crate::landing::land_over(crate::landing::Readers::Anyone, &dir.join(file_of(&hash)), &bytes_of(root, &rows))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_beside_a_package_holds_one_document_per_hash_and_reads_back() {
        assert_eq!(beside(Path::new("/x/kits/kit-1")), PathBuf::from("/x/kits/kit-1.proofs"));
        assert_eq!(beside(Path::new("/x/kits/kit-1/")), PathBuf::from("/x/kits/kit-1.proofs"));
        assert_eq!(beside(Path::new("/x/out.zkit")), PathBuf::from("/x/out.zkit.proofs"));
        assert_eq!(link_beside("https://h.example/kits/kit-1/"), "https://h.example/kits/kit-1.proofs");
        assert_eq!(file_of(&[0xab; 32]), format!("0x{}.json", "ab".repeat(32)));
        let root = [0x11u8; 20];
        let doc = bytes_of(&root, &[]);
        assert_eq!(read(&doc), Some((root, Vec::new())));
        let other_form = String::from_utf8(doc.clone()).unwrap().replace(FORM, "zikaron.anchor-proofs/2");
        assert_eq!(read(other_form.as_bytes()), None);
        let more = String::from_utf8(doc.clone()).unwrap().replacen('{', r#"{"extra":1,"#, 1);
        assert_eq!(read(more.as_bytes()), None);
        let bad_row = String::from_utf8(doc.clone()).unwrap().replace(r#""proofs":[]"#, r#""proofs":[{"chainId":1}]"#);
        assert_eq!(read(bad_row.as_bytes()), Some((root, Vec::new())), "a row that does not read is passed over");
        let dir = std::env::temp_dir().join(format!("zikaron-proofs-beside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let pkg = dir.join("kit-1");
        let _ = crate::landing::land_over(crate::landing::Readers::Anyone, &beside(&pkg), &doc);
        write_beside(&pkg, &root, &[]).unwrap();
        assert!(beside(&pkg).is_dir(), "the folder");
        let (h, other) = ([0x5a; 32], [0x5b; 32]);
        let _ = crate::landing::land_over(crate::landing::Readers::Anyone, &beside(&pkg).join(file_of(&h)), &doc);
        assert_eq!(beside_of(&pkg, &h, Some(&root)), Some(Vec::new()));
        assert_eq!(beside_of(&pkg, &h, Some(&[0x22; 20])), None, "another ledger's: none");
        assert_eq!(beside_of(&pkg, &other, None), None, "none for that hash: none");
        assert_eq!(beside_of(&dir.join("kit-2"), &h, None), None, "none beside: none");
        assert!(all_beside(&pkg).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
