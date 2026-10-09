//! Page parts shared by every page, laid out from the kit's controls: labelled fields, the location row,
//! detail heads, the details fold, button rows. Nothing here sets a size or a color of its own.

use super::*;

/// A field: its label (14, second ink, 8 above the control; an optional word after it in the quiet ink),
/// then the control.
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

/// The location row: the label (132), the chosen folder's name in monospace (the whole path on hover), or a
/// quiet "not chosen", and a button at the right. Returns whether the button was pressed.
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

/// The path mailbox (the file dialog does not block the frame). A place that offers "choose…" asks under its
/// own key in the frame its button is pressed ([`path_answer`]); the window opens the dialog at the end of
/// that frame (one at a time) and waits for the answer in the background; the chosen path lands here under
/// the asking key and the place takes it the next frame it is drawn. A cancel lands nothing; an answer no
/// place takes within two frames is dropped (the place is gone: its sheet was closed meanwhile).
#[derive(Clone, Default)]
pub(super) struct PathMail {
    /// Asked this frame: the place's key and what the dialog allows.
    pub(super) asked: Option<(egui::Id, crate::platform::Pick)>,
    /// The place whose dialog is open.
    pub(super) ticket: Option<egui::Id>,
    /// A landed answer: the place's key, the path, the frame it landed.
    pub(super) answer: Option<(egui::Id, String, u64)>,
}

pub(super) fn path_mail() -> egui::Id {
    egui::Id::new("zikaron-path-mail")
}

/// The one way a place asks for a path: under `site` (its own key), asking when `ask` (its "choose…" was
/// pressed this frame); gives the path the person chose for `site` when it has landed, once.
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

/// A path picker in a field: the location row without a label. Returns whether the path changed. Its key in
/// the path mailbox is the field the answer goes into (that field's place in the window, fixed for the
/// window's life), so two pickers drawn in one card never take each other's answer.
pub(super) fn pick_path(ui: &mut egui::Ui, slot: &mut String, kind: crate::platform::Pick) -> bool {
    let key_label = match kind {
        crate::platform::Pick::File => t(Key::PickFile),
        _ => t(Key::PickFolder),
    };
    let asked = path_row(ui, "", slot, key_label, t(Key::PickNone));
    let site = egui::Id::new(("zikaron-pick-path", slot as *const String as usize));
    match path_answer(ui.ctx(), site, asked, kind) {
        Some(p) => {
            *slot = p;
            true
        }
        None => false,
    }
}

/// A row of buttons, 8 apart, wrapping when narrow.
pub(super) fn keys_row<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(tk::S2, tk::S2);
        add(ui)
    })
    .inner
}

/// A titled card of key-value rows ("basic information").
pub(super) fn kv_section(ui: &mut egui::Ui, title: &str, rows: &[(&str, Val)]) {
    card::section(ui, title, "", |ui| {
        card::card(ui, |ui| kv::kv(ui, rows));
    });
}

/// The details fold in its own card: raw values (addresses, digests, ids, transactions) live only here.
pub(super) fn details_card(ui: &mut egui::Ui, id_salt: &str, rows: &[(&str, Val)]) {
    card::card(ui, |ui| {
        fold::fold(ui, id_salt, t(Key::SetEvidence), |ui| kv::kv(ui, rows));
    });
}

/// The details fold inline (inside a card or a sheet).
pub(super) fn details(ui: &mut egui::Ui, id_salt: &str, rows: &[(&str, Val)]) {
    fold::fold(ui, id_salt, t(Key::SetEvidence), |ui| kv::kv(ui, rows));
}

/// A hint line (14), second ink.
pub(super) fn hint(ui: &mut egui::Ui, s: &str) {
    states::hint(ui, s);
}

/// A card's small title.
pub(super) fn card_title(ui: &mut egui::Ui, s: &str) {
    card::card_title(ui, s);
}

/// A pill for an anchoring state.
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
    /// A record's name: the name of the entry in my ledger that anchored it; without a name, "unnamed record".
    pub(super) fn work_label(&self, work: &str) -> String {
        self.shell
            .rows
            .as_ref()
            .and_then(|(rows, _)| {
                rows.iter()
                    .filter(|r| r.kind == zikaron::tokens::EntryType::History)
                    .find(|r| r.work.as_deref().map(|w| w.eq_ignore_ascii_case(work)).unwrap_or(false))
                    .and_then(|r| r.facts.note.clone())
            })
            .unwrap_or_else(|| t(Key::UnnamedRecord).to_string())
    }

    /// Recently dealt-with addresses: the address book plus those granted in the register, deduplicated.
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

    /// The phase of a long-running button started by an action of kind `k`: busy while that kind runs (with its
    /// fraction when counted), a check or a cross for a moment after it lands.
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

    /// A long-running button: its phase follows the task of kind `k`. Returns whether it was pressed.
    pub(super) fn long_key(&self, ui: &mut egui::Ui, text: &str, role: Role, enabled: bool, k: crate::task::Kind) -> bool {
        key::show(ui, key::Key::new(text, role).enabled(enabled).phase(self.phase_of(k))).clicked()
    }

    /// The stage line under a long-running button: the stage the task reports, the previous one checked; a
    /// counted stage shows its count. Nothing while the task is not running.
    pub(super) fn stage_line(&self, ui: &mut egui::Ui, k: crate::task::Kind) {
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
            // The stage just passed, checked.
            zikaron_ui::icons::glyph_at(&p, Glyph::Ok, egui::pos2(x + 6.0, cy), 12.0, c(C::Ok).gamma_multiply(a));
            x += 12.0 + 6.0;
            let r = p.text(egui::pos2(x, cy), egui::Align2::LEFT_CENTER, t(words[at - 1]), Type::Small.font(), c(C::Ink3).gamma_multiply(a));
            x = r.right() + 12.0;
        }
        // The current stage: a breathing dot, the words fading in when they change.
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

/// The stage labels of a long task, in the order the task reports them (`task::stage_at` inside each task
/// marks where it is; a task with one stage shows one line).
pub(super) fn stage_words(k: crate::task::Kind) -> &'static [Key] {
    use crate::task::Kind;
    match k {
        Kind::Anchor => &[Key::StageSign, Key::StageBroadcast],
        Kind::Check => &[Key::StageParseGrant, Key::StageReadChain, Key::OsSixChecks, Key::StageCompareFile, Key::StageCompareTerms],
        Kind::Verify => &[Key::StageReadKit, Key::StageReadChain, Key::StageCompareEach],
        Kind::Book => &[Key::StageConnect, Key::StageScanBlocks],
        Kind::Diligence => &[Key::StageConnect, Key::StageScanBlocks, Key::StageCheckLedger],
        Kind::Kit => &[Key::StageGather, Key::StageCheckOriginals, Key::StageWriteKit],
        Kind::Badge => &[Key::StageGrantChain, Key::StageWriteBadge],
        Kind::Review => &[Key::StageReadChain, Key::StageReadUpstream],
        Kind::Reconcile => &[Key::StageReconcile],
        Kind::Archive => &[Key::DoMeasure],
        Kind::Publish => &[Key::StageFetchFiles],
        Kind::Chain => &[Key::StageReadChain],
        Kind::Depth => &[Key::StageReadChain],
        Kind::Gate => &[Key::StageReadChain],
        Kind::ReadNet => &[Key::StageConnect],
        Kind::Basis => &[Key::StageConnect],
        _ => &[],
    }
}

/// A date field's width in a search row ("2026-10-01" with its calendar mark).
pub(super) const DATE_W: f32 = 136.0;
/// The least a search field keeps beside the two date fields; narrower, the dates go to a row of their own.
const SEARCH_MIN_W: f32 = 180.0;

/// A date field with the calendar in the app's words, today read in the zone moments are shown in
/// ([`crate::when::day`]), so the day picked and the day a row shows are counted the same way.
pub(super) fn date_box(ui: &mut egui::Ui, id: &str, value: &mut String, hint: &str, now_secs: u64) -> bool {
    datepick::field(ui, id, value, hint, DATE_W, today_of(now_secs), &date_words())
}

/// The calendar's words in the app's language.
pub(super) fn date_words() -> datepick::Words<'static> {
    let weekdays = [Key::DateMon, Key::DateTue, Key::DateWed, Key::DateThu, Key::DateFri, Key::DateSat, Key::DateSun].map(t);
    datepick::Words { weekdays, month: month_title, today: t(Key::DateToday), clear: t(Key::PickClear) }
}

/// The calendar's month row: the year and the month, each followed by its word in the current language.
fn month_title(y: i32, m: u32) -> String {
    fill2(Key::DateMonth, &y.to_string(), &m.to_string())
}

/// Today as the calendar counts it: the day of `now_secs` in the zone moments are shown in.
pub(super) fn today_of(now_secs: u64) -> datepick::Day {
    datepick::parse(&crate::when::day(now_secs)).unwrap_or((1970, 1, 1))
}

/// A picker's width: a plain list, and a list of records (number, name, first anchor time).
pub(super) const PICK_W: f32 = 360.0;
pub(super) const PICK_WIDE_W: f32 = 560.0;

/// The two date fields "start date · end date" side by side.
pub(super) fn date_pair(ui: &mut egui::Ui, id: &str, range: &mut (String, String), now_secs: u64) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = tk::S2;
        date_box(ui, &format!("{id}-from"), &mut range.0, t(Key::DateFrom), now_secs);
        date_box(ui, &format!("{id}-to"), &mut range.1, t(Key::DateTo), now_secs);
    });
}

/// A list's search row: the search field, then "start date · end date" (rows outside the range, by anchor
/// time, are the caller's to drop through [`crate::when::within`]). On a narrow window the dates take a row
/// of their own under the search field.
pub(super) fn search_row(ui: &mut egui::Ui, id: &str, query: &mut String, hint: &str, range: &mut (String, String), now_secs: u64) {
    let w = ui.available_width();
    if w >= DATE_W * 2.0 + tk::S2 * 2.0 + SEARCH_MIN_W {
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

/// The one search rule: empty keeps everything; otherwise any field containing the text (case-insensitive).
pub(super) fn matches(query: &str, fields: &[&str]) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || fields.iter().any(|f| f.to_lowercase().contains(&q))
}
