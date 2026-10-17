//! Handing a ledger between two places, the pen going with it: one place writes at any time.
//!
//! Form-neutral. What an entry is, what makes entries one line and how a place reads and writes its own are the
//! caller's ([`Form`]); this module is the handoff's rules:
//!
//! - **The pen goes with the ledger.** A place that hands its ledger over ([`give`]) no longer writes
//!   ([`may_write`] refuses, naming the handoff) until a handoff comes back to it ([`take`]). Handing over again
//!   while handed makes the same handoff again (same id, same entries: nothing was written since), so a handoff
//!   file lost on the way is made anew and never a second pen.
//! - **Only a follow-on is taken.** A place takes a handoff whose entries hold every entry the place has, and
//!   make one line; it writes only the entries it lacks. A place holding an entry the handoff lacks, or a handoff
//!   that is not one line, is refused by name and nothing is written.
//! - **A handed place takes back only its own handoff's return.** Each handoff names the handoff its giver took
//!   its pen by (`after`); a handed place takes only a handoff after the one it gave. Its own handoff fed back to
//!   it, or an older one, is refused: the pen is elsewhere.
//!
//! The pen of every place this machine has handed or taken is one line in a list ([`Pens`]), keyed by whatever
//! the caller names a ledger by. A place with no line holds its pen (it never handed it).

/// A handoff's id: sixteen random bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandId(pub [u8; 16]);

impl HandId {
    pub fn hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn of_hex(s: &str) -> Option<HandId> {
        if s.len() != 32 || !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return None;
        }
        let mut out = [0u8; 16];
        for (i, o) in out.iter_mut().enumerate() {
            *o = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
        }
        Some(HandId(out))
    }
}

/// A handoff's own note, carried with its entries: its id, and the handoff its giver took the pen by (`None` when
/// the giver always held it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Note {
    pub id: HandId,
    pub after: Option<HandId>,
}

/// The note's first line.
pub const NOTE_HEAD: &str = "handoff/1";

impl Note {
    /// `handoff/1\nid <32 hex>\nafter <32 hex or ->\n`
    pub fn bytes(&self) -> Vec<u8> {
        format!("{NOTE_HEAD}\nid {}\nafter {}\n", self.id.hex(), self.after.map(|a| a.hex()).unwrap_or_else(|| "-".into())).into_bytes()
    }

    /// Read a note exactly as [`Note::bytes`] writes it; anything else is `None`.
    pub fn of(bytes: &[u8]) -> Option<Note> {
        let text = std::str::from_utf8(bytes).ok()?;
        let mut lines = text.strip_suffix('\n')?.split('\n');
        if lines.next()? != NOTE_HEAD {
            return None;
        }
        let id = HandId::of_hex(lines.next()?.strip_prefix("id ")?)?;
        let after = match lines.next()?.strip_prefix("after ")? {
            "-" => None,
            h => Some(HandId::of_hex(h)?),
        };
        lines.next().is_none().then_some(Note { id, after })
    }
}

/// Where a place's pen is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pen {
    /// Here: the place writes. `took` is the handoff it last took its ledger by (`None`: it always held it).
    Holds { took: Option<HandId> },
    /// Handed over by the handoff `id`, whose note says `after`.
    Handed { id: HandId, after: Option<HandId> },
}

/// The pens of the places one machine has handed or taken, one line each:
/// `<key, 64 hex> took <id>` or `<key> handed <id> <after or ->`. Closed shape; no key twice.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pens(Vec<([u8; 32], Pen)>);

fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

impl Pens {
    /// Read the list; `None` when it is not in its shape.
    pub fn parse(bytes: &[u8]) -> Option<Pens> {
        let text = std::str::from_utf8(bytes).ok()?;
        if !text.is_empty() && !text.ends_with('\n') {
            return None;
        }
        let mut out: Vec<([u8; 32], Pen)> = Vec::new();
        for line in text.lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            let key = hex32(parts.first()?)?;
            let pen = match parts.as_slice() {
                [_, "took", id] => Pen::Holds { took: Some(HandId::of_hex(id)?) },
                [_, "handed", id, "-"] => Pen::Handed { id: HandId::of_hex(id)?, after: None },
                [_, "handed", id, after] => Pen::Handed { id: HandId::of_hex(id)?, after: Some(HandId::of_hex(after)?) },
                _ => return None,
            };
            if out.iter().any(|(k, _)| *k == key) {
                return None;
            }
            out.push((key, pen));
        }
        Some(Pens(out))
    }

    /// The list's bytes.
    pub fn render(&self) -> Vec<u8> {
        let hex = |k: &[u8; 32]| k.iter().map(|b| format!("{b:02x}")).collect::<String>();
        self.0
            .iter()
            .map(|(k, p)| match p {
                Pen::Holds { took: Some(t) } => format!("{} took {}\n", hex(k), t.hex()),
                Pen::Holds { took: None } => String::new(),
                Pen::Handed { id, after } => format!("{} handed {} {}\n", hex(k), id.hex(), after.map(|a| a.hex()).unwrap_or_else(|| "-".into())),
            })
            .collect::<String>()
            .into_bytes()
    }

    /// The pen of the place keyed `key` (a place with no line holds it).
    pub fn pen(&self, key: &[u8; 32]) -> Pen {
        self.0.iter().find(|(k, _)| k == key).map(|(_, p)| *p).unwrap_or(Pen::Holds { took: None })
    }

    /// Set the pen of the place keyed `key`.
    pub fn set(&mut self, key: &[u8; 32], pen: Pen) {
        self.0.retain(|(k, _)| k != key);
        if pen != (Pen::Holds { took: None }) {
            self.0.push((*key, pen));
        }
    }
}

/// A ledger's form, as the caller has it: what identifies an entry, and whether entries make one line.
pub trait Form {
    type Entry;
    /// The entry's identity.
    fn id(&self, e: &Self::Entry) -> [u8; 32];
    /// Whether these entries, all of them, make one line.
    fn one_line(&self, all: &[Self::Entry]) -> bool;
}

/// Why a handoff step is refused. Closed set; nothing was written in any of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// This place handed its ledger over (by the handoff named), and has not taken it back: it does not write.
    Handed(HandId),
    /// This place handed its ledger over by `handed`, and this handoff is not after it: not its ledger coming back.
    NotBack { handed: HandId },
    /// This place holds an entry (named) the handoff does not: taking it would lose or fork that entry.
    Ahead([u8; 32]),
    /// The handoff's entries do not make one line.
    NotOneLine,
}

/// Whether a place whose pen is `pen` may write.
pub fn may_write(pen: &Pen) -> Result<(), Refusal> {
    match pen {
        Pen::Holds { .. } => Ok(()),
        Pen::Handed { id, .. } => Err(Refusal::Handed(*id)),
    }
}

/// Hand a place's ledger over: the handoff's note and the pen the place is left with (handed). `fresh` is the id a
/// new handoff takes. A place already handed makes the same handoff again (same note), never a new one.
pub fn give(pen: &Pen, fresh: HandId) -> (Note, Pen) {
    match *pen {
        Pen::Handed { id, after } => (Note { id, after }, *pen),
        Pen::Holds { took } => {
            let note = Note { id: fresh, after: took };
            (note, Pen::Handed { id: note.id, after: note.after })
        }
    }
}

/// Take a handoff into a place whose pen is `pen` and whose entries are `mine`: the handoff's entries (`theirs`, in
/// the order to write them) that the place lacks, and the pen it then holds. Judged in this order: a handed place
/// takes only its own handoff's return; every entry here must be in the handoff; the handoff must be one line.
pub fn take<'a, F: Form>(form: &F, pen: &Pen, mine: &[F::Entry], note: &Note, theirs: &'a [F::Entry]) -> Result<(Vec<&'a F::Entry>, Pen), Refusal> {
    if let Pen::Handed { id, .. } = pen {
        if note.after != Some(*id) {
            return Err(Refusal::NotBack { handed: *id });
        }
    }
    let ids: Vec<[u8; 32]> = theirs.iter().map(|e| form.id(e)).collect();
    if let Some(e) = mine.iter().find(|e| !ids.contains(&form.id(e))) {
        return Err(Refusal::Ahead(form.id(e)));
    }
    if !form.one_line(theirs) {
        return Err(Refusal::NotOneLine);
    }
    let have: Vec<[u8; 32]> = mine.iter().map(|e| form.id(e)).collect();
    let new: Vec<&F::Entry> = theirs.iter().filter(|e| !have.contains(&form.id(e))).collect();
    Ok((new, Pen::Holds { took: Some(note.id) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ledger of numbered entries: entry `n` follows `n - 1`; a pair `(n, fork)` with `fork != 0` is a second
    /// entry at `n`.
    struct Line;
    impl Form for Line {
        type Entry = (u8, u8);
        fn id(&self, e: &(u8, u8)) -> [u8; 32] {
            let mut x = [0u8; 32];
            x[0] = e.0;
            x[1] = e.1;
            x
        }
        fn one_line(&self, all: &[(u8, u8)]) -> bool {
            let mut seqs: Vec<u8> = all.iter().map(|e| e.0).collect();
            seqs.sort();
            seqs.windows(2).all(|w| w[0] != w[1])
        }
    }

    fn id(n: u8) -> HandId {
        HandId([n; 16])
    }

    #[test]
    fn the_pen_goes_with_the_ledger_and_comes_back_only_by_its_return() {
        // A holds 0..=2, hands over.
        let a_pen = Pen::Holds { took: None };
        assert_eq!(may_write(&a_pen), Ok(()));
        let (h1, a_pen) = give(&a_pen, id(1));
        assert_eq!(h1, Note { id: id(1), after: None });
        assert_eq!(may_write(&a_pen), Err(Refusal::Handed(id(1))), "交出即停写");
        assert_eq!(give(&a_pen, id(9)).0, h1, "交出后再交,是同一份");
        let a: Vec<(u8, u8)> = vec![(0, 0), (1, 0), (2, 0)];
        // B has 0..=1 and takes A's: it writes 2 only.
        let b: Vec<(u8, u8)> = vec![(0, 0), (1, 0)];
        let (new, b_pen) = take(&Line, &Pen::Holds { took: None }, &b, &h1, &a).ok().expect("顺接");
        assert_eq!(new, vec![&(2, 0)]);
        assert_eq!(b_pen, Pen::Holds { took: Some(id(1)) });
        // A's own handoff fed back to A: not its return.
        assert_eq!(take(&Line, &a_pen, &a, &h1, &a), Err(Refusal::NotBack { handed: id(1) }));
        // B writes 3 and hands back: the return names h1, A takes it.
        let b2: Vec<(u8, u8)> = vec![(0, 0), (1, 0), (2, 0), (3, 0)];
        let (h2, b_pen) = give(&b_pen, id(2));
        assert_eq!(h2.after, Some(id(1)));
        assert_eq!(may_write(&b_pen), Err(Refusal::Handed(id(2))));
        let (new, a_pen) = take(&Line, &a_pen, &a, &h2, &b2).ok().expect("收回");
        assert_eq!(new, vec![&(3, 0)]);
        assert_eq!(may_write(&a_pen), Ok(()), "收回顺接件即复写");
    }

    #[test]
    fn only_a_follow_on_is_taken() {
        let h = Note { id: id(5), after: None };
        let holds = Pen::Holds { took: None };
        // Here has an entry the handoff lacks.
        assert_eq!(take(&Line, &holds, &[(0, 0), (1, 0), (2, 0)], &h, &[(0, 0), (1, 0)]).err(), Some(Refusal::Ahead(Line.id(&(2, 0)))));
        // Two sides wrote: here 2a, the handoff 2b.
        assert_eq!(take(&Line, &holds, &[(0, 0), (1, 0), (2, 1)], &h, &[(0, 0), (1, 0), (2, 2)]).err(), Some(Refusal::Ahead(Line.id(&(2, 1)))));
        // The handoff itself forks.
        assert_eq!(take(&Line, &holds, &[(0, 0)], &h, &[(0, 0), (1, 0), (1, 1)]).err(), Some(Refusal::NotOneLine));
        // The same entries: nothing to write, the pen is taken.
        let (new, pen) = take(&Line, &holds, &[(0, 0)], &h, &[(0, 0)]).ok().expect("同");
        assert!(new.is_empty());
        assert_eq!(pen, Pen::Holds { took: Some(id(5)) });
        // An empty place takes everything.
        assert_eq!(take(&Line, &holds, &[], &h, &[(0, 0), (1, 0)]).ok().map(|(n, _)| n.len()), Some(2));
    }

    #[test]
    fn the_note_and_the_list_read_back_as_written_and_nothing_else() {
        for n in [Note { id: id(1), after: None }, Note { id: id(2), after: Some(id(3)) }] {
            assert_eq!(Note::of(&n.bytes()), Some(n));
        }
        for bad in ["handoff/1\nid 00\nafter -\n", "handoff/2\nid 01010101010101010101010101010101\nafter -\n", "handoff/1\nid 01010101010101010101010101010101\nafter -", "handoff/1\nid 01010101010101010101010101010101\nafter -\nx\n"] {
            assert_eq!(Note::of(bad.as_bytes()), None, "{bad:?}");
        }
        let mut p = Pens::default();
        p.set(&[1u8; 32], Pen::Handed { id: id(1), after: None });
        p.set(&[2u8; 32], Pen::Holds { took: Some(id(2)) });
        p.set(&[3u8; 32], Pen::Handed { id: id(3), after: Some(id(4)) });
        assert_eq!(Pens::parse(&p.render()), Some(p.clone()));
        assert_eq!(p.pen(&[9u8; 32]), Pen::Holds { took: None }, "无行即持笔");
        p.set(&[1u8; 32], Pen::Holds { took: None });
        assert_eq!(Pens::parse(&p.render()).map(|x| x.pen(&[1u8; 32])), Some(Pen::Holds { took: None }));
        let k = "ab".repeat(32);
        for bad in [format!("{k} took 00\n"), format!("{k} kept {}\n", id(1).hex()), format!("{k} took {0}\n{k} took {0}\n", id(1).hex()), format!("{k} took {}", id(1).hex())] {
            assert_eq!(Pens::parse(bad.as_bytes()), None, "{bad:?}");
        }
    }
}
