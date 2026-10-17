//! Where a record package says it is anchored: the fixed last line of its note (`note_md`), written by the
//! desktop's export and the command line's from a home alike, read back by verification to know which network to
//! ask.

/// Where an exported kit says it is anchored: its home's chain, registry and start block at export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchoredOn {
    pub chain_id: u64,
    pub from_block: u64,
    /// The registry contract, `0x` and forty lowercase hex digits.
    pub registry: String,
}

/// The line's fixed head and the separator between its three parts (space, U+00B7, space).
const LINE_HEAD: &str = "anchored-on: eip155:";
const LINE_SEP: &str = " \u{b7} ";

impl AnchoredOn {
    /// The fixed last line of a kit's `note_md`, the same in every language:
    /// `anchored-on: eip155:<chain> · registry <address> · from <block>`.
    pub fn line(&self) -> String {
        format!("{LINE_HEAD}{}{LINE_SEP}registry {}{LINE_SEP}from {}", self.chain_id, self.registry, self.from_block)
    }

    /// Parse a line written by [`AnchoredOn::line`], exactly: decimal numbers without leading zeros (each within
    /// the canonical JSON integer limit, so the index row reads back) and a lowercase address. Anything else is
    /// `None`.
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

/// Split a manifest note into the author's text and the anchoring point, which only the last non-empty line
/// can carry, and only when it matches exactly. Lines split at `\n` (a preceding `\r` belongs to the break);
/// whitespace-only lines count as empty, so trailing blank lines neither hide the anchoring line nor survive
/// its removal. The inverse of [`note_with`].
pub fn split_note(note: &str) -> (String, Option<AnchoredOn>) {
    let mut at = note.len();
    for piece in note.split_inclusive('\n').collect::<Vec<_>>().into_iter().rev() {
        at -= piece.len();
        let line = piece.strip_suffix('\n').unwrap_or(piece);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() {
            continue;
        }
        // The author's text is everything before this line, its own last line break left out.
        let before = &note[..at];
        let author = before.strip_suffix('\n').map(|b| b.strip_suffix('\r').unwrap_or(b)).unwrap_or(before);
        return match AnchoredOn::of_line(line) {
            Some(a) => (author.to_string(), Some(a)),
            None => (note.to_string(), None),
        };
    }
    (note.to_string(), None)
}

/// The note a kit carries: the author's text, then the anchoring line when one is given. An existing
/// anchoring line is removed first, so exporting again never doubles it.
pub fn note_with(note: &str, at: Option<&AnchoredOn>) -> String {
    let author = split_note(note).0;
    match at {
        None => author,
        Some(a) if author.is_empty() => a.line(),
        Some(a) => format!("{author}\n{}", a.line()),
    }
}

