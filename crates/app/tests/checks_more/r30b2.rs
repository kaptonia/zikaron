use super::a8_send::{bench, chain, node, tidy, Node, PLENTY};
use app::action::{apply, Action, Applied};
use app::task::{Done, Kind};
use std::time::{Duration, Instant};

fn slow(ms: u64) -> Node {
    node(move |m, b| {
        if m == "eth_blockNumber" {
            std::thread::sleep(Duration::from_millis(ms));
        }
        chain(m, b, PLENTY)
    })
}

#[test]
fn a_reading_over_a_replaced_node_table_lands_nothing() {
    if super::alone_in(module_path!(), "a_reading_over_a_replaced_node_table_lands_nothing") {
        return;
    }
    let (a, b) = (slow(800), slow(800));
    let fast = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30b2-renoded", &[&fast]);
    let specs = |ns: &[&Node]| ns.iter().map(|n| format!("31337={}", n.url)).collect::<Vec<_>>().join(" ");
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: specs(&[&a, &b]) }), Applied::Endpoints(2)));
    assert!(shell.tasks.in_flight(Kind::Chain), "the slow table is being read");
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: specs(&[&fast]) }), Applied::Endpoints(1)));
    assert!(shell.tasks.in_flight(Kind::Chain), "the new table is read at once");
    let until = Instant::now() + Duration::from_secs(30);
    let mut seen: Vec<(usize, bool)> = Vec::new();
    while Instant::now() < until && seen.len() < 2 {
        for o in shell.drain().into_iter().filter(|o| o.kind == Kind::Chain) {
            let n = match &o.result {
                Ok(Done::Chain { sources, .. }) => *sources,
                _ => 0,
            };
            seen.push((n, o.renoded));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(seen, vec![(1, false), (2, true)], "the new table's reading lands first, the old one after and dropped");
    assert!(matches!(shell.chain, Some(Done::Chain { sources: 1, .. })), "the reading held is the table's as it is: {:?}", shell.chain);
    assert!(!shell.tasks.in_flight(Kind::Chain), "nothing left in flight");
    tidy();
}

#[test]
fn a_reading_for_a_key_change_is_no_steps_transaction() {
    if super::alone_in(module_path!(), "a_reading_for_a_key_change_is_no_steps_transaction") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30b2-waits", &[&n]);
    let looked = apply(&mut shell, Action::Rotate { step: app::action::RotateStep::Look });
    assert_eq!(looked, Applied::Started(Kind::Rotate), "{looked:?}");
    assert_eq!(shell.rot_sending, None, "a reading sends nothing");
    for s in [app::rotatex::Stage::Declare { chain: 31337 }, app::rotatex::Stage::Accede { chain: 31337 }, app::rotatex::Stage::Write] {
        assert!(!app::rotatex::step_waits(&s, shell.rot_sending), "{s:?}: no step waits while the chain is read");
    }
    shell.rot_sending = Some(31337);
    assert!(app::rotatex::step_waits(&app::rotatex::Stage::Declare { chain: 31337 }, shell.rot_sending));
    let until = Instant::now() + Duration::from_secs(60);
    while shell.tasks.in_flight(Kind::Rotate) && Instant::now() < until {
        shell.drain();
        std::thread::sleep(Duration::from_millis(5));
    }
    shell.drain();
    assert_eq!(shell.rot_sending, None, "the task landed: no step's transaction is out");
    tidy();
}

#[test]
fn the_writing_rules_go_through_the_door_by_their_own_names() {
    if super::alone_in(module_path!(), "the_writing_rules_go_through_the_door_by_their_own_names") {
        return;
    }
    use zikaron_glue::door::{Reply as Door, Rule};
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30b2-rules", &[&n]);
    let id = app::ledgerx::table(shell.home.as_ref().expect("a home"), None, &[]).expect("the table").rows.iter().find(|r| r.kind == zikaron_glue::book::Kind::History).map(|r| r.id.clone()).expect("a record");
    let refused = |a: Applied| match a {
        Applied::Trouble(f) => app::door::refused(&f),
        other => panic!("refused: {other:?}"),
    };
    let first = apply(&mut shell, Action::Retract { subject: id.clone(), note_md: "删".into() });
    assert!(!matches!(first, Applied::Trouble(_)), "{first:?}");
    let again = refused(apply(&mut shell, Action::Retract { subject: id.clone(), note_md: "再删".into() }));
    assert!(matches!(&again, Door::RuleRefused { rule: Rule::Retraction, token: Some(t), entry_id: None, names: None, .. } if t == "E_RETRACT_REPEATED"), "{again:?}");
    let absent = format!("0x{}", "ab".repeat(32));
    let noted = refused(apply(&mut shell, Action::Annotate { subject: absent.clone(), note_md: "注".into() }));
    assert!(matches!(&noted, Door::RuleRefused { rule: Rule::Annotation, token: Some(t), entry_id: Some(e), .. } if t == "E_NAME_NOT_IN_LEDGER" && *e == absent), "{noted:?}");
    let revoked = refused(apply(&mut shell, Action::Revoke { grant: absent.clone(), case: String::new() }));
    assert!(matches!(&revoked, Door::RuleRefused { rule: Rule::Revocation, token: Some(t), entry_id: Some(e), .. } if t == "E_REVOKE_NOT_IN_LEDGER" && *e == absent), "{revoked:?}");
    app::lang::set(app::lang::Lang::Zh);
    match refused(apply(&mut shell, Action::Annotate { subject: String::new(), note_md: String::new() })) {
        Door::Refused { code, tail, .. } => {
            assert_eq!(code, "ENTRY_REFUSED");
            let (_, en) = app::lang::both(app::lang::Key::Tail019);
            assert_eq!(tail, en, "the evidence in the command line's machine words, whatever the desktop speaks");
        }
        other => panic!("{other:?}"),
    }
    tidy();
}

#[test]
fn a_desktop_refusal_says_its_evidence_in_each_language_and_the_output_keeps_one() {
    if super::alone_in(module_path!(), "a_desktop_refusal_says_its_evidence_in_each_language_and_the_output_keeps_one") {
        return;
    }
    use zikaron_glue::door::Reply as Door;
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30b2-y3", &[&n]);
    let (tail_zh, tail_en) = app::lang::both(app::lang::Key::Tail019);
    for speaks in [app::lang::Lang::Zh, app::lang::Lang::En] {
        app::lang::set(speaks);
        match apply(&mut shell, Action::Annotate { subject: String::new(), note_md: String::new() }) {
            Applied::Trouble(f) => match app::door::refused(&f) {
                Door::Refused { code, tail, zh, en, .. } => {
                    assert_eq!(code, "ENTRY_REFUSED");
                    assert_eq!(tail, tail_en, "{speaks:?}: the output's evidence in the command line's machine words");
                    assert!(zh.ends_with(tail_zh), "{speaks:?}: the Chinese line ends with the evidence in Chinese: {zh}");
                    assert!(en.ends_with(tail_en), "{speaks:?}: the English line ends with the evidence in English: {en}");
                    let (what_zh, what_en) = app::lang::both(f.what_key().expect("a known refusal"));
                    assert!(zh.starts_with(what_zh.trim_end_matches('。')) && en.starts_with(what_en.trim_end_matches('.')), "the sentence first: {zh} / {en}");
                }
                other => panic!("{other:?}"),
            },
            other => panic!("refused: {other:?}"),
        }
    }
    tidy();
}

#[test]
fn a_faults_evidence_of_words_is_kept_in_both_languages() {
    use app::fault::{Fault, Known};
    use app::lang::{both_of, filln, Key, Lang};
    let f = Fault::known(Known::QueueShape, both_of(|| filln(Key::Tail200, &["/q"])));
    let (zh, en) = app::lang::both(Key::Tail200);
    assert_eq!(f.tail_in(Lang::Zh), zh.replace("{0}", "/q"));
    assert_eq!(f.tail_in(Lang::En), en.replace("{0}", "/q"));
    assert_eq!(f.tail(), f.tail_in(app::lang::lang()), "the tail is the one in the language spoken");
    let v = Fault::known(Known::QueueShape, "/q".to_string());
    assert_eq!((v.tail_in(Lang::Zh), v.tail_in(Lang::En)), ("/q", "/q"), "a value is the same in both");
    let before = app::lang::lang();
    let _ = both_of(|| app::lang::t(Key::Tail019));
    assert_eq!(app::lang::lang(), before);
}

#[test]
fn every_worded_evidence_is_made_in_both_languages() {
    let mut bare = Vec::new();
    let mut seen = 0;
    let mut stack = vec![super::src()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).expect("src reads").flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&p).expect("a source");
            for tail in tails(&text) {
                seen += 1;
                let worded = ["lang::", "filln(", "fill1(", "fill2(", "fill3(", "said_of(", "trouble_said(", "t(Key::"].iter().any(|w| tail.contains(w));
                if worded && !tail.starts_with("crate::lang::both_of(") {
                    bare.push(format!("{}: {tail}", p.display()));
                }
            }
        }
    }
    assert!(seen > 100, "the scan reads the faults ({seen})");
    assert!(bare.is_empty(), "evidence of words in one language only:\n{}", bare.join("\n"));
}

fn tails(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find("Fault::known(") {
        let open = from + at + "Fault::known(".len() - 1;
        from = open + 1;
        let (mut depth, mut i, mut args, mut start) = (0i32, open, Vec::new(), open + 1);
        while i < b.len() {
            match b[i] {
                b'"' => {
                    i += 1;
                    while i < b.len() && b[i] != b'"' {
                        if b[i] == b'\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                }
                b'\'' if i + 2 < b.len() && (b[i + 2] == b'\'' || (b[i + 1] == b'\\' && i + 3 < b.len() && b[i + 3] == b'\'')) => {
                    i += if b[i + 1] == b'\\' { 3 } else { 2 };
                }
                b'/' if b.get(i + 1) == Some(&b'/') => {
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                }
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        args.push(&text[start..i]);
                        break;
                    }
                }
                b',' if depth == 1 => {
                    args.push(&text[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
            i += 1;
        }
        if let Some(t) = args.get(1) {
            out.push(t.split_whitespace().collect::<Vec<_>>().join(" "));
        }
    }
    out
}
