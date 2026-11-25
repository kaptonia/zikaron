use crate::fault::{Fault, Known};
use crate::key::Address;
use zikaron::json::Value;

pub const KIND_HANDOVER: &str = "handover";
pub const KIND_ROTATION: &str = "keyrotation";

pub fn fill(kind: &str, effective: &str, statement_md: &str, now: u64) -> (String, String) {
    let effective = if effective.trim().is_empty() { now.to_string() } else { effective.to_string() };
    let statement = if statement_md.trim().is_empty() { kind_words(kind).unwrap_or("").to_string() } else { statement_md.to_string() };
    (effective, statement)
}

pub fn kind_words(kind: &str) -> Option<&'static str> {
    use crate::lang::{t, Key};
    match kind.trim() {
        KIND_ROTATION => Some(t(Key::SucceedKindRotation)),
        KIND_HANDOVER => Some(t(Key::SucceedKindHandover)),
        _ => None,
    }
}

pub fn succession_body(
    to: &str,
    kind: &str,
    effective: &str,
    statement_md: &str,
) -> Result<Value, Fault> {
    crate::trace::mark(crate::feature::Feature::W11);
    let need = |s: &str, what: &str| -> Result<String, Fault> {
        let t = s.trim();
        if t.is_empty() {
            return Err(Fault::known(Known::FieldMissing, what.to_string()));
        }
        Ok(t.to_string())
    };
    let eff: u64 = crate::fault::whole_within_ceiling(effective, crate::lang::Key::Tail217)?;
    Ok(Value::Obj(vec![
        ("effective".to_string(), Value::Int(eff)),
        ("kind".to_string(), Value::Str(need(kind, "kind")?)),
        ("statement_md".to_string(), Value::Str(need(statement_md, "statement_md")?)),
        ("to".to_string(), Value::Str(need(to, "to")?)),
    ]))
}

pub struct Sighting {
    pub to: Address,
    pub anchors: usize,
    pub asked: usize,
}

pub fn look_at(
    eps: &[crate::chainx::Endpoint],
    ground: &crate::auditx::Ground,
    to: &Address,
) -> Result<Sighting, Fault> {
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail218).to_string())));
    }
    let mut g = ground.clone();
    g.senders = vec![to.hex()];
    let scanned = crate::auditx::scan_once(eps, &g)?;
    Ok(Sighting { to: *to, anchors: scanned.anchors, asked: scanned.asked })
}

pub fn seated_here(reg: &crate::identity::Registry, to: &Address) -> Result<(), Fault> {
    for row in &reg.rows {
        for seat in crate::roles::Role::ALL {
            let Some(o) = row.office(seat) else { continue };
            let held = row.address(seat) == Some(*to) || o.current == to.0 || o.former.contains(&to.0) || o.next == Some(to.0);
            if held {
                return Err(taken(Fault::known(Known::AlreadyRooted, to.hex()), crate::lang::Key::FaultWhatGivenSeatedHere));
            }
        }
    }
    Ok(())
}

pub fn seated_in_book(eps: &[crate::chainx::Endpoint], chain: u64, to: &Address) -> Result<(), Fault> {
    let crate::chainx::book::State::At(at) = crate::chainx::book::state(eps, chain)? else { return Ok(()) };
    let root = crate::chainx::book::address(eps, chain, &at, zikaron_anchor::book::Call::Root, &to.0)?;
    if root != to.0 || crate::chainx::book::entered(eps, chain, &at, &to.0)? {
        return Err(taken(Fault::known(Known::AlreadyRooted, to.hex()), crate::lang::Key::FaultWhatGivenInBook));
    }
    Ok(())
}

pub fn taken(f: Fault, what: crate::lang::Key) -> Fault {
    f.worded(Some(what), Some(crate::lang::Key::FaultNextGivenTaken))
}

pub fn handed_over(items: &[Vec<u8>], mine: Option<Address>) -> Option<String> {
    let me = mine?.hex();
    let read = zikaron_glue::book::read(items, &zikaron_glue::book::empty_fragment()).ok()?;
    let wrote = read.successions().any(|e| e.author.eq_ignore_ascii_case(&me) && e.to().is_some_and(|to| !to.eq_ignore_ascii_case(&me)));
    if !wrote {
        return None;
    }
    read.office.filter(|o| !o.eq_ignore_ascii_case(&me))
}
