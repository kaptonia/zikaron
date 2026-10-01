//! The audit input: the scan's output plus three fields gives the core's five inputs.
//!
//! `{basis, anchors, evidence}` comes from this crate's scan; `root`, `pile` and `unavailable` come from the
//! ledger side (the pile is the storage crate's strict read). Together they are the audit-input shape of
//! `HARNESS.md`, handed to the core for the report.
//!
//! This module only assembles; it never judges. Every audit conclusion comes from the core's report.

use zikaron::json::{canon_bytes, Value};

/// Assemble the scan fragment and the three ledger-side fields into an audit input.
///
/// A public entry point, so it emits a trace mark: a caller that only drives assembly would otherwise look
/// like it touched only the core.
///
/// `fragment` is the output of [`crate::scan::fragment`] (or a byte-identical recording); `pile` and
/// `unavailable` use the transport spelling of `HARNESS.md`.
pub fn assemble(fragment: &Value, root: &str, pile: &[String], unavailable: &[String]) -> Option<Value> {
    crate::seam();
    let anchors = fragment.member("anchors")?.clone();
    let basis = fragment.member("basis")?.clone();
    let evidence = fragment.member("evidence")?.clone();
    Some(Value::Obj(vec![
        ("anchors".into(), anchors),
        ("basis".into(), basis),
        ("evidence".into(), evidence),
        ("pile".into(), Value::Arr(pile.iter().map(|x| Value::Str(x.clone())).collect())),
        ("root".into(), Value::Str(root.to_string())),
        ("unavailable".into(), Value::Arr(unavailable.iter().map(|x| Value::Str(x.clone())).collect())),
    ]))
}

/// Hand to the core for the report. The only audit outlet: it takes the caller's raw file bytes and returns
/// the bytes the core writes.
///
/// It takes bytes because rewriting a parsed value first would launder what the core must refuse: a leading
/// zero in an ignored member, a lone surrogate, a BOM or non-UTF-8 would disappear before the core saw them.
/// The core reads the exact input; the transport reader of this crate serves only the scan side (node answers
/// are outside the §3 value domain).
pub fn audit(bytes: &[u8]) -> Vec<u8> {
    crate::seam();
    match zikaron::json::parse_tests_1_3(bytes) {
        Ok(v) => canon_bytes(&zikaron::audit::audit(&v)),
        Err(_) => canon_bytes(&zikaron::audit::no_label()),
    }
}
