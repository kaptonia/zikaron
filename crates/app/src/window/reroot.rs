use super::*;
use crate::facex::Side;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub name: &'static str,
    pub title: String,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Go {
    Make,
    Write(Action),
    Send,
    Reread,
    Fee,
    Switch(String),
    Finish,
}

pub fn go_said(g: &Go) -> String {
    match g {
        Go::Make => "make".to_string(),
        Go::Write(Action::FaceReroot { .. }) => "write:reroot".to_string(),
        Go::Write(Action::FaceHome { chain }) => format!("write:home:{chain}"),
        Go::Write(Action::FaceOrigin { .. }) => "write:origin".to_string(),
        Go::Write(Action::Genesis { .. }) => "write:genesis".to_string(),
        Go::Write(_) => "write".to_string(),
        Go::Send => "send".to_string(),
        Go::Reread => "reread".to_string(),
        Go::Fee => "fee".to_string(),
        Go::Switch(id) => format!("switch:{id}"),
        Go::Finish => "finish".to_string(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Flow {
    pub side: Side,
    pub old: String,
    pub new: Option<String>,
    pub chain: Option<u64>,
    pub steps: Vec<Step>,
    pub now: usize,
    pub go: Go,
    pub why: String,
    pub fund: String,
    pub entry: Option<String>,
}

pub(super) fn waiting<'a>(shell: &'a Shell, table: &zikaron_glue::face::Table, shape: &str) -> Option<(&'a crate::ledgerx::Row, Option<bool>)> {
    let seated = zikaron_glue::facegate::seated_ids(table);
    let (rows, _) = shell.rows.as_ref()?;
    rows.iter()
        .filter(|r| r.kind == zikaron_glue::book::Kind::Face && r.facts.face.as_ref().is_some_and(|(s, _, _)| *s == shape) && !seated.contains(&r.id.to_ascii_lowercase()))
        .max_by_key(|r| r.seq)
        .map(|r| (r, shell.queue.items.iter().find(|q| q.id.eq_ignore_ascii_case(&r.id)).map(|q| q.step.in_flight())))
}

pub fn flow(shell: &Shell) -> Option<Flow> {
    let pair = shell.face_pair.as_ref();
    let own = match &shell.persona {
        crate::facex::State::Read(b) => Some(&b.table),
        _ => None,
    };
    let side = pair.map(|p| p.side).unwrap_or(Side::Old);
    if own.is_none() && side == Side::Old {
        return None;
    }
    let me = own.map(|t| t.root.clone()).or_else(|| shell.identities.as_ref().and_then(|r| r.now().and_then(|(row, _)| row.address(crate::roles::Role::Author))).map(|a| a.hex()))?;
    let (old_t, new_t) = match side {
        Side::Old => (own, pair.and_then(|p| p.table())),
        Side::New => (pair.and_then(|p| p.table()), own),
    };
    let (old, new) = match (side, pair) {
        (Side::Old, Some(p)) => (me.clone(), Some(p.other.hex())),
        (Side::Old, None) => (me.clone(), None),
        (Side::New, Some(p)) => (p.other.hex(), Some(me.clone())),
        (Side::New, None) => (me.clone(), None),
    };
    let (old_id, new_id) = match (side, pair) {
        (Side::Old, Some(p)) => (None, p.other_id.clone()),
        (Side::New, Some(p)) => (p.other_id.clone(), None),
        _ => (None, None),
    };
    let chain = match side {
        Side::Old => pair.and_then(|p| p.other_chain),
        Side::New => shell.settings.chain_id,
    };
    let joined = pair.and_then(|p| p.joined(&shell.persona)) == Some(true);
    let fee = shell.reroot_fee.as_ref().filter(|f| Some(f.to.hex()) == new);
    let reroot_written = own.filter(|_| side == Side::Old).and_then(|t| waiting(shell, t, "reroot"));
    let seated_reroot = old_t.is_some_and(|t| t.reroot.is_some());
    let home_written = own.filter(|_| side == Side::New).and_then(|t| waiting(shell, t, "home"));
    let origin_written = own.filter(|_| side == Side::New).and_then(|t| waiting(shell, t, "origin"));
    let done = [
        new.is_some(),
        seated_reroot || reroot_written.is_some_and(|(_, q)| q != Some(false)),
        seated_reroot,
        new_t.is_some_and(|t| t.home.is_some()) || fee.is_some_and(|f| f.met()) || (side == Side::New && fee.is_none() && seated_reroot),
        new_t.is_some_and(|t| t.home.is_some()),
        new_t.and_then(|t| t.origin.as_ref()).is_some_and(|o| o.other.eq_ignore_ascii_case(&old)),
        joined,
    ];
    let now = done.iter().position(|d| !d).unwrap_or(done.len());
    let titles = [Key::PsStepMake, Key::PsStepReroot, Key::PsStepReadOld, Key::PsStepFee, Key::PsStepHome, Key::PsStepOrigin, Key::PsStepJoined];
    let names = ["make", "reroot", "readOld", "fee", "home", "origin", "joined"];
    let steps: Vec<Step> = (0..7).map(|i| Step { name: names[i], title: t(titles[i]).to_string(), done: done[i] }).collect();
    let mut why = String::new();
    let go = match (now, side) {
        (7, _) => Go::Finish,
        (n, Side::New) if n <= 3 => old_id.clone().map(Go::Switch).unwrap_or(Go::Reread),
        (n, Side::Old) if n >= 4 => new_id.clone().map(Go::Switch).unwrap_or(Go::Reread),
        (0, _) => Go::Make,
        (1, _) => match reroot_written {
            Some((_, Some(false))) => Go::Send,
            _ => Go::Write(Action::FaceReroot { to: new.clone().unwrap_or_default() }),
        },
        (3, _) => match fee {
            Some(f) if f.can_send() => Go::Fee,
            Some(_) => {
                why = t(Key::PsOfficeShort).to_string();
                Go::Reread
            }
            None => Go::Reread,
        },
        (4, _) if !shell.rooted => Go::Write(Action::Genesis { statement: String::new() }),
        (4, _) => match home_written {
            Some((_, Some(false))) => Go::Send,
            Some(_) => Go::Reread,
            None => Go::Write(Action::FaceHome { chain: chain.unwrap_or_default() }),
        },
        (5, _) => match origin_written {
            Some((_, Some(false))) => Go::Send,
            Some(_) => Go::Reread,
            None => Go::Write(Action::FaceOrigin { from: old.clone() }),
        },
        _ => Go::Reread,
    };
    let fund = if why.is_empty() { String::new() } else { new.clone().unwrap_or_default() };
    let entry = match now {
        1 => reroot_written,
        4 => home_written,
        5 => origin_written,
        _ => None,
    }
    .filter(|(_, q)| *q == Some(false))
    .map(|(r, _)| r.id.clone());
    Some(Flow { side, old, new, chain, steps, now, go, why, fund, entry })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Part {
    #[default]
    Intro,
    Words,
    Check,
    Steps,
    Send,
    Fee,
}

#[derive(Default)]
pub struct RerootUx {
    pub open: bool,
    pub part: Part,
    pub name: String,
    pub chain: String,
    pub seen: bool,
    pub answers: [crate::secret::Secret; 3],
    pub trouble: Option<crate::fault::Fault>,
    pub pressed: Option<(crate::task::Kind, u64)>,
    pub lane: Vec<String>,
    pub estimate: Option<usize>,
    pub estimated: Option<u64>,
    pub then_send: bool,
    pub landed: Option<crate::fault::Fault>,
}

pub const PRESETS: [u64; 3] = [1, 42161, 10];

pub fn chain_trouble(text: &str) -> Option<&'static str> {
    if text.trim().is_empty() {
        return Some("");
    }
    match text.trim().parse::<u64>() {
        Err(_) => Some(t(Key::PsChainIdWhole)),
        Ok(c) if crate::action::reroot_row(c).is_err() => Some(t(Key::PsChainNotDeployed)),
        Ok(_) => None,
    }
}

impl Win {
    pub(super) fn reroot_open(&mut self) {
        let started = flow(&self.shell).is_some_and(|f| f.new.is_some());
        let r = &mut self.ux.reroot;
        r.open = true;
        r.trouble = None;
        r.pressed = None;
        r.estimated = None;
        r.part = if started { Part::Steps } else { Part::Intro };
        if !started {
            r.name.clear();
            r.chain = self.shell.settings.chain_id.map(|c| c.to_string()).unwrap_or_default();
        }
    }

    fn reroot_close(&mut self) {
        let r = &mut self.ux.reroot;
        r.open = false;
        r.seen = false;
        r.answers = Default::default();
        r.trouble = None;
        r.pressed = None;
        r.estimated = None;
        r.landed = None;
        if self.shell.new_words.is_some() {
            let _ = apply(&mut self.shell, Action::DropFresh);
        }
    }

    fn reroot_reread(&mut self, now: f64) {
        self.auto(Action::FaceReread, now);
        self.auto(Action::FaceBooks, now);
        if flow(&self.shell).is_some_and(|f| f.now == 3 && f.side == Side::Old) && !self.shell.tasks.in_flight(crate::task::Kind::RerootFee) {
            self.auto(Action::FaceRerootFee { go: false }, now);
        }
    }

    pub(super) fn reroot_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.reroot.open {
            return;
        }
        if self.ux.reroot.part == Part::Check && self.shell.new_words.is_none() && !self.shell.tasks.in_flight(crate::task::Kind::Vault) {
            let me = match &self.shell.persona {
                crate::facex::State::Read(b) => crate::key::Address::parse(&b.table.root),
                _ => None,
            };
            if me.is_some_and(|m| self.shell.identities.as_ref().is_some_and(|reg| reg.rows.iter().any(|r| r.reroot_of == Some(m)))) {
                self.ux.reroot.part = Part::Steps;
                self.ux.reroot.answers = Default::default();
                self.reroot_reread(now);
            }
        }
        let f = flow(&self.shell);
        let part = self.ux.reroot.part;
        if let Some((kind, mark)) = self.ux.reroot.pressed {
            if self.shell.tasks.answered_since(kind, mark) {
                self.ux.reroot.pressed = None;
                let then_send = std::mem::take(&mut self.ux.reroot.then_send);
                let landed = self.ux.reroot.landed.take();
                self.ux.reroot.trouble = self.shell.failed.get(&kind).cloned().or(landed);
                if self.ux.reroot.trouble.is_none() {
                    if let Some(Applied::Trouble(f)) = self.shell.said.get(&kind) {
                        self.ux.reroot.trouble = Some(f.clone());
                    }
                }
                if self.ux.reroot.trouble.is_none() && kind == crate::task::Kind::Anchor {
                    let lane = std::mem::take(&mut self.ux.reroot.lane);
                    self.ux.reroot.trouble = self.shell.queue.items.iter().filter(|q| lane.contains(&q.id)).find_map(|q| match &q.step {
                        crate::queue::Step::Refused { said } if !said.is_empty() => crate::fault::Known::of_str(said).map(|k| crate::fault::Fault::known(k, said.clone())),
                        _ => None,
                    });
                }
                if self.ux.reroot.trouble.is_none() {
                    self.ux.reroot.part = if then_send && self.card_batch().is_some() { Part::Send } else { Part::Steps };
                    self.ux.reroot.estimate = None;
                    self.ux.reroot.estimated = None;
                }
                self.reroot_reread(now);
            }
        }
        if part == Part::Steps && f.as_ref().is_some_and(|f| f.now == 3 && f.side == Side::Old) && self.shell.reroot_fee.is_none() && !self.shell.tasks.in_flight(crate::task::Kind::RerootFee) {
            self.auto(Action::FaceRerootFee { go: false }, now);
        }
        if part == Part::Send && self.ux.reroot.estimate.is_none() {
            let n = self.shell.sendable_now();
            if n > 0 {
                self.ux.reroot.estimate = Some(n);
                let mark = self.shell.tasks.landings(crate::task::Kind::Gas);
                match apply(&mut self.shell, Action::EstimateGas { count: n }) {
                    Applied::Trouble(f) => self.ux.reroot.trouble = Some(f),
                    _ => self.ux.reroot.estimated = Some(mark),
                }
            }
        }
        if let Some(mark) = self.ux.reroot.estimated {
            if self.shell.tasks.answered_since(crate::task::Kind::Gas, mark) {
                self.ux.reroot.estimated = None;
                if let Some(f) = self.shell.failed.get(&crate::task::Kind::Gas) {
                    self.ux.reroot.trouble = Some(f.clone());
                }
            }
        }
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault)
            || self.shell.tasks.in_flight(crate::task::Kind::Anchor)
            || self.shell.tasks.in_flight(crate::task::Kind::Gas)
            || self.shell.tasks.in_flight(crate::task::Kind::Persona)
            || self.shell.tasks.in_flight(crate::task::Kind::RerootFee)
            || self.shell.tasks.in_flight(crate::task::Kind::Gate)
            || self.shell.tasks.in_flight(crate::task::Kind::Seat)
            || self.ux.reroot.pressed.is_some();
        let phase = if busy { Phase::Busy { frac: None } } else { Phase::Idle };
        let own_send = matches!(self.ux.reroot.pressed, Some((crate::task::Kind::Anchor | crate::task::Kind::RerootFee, _)));
        let busy_say = self.busy_said(
            own_send,
            &[crate::task::Kind::Gas, crate::task::Kind::Anchor, crate::task::Kind::RerootFee, crate::task::Kind::Persona, crate::task::Kind::Gate, crate::task::Kind::Vault, crate::task::Kind::Seat],
        );
        let trouble = self.ux.reroot.trouble.clone();
        let fresh = self.shell.new_words.as_ref().map(|w| (w.words(), w.picks));
        let old_home = match &self.shell.persona {
            crate::facex::State::Read(b) => b.table.home,
            _ => None,
        };
        let gas = self.shell.gas.filter(|(n, _)| Some(*n) == self.ux.reroot.estimate).map(|(_, g)| g);
        let cap = self.shell.fees.unwrap_or_else(zikaron_anchor::send::Fees::fallback).cap_wei();
        let gas_said = gas.map(|g| format!("{g} \u{b7} {}", eth_said(cap))).unwrap_or_default();
        let fee = self.shell.reroot_fee.clone();
        let office = self.shell.anchor.map(|a| a.hex()).unwrap_or_default();
        let batch = self.shell.next_batch(usize::MAX);
        let ahead = match f.as_ref().and_then(|f| f.entry.as_ref()) {
            Some(e) if self.goes_later(e, usize::MAX) => batch.len(),
            _ => 0,
        };
        let anchoring = self.shell.settings.chain_id;
        let can_send = self.card_batch().is_some();
        let (mut go, mut close, mut next): (Option<Action>, bool, Option<Part>) = (None, false, None);
        let mut make = false;
        let mut switch: Option<String> = None;
        let mut reread = false;
        let mut finish = false;
        let read_to = match &self.shell.persona {
            crate::facex::State::Read(b) => fill1(Key::PsStepReadTo, &crate::when::when(b.moment)),
            _ => t(Key::PsNotReadPill).to_string(),
        };
        let out = sheet::show(
            ctx,
            sheet::Spec::new("reroot", tk::SHEET_WIDE),
            self,
            |ui, me| {
                let r = &mut me.ux.reroot;
                sheet::title(ui, t(Key::PsReroot), "");
                match r.part {
                    Part::Intro => {
                        let root = f.as_ref().map(|f| f.old.clone()).unwrap_or_default();
                        zikaron_ui::fields::rows(ui, |ui, fr| {
                            let _ = fr.info(ui, t(Key::PsOldRoot), card::Show::Mono(&root));
                            let _ = fr.info(ui, t(Key::PsTabHome), card::Show::Text(&old_home.map(|c| crate::readnets::chain_name(c, None).to_string()).unwrap_or_default()));
                        });
                        fields::stack(ui, |ui, st| {
                            st.field(ui, t(Key::PsNewIdentity), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut r.name, "", w, fields::Look::default()));
                            st.field(ui, t(Key::PsHomeChainId), None, chain_trouble(&r.chain).map(|w| (w, true)), |ui, w| fields::line(ui, &mut r.chain, "", w, fields::Look { mono: true, ..Default::default() }));
                        });
                        ui.horizontal_wrapped(|ui| {
                            for c in PRESETS {
                                let name = crate::readnets::chain_name(c, None).to_string();
                                let resp = key::key(ui, &name, Role::Secondary, true);
                                zikaron_ui::probe::spot(ui.ctx(), &format!("reroot:preset:{c}"), resp.rect);
                                if resp.clicked() {
                                    r.chain = c.to_string();
                                }
                            }
                        });
                    }
                    Part::Words => {
                        let words = fresh.as_ref().map(|(w, _)| w.clone());
                        let resp = zikaron_ui::sheetparts::words(ui, "reroot-words", words.as_deref(), r.seen, t(Key::IdShowNewWords));
                        zikaron_ui::probe::spot(ui.ctx(), "reroot:words", resp.rect);
                        if resp.clicked() {
                            r.seen = true;
                        }
                    }
                    Part::Check => {
                        if let Some((_, picks)) = fresh.as_ref() {
                            fields::stack(ui, |ui, st| {
                                for (k, p) in picks.iter().enumerate() {
                                    st.field(ui, &fill1(Key::IdWordN, &(p + 1).to_string()), None, None, |ui, w| input::secret_line_w_marked(ui, &mut r.answers[k], "", w, false));
                                }
                            });
                        }
                    }
                    Part::Steps => {
                        if let Some(f) = f.as_ref() {
                            zikaron_ui::fields::rows(ui, |ui, fr| {
                                for (i, s) in f.steps.iter().enumerate() {
                                    if i == 0 {
                                        let _ = fr.info(ui, t(Key::PsInOld), card::Show::Empty(""));
                                    }
                                    if i == 4 {
                                        let _ = fr.info(ui, t(Key::PsInNew), card::Show::Empty(""));
                                    }
                                    let m = if s.done {
                                        Mark::Ok
                                    } else if i == f.now && busy {
                                        Mark::Work
                                    } else {
                                        Mark::Todo
                                    };
                                    let _ = fr.line(ui, None, &s.title, "", false, false, c(C::Ink), |ui| {
                                        let (rr, _) = ui.allocate_exact_size(egui::vec2(tk::MARK, tk::MARK), egui::Sense::hover());
                                        zikaron_ui::mark::paint_mark(ui.ctx(), ui.painter(), rr.center(), m, tk::MARK);
                                    });
                                }
                            });
                            if !f.why.is_empty() {
                                zikaron_ui::callout::callout(ui, &f.why);
                                if !f.fund.is_empty() {
                                    let copied = card::whole(ui, &f.fund, ui.available_width());
                                    tell_copied(ui.ctx(), copied);
                                }
                            } else if matches!(f.go, Go::Reread) {
                                zikaron_ui::callout::callout(ui, &read_to);
                            } else if matches!(f.go, Go::Send) {
                                zikaron_ui::callout::callout(ui, t(if can_send { Key::PsWrittenWaits } else { Key::PsWrittenAfter }));
                            }
                        }
                    }
                    Part::Send => {
                        let what = match f.as_ref().map(|f| f.now) {
                            _ if ahead > 0 => format!("{} \u{b7} {}", t(Key::U3BatchLabel), fill1(Key::U3EntriesCount, &ahead.to_string())),
                            Some(1) => t(Key::PsRerootFinal).to_string(),
                            Some(4) => t(Key::PsStepHome).to_string(),
                            _ => t(Key::PsStepOrigin).to_string(),
                        };
                        let chain = if ahead > 0 { anchoring } else { f.as_ref().and_then(|f| if f.now == 1 { old_home } else { f.chain }) };
                        let then = (ahead > 0).then(|| f.as_ref().and_then(|f| f.steps.get(f.now)).map(|s| fill1(Key::PsSendThen, &s.title))).flatten();
                        zikaron_ui::fields::rows(ui, |ui, fr| {
                            let _ = fr.info(ui, &what, card::Show::Empty(""));
                            if let Some(then) = &then {
                                let _ = fr.info(ui, then, card::Show::Empty(""));
                            }
                            let _ = fr.info(ui, t(Key::NetGroup), card::Show::Text(&chain.map(|c| crate::readnets::chain_name(c, None).to_string()).unwrap_or_default()));
                            let _ = fr.info(ui, t(Key::PsSendingKey), card::Show::Mono(&office));
                            let _ = fr.info(ui, t(Key::U3GasEstimate), if gas.is_some() { card::Show::Mono(&gas_said) } else { card::Show::Empty(t(Key::U3Estimating)) });
                        });
                    }
                    Part::Fee => {
                        if let Some(fee) = fee.as_ref() {
                            zikaron_ui::fields::rows(ui, |ui, fr| {
                                let _ = fr.info(ui, &fill1(Key::PsTransfer, &eth_said(fee.give)), card::Show::Empty(""));
                                let _ = fr.info(ui, t(Key::NetGroup), card::Show::Text(&crate::readnets::chain_name(fee.chain, None).to_string()));
                                let _ = fr.info(ui, t(Key::PsSendingKey), card::Show::Mono(&office));
                                let _ = fr.info(ui, t(Key::U3GasEstimate), card::Show::Mono(&eth_said(fee.cost)));
                            });
                        }
                    }
                }
                if let Some(tr) = &trouble {
                    states::err_box(ui, "reroot-trouble", tr.human(), &tr.next(), t(Key::U3RawError), &tr.shown());
                }
            },
            |ui, me| {
                let r = &me.ux.reroot;
                let primary = |ui: &mut egui::Ui, word: &str, on: bool| {
                    let resp = key::show(ui, key::Key::new(word, Role::Primary).enabled(on && !busy).phase(phase));
                    zikaron_ui::probe::spot(ui.ctx(), "reroot:go", resp.rect);
                    resp.clicked() && on && !busy
                };
                match r.part {
                    Part::Intro => {
                        if primary(ui, t(Key::RotStart), chain_trouble(&r.chain).is_none()) {
                            make = true;
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, !busy).clicked();
                    }
                    Part::Words => {
                        if primary(ui, t(Key::IdDoCopied), r.seen) {
                            next = Some(Part::Check);
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, !busy).clicked();
                    }
                    Part::Check => {
                        if primary(ui, t(Key::IdDoConfirm), r.answers.iter().all(|a| a.chars() > 0)) {
                            let answers: Vec<(usize, crate::secret::Secret)> = fresh.as_ref().map(|(_, picks)| picks.iter().copied().zip(r.answers.iter().cloned()).collect()).unwrap_or_default();
                            go = Some(Action::FaceRerootMake { answers, label: r.name.trim().to_string(), chain: r.chain.trim().parse().unwrap_or(0) });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, !busy).clicked();
                    }
                    Part::Steps => {
                        let Some(f) = f.as_ref() else {
                            close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                            return;
                        };
                        match &f.go {
                            Go::Finish => finish = primary(ui, t(Key::WizFinish), true),
                            Go::Switch(id) => {
                                let word = t(if f.side == Side::Old { Key::PsToNew } else { Key::PsToOld });
                                if primary(ui, word, true) {
                                    switch = Some(id.clone());
                                }
                            }
                            Go::Make => make = primary(ui, t(Key::PsMake), true),
                            Go::Write(a) => {
                                if primary(ui, t(Key::PsWrite), true) {
                                    go = Some(a.clone());
                                }
                            }
                            Go::Send => {
                                if primary(ui, t(Key::U3SendTitle), can_send) {
                                    next = Some(Part::Send);
                                }
                            }
                            Go::Fee => {
                                if primary(ui, t(Key::PsSendFee), true) {
                                    next = Some(Part::Fee);
                                }
                            }
                            Go::Reread => reread = primary(ui, t(Key::PsReread), true),
                        }
                        if f.go != Go::Finish {
                            let resp = key::key(ui, t(Key::PsLater), Role::Secondary, true);
                            zikaron_ui::probe::spot(ui.ctx(), "reroot:later", resp.rect);
                            close = resp.clicked();
                        }
                    }
                    Part::Send | Part::Fee => {
                        let send = r.part == Part::Send;
                        let (word, on, why) = if send {
                            (t(Key::U3SendGo), gas.is_some(), t(Key::U3Estimating))
                        } else {
                            (t(Key::PsSendFee), fee.as_ref().is_some_and(|f| f.can_send()), t(Key::PsOfficeShort))
                        };
                        let resp = page::Pen::new().press_saying_why(ui, word, busy_say, on && !busy, phase, if on { "" } else { why });
                        zikaron_ui::probe::spot(ui.ctx(), "reroot:go", resp.rect);
                        if resp.clicked() && on && !busy {
                            go = Some(if send { Action::SendBatch { count: r.estimate.unwrap_or(1) } } else { Action::FaceRerootFee { go: true } });
                        }
                        if key::key(ui, t(Key::CfBack), Role::Secondary, !busy).clicked() {
                            next = Some(Part::Steps);
                        }
                    }
                }
            },
        );
        if close || (out.esc && !busy) {
            self.reroot_close();
            return;
        }
        if let Some(p) = next {
            self.ux.reroot.part = p;
            self.ux.reroot.trouble = None;
            self.ux.reroot.estimate = None;
            self.ux.reroot.estimated = None;
        }
        if reread {
            self.reroot_reread(now);
        }
        if finish {
            self.reroot_close();
            self.toasts.say(t(Key::PsChecked), Tone::Note, now);
            return;
        }
        if let Some(id) = switch {
            let _ = self.act(Action::SwitchIdentity { id }, now);
            return;
        }
        if make {
            match apply(&mut self.shell, Action::NewIdentity) {
                Applied::FreshWords => {
                    self.ux.reroot.part = Part::Words;
                    self.ux.reroot.seen = false;
                    self.ux.reroot.trouble = None;
                }
                Applied::Trouble(f) => self.ux.reroot.trouble = Some(f),
                _ => {}
            }
            return;
        }
        let Some(a) = go else { return };
        self.reroot_apply(a, now);
    }

    fn reroot_apply(&mut self, a: Action, now: f64) {
        self.ux.reroot.trouble = None;
        let marks: Vec<(crate::task::Kind, u64)> = [crate::task::Kind::Gate, crate::task::Kind::Anchor, crate::task::Kind::RerootFee].iter().map(|k| (*k, self.shell.tasks.landings(*k))).collect();
        self.ux.reroot.lane = self.shell.next_batch_here(usize::MAX);
        self.ux.reroot.then_send = match &a {
            Action::FaceReroot { .. } | Action::FaceHome { .. } | Action::FaceOrigin { .. } | Action::Genesis { .. } => true,
            Action::SendBatch { .. } => flow(&self.shell).and_then(|f| f.entry).is_some_and(|e| !self.ux.reroot.lane.iter().any(|b| b.eq_ignore_ascii_case(&e))),
            _ => false,
        };
        match apply(&mut self.shell, a) {
            Applied::Queued { .. } => {
                self.ux.reroot.part = if self.card_batch().is_some() { Part::Send } else { Part::Steps };
                self.ux.reroot.estimate = None;
                self.ux.reroot.estimated = None;
                if self.ux.reroot.part == Part::Steps {
                    self.reroot_reread(now);
                }
            }
            Applied::Genesised { .. } => match {
                self.shell.book_changed();
                self.shell.count_here();
                flow(&self.shell).map(|f| f.go)
            } {
                Some(Go::Write(home @ Action::FaceHome { .. })) => self.reroot_apply(home, now),
                _ => self.reroot_reread(now),
            },
            Applied::RerootMade { .. } => {
                self.ux.reroot.part = Part::Steps;
                self.ux.reroot.answers = Default::default();
                self.reroot_reread(now);
            }
            Applied::Started(k) if marks.iter().any(|(m, _)| *m == k) => {
                self.ux.reroot.pressed = marks.iter().find(|(m, _)| *m == k).copied();
                if k == crate::task::Kind::Gate {
                    self.ux.reroot.landed = None;
                    self.ux.gate_site = Some(super::VaultSite::Reroot);
                }
            }
            Applied::Started(crate::task::Kind::Vault) => {}
            Applied::Trouble(f) => {
                self.ux.reroot.trouble = Some(f);
                self.reroot_reread(now);
            }
            _ => {}
        }
    }
}
