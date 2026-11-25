use crate::{Code, End, Trouble, Why};
use zikaron::json::Value;
use zikaron_seal::unfetched::State;

pub fn standing(mark: Option<State>, pile: &[Vec<u8>]) -> Option<State> {
    match mark {
        Some(State::NewerElsewhere { lacks, .. }) if !lacks.is_empty() => {
            let held: std::collections::BTreeSet<String> = pile.iter().map(|b| zikaron::hexfmt::encode(&zikaron_glue::book::entry_id(b)).trim_start_matches("0x").to_ascii_lowercase()).collect();
            let rest: Vec<String> = lacks.into_iter().filter(|h| !held.contains(h)).collect();
            (!rest.is_empty()).then(|| State::NewerElsewhere { missing: rest.len(), lacks: rest })
        }
        m => m,
    }
}

fn refusal(s: &State) -> Trouble {
    match s {
        State::Unfetched => Trouble::text(Code::NotFetched, ""),
        State::NewerElsewhere { missing, .. } => Trouble::text(Code::NewerElsewhere, missing.to_string()),
    }
}

pub fn check<E: End>(end: &E) -> Result<(), E::Fail> {
    let mark = end.mark()?;
    match &mark {
        Some(State::Unfetched) => return Err(Trouble::text(Code::NotFetched, "").into()),
        Some(State::NewerElsewhere { missing, lacks }) if lacks.is_empty() => return Err(Trouble::text(Code::NewerElsewhere, missing.to_string()).into()),
        _ => {}
    }
    let pile = end.pile()?;
    if let Some(m @ State::NewerElsewhere { .. }) = mark {
        match standing(Some(m), &pile) {
            Some(s) => return Err(refusal(&s).into()),
            None => end.lift()?,
        }
    }
    end.tail()?;
    let names = breaks(&pile);
    if !names.is_empty() {
        return Err(Trouble::new(Code::Broken, Why::Text(names.join(" "))).into());
    }
    let Some(root) = zikaron_glue::book::root_entry(&pile).map(|e| e.id) else { return Ok(()) };
    match zikaron_seal::pens::may_write(&end.machine()?, &root) {
        Err(t) if t.code == zikaron_seal::Code::HandedOff => Err(Trouble::text(Code::HandedOff, t.tail).into()),
        Err(t) => Err(t.into()),
        Ok(()) => Ok(()),
    }
}

pub fn lift<E: End>(end: &E) -> Result<(), E::Fail> {
    if let Some(State::NewerElsewhere { missing, lacks }) = end.mark()? {
        if !lacks.is_empty() && standing(Some(State::NewerElsewhere { missing, lacks }), &end.pile()?).is_none() {
            end.lift()?;
        }
    }
    Ok(())
}

pub fn breaks(pile: &[Vec<u8>]) -> Vec<String> {
    let Ok(read) = zikaron_glue::book::read(pile, &zikaron_glue::book::empty_fragment()) else { return Vec::new() };
    let mut names: Vec<String> = read
        .rows("findings")
        .iter()
        .filter(|f| matches!(f.member("hard"), Some(Value::Bool(true))))
        .filter_map(|f| f.member("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_naming_its_anchors_is_judged_against_the_ledger() {
        let (a, b) = (b"entry a".to_vec(), b"entry b".to_vec());
        let id = |x: &[u8]| zikaron::hexfmt::encode(&zikaron_glue::book::entry_id(x));
        let named = State::lacking(&[id(&a), id(&b)]);
        assert_eq!(named, State::NewerElsewhere { missing: 2, lacks: { let mut l = vec![id(&a)[2..].to_string(), id(&b)[2..].to_string()]; l.sort(); l } });
        assert_eq!(standing(Some(named.clone()), &[]), Some(named.clone()), "none here");
        assert_eq!(standing(Some(named.clone()), &[a.clone()]), Some(State::NewerElsewhere { missing: 1, lacks: vec![id(&b)[2..].to_string()] }), "one of two");
        assert_eq!(standing(Some(named), &[b.clone(), a.clone()]), None, "both here");
        let counted = State::NewerElsewhere { missing: 2, lacks: Vec::new() };
        assert_eq!(standing(Some(counted.clone()), &[a.clone(), b.clone()]), Some(counted), "a count alone is judged by a tail check only");
        assert_eq!(standing(Some(State::Unfetched), &[a, b]), Some(State::Unfetched));
        assert_eq!(standing(None, &[]), None);
    }
}
