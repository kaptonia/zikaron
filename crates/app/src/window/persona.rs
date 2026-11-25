use super::*;
use crate::nav::tab as T;
use crate::nav::View;

#[derive(Clone, Debug, PartialEq)]
pub enum Does {
    Reread,
    Sheet(Sheet),
    Send,
    Reroot,
    Act(&'static str, Action),
    Copy(String),
    Next,
}

impl Does {
    pub fn said(&self) -> String {
        match self {
            Does::Reread => "reread".to_string(),
            Does::Sheet(s) => format!("sheet:{}", s.name()),
            Does::Send => "send".to_string(),
            Does::Reroot => "reroot".to_string(),
            Does::Act(n, _) => format!("act:{n}"),
            Does::Copy(_) => "copy".to_string(),
            Does::Next => "next".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sheet {
    Name,
    Account { platform: String, handle: String },
    Key { address: String, role: String },
    Release { subject: String, what: String },
    Reseat { entry: String, what: String },
    Home,
    Enter { chain: u64 },
}

impl Sheet {
    pub fn name(&self) -> &'static str {
        match self {
            Sheet::Name => "name",
            Sheet::Account { .. } => "account",
            Sheet::Key { .. } => "key",
            Sheet::Release { .. } => "release",
            Sheet::Reseat { .. } => "reseat",
            Sheet::Home => "home",
            Sheet::Enter { .. } => "enter",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Press {
    pub name: String,
    pub word: String,
    pub on: bool,
    pub why: String,
    pub does: Does,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    Mono(String),
    Quiet(String),
    Copy(String),
    Name { lay: String, copy: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub name: String,
    pub title: String,
    pub sub: String,
    pub value: Option<Value>,
    pub pills: Vec<(String, PillTone)>,
    pub key: Option<Press>,
    pub key2: Option<Press>,
}

impl Line {
    fn new(name: impl Into<String>, title: impl Into<String>) -> Line {
        Line { name: name.into(), title: title.into(), sub: String::new(), value: None, pills: Vec::new(), key: None, key2: None }
    }
    fn sub(mut self, s: impl Into<String>) -> Line {
        self.sub = s.into();
        self
    }
    fn value(mut self, v: Value) -> Line {
        self.value = Some(v);
        self
    }
    fn pill(mut self, w: impl Into<String>, tone: PillTone) -> Line {
        self.pills.push((w.into(), tone));
        self
    }
    fn key(mut self, p: Press) -> Line {
        self.key = Some(p);
        self
    }
    fn key2(mut self, p: Press) -> Line {
        self.key2 = Some(p);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub name: &'static str,
    pub title: String,
    pub head: Option<Press>,
    pub lines: Vec<Line>,
    pub empty: String,
    pub foot: String,
}

impl Block {
    fn new(name: &'static str, title: impl Into<String>) -> Block {
        Block { name, title: title.into(), head: None, lines: Vec::new(), empty: String::new(), foot: String::new() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub tab: u8,
    pub read: &'static str,
    pub blocks: Vec<Block>,
    pub reread: Press,
    pub sealed: bool,
    pub carry: Option<bool>,
}

struct Gate {
    on: bool,
    why: String,
}

fn press(name: impl Into<String>, word: &str, gate: &Gate, does: Does) -> Press {
    Press { name: name.into(), word: word.to_string(), on: gate.on, why: if gate.on { String::new() } else { gate.why.clone() }, does }
}

fn short(v: &str) -> String {
    card::short(v)
}

fn hex20(a: &[u8; 20]) -> String {
    zikaron::hexfmt::encode(a)
}

fn span_words(s: &zikaron_glue::face::Span) -> String {
    match &s.until {
        Some(u) => format!("{} \u{b7} {}", crate::when::when(s.since.time), crate::when::when(u.time)),
        None => crate::when::when(s.since.time),
    }
}

fn spans_words(r: &zikaron_glue::face::Row) -> String {
    r.spans.iter().map(span_words).collect::<Vec<_>>().join("; ")
}

fn open_span(r: &zikaron_glue::face::Row) -> Option<&zikaron_glue::face::Span> {
    r.spans.iter().rev().find(|s| s.release.is_none())
}

pub(super) fn chain_name(shell: &Shell, chain: u64) -> String {
    let typed = shell.read_nets.as_ref().and_then(|ns| ns.iter().find(|n| n.chain_id == chain).and_then(|n| n.name.clone()));
    crate::readnets::chain_name(chain, typed.as_deref())
}

fn misread_said(reason: &str) -> &'static str {
    t(match reason {
        "SHAPE" => Key::PsWhyShape,
        "ROOT" => Key::PsWhyRoot,
        "AUTHOR" => Key::PsWhyAuthor,
        "SIGNATURE" => Key::PsWhySignature,
        "SUBJECT" => Key::PsWhySubject,
        _ => Key::PsUnseated,
    })
}

fn pending<'a>(shell: &'a Shell, table: Option<&zikaron_glue::face::Table>) -> Vec<(&'a crate::ledgerx::Row, Option<bool>)> {
    let Some((rows, _)) = shell.rows.as_ref() else { return Vec::new() };
    let mut seated: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    if let Some(tb) = table {
        seated = zikaron_glue::facegate::seated_ids(tb);
        for (e, _) in &tb.misread {
            seated.insert(e.to_ascii_lowercase());
        }
    }
    rows.iter()
        .filter(|r| r.kind == zikaron_glue::book::Kind::Face && !seated.contains(&r.id.to_ascii_lowercase()))
        .map(|r| (r, shell.queue.items.iter().find(|q| q.id == r.id).map(|q| q.step.in_flight())))
        .collect()
}

fn pending_line(shell: &Shell, row: &crate::ledgerx::Row, queued: Option<bool>, gate: &Gate, send_gate: &Gate) -> Line {
    let chain_now = shell.chain_now();
    let line = Line::new(format!("pending:{}", row.id), human_summary(row, chain_now)).sub(format!("#{}", row.seq));
    let (word, tone) = pending_state(shell, row, queued);
    let line = line.pill(t(word), tone);
    match queued {
        Some(false) => line.key(press(format!("send:{}", row.id), t(Key::U3SendTitle), send_gate, Does::Send)),
        Some(true) => line,
        None if sent_here(shell, row) => line,
        None => line.key(press(format!("reseat:{}", row.id), t(Key::PsReseat), gate, Does::Sheet(Sheet::Reseat { entry: row.id.clone(), what: human_summary(row, chain_now) }))),
    }
}

fn sent_here(shell: &Shell, row: &crate::ledgerx::Row) -> bool {
    shell.queue.anchored.iter().any(|(x, _)| x.eq_ignore_ascii_case(&row.id))
}

fn pending_state(shell: &Shell, row: &crate::ledgerx::Row, queued: Option<bool>) -> (Key, PillTone) {
    match queued {
        Some(_) => (Key::ItemUnanchored, PillTone::Warn),
        None if sent_here(shell, row) => (Key::PsUnseated, PillTone::Grey),
        None => (Key::PsUnseated, PillTone::Warn),
    }
}

pub(super) fn row_named(shell: &Shell, id: &str) -> String {
    let chain_now = shell.chain_now();
    shell.rows.as_ref().and_then(|(rows, _)| rows.iter().find(|r| r.id.eq_ignore_ascii_case(id))).map(|x| human_summary(x, chain_now)).unwrap_or_else(|| short(id))
}

pub fn model(shell: &Shell, tab: u8) -> Model {
    let busy = shell.tasks.in_flight(crate::task::Kind::Persona) || shell.tasks.in_flight(crate::task::Kind::FaceBooks);
    let reread = Press { name: "reread".into(), word: t(Key::PsReread).to_string(), on: !busy && shell.home.is_some(), why: String::new(), does: Does::Reread };
    let (read, table, built) = match &shell.persona {
        crate::facex::State::Read(b) => ("read", Some(&b.table), Some(b.as_ref())),
        crate::facex::State::Unread => ("unread", None, None),
        crate::facex::State::NotRead(_) => ("notRead", None, None),
    };
    let anchor = shell.anchor.map(|a| a.hex());
    let rotating = table.is_some_and(|tb| anchor.as_deref().is_some_and(|a| !tb.office.eq_ignore_ascii_case(a)));
    let gate = match (table, shell.may_write_entries_shown()) {
        (None, _) => Gate { on: false, why: t(Key::PsWhyUnread).to_string() },
        (Some(_), _) if rotating => Gate { on: false, why: t(Key::PsWhyRotating).to_string() },
        (Some(tb), _) if super::reroot::waiting(shell, tb, "reroot").is_some() => Gate { on: false, why: t(Key::FaultWhatFaceSealed).to_string() },
        (Some(_), Err(f)) => Gate { on: false, why: f.human().to_string() },
        (Some(_), Ok(())) => Gate { on: true, why: String::new() },
    };
    let sealed = table.is_some_and(|tb| zikaron_glue::facegate::sealed(tb).is_err());
    let mut carry = None;
    let blocks = match (tab, table, built) {
        (_, None, _) => {
            let mut out = vec![not_read(shell)];
            if let Some(p) = shell.face_pair.as_ref().filter(|p| p.side == crate::facex::Side::New) {
                let mut b = Block::new("cameFrom", "");
                b.lines.push(Line::new("cameFrom", t(Key::PsReroot)).value(Value::Quiet(p.other.hex())).key(press("goOn", t(Key::Continue), &Gate { on: true, why: String::new() }, Does::Reroot)));
                out.push(b);
            }
            out
        }
        (_, Some(tb), _) if sealed => vec![moved(shell, tb)],
        (T::PERSONA_NAMES, Some(tb), _) => names(shell, tb, &gate, rotating),
        (T::PERSONA_HOME, Some(tb), _) => home_tab(shell, tb, &gate, rotating),
        (_, Some(tb), Some(b)) => {
            let mut out = overview(shell, tb, b, rotating);
            if let Some((block, done)) = carry_block(shell, tb, &gate) {
                carry = Some(done);
                out.push(block);
            }
            out
        }
        (_, Some(_), None) => Vec::new(),
    };
    Model { tab, read, blocks, reread, sealed, carry }
}

fn not_read(shell: &Shell) -> Block {
    not_read_of(shell, &shell.persona)
}

fn not_read_of(shell: &Shell, state: &crate::facex::State) -> Block {
    let mut b = Block::new("unread", "");
    match state {
        crate::facex::State::NotRead(m) => {
            b.lines.push(Line::new("notRead", crate::facex::missing_said(m)).pill(t(Key::PsNotReadPill), PillTone::Warn));
            let left = match m {
                crate::facex::Missing::NoChain(l) | crate::facex::Missing::NoTable(l) => l.clone(),
                crate::facex::Missing::NoRoot => Vec::new(),
            };
            for (chain, why) in &left {
                b.lines.push(Line::new(format!("left:{chain}"), chain_name(shell, *chain)).sub(crate::facex::left_said(why)).pill(t(Key::PsLeftPill), PillTone::Warn));
            }
        }
        _ => b.lines.push(Line::new("unread", t(Key::WbNotRead)).value(Value::Quiet(String::new()))),
    }
    b
}

fn overview(shell: &Shell, tb: &zikaron_glue::face::Table, b: &crate::facex::Built, rotating: bool) -> Vec<Block> {
    let mut head = Block::new("overview", "");
    let label = shell.identities.as_ref().and_then(|r| r.now().map(|(row, _)| super::identity::id_name(row))).unwrap_or_else(|| t(Key::Unnamed).to_string());
    head.lines.push(match &tb.persona {
        Some(p) => Line::new("persona", t(Key::FaceSummaryPersona)).value(Value::Name { lay: p.name.lay_alone(), copy: p.name.copy_name().to_string() }),
        None => Line::new("persona", t(Key::FaceSummaryPersona)).value(Value::Quiet(t(Key::PsNoName).to_string())),
    });
    head.lines.push(Line::new("label", t(Key::PsLabel)).value(Value::Text(label)));
    head.lines.push(Line::new("root", t(Key::IdRoot)).value(Value::Copy(tb.root.clone())));
    head.lines.push(Line::new("marker", t(Key::PsMarker)).value(Value::Copy(tb.marker.clone())));
    let generation = tb.lineage.iter().position(|l| l.key.eq_ignore_ascii_case(&tb.office)).map(|i| i + 1).unwrap_or(tb.lineage.len());
    let mut office = Line::new("office", t(Key::PsOffice)).value(Value::Mono(short(&tb.office))).pill(fill1(Key::PsGeneration, &generation.to_string()), PillTone::Grey);
    if rotating {
        office = office.pill(t(Key::PsRotating), PillTone::Warn);
    }
    head.lines.push(office);
    head.lines.push(match (home_left(shell), tb.home) {
        (Some(c), _) => Line::new("home", t(Key::PsTabHome)).value(Value::Text(chain_name(shell, c))).pill(t(Key::PsNotReadPill), PillTone::Warn),
        (None, Some(c)) => Line::new("home", t(Key::PsTabHome)).value(Value::Text(chain_name(shell, c))),
        (None, None) => Line::new("home", t(Key::PsTabHome)).value(Value::Quiet(t(Key::PsNoHome).to_string())),
    });
    let accounts = tb.faces.iter().filter(|r| r.bound && matches!(r.named, zikaron_glue::face::Named::Account { .. })).count();
    let keys = tb.faces.iter().filter(|r| r.bound && matches!(r.named, zikaron_glue::face::Named::Key { .. })).count();
    let mut faces = Line::new("faces", t(Key::PsFaces))
        .pill(fill1(Key::PsAccountsN, &accounts.to_string()), if accounts > 0 { PillTone::Ok } else { PillTone::Grey })
        .pill(fill1(Key::PsKeysN, &keys.to_string()), if keys > 0 { PillTone::Ok } else { PillTone::Grey });
    if !tb.misread.is_empty() {
        faces = faces.pill(fill1(Key::PsUnseatedN, &tb.misread.len().to_string()), PillTone::Warn);
    }
    head.lines.push(faces);
    head.lines.push(
        Line::new("readTo", t(Key::PsReadTo))
            .value(Value::Text(crate::when::when(b.moment)))
            .pill(fill1(Key::PsUnreadN, &tb.unread.to_string()), PillTone::Grey)
            .pill(fill1(Key::PsUnprovenN, &tb.unproven.to_string()), if tb.unproven > 0 { PillTone::Warn } else { PillTone::Grey }),
    );
    if let Some(h) = crate::facex::hint(tb) {
        head.lines.push(Line::new("unprovenHint", h).pill(t(Key::PsReread), PillTone::Warn));
    }
    for (chain, why) in &b.left {
        head.lines.push(Line::new(format!("left:{chain}"), chain_name(shell, *chain)).sub(crate::facex::left_said(why)).pill(t(Key::PsLeftPill), PillTone::Warn));
    }
    vec![head]
}

fn needs_home(gate: &Gate) -> Block {
    let mut b = Block::new("homeFirst", "");
    b.lines.push(Line::new("homeFirst", t(Key::PsOpenFirst)).key(press("openHome", t(Key::PsOpen), gate, Does::Sheet(Sheet::Home))));
    b
}

fn names(shell: &Shell, tb: &zikaron_glue::face::Table, gate: &Gate, rotating: bool) -> Vec<Block> {
    use zikaron_glue::face::Named;
    let mut out = Vec::new();
    if rotating {
        let mut r = Block::new("rotating", "");
        r.lines.push(Line::new("rotating", t(Key::PsWhyRotating)).pill(t(Key::PsRotating), PillTone::Warn));
        out.push(r);
    }
    let waiting = pending(shell, Some(tb));
    if !waiting.is_empty() {
        let mut w = Block::new("pending", t(Key::ItemUnanchored));
        let send_gate = Gate { on: !shell.tasks.in_flight(crate::task::Kind::Anchor), why: t(Key::U3Sending).to_string() };
        for (row, queued) in waiting {
            w.lines.push(pending_line(shell, row, queued, gate, &send_gate));
        }
        out.push(w);
    }
    if tb.home.is_none() {
        out.push(needs_home(gate));
        return out;
    }
    let mut nb = Block::new("names", t(Key::FaceSummaryPersona));
    nb.head = Some(press("name", t(if tb.persona.is_some() { Key::PsChangeName } else { Key::PsSetName }), gate, Does::Sheet(Sheet::Name)));
    nb.empty = t(Key::PsNoNames).to_string();
    let ruling = tb.persona.as_ref().map(|p| p.entry.to_ascii_lowercase());
    for r in tb.faces.iter().rev() {
        let Named::Persona(n) = &r.named else { continue };
        for s in r.spans.iter().rev() {
            let mut l = Line::new(format!("name:{}", s.entry), n.lay_alone()).sub(span_words(s));
            l = if s.until.is_some() {
                l.pill(t(Key::PsReleased), PillTone::Grey)
            } else if ruling.as_deref() == Some(&s.entry.to_ascii_lowercase()) {
                l.pill(t(Key::IdIsCurrent), PillTone::Ok)
            } else {
                l.pill(t(Key::BadgeLive), PillTone::Blue)
            };
            if s.release.is_none() && s.until.is_none() {
                l = l.key(press(format!("release:{}", s.entry), t(Key::FaceSummaryRelease), gate, Does::Sheet(Sheet::Release { subject: s.entry.clone(), what: n.lay_alone() })));
            }
            nb.lines.push(l);
        }
    }
    out.push(nb);
    let mut ab = Block::new("accounts", t(Key::PsAccounts));
    ab.head = Some(press("account", t(Key::PsBindAccount), gate, Does::Sheet(Sheet::Account { platform: String::new(), handle: String::new() })));
    ab.empty = t(Key::PsNoAccounts).to_string();
    for r in &tb.faces {
        let Named::Account { platform, handle } = &r.named else { continue };
        let mut l = Line::new(format!("account:{platform}:{handle}"), format!("{platform} \u{b7} {handle}")).sub(spans_words(r));
        if let (true, Some(s)) = (r.bound, open_span(r)) {
            l = l.pill(t(Key::BadgeLive), PillTone::Ok).key(press(format!("release:{}", s.entry), t(Key::FaceSummaryRelease), gate, Does::Sheet(Sheet::Release { subject: s.entry.clone(), what: format!("{platform} \u{b7} {handle}") })));
        } else {
            l = l.pill(t(Key::PsReleased), PillTone::Grey).key(press(format!("rebind:{platform}:{handle}"), t(Key::PsRebind), gate, Does::Sheet(Sheet::Account { platform: platform.clone(), handle: handle.clone() })));
        }
        ab.lines.push(l);
    }
    out.push(ab);
    let mut kb = Block::new("keys", t(Key::Address));
    kb.head = Some(press("key", t(Key::PsBindKey), gate, Does::Sheet(Sheet::Key { address: String::new(), role: String::new() })));
    kb.empty = t(Key::PsNoKeys).to_string();
    for r in &tb.faces {
        let Named::Key { address, role } = &r.named else { continue };
        let mut l = Line::new(format!("key:{address}:{role}"), format!("{} \u{b7} {role}", short(address))).sub(spans_words(r));
        if let (true, Some(s)) = (r.bound, open_span(r)) {
            l = l.pill(t(Key::BadgeLive), PillTone::Ok).key(press(format!("release:{}", s.entry), t(Key::FaceSummaryRelease), gate, Does::Sheet(Sheet::Release { subject: s.entry.clone(), what: format!("{} \u{b7} {role}", short(address)) })));
        } else {
            l = l.pill(t(Key::PsReleased), PillTone::Grey).key(press(format!("rebind:{address}:{role}"), t(Key::PsRebind), gate, Does::Sheet(Sheet::Key { address: address.clone(), role: role.clone() })));
        }
        kb.lines.push(l);
    }
    out.push(kb);
    if !tb.misread.is_empty() {
        let mut ub = Block::new("unseated", t(Key::PsUnseated));
        for (entry, reason) in &tb.misread {
            let what = row_named(shell, entry);
            ub.lines.push(Line::new(format!("unseated:{entry}"), what.clone()).sub(misread_said(reason)).key(press(format!("reseat:{entry}"), t(Key::PsReseat), gate, Does::Sheet(Sheet::Reseat { entry: entry.clone(), what }))));
        }
        out.push(ub);
    }
    out
}

fn book_state(b: &crate::facex::Book, root_key_here: bool, before: Option<[u8; 20]>, handed: Option<[u8; 20]>) -> (String, PillTone, Option<Result<(), String>>, Option<String>) {
    use crate::rotatex::{Closed, Enrol};
    match b {
        crate::facex::Book::Off => (t(Key::FaceLeftNoBook).to_string(), PillTone::Grey, None, None),
        crate::facex::Book::Wrong => (t(Key::FaceLeftBookWrong).to_string(), PillTone::Bad, None, None),
        crate::facex::Book::Unread(f) => (t(Key::FaceLeftBookUnread).to_string(), PillTone::Warn, None, Some(f.human().to_string())),
        crate::facex::Book::Read { under, plan } => {
            let held = under.map(|k| short(&hex20(&k)));
            match plan {
                Ok(Enrol::Held) => (t(Key::PsHeld).to_string(), PillTone::Ok, None, held),
                Ok(Enrol::Declare) | Ok(Enrol::Accede) => (t(Key::PsNotEntered).to_string(), PillTone::Warn, Some(if root_key_here { Ok(()) } else { Err(t(Key::PsRootKeyAbsent).to_string()) }), held),
                Err(Closed::OtherKey(k)) if handed == Some(*k) => (t(Key::PsHandedTakenUp).to_string(), PillTone::Ok, None, held),
                Err(Closed::OtherKey(k)) if before.is_some_and(|b| b != *k) => (t(Key::PsTakenUp).to_string(), PillTone::Bad, None, held),
                Err(Closed::OtherKey(_)) => (t(Key::PsOldKey).to_string(), PillTone::Warn, None, held),
                Err(Closed::RootElsewhere(_)) => (t(Key::PsRootElsewhere).to_string(), PillTone::Bad, Some(Err(t(Key::FaultNextBookClosed).to_string())), held),
                Err(Closed::OfficeEntered) => (t(Key::PsOfficeEntered).to_string(), PillTone::Bad, Some(Err(t(Key::FaultNextBookClosed).to_string())), held),
            }
        }
    }
}

fn home_tab(shell: &Shell, tb: &zikaron_glue::face::Table, gate: &Gate, rotating: bool) -> Vec<Block> {
    let mut out = Vec::new();
    let mut head = Block::new("home", "");
    let move_key = press("openHome", t(if tb.home.is_some() { Key::PsMove } else { Key::PsOpen }), gate, Does::Sheet(Sheet::Home));
    let mut line = match (home_left(shell), tb.home) {
        (Some(c), _) => Line::new("home", chain_name(shell, c)).pill(t(Key::PsNotReadPill), PillTone::Warn),
        (None, Some(c)) => Line::new("home", chain_name(shell, c)).pill(t(Key::PsTabHome), PillTone::Ok),
        (None, None) => Line::new("home", t(Key::PsNoHome)),
    };
    if let Some(o) = tb.homes.last() {
        line = line.sub(crate::when::when(o.time));
    }
    if rotating {
        line = line.pill(t(Key::PsRotating), PillTone::Warn);
    }
    head.lines.push(line.key(move_key));
    if let (Some(h), Some(a)) = (tb.home, shell.settings.chain_id) {
        if h != a {
            head.lines.push(Line::new("anchorChain", t(Key::PsAnchorChain)).value(Value::Text(chain_name(shell, a))).pill(t(Key::PsNotHome), PillTone::Warn));
        }
    }
    out.push(head);
    let homes_waiting: Vec<(&crate::ledgerx::Row, Option<bool>)> = pending(shell, Some(tb)).into_iter().filter(|(r, _)| r.facts.face.as_ref().is_some_and(|(shape, _, _)| *shape == "home")).collect();
    if !homes_waiting.is_empty() {
        let mut w = Block::new("pending", t(Key::ItemUnanchored));
        let send_gate = Gate { on: !shell.tasks.in_flight(crate::task::Kind::Anchor), why: t(Key::U3Sending).to_string() };
        for (row, queued) in homes_waiting {
            w.lines.push(pending_line(shell, row, queued, gate, &send_gate));
        }
        out.push(w);
    }
    let mut books = Block::new("books", t(Key::PsBooks));
    books.empty = t(Key::WbNotRead).to_string();
    if let Some(fb) = shell.face_books.as_ref() {
        let before = crate::action::seat_before(shell).map(|a| a.0);
        let handed = shell.handed.as_deref().and_then(crate::key::Address::parse).map(|a| a.0);
        for (chain, b) in &fb.chains {
            let (said, tone, enter, held) = book_state(b, fb.root_key_here, before, handed);
            let mut l = Line::new(format!("book:{chain}"), chain_name(shell, *chain)).pill(said, tone);
            if tb.home == Some(*chain) {
                l = l.sub(t(Key::PsTabHome));
            }
            if let Some(h) = held {
                l = l.value(Value::Mono(h));
            }
            if let Some(e) = enter {
                l = match &e {
                    Err(why) => l.sub(why.clone()).key(press(format!("reroot:{chain}"), t(Key::PsReroot), gate, Does::Reroot)),
                    Ok(()) if rotating => l.key(press(format!("enter:{chain}"), t(Key::PsEnter), &Gate { on: false, why: t(Key::PsWhyRotating).to_string() }, Does::Sheet(Sheet::Enter { chain: *chain }))),
                    Ok(()) => l.key(press(format!("enter:{chain}"), t(Key::PsEnter), &Gate { on: shell.may_write_entries_shown().is_ok() && !shell.tasks.in_flight(crate::task::Kind::Rotate), why: t(Key::U3Sending).to_string() }, Does::Sheet(Sheet::Enter { chain: *chain }))),
                };
            }
            books.lines.push(l);
        }
    }
    out.push(books);
    if !tb.homes.is_empty() {
        let mut hb = Block::new("homes", t(Key::PsHomes));
        for o in tb.homes.iter().rev() {
            hb.lines.push(Line::new(format!("opening:{}", o.entry), chain_name(shell, o.chain)).sub(crate::when::when(o.time)).value(Value::Copy(o.tx.clone())));
        }
        out.push(hb);
    }
    match shell.face_pair.as_ref() {
        Some(p) if p.side == crate::facex::Side::New => out.push(came_from(shell, tb, p)),
        pair => out.push(reroot_block(shell, tb, gate, pair)),
    }
    out
}

fn home_left(shell: &Shell) -> Option<u64> {
    let ledger_home = shell.rows.as_ref().and_then(|(rows, _)| {
        rows.iter().filter(|r| r.kind == zikaron_glue::book::Kind::Face).filter_map(|r| r.facts.face.as_ref().filter(|(s, _, _)| *s == "home").and_then(|(_, c, _)| c.parse::<u64>().ok()).map(|c| (r.seq, c))).max().map(|(_, c)| c)
    })?;
    match &shell.persona {
        crate::facex::State::Read(b) if b.left.iter().any(|(c, _)| *c == ledger_home) => Some(ledger_home),
        _ => None,
    }
}

fn reroot_block(shell: &Shell, tb: &zikaron_glue::face::Table, gate: &Gate, pair: Option<&crate::facex::Pair>) -> Block {
    let mut b = Block::new("reroot", t(Key::PsReroot));
    let left_home = home_left(shell).is_some();
    let elsewhere = shell.face_books.as_ref().is_some_and(|fb| fb.chains.iter().any(|(c, b)| Some(*c) != tb.home && matches!(b, crate::facex::Book::Read { plan: Ok(crate::rotatex::Enrol::Held), .. })));
    let g = match (tb.home, left_home) {
        (_, true) if elsewhere => Gate { on: false, why: t(Key::PsHomeLeft).to_string() },
        (_, true) => Gate { on: false, why: t(Key::PsNoHomeToMove).to_string() },
        (None, false) => Gate { on: false, why: t(Key::PsOpenFirst).to_string() },
        (Some(_), false) => Gate { on: gate.on, why: gate.why.clone() },
    };
    let line = Line::new("newRoot", t(Key::PsNewRoot));
    b.lines.push(match pair {
        None => line.value(Value::Quiet(t(Key::PsNotMoved).to_string())).key(press("reroot", t(Key::PsReroot), &g, Does::Reroot)),
        Some(p) => line.value(Value::Mono(short(&p.other.hex()))).pill(t(Key::PsUnderWay), PillTone::Blue).key(press("goOn", t(Key::Continue), &Gate { on: true, why: String::new() }, Does::Reroot)),
    });
    b
}

fn came_from(shell: &Shell, tb: &zikaron_glue::face::Table, p: &crate::facex::Pair) -> Block {
    let mut b = Block::new("cameFrom", "");
    let named_here = p.table().and_then(|o| o.reroot.as_ref()).is_some_and(|l| l.other.eq_ignore_ascii_case(&tb.root));
    let title = t(if tb.origin.is_some() {
        Key::PsCameFrom
    } else if named_here {
        Key::PsMovedHere
    } else {
        Key::PsReroot
    });
    let mut l = Line::new("cameFrom", title).value(Value::Quiet(p.other.hex()));
    l = match p.joined(&shell.persona) {
        Some(true) => l.pill(t(Key::PsChecked), PillTone::Grey),
        _ => l.key(press("goOn", t(Key::Continue), &Gate { on: true, why: String::new() }, Does::Reroot)),
    };
    b.lines.push(l);
    b
}

fn moved(shell: &Shell, tb: &zikaron_glue::face::Table) -> Block {
    let mut b = Block::new("moved", "");
    let Some(link) = tb.reroot.as_ref() else { return b };
    let joined = shell.face_pair.as_ref().filter(|p| p.side == crate::facex::Side::Old).and_then(|p| p.joined(&shell.persona));
    let mut l = Line::new("movedTo", t(Key::PsMovedTo)).sub(format!("{} \u{b7} {}", crate::when::when(link.since.time), chain_name(shell, link.since.chain))).value(Value::Mono(link.other.clone()));
    let held = crate::key::Address::parse(&link.other).is_some_and(|to| shell.identities.as_ref().is_some_and(|reg| reg.rows.iter().any(|r| r.seats().into_iter().any(|s| r.address(s) == Some(to)))));
    let go_on = press("goOn", t(Key::Continue), &Gate { on: true, why: String::new() }, Does::Reroot);
    l = match joined {
        Some(true) => l.pill(t(Key::PsChecked), PillTone::Ok),
        Some(false) => l.pill(t(Key::PsNotAnswered), PillTone::Warn),
        None => l.pill(t(Key::PsNotReadPill), PillTone::Warn),
    };
    if joined != Some(true) && held {
        l = l.key(go_on);
    }
    b.lines.push(l);
    b
}

fn carry_block(shell: &Shell, tb: &zikaron_glue::face::Table, gate: &Gate) -> Option<(Block, bool)> {
    use zikaron_glue::face::Named;
    let pair = shell.face_pair.as_ref().filter(|p| p.side == crate::facex::Side::New)?;
    if pair.joined(&shell.persona) != Some(true) {
        return None;
    }
    let old = pair.table()?;
    let waiting = pending(shell, Some(tb));
    let wait = |shape: &str, a: &str, b: &str| waiting.iter().find(|(r, _)| r.facts.face.as_ref().is_some_and(|(s, x, y)| *s == shape && (shape == "persona" || (x.eq_ignore_ascii_case(a) && y == b)))).copied();
    let mut blk = Block::new("carry", t(Key::PsCarry));
    let mut done = true;
    let state = |l: Line, has: Option<bool>, waits: Option<(&crate::ledgerx::Row, Option<bool>)>| -> (Line, bool) {
        match (has, waits) {
            (Some(bound), _) => (l.pill(t(if bound { Key::BadgeLive } else { Key::PsReleased }), if bound { PillTone::Ok } else { PillTone::Grey }), true),
            (None, Some((row, queued))) => {
                let (word, tone) = pending_state(shell, row, queued);
                (l.pill(t(word), tone), false)
            }
            (None, None) => (l, false),
        }
    };
    if let Some(p) = &old.persona {
        let has = tb.faces.iter().find(|r| matches!(&r.named, Named::Persona(n) if n.copy_name() == p.name.copy_name())).map(|r| r.bound);
        let (mut l, ok) = state(Line::new("carry:name", t(Key::FaceSummaryPersona)).value(Value::Name { lay: p.name.lay_alone(), copy: p.name.copy_name().to_string() }), has, wait("persona", "", ""));
        if has.is_none() && wait("persona", "", "").is_none() {
            l = l.key(press("carry:name", t(Key::PsSetAsName), gate, Does::Act("faceName", Action::FaceName { name: p.name.copy_name().to_string() })));
        }
        done &= ok;
        blk.lines.push(l);
    }
    for r in old.faces.iter().filter(|r| r.bound) {
        match &r.named {
            Named::Account { platform, handle } => {
                let has = tb.faces.iter().find(|x| matches!(&x.named, Named::Account { platform: p, handle: h } if p == platform && h == handle)).map(|x| x.bound);
                let w = wait("account", platform, handle);
                let (mut l, ok) = state(Line::new(format!("carry:account:{platform}:{handle}"), t(Key::PsAccounts)).value(Value::Text(format!("{platform} \u{b7} {handle}"))), has, w);
                if has.is_none() && w.is_none() {
                    l = l
                        .key(press(format!("carry:bind:{platform}:{handle}"), t(Key::PsBind), gate, Does::Act("faceAccount", Action::FaceAccount { platform: platform.clone(), handle: handle.clone(), note: String::new() })))
                        .key2(press(format!("carry:marker:{platform}:{handle}"), t(Key::PsCopyMarker), &Gate { on: true, why: String::new() }, Does::Copy(tb.marker.clone())));
                }
                done &= ok;
                blk.lines.push(l);
            }
            Named::Key { address, role } => {
                let has = tb.faces.iter().find(|x| matches!(&x.named, Named::Key { address: a, role: ro } if a.eq_ignore_ascii_case(address) && ro == role)).map(|x| x.bound);
                let w = wait("key", address, role);
                let (mut l, ok) = state(Line::new(format!("carry:key:{address}:{role}"), t(Key::Address)).value(Value::Mono(format!("{} \u{b7} {role}", short(address)))), has, w);
                if has.is_none() && w.is_none() {
                    l = l.key(press(format!("carry:bind:{address}:{role}"), t(Key::PsBind), gate, Does::Sheet(Sheet::Key { address: address.clone(), role: role.clone() })));
                }
                done &= ok;
                blk.lines.push(l);
            }
            Named::Persona(_) => {}
        }
    }
    (!blk.lines.is_empty()).then_some((blk, done))
}

pub fn other_block(shell: &Shell, first: &crate::facex::State, moves: &[crate::facex::Hop], at: usize) -> Block {
    use zikaron_glue::face::Named;
    let mut b = Block::new("otherFaces", t(Key::PsFacesOf));
    let at = at.min(moves.len());
    let state = if at == 0 { first } else { &moves[at - 1].state };
    let next = moves.get(at);
    let built = match state {
        crate::facex::State::Read(x) => x.as_ref(),
        crate::facex::State::NotRead(_) => {
            let mut n = not_read_of(shell, state);
            n.name = "otherFaces";
            n.title = t(Key::PsFacesOf).to_string();
            return n;
        }
        crate::facex::State::Unread => {
            b.lines.push(Line::new("unread", t(Key::WbNotRead)).value(Value::Quiet(String::new())));
            return b;
        }
    };
    let tb = &built.table;
    let mut sealed_joined = false;
    if let Some(l) = tb.reroot.as_ref() {
        b.lines.push(match next {
            Some(h) if h.again => Line::new("walkStop", t(Key::PsWalkStop)).value(Value::Mono(h.root.hex())),
            Some(h) if h.joined == Some(true) => {
                sealed_joined = true;
                Line::new("movedTo", t(Key::PsMovedTo)).value(Value::Mono(l.other.clone())).pill(t(Key::PsChecked), PillTone::Ok).key(press("openNew", t(Key::PsOpenNewRoot), &Gate { on: true, why: String::new() }, Does::Next))
            }
            Some(h) if h.joined == Some(false) => Line::new("movedTo", t(Key::PsSaysMovedTo)).value(Value::Mono(l.other.clone())).pill(t(Key::PsNotAnswered), PillTone::Warn),
            Some(_) => Line::new("movedTo", t(Key::PsSaysMovedTo)).value(Value::Mono(l.other.clone())).pill(t(Key::PsNotReadPill), PillTone::Warn),
            None => Line::new("walkStop", t(Key::PsWalkStop)).value(Value::Mono(l.other.clone())),
        });
    }
    let live = |bound: bool| match (bound, sealed_joined) {
        (true, true) => (t(Key::PsWhenSealed), PillTone::Grey),
        (true, false) => (t(Key::BadgeLive), PillTone::Ok),
        (false, _) => (t(Key::PsReleased), PillTone::Grey),
    };
    b.lines.push(match &tb.persona {
        Some(p) if sealed_joined => Line::new("persona", t(Key::FaceSummaryPersona)).value(Value::Name { lay: p.name.lay_alone(), copy: p.name.copy_name().to_string() }).pill(t(Key::PsWhenSealed), PillTone::Grey),
        Some(p) => Line::new("persona", t(Key::FaceSummaryPersona)).value(Value::Name { lay: p.name.lay_alone(), copy: p.name.copy_name().to_string() }),
        None => Line::new("persona", t(Key::FaceSummaryPersona)).value(Value::Quiet(t(Key::PsNoName).to_string())),
    });
    b.lines.push(Line::new("marker", t(Key::PsMarker)).value(Value::Copy(tb.marker.clone())));
    let mut any_account = false;
    for r in &tb.faces {
        let (name, title) = match &r.named {
            Named::Account { platform, handle } => {
                any_account = true;
                (format!("account:{platform}:{handle}"), format!("{} \u{b7} {platform} \u{b7} {handle}", t(Key::PsAccounts)))
            }
            Named::Key { address, role } => (format!("key:{address}:{role}"), format!("{} \u{b7} {} \u{b7} {role}", t(Key::Address), short(address))),
            Named::Persona(_) => continue,
        };
        let state = live(r.bound);
        b.lines.push(Line::new(name, title).sub(spans_words(r)).pill(state.0, state.1));
    }
    if any_account {
        b.foot = t(Key::PsHalfProof).to_string();
    }
    b.lines.push(match tb.home {
        Some(c) => Line::new("home", t(Key::PsTabHome)).value(Value::Text(chain_name(shell, c))),
        None => Line::new("home", t(Key::PsTabHome)).value(Value::Quiet(t(Key::PsNoHome).to_string())),
    });
    b.lines.push(Line::new("office", t(Key::PsOffice)).value(Value::Copy(tb.office.clone())));
    b.lines.push(Line::new("lineage", t(Key::Lineage)).value(Value::Mono(tb.lineage.iter().map(|l| short(&l.key)).collect::<Vec<_>>().join(" \u{2192} "))));
    b.lines.push(
        Line::new("readTo", t(Key::PsReadTo))
            .value(Value::Text(crate::when::when(built.moment)))
            .pill(fill1(Key::PsUnreadN, &tb.unread.to_string()), PillTone::Grey)
            .pill(fill1(Key::PsUnprovenN, &tb.unproven.to_string()), if tb.unproven > 0 { PillTone::Warn } else { PillTone::Grey }),
    );
    if let Some(h) = crate::facex::hint(tb) {
        b.lines.push(Line::new("unprovenHint", h).pill(t(Key::PsReread), PillTone::Warn));
    }
    b.lines.push(Line::new("basis", t(Key::PsBasis)).value(Value::Text(tb.basis.iter().map(|x| chain_name(shell, x.chain)).collect::<Vec<_>>().join(t(Key::ListJoin)))));
    for (chain, why) in &built.left {
        b.lines.push(Line::new(format!("left:{chain}"), chain_name(shell, *chain)).sub(crate::facex::left_said(why)).pill(t(Key::PsLeftPill), PillTone::Warn));
    }
    b
}

#[derive(Default)]
pub(super) struct PersonaUx {
    pub(super) sheet: Option<Sheet>,
    pub(super) name: String,
    pub(super) platform: String,
    pub(super) handle: String,
    pub(super) note: String,
    pub(super) address: String,
    pub(super) role: String,
    pub(super) sig: String,
    pub(super) pin: crate::secret::Secret,
    pub(super) home: Option<u64>,
    pub(super) trouble: Option<crate::fault::Fault>,
    pub(super) shake: Option<f64>,
    pub(super) seen: Option<[u64; 4]>,
    pub(super) consent: bool,
    pub(super) consent_id: String,
    pub(super) consent_role: String,
    pub(super) consent_pin: crate::secret::Secret,
    pub(super) consent_out: Option<String>,
    pub(super) carry_seen: Option<Option<f64>>,
    pub(super) asked_unread: bool,
}

pub const SEAT_EVERY: f64 = 12.0;

pub const SEAT_BOUND: f64 = 180.0;

pub const CARRY_LINGER: f64 = 2.0;

impl Win {
    pub(super) fn persona_page(&mut self, ui: &mut egui::Ui, tab: u8, now: f64) {
        self.ensure_rows(now);
        self.persona_follow(now);
        self.persona_follow_seat(now);
        if let Some(at) = self.seat_due(now) {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64((at - now).max(0.5)));
        }
        let mut m = model(&self.shell, tab);
        if !m.sealed {
            let list = crate::nav::tabs(View::Persona, self.shell.settings.role);
            let labels: Vec<&str> = list.iter().map(|x| t(tab_key(View::Persona, *x))).collect();
            let cur = list.iter().position(|x| *x == tab).unwrap_or(0);
            if let Some(i) = stagger(ui, 0, |ui| seg::tabs(ui, "persona-tabs", &labels, cur)) {
                self.seg_tab(Place::View(View::Persona, list[i]), now);
            }
        }
        match m.carry {
            Some(false) => self.ux.persona.carry_seen = Some(None),
            Some(true) => {
                if self.ux.persona.carry_seen == Some(None) {
                    self.ux.persona.carry_seen = Some(Some(now));
                }
                let lingering = matches!(self.ux.persona.carry_seen, Some(Some(at)) if now - at < CARRY_LINGER);
                if lingering {
                    ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(0.2));
                } else {
                    m.blocks.retain(|b| b.name != "carry");
                }
            }
            None => {}
        }
        let mut hit: Option<Does> = None;
        tab_body(ui, "persona", u64::from(tab), |ui| {
            for (i, b) in m.blocks.iter().enumerate() {
                if let Some(d) = stagger(ui, i + 1, |ui| draw_block(ui, b)) {
                    hit = Some(d);
                }
            }
        });
        if let Some(d) = hit {
            if let Does::Copy(text) = &d {
                ui.ctx().copy_text(text.clone());
                tell_copied(ui.ctx(), Some(text.clone()));
            }
            self.persona_do(d, now);
        }
    }

    pub(super) fn persona_acts(&mut self, ui: &mut egui::Ui, tab: u8, now: f64) {
        let m = model(&self.shell, tab);
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Persona) || self.shell.tasks.in_flight(crate::task::Kind::FaceBooks);
        let phase = if busy { Phase::Busy { frac: None } } else { Phase::Idle };
        let r = key::show(ui, key::Key::new(&m.reread.word, Role::Secondary).enabled(m.reread.on).phase(phase));
        zikaron_ui::probe::spot(ui.ctx(), &format!("persona:{}", m.reread.name), r.rect);
        if r.clicked() && m.reread.on {
            self.persona_do(Does::Reread, now);
        }
    }

    pub(super) fn persona_do(&mut self, d: Does, now: f64) {
        match d {
            Does::Reread => {
                let _ = self.act(Action::FaceReread, now);
                let _ = self.act(Action::FaceBooks, now);
                self.ux.persona.seen = Some(self.persona_marks());
            }
            Does::Send => {
                let n = self.shell.sendable_here();
                if n > 0 {
                    self.u3_open_confirm(U3Confirm::Send { count: n });
                }
            }
            Does::Sheet(s) => self.persona_open(s),
            Does::Reroot => self.reroot_open(),
            Does::Act(_, a) => {
                let r = self.act_carded(a, now);
                self.persona_back(r, now);
            }
            Does::Copy(_) | Does::Next => {}
        }
    }

    pub(super) fn persona_open(&mut self, s: Sheet) {
        self.sheets_clear();
        let p = &mut self.ux.persona;
        p.trouble = None;
        p.shake = None;
        p.pin.clear();
        p.note.clear();
        p.sig.clear();
        match &s {
            Sheet::Name => p.name.clear(),
            Sheet::Account { platform, handle } => {
                p.platform = platform.clone();
                p.handle = handle.clone();
            }
            Sheet::Key { address, role } => {
                p.address = address.clone();
                p.role = role.clone();
            }
            Sheet::Home => p.home = None,
            _ => {}
        }
        p.sheet = Some(s);
    }

    fn persona_marks(&self) -> [u64; 4] {
        use crate::task::Kind as K;
        [self.shell.tasks.landings(K::Anchor), self.shell.tasks.landings(K::Gate), self.shell.tasks.landings(K::Rotate), self.shell.tasks.landings(K::Audit)]
    }

    fn persona_follow(&mut self, now: f64) {
        let unread = matches!(self.shell.persona, crate::facex::State::Unread);
        if !unread {
            self.ux.persona.asked_unread = false;
        } else if !self.ux.persona.asked_unread && !self.shell.tasks.in_flight(crate::task::Kind::Persona) && !self.shell.tasks.in_flight(crate::task::Kind::FaceBooks) {
            self.ux.persona.asked_unread = true;
            self.auto(Action::FaceReread, now);
            self.auto(Action::FaceBooks, now);
            self.ux.persona.seen = Some(self.persona_marks());
            return;
        }
        let marks = self.persona_marks();
        let Some(seen) = self.ux.persona.seen else {
            self.ux.persona.seen = Some(marks);
            return;
        };
        if seen == marks || self.shell.tasks.in_flight(crate::task::Kind::Persona) {
            return;
        }
        self.ux.persona.seen = Some(marks);
        let table = match &self.shell.persona {
            crate::facex::State::Read(b) => Some(&b.table),
            _ => None,
        };
        let waiting = !pending(&self.shell, table).is_empty();
        let entered = marks[2] != seen[2];
        if waiting || entered {
            self.auto(Action::FaceReread, now);
        }
        if entered && !self.shell.tasks.in_flight(crate::task::Kind::FaceBooks) {
            self.auto(Action::FaceBooks, now);
        }
    }

    fn seat_due(&self, now: f64) -> Option<f64> {
        let table = match &self.shell.persona {
            crate::facex::State::Read(b) => Some(&b.table),
            _ => return None,
        };
        let sent = self.shell.tasks.landed_at(crate::task::Kind::Anchor)?;
        if now > sent + SEAT_BOUND || !pending(&self.shell, table).iter().any(|(r, q)| q.is_none() && sent_here(&self.shell, r)) {
            return None;
        }
        let last = self.shell.tasks.landed_at(crate::task::Kind::Persona).unwrap_or(sent);
        Some(last.max(sent) + SEAT_EVERY)
    }

    fn persona_follow_seat(&mut self, now: f64) {
        if self.shell.tasks.in_flight(crate::task::Kind::Persona) {
            return;
        }
        if self.seat_due(now).is_some_and(|at| now >= at) {
            self.auto(Action::FaceReread, now);
        }
    }

    pub(super) fn persona_back(&mut self, r: Applied, now: f64) {
        match r {
            Applied::Queued { .. } => {
                self.ux.persona.sheet = None;
                self.ux.persona.pin.clear();
            }
            Applied::Consented { text } => {
                self.ux.persona.consent_pin.clear();
                self.ux.persona.consent_out = Some(text);
            }
            Applied::Trouble(f) => {
                if f.which() == Some(crate::fault::Known::PinWrong) {
                    self.ux.persona.shake = Some(now);
                }
                self.ux.persona.pin.clear();
                self.ux.persona.consent_pin.clear();
                self.ux.persona.trouble = Some(f);
            }
            _ if self.ux.persona.consent => {}
            _ => self.ux.persona.sheet = None,
        }
    }

    pub(super) fn persona_sheets(&mut self, ctx: &egui::Context, now: f64) {
        let Some(which) = self.ux.persona.sheet.clone() else { return };
        let trouble = self.ux.persona.trouble.clone();
        let show_trouble = |ui: &mut egui::Ui| {
            if let Some(f) = &trouble {
                states::err_box(ui, "persona-trouble", f.human(), &f.next(), t(Key::U3RawError), &f.shown());
            }
        };
        let table = match &self.shell.persona {
            crate::facex::State::Read(b) => Some(b.table.clone()),
            _ => None,
        };
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault) || self.shell.tasks.in_flight(crate::task::Kind::Gate) || self.shell.tasks.in_flight(crate::task::Kind::Rotate);
        let phase = if busy { Phase::Busy { frac: None } } else { Phase::Idle };
        let (mut go, mut close): (Option<Action>, bool) = (None, false);
        let shake = self.ux.persona.shake;
        let title = match &which {
            Sheet::Name => t(if table.as_ref().is_some_and(|tb| tb.persona.is_some()) { Key::PsChangeName } else { Key::PsSetName }),
            Sheet::Account { .. } => t(Key::PsBindAccount),
            Sheet::Key { .. } => t(Key::PsBindKey),
            Sheet::Release { .. } => t(Key::FaceSummaryRelease),
            Sheet::Reseat { .. } => t(Key::PsReseat),
            Sheet::Home => t(if table.as_ref().and_then(|tb| tb.home).is_some() { Key::PsMove } else { Key::PsOpen }),
            Sheet::Enter { .. } => t(Key::PsEnter),
        };
        let local = |a: &str| crate::key::Address::parse(a.trim()).is_some_and(|a| self.shell.identities.as_ref().is_some_and(|reg| reg.rows.iter().any(|r| r.seats().into_iter().any(|s| r.current(s) == Some(a)))));
        let address_local = local(&self.ux.persona.address);
        let root = table.as_ref().map(|tb| tb.root.clone()).unwrap_or_default();
        let marker = table.as_ref().map(|tb| tb.marker.clone()).unwrap_or_default();
        let books = self.shell.face_books.clone();
        let current_home = table.as_ref().and_then(|tb| tb.home);
        let chains: Vec<(u64, String)> = self.shell.basis_chains().iter().map(|(c, _, _)| (*c, chain_name(&self.shell, *c))).collect();
        let out = sheet::show(
            ctx,
            sheet::Spec::new(&format!("persona-{}", which.name()), tk::SHEET_NARROW).shake(shake),
            self,
            |ui, me| {
                sheet::title(ui, title, "");
                let p = &mut me.ux.persona;
                let plain = fields::Look::default();
                let mono = fields::Look { mono: true, ..Default::default() };
                match &which {
                    Sheet::Name => {
                        let n = zikaron_glue::face::nfc(&p.name).chars().count();
                        fields::stack(ui, |ui, st| {
                            st.field(ui, t(Key::FaceSummaryPersona), Some(&format!("{n} / 32")), None, |ui, w| fields::line(ui, &mut p.name, "", w, plain));
                        });
                        sheet::foot_note(ui, t(Key::PsSaidPublic));
                    }
                    Sheet::Account { .. } => {
                        fields::stack(ui, |ui, st| {
                            st.field(ui, t(Key::PsPlatform), None, None, |ui, w| fields::line(ui, &mut p.platform, "github", w, plain));
                            st.field(ui, t(Key::PsHandle), None, None, |ui, w| fields::line(ui, &mut p.handle, "", w, mono));
                            st.field(ui, t(Key::KitNoteLabel), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut p.note, "", w, plain));
                        });
                        zikaron_ui::fields::rows(ui, |ui, f| {
                            let _ = f.info(ui, t(Key::PsMarker), card::Show::Copy(&marker));
                        });
                        sheet::foot_note(ui, t(Key::PsSaidAccount));
                    }
                    Sheet::Key { .. } => {
                        fields::stack(ui, |ui, st| {
                            let addr_bad = crate::addressedx::not_address(&p.address, crate::addressedx::Takes::OrConsent);
                            st.field(ui, t(Key::Address), None, addr_bad.then(|| (t(Key::KitToWhyAddress), true)), |ui, w| fields::line(ui, &mut p.address, t(Key::PsAddressHint), w, fields::Look { bad: addr_bad, ..mono }));
                            st.field(ui, t(Key::PsRole), None, None, |ui, w| fields::line(ui, &mut p.role, "payout", w, plain));
                            st.field(ui, t(Key::KitNoteLabel), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut p.note, "", w, plain));
                            if address_local {
                                let _ = pin_field(st, ui, t(Key::PsLocalKey), None, "persona-pin", &mut p.pin, shake, !busy);
                            } else {
                                st.field(ui, t(Key::PsSig), None, None, |ui, w| fields::line(ui, &mut p.sig, "0x\u{2026}", w, mono));
                            }
                        });
                        if p.address.trim().starts_with(zikaron_glue::face::CONSENT_PREFIX) {
                            match zikaron_glue::face::Consent::parse(p.address.trim()) {
                                Ok(c) if hex20(&c.root).eq_ignore_ascii_case(&root) => {
                                    p.sig = zikaron::hexfmt::encode(&c.sig);
                                    p.role = c.role.clone();
                                    p.address = hex20(&c.address);
                                    p.trouble = None;
                                }
                                Ok(_) => p.trouble = Some(crate::fault::Fault::known(crate::fault::Known::ConsentRefused, "ROOT".to_string())),
                                Err(_) => p.trouble = Some(crate::fault::Fault::known(crate::fault::Known::ConsentRefused, "FORM".to_string())),
                            }
                        }
                        sheet::foot_note(ui, t(Key::PsSaidKey));
                    }
                    Sheet::Release { what, .. } => {
                        zikaron_ui::fields::rows(ui, |ui, f| {
                            let _ = f.info(ui, t(Key::FaceSummaryRelease), card::Show::Text(what));
                        });
                        sheet::foot_note(ui, t(Key::PsSaidRelease));
                    }
                    Sheet::Reseat { what, .. } => {
                        zikaron_ui::fields::rows(ui, |ui, f| {
                            let _ = f.info(ui, t(Key::PsUnseated), card::Show::Text(what));
                        });
                    }
                    Sheet::Home => {
                        zikaron_ui::fields::rows(ui, |ui, f| {
                            for (chain, name) in &chains {
                                let held = books.as_ref().and_then(|b| b.chains.iter().find(|(c, _)| c == chain)).map(|(_, b)| matches!(b, crate::facex::Book::Read { plan: Ok(crate::rotatex::Enrol::Held), .. }));
                                let open = held == Some(true) && current_home != Some(*chain);
                                let why = if current_home == Some(*chain) {
                                    t(Key::PsCurrentHome)
                                } else if held == Some(true) {
                                    ""
                                } else {
                                    t(Key::PsEnterFirst)
                                };
                                let (resp, _) = f.choice(
                                    ui,
                                    open,
                                    p.home == Some(*chain),
                                    |ui| {
                                        ui.label(egui::RichText::new(name).font(Type::Body.font()).color(c(if open { C::Ink } else { C::Ink3 })));
                                    },
                                    |ui| {
                                        if !why.is_empty() {
                                            ui.label(egui::RichText::new(why).font(Type::Small.font()).color(c(C::Ink3)));
                                        }
                                    },
                                );
                                zikaron_ui::probe::spot(ui.ctx(), &format!("persona:pick:{chain}"), resp.rect);
                                if open && resp.clicked() {
                                    p.home = Some(*chain);
                                }
                            }
                        });
                        sheet::foot_note(ui, t(Key::PsSaidPublic));
                    }
                    Sheet::Enter { chain } => {
                        let name = chains.iter().find(|(c, _)| c == chain).map(|(_, n)| n.clone()).unwrap_or_else(|| chain.to_string());
                        zikaron_ui::fields::rows(ui, |ui, f| {
                            let _ = f.info(ui, t(Key::NetGroup), card::Show::Text(&name));
                        });
                    }
                }
                show_trouble(ui);
            },
            |ui, me| {
                let p = &me.ux.persona;
                let blank = |s: &str| s.trim().is_empty();
                let (word, need): (&str, Vec<(bool, Key)>) = match &which {
                    Sheet::Name => (t(Key::PsWrite), vec![(blank(&p.name), Key::FaceSummaryPersona)]),
                    Sheet::Account { .. } => (t(Key::PsWrite), vec![(blank(&p.platform), Key::PsPlatform), (blank(&p.handle), Key::PsHandle)]),
                    Sheet::Key { address: again, .. } => {
                        let signed = if address_local { (p.pin.chars() != crate::keybox::PIN_LEN, Key::IdPinGate) } else { (again.is_empty() && blank(&p.sig) && !p.address.trim().starts_with(zikaron_glue::face::CONSENT_PREFIX), Key::PsSig) };
                        (t(Key::PsWrite), vec![(blank(&p.address), Key::Address), (blank(&p.role), Key::PsRole), signed])
                    }
                    Sheet::Release { .. } => (t(Key::PsReleaseGo), Vec::new()),
                    Sheet::Reseat { .. } => (t(Key::PsReseat), Vec::new()),
                    Sheet::Home => (t(Key::PsWrite), vec![(p.home.is_none(), Key::NetGroup)]),
                    Sheet::Enter { .. } => (t(Key::PsEnter), Vec::new()),
                };
                let need: Vec<&str> = need.into_iter().filter(|(missing, _)| *missing).map(|(_, k)| t(k)).collect();
                let addr_bad = matches!(which, Sheet::Key { .. }) && crate::addressedx::not_address(&p.address, crate::addressedx::Takes::OrConsent);
                let ready = need.is_empty() && !addr_bad;
                let still = if addr_bad { t(Key::KitToWhyAddress).to_string() } else { fill1(Key::GrantStillNeed, &need.join(t(Key::ListJoin))) };
                let k = key::Key::new(word, Role::Primary).enabled(ready && !busy).phase(phase);
                let r = key::show(ui, if ready { k } else { k.why(&still) });
                zikaron_ui::probe::spot(ui.ctx(), "persona:go", r.rect);
                if r.clicked() && ready && !busy {
                    go = Some(match &which {
                        Sheet::Name => Action::FaceName { name: p.name.clone() },
                        Sheet::Account { .. } => Action::FaceAccount { platform: p.platform.trim().to_string(), handle: p.handle.trim().to_string(), note: p.note.clone() },
                        Sheet::Key { .. } => Action::FaceKey { address: p.address.trim().to_string(), role: p.role.trim().to_string(), note: p.note.clone(), consent: p.sig.trim().to_string(), sign_here: address_local, pin: p.pin.clone() },
                        Sheet::Release { subject, .. } => Action::FaceRelease { subject: subject.clone() },
                        Sheet::Reseat { entry, .. } => Action::FaceReseat { entry: entry.clone() },
                        Sheet::Home => Action::FaceHome { chain: p.home.unwrap_or(0) },
                        Sheet::Enter { chain } => Action::Rotate { step: crate::action::RotateStep::Enroll { chain: *chain } },
                    });
                }
                close = key::key(ui, t(Key::CfBack), Role::Secondary, !busy).clicked();
            },
        );
        if close || (out.esc && !busy) {
            self.ux.persona.sheet = None;
            self.ux.persona.pin.clear();
            return;
        }
        if let Some(a) = go {
            self.ux.persona.trouble = None;
            let r = self.act_carded(a, now);
            self.vault_or(VaultSite::Persona, r, now);
        }
    }
}

impl Win {
    pub(super) fn consent_open(&mut self) {
        self.sheets_clear();
        let p = &mut self.ux.persona;
        p.consent = true;
        p.consent_id.clear();
        p.consent_role.clear();
        p.consent_pin.clear();
        p.consent_out = None;
        p.trouble = None;
        p.shake = None;
    }

    fn consent_close(&mut self) {
        let p = &mut self.ux.persona;
        p.consent = false;
        p.consent_pin.clear();
        p.consent_out = None;
        p.trouble = None;
    }

    pub(super) fn consent_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.persona.consent {
            return;
        }
        let trouble = self.ux.persona.trouble.clone();
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault);
        let len = crate::keybox::PIN_LEN;
        let shake = self.ux.persona.shake;
        let mine = self.shell.anchor.map(|a| a.hex()).unwrap_or_default();
        let out_text = self.ux.persona.consent_out.clone();
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("persona-consent", tk::SHEET_NARROW).shake(shake),
            self,
            |ui, me| {
                let p = &mut me.ux.persona;
                match &out_text {
                    Some(text) => {
                        sheet::title(ui, t(Key::ConsentDone), "");
                        let mut copied = None;
                        fields::stack(ui, |ui, st| {
                            st.field(ui, t(Key::ConsentText), None, None, |ui, w| copied = card::whole(ui, text, w));
                        });
                        tell_copied(ui.ctx(), copied);
                    }
                    None => {
                        sheet::title(ui, t(Key::SetConsentRow), "");
                        let id_bad = crate::addressedx::not_address(&p.consent_id, crate::addressedx::Takes::OrMarker);
                        fields::stack(ui, |ui, st| {
                            st.field(ui, t(Key::ConsentIdentity), None, id_bad.then(|| (t(Key::ConsentIdentityWhy), true)), |ui, w| fields::line(ui, &mut p.consent_id, t(Key::ConsentIdentityHint), w, fields::Look { mono: true, bad: id_bad, ..Default::default() }));
                            st.field(ui, t(Key::PsRole), None, None, |ui, w| fields::line(ui, &mut p.consent_role, "payout", w, fields::Look::default()));
                        });
                        zikaron_ui::fields::rows(ui, |ui, f| {
                            let _ = f.info(ui, t(Key::Address), card::Show::Mono(&mine));
                        });
                        fields::stack(ui, |ui, st| {
                            let _ = pin_field(st, ui, t(Key::IdPinGate), None, "persona-consent-pin", &mut p.consent_pin, shake, !busy);
                        });
                        sheet::foot_note(ui, t(Key::ConsentSaid));
                    }
                }
                if let Some(f) = &trouble {
                    states::err_box(ui, "persona-consent-trouble", f.human(), &f.next(), t(Key::U3RawError), &f.shown());
                }
            },
            |ui, me| {
                let p = &me.ux.persona;
                if out_text.is_some() {
                    close = page::Page::new().primary(ui, t(Key::WizFinish)).1.clicked();
                    return;
                }
                let id_bad = crate::addressedx::not_address(&p.consent_id, crate::addressedx::Takes::OrMarker);
                let still = if id_bad { t(Key::ConsentIdentityWhy).to_string() } else { still_needs(&[(p.consent_id.trim().is_empty(), Key::ConsentIdentity), (p.consent_role.trim().is_empty(), Key::PsRole), (p.consent_pin.chars() != len, Key::IdPinGate)]) };
                let ready = still.is_empty() && !busy;
                let k = key::Key::new(t(Key::ConsentGo), Role::Primary).enabled(ready).phase(if busy { Phase::Busy { frac: None } } else { Phase::Idle });
                let r = key::show(ui, k.why(&still));
                zikaron_ui::probe::spot(ui.ctx(), "persona:consentGo", r.rect);
                go = r.clicked() && ready;
                close = key::key(ui, t(Key::CfBack), Role::Secondary, !busy).clicked();
            },
        );
        if close || (out.esc && !busy) {
            self.consent_close();
            return;
        }
        if go {
            self.ux.persona.trouble = None;
            let p = &mut self.ux.persona;
            let a = Action::FaceConsent { identity: p.consent_id.trim().to_string(), role: p.consent_role.trim().to_string(), pin: std::mem::take(&mut p.consent_pin) };
            let r = self.act_carded(a, now);
            self.vault_or(VaultSite::Persona, r, now);
        }
    }
}

pub(super) fn draw_block(ui: &mut egui::Ui, b: &Block) -> Option<Does> {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        draw_block_in(ui, b)
    })
    .inner
}

fn draw_block_in(ui: &mut egui::Ui, b: &Block) -> Option<Does> {
    let mut hit: Option<Does> = None;
    let take = |ui: &egui::Ui, p: &Press, r: &egui::Response, hit: &mut Option<Does>| {
        zikaron_ui::probe::spot(ui.ctx(), &format!("persona:{}", p.name), r.rect);
        if r.clicked() && p.on {
            *hit = Some(p.does.clone());
        }
    };
    if !b.title.is_empty() || b.head.is_some() {
        card::group_head(ui, &b.title, |ui| {
            if let Some(p) = &b.head {
                let r = key::show(ui, key::Key::new(&p.word, Role::Secondary).enabled(p.on).why(&p.why));
                take(ui, p, &r, &mut hit);
            }
        });
    }
    card::form(ui, |ui, f| {
        if b.lines.is_empty() {
            let _ = f.info(ui, &b.empty, card::Show::Empty(""));
        }
        for l in &b.lines {
            if l.key.is_none() && l.pills.is_empty() {
                let _ = match &l.value {
                    Some(Value::Copy(v)) => f.info(ui, &l.title, card::Show::Copy(v)),
                    Some(Value::Name { lay, copy }) => f.info(ui, &l.title, card::Show::CopyAs(lay, copy)),
                    Some(Value::Mono(v)) => f.info(ui, &l.title, card::Show::Mono(v)),
                    Some(Value::Text(v)) => f.info(ui, &l.title, card::Show::Text(v)),
                    Some(Value::Quiet(v)) => f.info(ui, &l.title, card::Show::Empty(v)),
                    None => f.info(ui, &l.title, card::Show::Empty("")),
                };
                continue;
            }
            let _ = f.line(ui, None, &l.title, &l.sub, false, false, c(C::Ink), |ui| {
                if let Some(p) = &l.key {
                    let r = key::show(ui, key::Key::new(&p.word, Role::Secondary).enabled(p.on).why(&p.why));
                    take(ui, p, &r, &mut hit);
                }
                if let Some(p) = &l.key2 {
                    let r = key::show(ui, key::Key::new(&p.word, Role::Secondary).enabled(p.on).why(&p.why));
                    take(ui, p, &r, &mut hit);
                }
                for (w, tone) in l.pills.iter().rev() {
                    let _ = mark::pill(ui, w, *tone);
                }
                match &l.value {
                    Some(Value::Mono(v)) | Some(Value::Copy(v)) => {
                        ui.label(egui::RichText::new(v).font(Type::MonoSmall.font()).color(c(C::Ink)));
                    }
                    Some(Value::Text(v)) => {
                        ui.label(egui::RichText::new(v).font(Type::Note.font()).color(c(C::Ink2)));
                    }
                    Some(Value::Quiet(v)) => {
                        ui.label(egui::RichText::new(v).font(Type::Note.font()).color(c(C::Ink3)));
                    }
                    Some(Value::Name { lay, .. }) => {
                        ui.label(egui::RichText::new(lay).font(Type::Note.font()).color(c(C::Ink)));
                    }
                    None => {}
                }
            });
        }
    });
    if !b.foot.is_empty() {
        card::group_foot(ui, &b.foot);
    }
    hit
}

#[derive(Clone, Debug)]
pub enum PersonaStep {
    Tab(u8),
    Press(String),
    Fill(&'static str, String),
    Send,
    SendAll,
    Settle,
    Hook(String),
    Wait(f64),
    Opened,
}

#[derive(Clone, Debug)]
pub struct PersonaOut {
    pub step: String,
    pub pressed: Option<bool>,
    pub model: Model,
    pub sheet: Option<String>,
    pub send: Option<usize>,
    pub trouble: Option<String>,
    pub copied: Vec<String>,
    pub keys: Vec<String>,
    pub texts: Vec<String>,
    pub failed: Vec<String>,
    pub card: Option<String>,
    pub consent: Option<String>,
    pub pass_shown: bool,
    pub reroot: Option<String>,
    pub reroot_steps: Vec<String>,
    pub reroot_code: Option<String>,
    pub reroot_t0: Option<f64>,
    pub entry: u64,
    pub blank: usize,
    pub cut_short: Vec<String>,
}

pub fn probe_persona_run(ctx: &egui::Context, shell: Shell, start: Place, steps: &[PersonaStep], mut hook: impl FnMut(&mut Shell, &str), width: f32, height: f32) -> (Shell, Vec<PersonaOut>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(start, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let clock = std::cell::Cell::new(0.0_f64);
    let blank = std::cell::Cell::new(0usize);
    let run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock.set(clock.get() + 0.1);
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock.get()), events, ..Default::default() };
        let out = ctx.run_ui(input, |ui| win.draw(ui));
        if win.ux.reroot.open && matches!(win.ux.nav.place, Some(Place::View(View::Persona, _))) && matches!(win.shell.persona, crate::facex::State::Unread) {
            blank.set(blank.get() + 1);
        }
        out
    };
    let copied_of = |out: &egui::FullOutput| -> Vec<String> {
        out.platform_output.commands.iter().filter_map(|c| match c {
            egui::OutputCommand::CopyText(s) => Some(s.clone()),
            _ => None,
        }).collect()
    };
    let mut out = run(&mut win, Vec::new());
    let first_opened = matches!(steps.first(), Some(PersonaStep::Opened));
    for _ in 0..if first_opened { 0 } else { 4 } {
        out = run(&mut win, Vec::new());
    }
    let busy_kinds = crate::task::Kind::ALL;
    let mut outs = Vec::new();
    for step in steps {
        let mut pressed = None;
        let mut copied = Vec::new();
        blank.set(0);
        const ANSWERS: [crate::task::Kind; 3] = [crate::task::Kind::Vault, crate::task::Kind::Persona, crate::task::Kind::FaceBooks];
        let before: Vec<u64> = ANSWERS.iter().map(|k| win.shell.tasks.starts(*k)).collect();
        match step {
            PersonaStep::Tab(t) => win.go(Place::View(View::Persona, *t), clock.get()),
            PersonaStep::Press(name) => {
                let want = if name.contains(':') && (name.starts_with("cli:") || name.starts_with("persona:") || name.starts_with("reroot:")) { name.clone() } else { format!("persona:{name}") };
                let at = zikaron_ui::probe::spots(ctx).into_iter().find(|(n, _)| *n == want).map(|(_, r)| r.center());
                pressed = Some(at.is_some());
                if let Some(at) = at {
                    let press = |down| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: down, modifiers: egui::Modifiers::NONE };
                    let _ = run(&mut win, vec![egui::Event::PointerMoved(at)]);
                    let o = run(&mut win, vec![press(true), press(false)]);
                    copied.extend(copied_of(&o));
                    let o = run(&mut win, vec![egui::Event::PointerGone]);
                    copied.extend(copied_of(&o));
                }
            }
            PersonaStep::Fill(field, text) => {
                let p = &mut win.ux.persona;
                match *field {
                    "name" => p.name = text.clone(),
                    "platform" => p.platform = text.clone(),
                    "handle" => p.handle = text.clone(),
                    "note" => p.note = text.clone(),
                    "address" => p.address = text.clone(),
                    "role" => p.role = text.clone(),
                    "sig" => p.sig = text.clone(),
                    "pin" => p.pin = crate::secret::Secret::of(text),
                    "consentId" => p.consent_id = text.clone(),
                    "consentRole" => p.consent_role = text.clone(),
                    "consentPin" => p.consent_pin = crate::secret::Secret::of(text),
                    "passNote" => win.ux.pass_note = text.clone(),
                    "passPin" => win.ux.pass_pin = crate::secret::Secret::of(text),
                    "rerootName" => win.ux.reroot.name = text.clone(),
                    "rerootChain" => win.ux.reroot.chain = text.clone(),
                    "rerootCheck" => {
                        if let Some(f) = win.shell.new_words.as_ref() {
                            let words = f.words();
                            for (k, p) in f.picks.iter().enumerate() {
                                let w = words.get(*p).map(|w| w.expose().to_string()).unwrap_or_default();
                                win.ux.reroot.answers[k] = crate::secret::Secret::of(&if text == "wrong" && k == 0 { format!("{w}x") } else { w });
                            }
                        }
                    }
                    _ => {}
                }
            }
            PersonaStep::Send => {
                if let Some(U3Confirm::Send { count }) = win.ux.u3.confirm.clone() {
                    for _ in 0..600 {
                        let _ = run(&mut win, Vec::new());
                        if win.shell.gas.is_some_and(|(n, _)| n == count) && !win.shell.tasks.in_flight(crate::task::Kind::Gas) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    pressed = Some(true);
                    win.send_go(count, clock.get());
                } else {
                    pressed = Some(false);
                }
            }
            PersonaStep::SendAll => {
                let mut sent = 0;
                for _ in 0..8 {
                    if win.shell.sendable_here() == 0 {
                        break;
                    }
                    if !matches!(win.ux.u3.confirm, Some(U3Confirm::Send { .. })) {
                        win.persona_do(Does::Send, clock.get());
                    }
                    let Some(U3Confirm::Send { count }) = win.ux.u3.confirm.clone() else { break };
                    for _ in 0..600 {
                        let _ = run(&mut win, Vec::new());
                        if win.shell.gas.is_some_and(|(n, _)| n == count) && !win.shell.tasks.in_flight(crate::task::Kind::Gas) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    win.send_go(count, clock.get());
                    sent += 1;
                    for _ in 0..3000 {
                        let _ = run(&mut win, Vec::new());
                        if !busy_kinds.iter().any(|k| win.shell.tasks.in_flight(*k)) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
                pressed = Some(sent > 0);
            }
            PersonaStep::Settle => {
                for _ in 0..3000 {
                    let o = run(&mut win, Vec::new());
                    copied.extend(copied_of(&o));
                    if !busy_kinds.iter().any(|k| win.shell.tasks.in_flight(*k)) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            PersonaStep::Hook(label) => hook(&mut win.shell, label),
            PersonaStep::Opened => {}
            PersonaStep::Wait(secs) => {
                let until = clock.get() + secs;
                while clock.get() < until {
                    let o = run(&mut win, Vec::new());
                    copied.extend(copied_of(&o));
                    if busy_kinds.iter().any(|k| win.shell.tasks.in_flight(*k)) {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                }
            }
        }
        let asked: Vec<crate::task::Kind> = ANSWERS.iter().zip(&before).filter(|(k, n)| win.shell.tasks.starts(**k) > **n).map(|(k, _)| *k).collect();
        for _ in 0..if matches!(step, PersonaStep::Opened) { 0 } else { 4 } {
            out = run(&mut win, Vec::new());
            copied.extend(copied_of(&out));
        }
        if !matches!(step, PersonaStep::Opened) {
            let started = |win: &Win| asked.iter().any(|k| win.shell.tasks.in_flight(*k));
            for _ in 0..3000 {
                if !started(&win) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
                out = run(&mut win, Vec::new());
                copied.extend(copied_of(&out));
            }
        }
        let tab = match win.ux.nav.place {
            Some(Place::View(View::Persona, t)) => t,
            _ => 0,
        };
        let sheet = if win.ux.persona.consent {
            Some("consent".to_string())
        } else if win.ux.pass_open {
            Some("pass".to_string())
        } else {
            win.ux.persona.sheet.as_ref().map(|s| s.name().to_string())
        };
        let trouble = win.ux.persona.trouble.as_ref().or(win.ux.pass_trouble.as_ref()).map(|f| f.raw());
        let send = match &win.ux.u3.confirm {
            Some(U3Confirm::Send { count }) => Some(*count),
            _ => None,
        };
        outs.push(PersonaOut {
            step: format!("{step:?}"),
            pressed,
            model: model(&win.shell, tab),
            sheet,
            send,
            trouble,
            copied,
            keys: zikaron_ui::probe::spots(ctx).into_iter().map(|(n, _)| n).collect(),
            texts: texts_of(&out).into_iter().map(|d| d.text).collect(),
            failed: win.shell.failed.iter().map(|(k, f)| format!("{} {}", k.as_str(), f.raw())).collect(),
            card: win.ux.u3.confirm.as_ref().map(|c| format!("{c:?}")),
            consent: win.ux.persona.consent_out.clone(),
            pass_shown: win.ux.pass_out.is_some(),
            reroot: win.ux.reroot.open.then(|| {
                let f = super::reroot::flow(&win.shell);
                format!(
                    "{:?} now={} go={} why={} chain={} chainSaid={} trouble={}",
                    win.ux.reroot.part,
                    f.as_ref().map(|f| f.now).unwrap_or(0),
                    f.as_ref().map(|f| super::reroot::go_said(&f.go)).unwrap_or_default(),
                    f.as_ref().map(|f| f.why.clone()).unwrap_or_default(),
                    win.ux.reroot.chain,
                    super::reroot::chain_trouble(&win.ux.reroot.chain).unwrap_or(""),
                    win.ux.reroot.trouble.as_ref().map(|t| t.raw()).unwrap_or_default()
                )
            }),
            reroot_code: win.ux.reroot.trouble.as_ref().and_then(|t| t.which()).map(|k| k.as_str().to_string()),
            reroot_steps: super::reroot::flow(&win.shell).map(|f| f.steps.iter().enumerate().map(|(i, s)| format!("{} {}", s.name, if s.done { "done" } else if i == f.now { "now" } else { "todo" })).collect()).unwrap_or_default(),
            reroot_t0: zikaron_ui::sheet::entered_at(ctx, "reroot"),
            entry: win.ux.nav.entry_key,
            blank: blank.get(),
            cut_short: zikaron_ui::probe::cuts_short(ctx).into_iter().filter(|(l, _)| l.order == egui::Order::Foreground).map(|(_, t)| t).collect(),
        });
    }
    (win.shell, outs)
}
