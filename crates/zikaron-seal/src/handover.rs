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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Note {
    pub id: HandId,
    pub after: Option<HandId>,
}

pub const NOTE_HEAD: &str = "handoff/1";

impl Note {
    pub fn bytes(&self) -> Vec<u8> {
        format!("{NOTE_HEAD}\nid {}\nafter {}\n", self.id.hex(), self.after.map(|a| a.hex()).unwrap_or_else(|| "-".into())).into_bytes()
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pen {
    Holds { took: Option<HandId> },
    Handed { id: HandId, after: Option<HandId> },
}

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

    pub fn pen(&self, key: &[u8; 32]) -> Pen {
        self.0.iter().find(|(k, _)| k == key).map(|(_, p)| *p).unwrap_or(Pen::Holds { took: None })
    }

    pub fn set(&mut self, key: &[u8; 32], pen: Pen) {
        self.0.retain(|(k, _)| k != key);
        if pen != (Pen::Holds { took: None }) {
            self.0.push((*key, pen));
        }
    }
}

pub trait Form {
    type Entry;
    fn id(&self, e: &Self::Entry) -> [u8; 32];
    fn one_line(&self, all: &[Self::Entry]) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Handed(HandId),
    NotBack { handed: HandId },
    Ahead([u8; 32]),
    NotOneLine,
}

pub fn may_write(pen: &Pen) -> Result<(), Refusal> {
    match pen {
        Pen::Holds { .. } => Ok(()),
        Pen::Handed { id, .. } => Err(Refusal::Handed(*id)),
    }
}

pub fn give(pen: &Pen, fresh: HandId) -> (Note, Pen) {
    match *pen {
        Pen::Handed { id, after } => (Note { id, after }, *pen),
        Pen::Holds { took } => {
            let note = Note { id: fresh, after: took };
            (note, Pen::Handed { id: note.id, after: note.after })
        }
    }
}

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
        let a_pen = Pen::Holds { took: None };
        assert_eq!(may_write(&a_pen), Ok(()));
        let (h1, a_pen) = give(&a_pen, id(1));
        assert_eq!(h1, Note { id: id(1), after: None });
        assert_eq!(may_write(&a_pen), Err(Refusal::Handed(id(1))), "交出即停写");
        assert_eq!(give(&a_pen, id(9)).0, h1, "交出后再交,是同一份");
        let a: Vec<(u8, u8)> = vec![(0, 0), (1, 0), (2, 0)];
        let b: Vec<(u8, u8)> = vec![(0, 0), (1, 0)];
        let (new, b_pen) = take(&Line, &Pen::Holds { took: None }, &b, &h1, &a).ok().expect("顺接");
        assert_eq!(new, vec![&(2, 0)]);
        assert_eq!(b_pen, Pen::Holds { took: Some(id(1)) });
        assert_eq!(take(&Line, &a_pen, &a, &h1, &a), Err(Refusal::NotBack { handed: id(1) }));
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
        assert_eq!(take(&Line, &holds, &[(0, 0), (1, 0), (2, 0)], &h, &[(0, 0), (1, 0)]).err(), Some(Refusal::Ahead(Line.id(&(2, 0)))));
        assert_eq!(take(&Line, &holds, &[(0, 0), (1, 0), (2, 1)], &h, &[(0, 0), (1, 0), (2, 2)]).err(), Some(Refusal::Ahead(Line.id(&(2, 1)))));
        assert_eq!(take(&Line, &holds, &[(0, 0)], &h, &[(0, 0), (1, 0), (1, 1)]).err(), Some(Refusal::NotOneLine));
        let (new, pen) = take(&Line, &holds, &[(0, 0)], &h, &[(0, 0)]).ok().expect("同");
        assert!(new.is_empty());
        assert_eq!(pen, Pen::Holds { took: Some(id(5)) });
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
