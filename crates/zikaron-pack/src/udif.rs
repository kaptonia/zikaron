//! A disk image (UDIF) records a name for each partition block in its property list, written in the language
//! of the machine that made it (on a Chinese system, a Chinese word and full-width brackets around the
//! partition kind). The names are rewritten in one neutral form (`disk image (Apple_HFS : 0)`): the partition
//! kind and number stay, the words of the builder's language go. The property list sits between the data and the 512-byte trailer; its length in the trailer is
//! the only other field that changes (the checksums cover the data, not the names).

const TRAILER: usize = 512;
const XML_OFFSET: usize = 0xD8;
const XML_LENGTH: usize = 0xE0;

/// Rewrite the partition names of the UDIF image `img`. Returns the image and how many names changed.
pub fn neutral_names(img: &[u8]) -> Result<(Vec<u8>, usize), String> {
    if img.len() < TRAILER {
        return Err("too short to be a disk image".into());
    }
    let koly = &img[img.len() - TRAILER..];
    if &koly[0..4] != b"koly" {
        return Err("no UDIF trailer".into());
    }
    let be = |o: usize| u64::from_be_bytes(koly[o..o + 8].try_into().unwrap_or([0; 8])) as usize;
    let (xo, xl) = (be(XML_OFFSET), be(XML_LENGTH));
    if xo + xl != img.len() - TRAILER {
        return Err("the property list is not the last thing before the trailer".into());
    }
    let xml = std::str::from_utf8(&img[xo..xo + xl]).map_err(|_| "the property list is not UTF-8".to_string())?;
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    let mut changed = 0;
    loop {
        // Each name is the string after a `Name` or `CFName` key.
        let next = ["<key>Name</key>", "<key>CFName</key>"].iter().filter_map(|k| rest.find(k).map(|i| (i, k.len()))).min();
        let Some((i, kl)) = next else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..i + kl]);
        rest = &rest[i + kl..];
        let open = rest.find("<string>").ok_or("a name key without a string")?;
        let close = rest.find("</string>").ok_or("an unterminated name")?;
        out.push_str(&rest[..open + "<string>".len()]);
        let name = &rest[open + "<string>".len()..close];
        let neutral = neutral_name(name);
        if neutral != name {
            changed += 1;
        }
        out.push_str(&neutral);
        rest = &rest[close..];
    }
    let mut img2 = img[..xo].to_vec();
    img2.extend_from_slice(out.as_bytes());
    let mut k = koly.to_vec();
    k[XML_LENGTH..XML_LENGTH + 8].copy_from_slice(&(out.len() as u64).to_be_bytes());
    img2.extend_from_slice(&k);
    Ok((img2, changed))
}

/// The local words and brackets around `Apple_HFS` and its number → `disk image (Apple_HFS : 0)`; a name
/// with no `Apple_` kind keeps only its ASCII letters, digits and spaces.
fn neutral_name(name: &str) -> String {
    let Some(at) = name.find("Apple_") else {
        return name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '_').collect::<String>().trim().to_string();
    };
    let kind: String = name[at..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    let after = &name[at + kind.len()..];
    let num: String = after.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    let head = if kind == "Apple_Free" { "" } else { "disk image " };
    if num.is_empty() {
        format!("{head}({kind})")
    } else {
        format!("{head}({kind} : {num})")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_lose_the_builders_language() {
        assert_eq!(super::neutral_name("整个磁盘（Apple_HFS：0）"), "disk image (Apple_HFS : 0)");
        assert_eq!(super::neutral_name("disk image（Apple_HFS：4）"), "disk image (Apple_HFS : 4)");
        assert_eq!(super::neutral_name("（Apple_Free：3）"), "(Apple_Free : 3)");
    }
}
