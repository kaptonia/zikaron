use super::*;

pub(super) fn lang_pick(ui: &mut egui::Ui, id_salt: &str) -> Option<crate::lang::Lang> {
    let langs = crate::lang::Lang::ALL;
    let cur = langs.iter().position(|l| *l == crate::lang::lang()).unwrap_or(0);
    let labels: Vec<&str> = langs.iter().map(|l| l.label()).collect();
    zikaron_ui::covers::seg_small(ui, id_salt, &labels, cur).filter(|i| *i != cur).map(|i| langs[i])
}

pub(super) fn field<R>(ui: &mut egui::Ui, label: &str, optional: Option<&str>, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = tk::S2;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = tk::S2;
            paint::text(ui, label, Type::Note, c(C::Ink2));
            if let Some(o) = optional {
                paint::text(ui, o, Type::Note, c(C::Ink3));
            }
        });
        add(ui)
    })
    .inner
}

pub(super) fn path_row(ui: &mut egui::Ui, label: &str, path: &str, key_label: &str, none: &str) -> bool {
    let mut hit = false;
    let w = ui.available_width();
    ui.allocate_ui_with_layout(egui::vec2(w, tk::KEY_H), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = tk::S3;
        if !label.is_empty() {
            let (r, _) = ui.allocate_exact_size(egui::vec2(tk::LABEL_W - tk::S3 + tk::S2, tk::KEY_H), egui::Sense::hover());
            paint::at(ui.painter(), ui, egui::pos2(r.left(), r.center().y), egui::Align2::LEFT_CENTER, label, Type::Note, c(C::Ink2), r.width());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            hit = key::key(ui, key_label, Role::Secondary, true).clicked();
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let room = ui.available_width();
                let p = path.trim();
                if p.is_empty() {
                    paint::line(ui, none, Type::Body, c(C::Ink3), room);
                } else {
                    let r = paint::line(ui, &width::file_name(p), Type::Mono, c(C::Ink2), room);
                    zikaron_ui::layer::tip(r, p);
                }
            });
        });
    });
    hit
}

#[derive(Clone, Default)]
pub(super) struct PathMail {
    pub(super) asked: Option<(egui::Id, crate::platform::Pick)>,
    pub(super) ticket: Option<egui::Id>,
    pub(super) answer: Option<(egui::Id, String, u64)>,
}

pub(super) fn path_mail() -> egui::Id {
    egui::Id::new("zikaron-path-mail")
}

pub(super) fn path_answer(ctx: &egui::Context, site: egui::Id, ask: bool, kind: crate::platform::Pick) -> Option<String> {
    ctx.data_mut(|d| {
        let m = d.get_temp_mut_or_default::<PathMail>(path_mail());
        if ask {
            m.asked = Some((site, kind));
        }
        if m.answer.as_ref().is_some_and(|a| a.0 == site) {
            m.answer.take().map(|a| a.1)
        } else {
            None
        }
    })
}

pub(super) fn pick_label(kind: crate::platform::Pick) -> &'static str {
    match kind {
        crate::platform::Pick::File => t(Key::PickFile),
        crate::platform::Pick::Folder => t(Key::PickFolder),
        crate::platform::Pick::FileOrFolder if crate::platform::dialog_brings_folders() => t(Key::PickEither),
        crate::platform::Pick::FileOrFolder => t(Key::PickFile),
    }
}

pub(super) fn pick_path(ui: &mut egui::Ui, slot: &mut String, kind: crate::platform::Pick) -> bool {
    let asked = path_row(ui, "", slot, pick_label(kind), t(Key::PickNone));
    let site = egui::Id::new(("zikaron-pick-path", slot as *const String as usize));
    match path_answer(ui.ctx(), site, asked, kind) {
        Some(p) => {
            *slot = p;
            true
        }
        None => false,
    }
}

pub(super) fn foot_pick(ui: &mut egui::Ui, salt: &str, kind: crate::platform::Pick) -> Option<String> {
    let asked = ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| key::key(ui, pick_label(kind), Role::Secondary, true).clicked()).inner;
    path_answer(ui.ctx(), egui::Id::new(("zikaron-foot-pick", salt)), asked, kind)
}

pub(super) fn tell_copied(ctx: &egui::Context, copied: Option<String>) {
    if let Some(s) = copied {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("zikaron-copied"), s));
    }
}

pub(super) fn keys_row<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(tk::S2, tk::S2);
        add(ui)
    })
    .inner
}

pub(super) fn details_card(ui: &mut egui::Ui, id_salt: &str, rows: &[(&str, Val)]) {
    let (detail, widest) = detail_rows(ui, rows);
    let copied = card::form(ui, |ui, f| f.details(ui, id_salt, t(Key::SetEvidence), &detail, widest));
    tell_copied(ui.ctx(), copied);
}

pub(super) fn details_card_held(ui: &mut egui::Ui, id_salt: &str, rows: &[(&str, Val)], open: &mut bool) {
    let (detail, widest) = detail_rows(ui, rows);
    let copied = card::form(ui, |ui, f| f.details_held(ui, id_salt, t(Key::SetEvidence), &detail, widest, open));
    tell_copied(ui.ctx(), copied);
}

pub(super) fn details_row(ui: &mut egui::Ui, f: &mut card::Form, id_salt: &str, rows: &[(&str, Val)]) {
    let (detail, widest) = detail_rows(ui, rows);
    let copied = f.details(ui, id_salt, t(Key::SetEvidence), &detail, widest);
    tell_copied(ui.ctx(), copied);
}

pub(super) fn details_list(ui: &mut egui::Ui, id_salt: &str, rows: &[(&str, Val)]) {
    let (detail, widest) = detail_rows(ui, rows);
    let copied = card::form(ui, |ui, f| f.detail_rows(ui, id_salt, &detail, widest));
    tell_copied(ui.ctx(), copied);
}

fn detail_rows<'a>(ui: &egui::Ui, rows: &[(&'a str, Val)]) -> (Vec<(&'a str, card::Detail)>, f32) {
    let font = zikaron_ui::tokens::Type::Note.font();
    let widest = rows
        .iter()
        .flat_map(|(k, _)| [Some(*k), crate::lang::twin(k)])
        .flatten()
        .map(|k| ui.painter().layout_no_wrap(k.to_string(), font.clone(), egui::Color32::BLACK).size().x)
        .fold(0.0, f32::max);
    let detail = rows
        .iter()
        .map(|(k, v)| {
            let d = match v {
                Val::Mono(s) => card::Detail::Mono(s.clone()),
                Val::Figure(s) => card::Detail::Figure(s.clone()),
                Val::Quiet(s) => card::Detail::Quiet(s.clone()),
                Val::Text(s) | Val::Mark(_, s) => card::Detail::Text(s.clone()),
                Val::Loading(_) => card::Detail::Quiet(String::new()),
                Val::Amount(a) => card::Detail::Figure(a.whole.clone()),
            };
            (*k, d)
        })
        .collect();
    (detail, widest)
}

pub(super) fn hint(ui: &mut egui::Ui, s: &str) {
    states::hint(ui, s);
}

pub(super) fn lamp_hero(l: crate::ledgerx::Lamp, at: Option<u64>) -> zikaron_ui::card::HeroPill {
    let (tone, live) = lamp_pill(l);
    zikaron_ui::card::HeroPill { words: lamp_label(l, false, at), tone, live }
}

pub(super) fn lamp_pill_ui(ui: &mut egui::Ui, l: crate::ledgerx::Lamp, at: Option<u64>) {
    let (tone, live) = lamp_pill(l);
    let words = lamp_label(l, false, at);
    if live {
        mark::pill_live(ui, &words, tone);
    } else {
        mark::pill(ui, &words, tone);
    }
}

impl Win {
    pub(super) fn last_batch_rows(&self, ui: &mut egui::Ui, f: &mut card::Form) -> Option<String> {
        let b = crate::queue::last_batch(&self.shell.queue, self.shell.sent.as_ref())?;
        let count = |n: usize| fill1(Key::U3EntriesCount, &n.to_string());
        let (m, said) = match b.stands {
            crate::queue::Stands::Awaited(n) => (Mark::Busy, format!("{} \u{b7} {}", count(n), t(Key::LampSubmitted))),
            crate::queue::Stands::Included(n) => (Mark::Ok, format!("{} \u{b7} {}", count(n), t(Key::LampAnchored))),
            crate::queue::Stands::Ended { confirmed, sent, dropped } => (if confirmed { Mark::Ok } else { Mark::Busy }, fill2(Key::U3SentSay, &sent.to_string(), &dropped.to_string())),
        };
        let _ = f.info(ui, t(Key::U3LastSend), card::Show::Mark(m, &said));
        f.info(ui, t(Key::SendTx), card::Show::Copy(&format!("{} \u{b7} {}", b.chain, b.tx)))
    }

    pub(super) fn last_batch_card(&self, ui: &mut egui::Ui, close: bool) {
        use super::home::tiles_close_up;
        if crate::queue::last_batch(&self.shell.queue, self.shell.sent.as_ref()).is_none() {
            return;
        }
        if close {
            tiles_close_up(ui);
        }
        let copied = card::form(ui, |ui, f| self.last_batch_rows(ui, f));
        tell_copied(ui.ctx(), copied);
    }

    pub(super) fn work_label(&self, work: &str) -> String {
        self.shell
            .rows
            .as_ref()
            .and_then(|(rows, _)| {
                rows.iter()
                    .filter(|r| r.kind == zikaron_glue::book::Kind::History)
                    .find(|r| r.work.as_deref().map(|w| w.eq_ignore_ascii_case(work)).unwrap_or(false))
                    .and_then(|r| r.facts.record_name())
            })
            .unwrap_or_else(|| t(Key::UnnamedRecord).to_string())
    }

    pub(super) fn recent_addresses(&self) -> Vec<String> {
        let mut v: Vec<String> = self.shell.settings.book.clone();
        if let Some(g) = self.shell.grants.as_ref() {
            for r in g.iter().rev() {
                if !v.iter().any(|x| x.eq_ignore_ascii_case(&r.grantee)) {
                    v.push(r.grantee.clone());
                }
            }
        }
        v
    }

    pub(super) fn phase_of(&self, k: crate::task::Kind) -> Phase {
        if self.shell.tasks.in_flight(k) {
            return Phase::Busy { frac: crate::task::stage(k).and_then(|s| s.frac()) };
        }
        match self.ux.landed.get(&k) {
            Some((true, at)) => Phase::Done { at: *at },
            Some((false, at)) => Phase::Fail { at: *at },
            None => Phase::Idle,
        }
        .now(self.ux.now)
    }

    pub(super) fn busy_said(&self, sent: bool, kinds: &[crate::task::Kind]) -> &'static str {
        if sent {
            return t(Key::U3Sending);
        }
        match kinds.iter().copied().find(|k| self.shell.tasks.in_flight(*k)) {
            Some(crate::task::Kind::Gas) | None => t(Key::U3Estimating),
            Some(k) => t(super::own::task_key(k)),
        }
    }

    pub(super) fn long_key(&self, ui: &mut egui::Ui, text: &str, role: Role, enabled: bool, k: crate::task::Kind) -> bool {
        key::show(ui, key::Key::new(text, role).enabled(enabled).phase(self.phase_of(k))).clicked()
    }

    pub(super) fn stage_line(&self, ui: &mut egui::Ui, k: crate::task::Kind) {
        if !self.status_reports(k) {
            self.stage_row(ui, k);
        }
    }

    pub(super) fn stage_row(&self, ui: &mut egui::Ui, k: crate::task::Kind) {
        let words = stage_words(k);
        let running = self.shell.tasks.in_flight(k);
        let id = ui.id().with(("zikaron-stage", k as u8));
        let open = motion::flag(ui.ctx(), id.with("open"), running && !words.is_empty(), tk::MID);
        if open <= 0.0 {
            return;
        }
        let st = crate::task::stage(k).unwrap_or_default();
        let at = (st.at as usize).min(words.len().saturating_sub(1));
        let h = 22.0 * open;
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        let p = ui.painter().with_clip_rect(rect);
        let cy = rect.top() + 11.0;
        let mut x = rect.left();
        let a = open;
        if at > 0 {
            zikaron_ui::icons::glyph_at(&p, Glyph::Ok, egui::pos2(x + 6.0, cy), 12.0, c(C::Ok).gamma_multiply(a));
            x += 12.0 + 6.0;
            let r = p.text(egui::pos2(x, cy), egui::Align2::LEFT_CENTER, t(words[at - 1]), Type::Small.font(), c(C::Ink3).gamma_multiply(a));
            x = r.right() + 12.0;
        }
        let ph = motion::cycle(ui.ctx(), tk::CYCLE);
        let breath = motion::Curve::InOut.at(if ph < 0.5 { ph * 2.0 } else { 2.0 - ph * 2.0 });
        p.circle_filled(egui::pos2(x + 3.0, cy), 3.0 * (0.8 + 0.2 * breath), c(C::Accent).gamma_multiply(a * (0.35 + 0.65 * breath)));
        x += 6.0 + 6.0;
        let age = motion::age(ui.ctx(), id.with("words"), at as u64);
        let wa = (age / tk::FAST).clamp(0.0, 1.0);
        let r = p.text(egui::pos2(x, cy + 3.0 * (1.0 - wa)), egui::Align2::LEFT_CENTER, t(words[at]), Type::Small.font(), c(C::Ink2).gamma_multiply(a * wa));
        if st.total > 0 {
            p.text(egui::pos2(r.right() + 6.0, cy), egui::Align2::LEFT_CENTER, format!("{} / {}", st.done, st.total), Type::MonoSmall.font(), c(C::Ink3).gamma_multiply(a));
        }
    }
}

pub(super) fn stage_words(k: crate::task::Kind) -> &'static [Key] {
    use crate::task::Kind;
    match k {
        Kind::Anchor => &[Key::StageSign, Key::StageBroadcast],
        Kind::Check => &[Key::StageParseGrant, Key::StageReadChain, Key::OsSixChecks, Key::StageCompareFile, Key::StageCompareTerms],
        Kind::Verify => &[Key::StageReadKit, Key::StageReadChain, Key::StageCompareEach],
        Kind::Book => &[Key::StageConnect, Key::StageScanBlocks],
        Kind::Diligence => &[Key::StageConnect, Key::StageScanBlocks, Key::StageCheckLedger],
        Kind::Kit => &[Key::StageGather, Key::StageCheckOriginals, Key::StageWriteKit, Key::StageReadChain, Key::StageSeal],
        Kind::Badge => &[Key::StageGrantChain, Key::StageWriteBadge],
        Kind::Review => &[Key::StageReadChain, Key::StageReadUpstream],
        Kind::Reconcile => &[Key::StageReconcile],
        Kind::Archive => &[Key::DoMeasure],
        Kind::Publish => &[Key::StageFetchFiles],
        Kind::Backup => &[Key::BackupStageCollect, Key::BackupCipher, Key::SetPen, Key::BackupStageVerify],
        Kind::Chain => &[Key::StageReadChain],
        Kind::ChainTime => &[Key::StageReadChain],
        Kind::Rotate => &[Key::StageReadChain],
        Kind::Gate => &[Key::StageReadChain],
        Kind::ReadNet => &[Key::StageConnect],
        Kind::Basis => &[Key::StageConnect],
        _ => &[],
    }
}

pub(super) const DATE_W: f32 = 136.0;
const SEARCH_MIN_W: f32 = 180.0;

pub(super) fn date_box(ui: &mut egui::Ui, id: &str, value: &mut String, hint: &str, now_secs: u64) -> bool {
    datepick::field(ui, id, value, hint, DATE_W, today_of(now_secs), &date_words())
}

pub(super) fn date_words() -> datepick::Words<'static> {
    let weekdays = [Key::DateMon, Key::DateTue, Key::DateWed, Key::DateThu, Key::DateFri, Key::DateSat, Key::DateSun].map(t);
    datepick::Words { weekdays, month: month_title, year: year_alone, month_alone, today: t(Key::DateToday), clear: t(Key::PickClear) }
}

fn month_title(y: i32, m: u32) -> String {
    fill2(Key::DateMonth, &y.to_string(), &m.to_string())
}

fn year_alone(y: i32) -> String {
    fill1(Key::DateYearAlone, &y.to_string())
}

fn month_alone(m: u32) -> String {
    fill1(Key::DateMonthAlone, &m.to_string())
}

pub(super) fn today_of(now_secs: u64) -> datepick::Day {
    datepick::parse(&crate::when::day(now_secs)).unwrap_or((1970, 1, 1))
}

pub(super) const PICK_W: f32 = 360.0;
pub(super) const PICK_WIDE_W: f32 = 560.0;

pub(super) fn date_pair(ui: &mut egui::Ui, id: &str, range: &mut (String, String), now_secs: u64) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = tk::S2;
        date_box(ui, &format!("{id}-from"), &mut range.0, t(Key::DateFrom), now_secs);
        date_box(ui, &format!("{id}-to"), &mut range.1, t(Key::DateTo), now_secs);
    });
}

pub(super) fn search_row(ui: &mut egui::Ui, id: &str, query: &mut String, hint: &str, range: &mut (String, String), now_secs: u64) {
    let w = ui.available_width();
    let hint_w = ui.painter().layout_no_wrap(hint.to_string(), zikaron_ui::tokens::Type::Small.font(), egui::Color32::BLACK).size().x;
    let search_need = SEARCH_MIN_W.max(34.0 + hint_w + 12.0);
    if w >= DATE_W * 2.0 + tk::S2 * 2.0 + search_need {
        width::then(
            ui,
            |ui| {
                date_box(ui, &format!("{id}-to"), &mut range.1, t(Key::DateTo), now_secs);
                date_box(ui, &format!("{id}-from"), &mut range.0, t(Key::DateFrom), now_secs);
            },
            |ui, room| input::search(ui, query, hint, room),
        );
    } else {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = tk::S2;
            input::search(ui, query, hint, w);
            date_pair(ui, id, range, now_secs);
        });
    }
}

pub(super) fn matches(query: &str, fields: &[&str]) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || fields.iter().any(|f| f.to_lowercase().contains(&q))
}

fn boxed_cell<R>(ui: &mut egui::Ui, salt: &str, w: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> (R, egui::Rect) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, tk::INPUT_H), egui::Sense::hover());
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(("zikaron-boxed", salt)).max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    (add(&mut child), rect)
}

fn inside_end<R>(ui: &mut egui::Ui, frame: egui::Rect, w: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let zone = egui::Rect::from_min_max(egui::pos2(frame.right() - tk::INPUT_PAD_X - w, frame.top() + 1.0), egui::pos2(frame.right() - 1.0, frame.bottom() - 1.0));
    ui.painter().rect_filled(zone, egui::CornerRadius { nw: 0, sw: 0, ne: 7, se: 7 }, c(C::Surface));
    let inner = egui::Rect::from_min_max(egui::pos2(zone.left() + tk::S2, zone.top()), egui::pos2(zone.right() - tk::INPUT_PAD_X + 1.0, zone.bottom()));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    add(&mut child)
}

pub(super) fn secret_strength(ui: &mut egui::Ui, salt: &str, text: &mut crate::secret::Secret, hint: &str, w: f32) -> egui::Response {
    let (r, frame) = boxed_cell(ui, salt, w, |ui| input::secret_line_w_marked(ui, text, hint, w, false));
    if !text.is_empty() {
        let (lit, m, k) = match crate::keystore::strength(text.expose()) {
            crate::keystore::Strength::Weak => (1, Mark::Bad, Key::StrengthWeak),
            crate::keystore::Strength::Fair => (2, Mark::Warn, Key::StrengthFair),
            crate::keystore::Strength::Strong => (3, Mark::Ok, Key::StrengthStrong),
        };
        let bar = 3.0 * (34.0 + 4.0) + 4.0;
        let words = paint::galley(ui, t(k), Type::Tiny, c(C::Ink2)).size().x;
        inside_end(ui, frame, bar + words + tk::S2, |ui| pin::strength(ui, lit, m, t(k)));
    }
    r
}

pub(super) fn secret_again(ui: &mut egui::Ui, salt: &str, text: &mut crate::secret::Secret, first: &crate::secret::Secret, w: f32) -> egui::Response {
    let (r, frame) = boxed_cell(ui, salt, w, |ui| input::secret_line_w_marked(ui, text, "", w, false));
    if !text.is_empty() && text.expose() == first.expose() {
        let at = egui::pos2(frame.right() - tk::INPUT_PAD_X - tk::MARK / 2.0, frame.center().y);
        mark::paint_mark(ui.ctx(), ui.painter(), at, Mark::Ok, tk::MARK);
    }
    r
}

pub(super) fn pin_field(st: &mut fields::Stack, ui: &mut egui::Ui, label: &str, rules: Option<&str>, salt: &str, value: &mut crate::secret::Secret, shake: Option<f64>, active: bool) -> pin::PinRow {
    pin_field_why(st, ui, label, rules, None, salt, value, shake, active)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn pin_field_why(st: &mut fields::Stack, ui: &mut egui::Ui, label: &str, rules: Option<&str>, why: Option<&str>, salt: &str, value: &mut crate::secret::Secret, shake: Option<f64>, active: bool) -> pin::PinRow {
    st.field(ui, label, None, why.map(|w| (w, true)), |ui, w| {
        let row = zikaron_ui::sheetparts::pins(ui, salt, value, crate::keybox::PIN_LEN, shake, active);
        if let Some(s) = rules.filter(|s| !s.is_empty()) {
            ui.add_space(-4.0);
            paint::line(ui, s, Type::Small, c(C::Ink2), w);
        }
        row
    })
}

pub(super) fn file_row(f: &mut card::Form, ui: &mut egui::Ui, label: &str, path: &str) {
    let named = ui.painter().layout_no_wrap(label.to_string(), card::title_font(), c(C::Ink)).size().x;
    let room = (ui.available_width() - named - 32.0).max(40.0);
    let _ = f.line(ui, Some(card::Lead::Glyph(Glyph::File)), label, "", false, false, c(C::Ink), |ui| {
        if path.trim().is_empty() {
            paint::line(ui, t(Key::PickNone), Type::Note, c(C::Ink3), room);
        } else {
            let name = width::file_name(path);
            let r = paint::line(ui, &name, Type::Note, c(C::Ink), room);
            let _ = zikaron_ui::layer::tip(r, path);
        }
    });
}

pub(super) fn still_needs(items: &[(bool, Key)]) -> String {
    let need: Vec<&str> = items.iter().filter(|(missing, _)| *missing).map(|(_, k)| t(*k)).collect();
    if need.is_empty() {
        String::new()
    } else {
        crate::lang::fill1(Key::GrantStillNeed, &need.join(t(Key::ListJoin)))
    }
}

pub(super) fn rot_back_key(ui: &mut egui::Ui, backed: &mut bool) -> bool {
    *backed = true;
    key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked()
}

pub(super) fn foot_left(ui: &mut egui::Ui, text: &str) -> bool {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| key::key(ui, text, Role::Secondary, true).clicked()).inner
}

pub(super) fn eth_cap(wei: u128) -> String {
    eth_digits(wei, true)
}

pub(super) fn eth_held(wei: u128) -> String {
    eth_digits(wei, false)
}

fn eth_digits(wei: u128, up: bool) -> String {
    const WEI_DECIMALS: u32 = 18;
    if wei == 0 {
        return "0".to_string();
    }
    let digits = wei.ilog10() + 1;
    let first = (WEI_DECIMALS + 1).saturating_sub(digits);
    let places = 4.max(first + 2).min(WEI_DECIMALS);
    let step = 10u128.pow(WEI_DECIMALS - places);
    let mut scaled = wei / step;
    if up && wei % step != 0 {
        scaled += 1;
    }
    let scale = 10u128.pow(places);
    let mut frac = format!("{:0width$}", scaled % scale, width = places as usize);
    while frac.len() > 4 && frac.ends_with('0') {
        frac.pop();
    }
    format!("{}.{}", scaled / scale, frac)
}

pub(super) use crate::when::wall_secs;

impl Win {
    pub(super) fn progress(&self) -> crate::nav::Progress {
        crate::nav::Progress::of(&self.shell)
    }

    pub(super) fn auto(&mut self, a: Action, now: f64) {
        let _ = now;
        let _ = apply(&mut self.shell, a);
    }

    pub(super) fn one_drop(&mut self, d: &drop::Drop, now: f64) -> Option<String> {
        match d.dropped.as_slice() {
            [] => None,
            [one] => Some(one.clone()),
            _ => {
                self.toasts.say(t(Key::U3OneAtATime), Tone::Bad, now);
                None
            }
        }
    }

    pub(super) fn drop_or_pick(&mut self, ctx: &egui::Context, site: &str, d: &drop::Drop, kind: crate::platform::Pick, now: f64) -> Option<String> {
        match self.one_drop(d, now) {
            Some(p) => Some(p),
            None => path_answer(ctx, egui::Id::new(("zikaron-path-drop", site)), d.clicked, kind),
        }
    }
}

#[cfg(test)]
mod amounts {
    use super::{eth_cap, eth_held};

    const ETH: u128 = 1_000_000_000_000_000_000;
    const GWEI: u128 = 1_000_000_000;

    #[test]
    fn zero_is_zero() {
        assert_eq!(eth_cap(0), "0");
        assert_eq!(eth_held(0), "0");
    }

    #[test]
    fn one_wei_is_not_zero() {
        assert_eq!(eth_cap(1), "0.000000000000000001");
        assert_eq!(eth_held(1), "0.000000000000000001");
    }

    #[test]
    fn exactly_at_the_fourth_decimal() {
        assert_eq!(eth_cap(ETH / 10_000), "0.0001");
        assert_eq!(eth_held(ETH / 10_000), "0.0001");
    }

    #[test]
    fn six_gwei() {
        assert_eq!(eth_cap(6 * GWEI), "0.000000006");
        assert_eq!(eth_held(6 * GWEI), "0.000000006");
    }

    #[test]
    fn small_amounts_keep_three_significant_digits() {
        assert_eq!(eth_cap(94_500_000_000_000), "0.0000945");
        assert_eq!(eth_held(94_500_000_000_000), "0.0000945");
        assert_eq!(eth_cap(420_000_000_000_000), "0.00042");
        assert_eq!(eth_held(420_000_000_000_000), "0.00042");
        assert_eq!(eth_cap(94_512_345_678_901), "0.0000946");
        assert_eq!(eth_held(94_512_345_678_901), "0.0000945");
    }

    #[test]
    fn whole_amounts_keep_four_decimals() {
        assert_eq!(eth_cap(2 * ETH), "2.0000");
        assert_eq!(eth_held(2 * ETH), "2.0000");
        assert_eq!(eth_cap(2 * ETH + 1), "2.0001");
        assert_eq!(eth_held(2 * ETH + 1), "2.0000");
        assert_eq!(eth_held(12_345_678_900_000_000_000), "12.3456");
        assert_eq!(eth_cap(12_345_678_900_000_000_000), "12.3457");
    }

    #[test]
    fn the_top_of_u128() {
        assert_eq!(eth_cap(u128::MAX), "340282366920938463463.3747");
        assert_eq!(eth_held(u128::MAX), "340282366920938463463.3746");
    }

    #[test]
    fn rounding_up_carries_across_places() {
        assert_eq!(eth_cap(99_950_000_000_000), "0.0001");
        assert_eq!(eth_held(99_950_000_000_000), "0.0000999");
        assert_eq!(eth_cap(999_999_999_999_999_999), "1.0000");
        assert_eq!(eth_held(999_999_999_999_999_999), "0.9999");
    }
}

pub(super) fn kept<K: std::hash::Hash, V: Send + Sync + 'static>(ctx: &egui::Context, moved: u64, salt: &'static str, key: K, make: impl FnOnce() -> V) -> (std::sync::Arc<V>, u64) {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (moved, salt, crate::lang::lang() as u8, crate::when::zone() as u8).hash(&mut h);
    key.hash(&mut h);
    let k = h.finish();
    let id = egui::Id::new(("zikaron-kept", salt));
    if let Some((held, v)) = ctx.data(|d| d.get_temp::<(u64, std::sync::Arc<V>)>(id)) {
        if held == k {
            return (v, k);
        }
    }
    let v = std::sync::Arc::new(make());
    ctx.data_mut(|d| d.insert_temp(id, (k, v.clone())));
    (v, k)
}

pub(super) type Listed = Vec<(table::Row, String)>;

pub(super) fn listed_table(ui: &mut egui::Ui, salt: &str, spec: table::Spec, rows: &Listed) -> Option<String> {
    table::table_with(ui, salt, spec, rows.len(), |i| rows[i].0.clone()).clicked.and_then(|i| rows.get(i).map(|r| r.1.clone()))
}
