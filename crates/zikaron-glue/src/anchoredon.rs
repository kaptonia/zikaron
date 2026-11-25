#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchoredOn {
    pub chain_id: u64,
    pub from_block: u64,
    pub registry: String,
}

const LINE_HEAD: &str = "anchored-on: eip155:";
const LINE_SEP: &str = " \u{b7} ";

impl AnchoredOn {
    pub fn line(&self) -> String {
        format!("{LINE_HEAD}{}{LINE_SEP}registry {}{LINE_SEP}from {}", self.chain_id, self.registry, self.from_block)
    }

    pub fn of_line(line: &str) -> Option<AnchoredOn> {
        let rest = line.strip_prefix(LINE_HEAD)?;
        let (chain, rest) = rest.split_once(LINE_SEP)?;
        let (registry, from) = rest.strip_prefix("registry ")?.split_once(LINE_SEP)?;
        let from = from.strip_prefix("from ")?;
        let decimal = |t: &str| -> Option<u64> {
            let ok = !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) && (t == "0" || !t.starts_with('0'));
            t.parse::<u64>().ok().filter(|n| ok && *n <= zikaron::json::MAX_INT)
        };
        let lower_hex = registry.len() == 42
            && registry.starts_with("0x")
            && registry[2..].bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !lower_hex {
            return None;
        }
        Some(AnchoredOn { chain_id: decimal(chain)?, from_block: decimal(from)?, registry: registry.to_string() })
    }
}

pub fn split_note(note: &str) -> (String, Option<AnchoredOn>) {
    let mut at = note.len();
    for piece in note.split_inclusive('\n').collect::<Vec<_>>().into_iter().rev() {
        at -= piece.len();
        let line = piece.strip_suffix('\n').unwrap_or(piece);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() {
            continue;
        }
        let before = &note[..at];
        let author = before.strip_suffix('\n').map(|b| b.strip_suffix('\r').unwrap_or(b)).unwrap_or(before);
        return match AnchoredOn::of_line(line) {
            Some(a) => (author.to_string(), Some(a)),
            None => (note.to_string(), None),
        };
    }
    (note.to_string(), None)
}

pub fn note_with(note: &str, at: Option<&AnchoredOn>) -> String {
    let author = split_note(note).0;
    match at {
        None => author,
        Some(a) if author.is_empty() => a.line(),
        Some(a) => format!("{author}\n{}", a.line()),
    }
}

pub fn of_manifest(manifest: &[u8]) -> (String, Option<AnchoredOn>) {
    use zikaron::json::Value;
    let Ok(v) = zikaron::json::parse(manifest) else { return (String::new(), None) };
    let note = match v.member(crate::names::Field::NoteMd.as_str()) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    };
    let old = matches!(v.member(crate::names::Field::Spec.as_str()), Some(Value::Str(s)) if s == zikaron_kit::tokens::SPEC_KIT);
    if old {
        return split_note(&note);
    }
    let at = v.member(crate::names::Field::AnchoredOn.as_str()).and_then(|a| {
        let int = |k: crate::names::Field| match a.member(k.as_str()) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        };
        let registry = match a.member(crate::names::Field::Registry.as_str()) {
            Some(Value::Str(r)) if zikaron::hexfmt::is_hex20(r) => r.to_ascii_lowercase(),
            _ => return None,
        };
        Some(AnchoredOn { chain_id: int(crate::names::Field::ChainId)?, from_block: int(crate::names::Field::FromBlock)?, registry })
    });
    (note, at)
}
