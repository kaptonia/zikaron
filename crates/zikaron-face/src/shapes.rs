use crate::names;
use zikaron::hexfmt;
use zikaron::json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shape {
    Home,
    Account,
    Key,
    Persona,
    Release,
    Reroot,
    Origin,
}

impl Shape {
    pub const ALL: [Shape; 7] = [Shape::Home, Shape::Account, Shape::Key, Shape::Persona, Shape::Release, Shape::Reroot, Shape::Origin];

    pub fn as_str(self) -> &'static str {
        match self {
            Shape::Home => "home",
            Shape::Account => "account",
            Shape::Key => "key",
            Shape::Persona => "persona",
            Shape::Release => "release",
            Shape::Reroot => "reroot",
            Shape::Origin => "origin",
        }
    }

    pub fn other_root(self) -> Option<&'static str> {
        match self {
            Shape::Reroot => Some("to"),
            Shape::Origin => Some("from"),
            _ => None,
        }
    }
}

type Test = fn(&Value) -> bool;

fn str_of(v: &Value) -> Option<&str> {
    v.as_str()
}

fn hex20(v: &Value) -> bool {
    str_of(v).is_some_and(hexfmt::is_hex20)
}

fn hex32(v: &Value) -> bool {
    str_of(v).is_some_and(hexfmt::is_hex32)
}

fn hex65(v: &Value) -> bool {
    str_of(v).is_some_and(hexfmt::is_hex65)
}

fn int(v: &Value) -> bool {
    matches!(v, Value::Int(_))
}

fn string(v: &Value) -> bool {
    matches!(v, Value::Str(_))
}

fn label(v: &Value) -> bool {
    str_of(v).is_some_and(names::is_label)
}

fn handle(v: &Value) -> bool {
    str_of(v).is_some_and(names::is_handle)
}

fn name(v: &Value) -> bool {
    str_of(v).is_some_and(names::is_name)
}

fn act_home(v: &Value) -> bool {
    str_of(v) == Some("home")
}

fn act_bind(v: &Value) -> bool {
    str_of(v) == Some("bind")
}

fn act_release(v: &Value) -> bool {
    str_of(v) == Some("release")
}

fn act_reroot(v: &Value) -> bool {
    str_of(v) == Some("reroot")
}

fn act_origin(v: &Value) -> bool {
    str_of(v) == Some("origin")
}

fn kind_account(v: &Value) -> bool {
    str_of(v) == Some("account")
}

fn kind_key(v: &Value) -> bool {
    str_of(v) == Some("key")
}

fn kind_persona(v: &Value) -> bool {
    str_of(v) == Some("persona")
}

fn row(shape: Shape) -> (&'static [(&'static str, Test)], &'static [(&'static str, Test)]) {
    const NOTE: &[(&str, Test)] = &[("note_md", string)];
    match shape {
        Shape::Home => (&[("act", act_home), ("root", hex20), ("chain", int)], &[]),
        Shape::Account => (
            &[("act", act_bind), ("root", hex20), ("kind", kind_account), ("platform", label), ("handle", handle)],
            NOTE,
        ),
        Shape::Key => (
            &[
                ("act", act_bind),
                ("root", hex20),
                ("kind", kind_key),
                ("address", hex20),
                ("role", label),
                ("sig", hex65),
            ],
            NOTE,
        ),
        Shape::Persona => (&[("act", act_bind), ("root", hex20), ("kind", kind_persona), ("name_md", name)], &[]),
        Shape::Release => (&[("act", act_release), ("root", hex20), ("subject", hex32)], NOTE),
        Shape::Reroot => (&[("act", act_reroot), ("root", hex20), ("to", hex20)], NOTE),
        Shape::Origin => (&[("act", act_origin), ("root", hex20), ("from", hex20)], NOTE),
    }
}

fn has(members: &[(String, Value)], shape: Shape) -> bool {
    let (required, optional) = row(shape);
    let test_of = |k: &str| required.iter().chain(optional.iter()).find(|(n, _)| *n == k).map(|(_, t)| *t);
    let all_known = members.iter().all(|(k, v)| test_of(k).is_some_and(|t| t(v)));
    let all_there = required.iter().all(|(k, _)| members.iter().any(|(m, _)| m == k));
    all_known && all_there
}

pub fn shape_of(body: &Value) -> Option<Shape> {
    let Value::Obj(members) = body else { return None };
    let shape = Shape::ALL.into_iter().find(|&s| has(members, s))?;
    match shape.other_root() {
        Some(m) if text(body, m) == text(body, "root") => None,
        _ => Some(shape),
    }
}

pub fn text<'a>(body: &'a Value, key: &str) -> &'a str {
    body.member(key).and_then(Value::as_str).unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Facet {
    Account { platform: String, handle: String },
    Key { address: String, role: String },
    Persona { name: String },
}

pub fn facet_of(shape: Shape, body: &Value) -> Option<Facet> {
    let t = |k| text(body, k).to_string();
    match shape {
        Shape::Account => Some(Facet::Account { platform: t("platform"), handle: t("handle") }),
        Shape::Key => Some(Facet::Key { address: t("address"), role: t("role") }),
        Shape::Persona => Some(Facet::Persona { name: t("name_md") }),
        Shape::Home | Shape::Release | Shape::Reroot | Shape::Origin => None,
    }
}
