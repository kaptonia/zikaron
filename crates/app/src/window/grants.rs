use super::*;
use zikaron_ui::formparts;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum OutSheet {
    GrantFile(String),
    Badge(String),
    Upstream(String),
}

fn ymd(secs: u64) -> datepick::Day {
    datepick::parse(&crate::when::day(secs)).unwrap_or((1970, 1, 1))
}

fn month_short(m: u32) -> &'static str {
    const MONTHS: [Key; 12] = [
        Key::MonthShort1,
        Key::MonthShort2,
        Key::MonthShort3,
        Key::MonthShort4,
        Key::MonthShort5,
        Key::MonthShort6,
        Key::MonthShort7,
        Key::MonthShort8,
        Key::MonthShort9,
        Key::MonthShort10,
        Key::MonthShort11,
        Key::MonthShort12,
    ];
    t(MONTHS[(m.clamp(1, 12) - 1) as usize])
}

pub(super) fn span_say(w: Option<(u64, u64)>) -> String {
    super::faces::window_long(w)
}

fn days_say(a: u64, b: u64) -> String {
    match b.saturating_sub(a).div_ceil(86_400) {
        1 => t(Key::SpanOneDay).to_string(),
        n => fill1(Key::SpanDays, &n.to_string()),
    }
}

pub(super) fn span_trail(ui: &mut egui::Ui, w: Option<(u64, u64)>) {
    ui.label(egui::RichText::new(span_say(w)).font(Type::Note.font()).color(c(C::Ink2)));
}

pub(super) fn span_trail_days(ui: &mut egui::Ui, w: Option<(u64, u64)>) {
    let font = Type::Note.font();
    if let Some((a, b)) = w {
        ui.label(egui::RichText::new(days_say(a, b)).font(font.clone()).color(c(C::Ink3)));
    }
    ui.label(egui::RichText::new(span_say(w)).font(font).color(c(C::Ink2)));
}

fn days_from_civil((y, m, d): datepick::Day) -> i64 {
    let y = i64::from(if m <= 2 { y - 1 } else { y });
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn local_of(at: u64) -> i64 {
    let shown = crate::when::when(at);
    let num = |a: usize, b: usize| shown.get(a..b).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    days_from_civil(ymd(at)) * 86_400 + num(11, 13) * 3600 + num(14, 16) * 60 + num(17, 19)
}

fn at_local(day: datepick::Day, of_day: u64) -> u64 {
    let here = days_from_civil(day) * 86_400 + of_day as i64;
    let offset = |at: i64| local_of(at.max(0) as u64) - at.max(0);
    let first = here - offset(here);
    (here - offset(first)).max(0) as u64
}

const GRANT_START: [(Key, u64, u64); 2] = [(Key::GrantHour, 23, 3_600), (Key::GrantMinute, 59, 60)];
const GRANT_LASTS: [(Key, u64, u64); 3] = [(Key::CliLastsDays, 999, 86_400), (Key::CliLastsHours, 23, 3_600), (Key::CliLastsMinutes, 59, 60)];

fn parts_of<const N: usize>(table: &[(Key, u64, u64); N]) -> [(u64, u64); N] {
    table.map(|(_, most, unit)| (most, unit))
}

fn custom_start(day: &str, at: &[String; 2], today: datepick::Day) -> Option<u64> {
    use zikaron_ui::lasts::Read;
    let of_day = match zikaron_ui::lasts::read(at, &parts_of(&GRANT_START)) {
        Read::Empty => 0,
        Read::Total(n) => n,
        Read::Over(_) => return None,
    };
    Some(at_local(datepick::parse(day).unwrap_or(today), of_day))
}

fn custom_window(day: &str, at: &[String; 2], lasts: &[String; 3], today: datepick::Day) -> Option<(u64, u64)> {
    let start = custom_start(day, at, today)?;
    match zikaron_ui::lasts::read(lasts, &parts_of(&GRANT_LASTS)) {
        zikaron_ui::lasts::Read::Total(n) if n > 0 => Some((start, start + n)),
        _ => None,
    }
}

fn lasts_of(n: u64) -> [String; 3] {
    [(n / 86_400).to_string(), (n % 86_400 / 3_600).to_string(), (n % 3_600 / 60).to_string()]
}

fn lasts_to(day: &str, at: &[String; 2], end: datepick::Day, today: datepick::Day) -> Option<[String; 3]> {
    let start = custom_start(day, at, today)?;
    let stop = at_local(end, local_of(start).rem_euclid(86_400) as u64);
    (stop > start).then(|| lasts_of(stop - start))
}

fn custom_of((a, b): (u64, u64)) -> (String, [String; 2], [String; 3]) {
    let of_day = local_of(a).rem_euclid(86_400) as u64;
    (datepick::fmt(ymd(a)), [(of_day / 3_600).to_string(), (of_day % 3_600 / 60).to_string()], lasts_of(b.saturating_sub(a)))
}

fn block_words(secs: u64, cross: bool) -> (String, String) {
    let (y, m, d) = ymd(secs);
    let month = if cross { fill2(Key::SpanBlockYear, month_short(m), &y.to_string()) } else { month_short(m).to_string() };
    (month, d.to_string())
}

const GRANT_NO_W: f32 = 120.0;

fn folder_row(f: &mut card::Form, ui: &mut egui::Ui, label: &str, path: &str) {
    let label_w = paint::galley(ui, label, Type::Body, c(C::Ink)).size().x;
    let _ = f.line(ui, Some(card::Lead::Glyph(Glyph::Folder)), label, "", false, false, c(C::Ink), |ui| {
        if path.trim().is_empty() {
            ui.label(egui::RichText::new(t(Key::PickNone)).font(Type::Note.font()).color(c(C::Ink3)));
        } else {
            let font = Type::MonoSmall.font();
            let room = (ui.available_width() - label_w - tk::S5).max(40.0);
            let shown = width::elide_mid(ui, path, font.clone(), room);
            let r = ui.label(egui::RichText::new(&shown).font(font).color(c(C::Ink)));
            if shown != path {
                let _ = zikaron_ui::layer::tip(r, path);
            }
        }
    });
}

fn standing(ui: &mut egui::Ui, m: Mark, s: Option<&str>) {
    let Some(s) = s.filter(|s| !s.is_empty()) else { return };
    let kind = match m {
        Mark::Bad => states::Banner::Bad,
        Mark::Warn => states::Banner::Warn,
        Mark::Ok => states::Banner::Ok,
        _ => states::Banner::Note,
    };
    states::banner(ui, kind, s, |_| ());
}

impl Win {
    pub(super) fn grants_list(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
        let chain_now = self.chain_now();
        let mut query = self.ux.search.get("grants").cloned().unwrap_or_default();
        let mut range = self.ux.range.get("grants").cloned().unwrap_or_default();
        let today = (self.shell.clock)();
        stagger(ui, 0, |ui| search_row(ui, "grants", &mut query, t(Key::SearchGrants), &mut range, today));
        let (view, generation) = kept(ui.ctx(), self.shell.moved, "grants", (&query, &range, chain_now), || {
            let all = self.shell.grants.as_deref();
            let rows = self.shell.rows.as_ref().map(|(rs, _)| rs.as_slice()).unwrap_or_default();
            let entry = |id: &str| rows.iter().find(|r| r.id.eq_ignore_ascii_case(id));
            let listed: Listed = all
                .unwrap_or_default()
                .iter()
                .rev()
                .filter(|row| {
                    let badge = row.badge(chain_now);
                    matches(&query, &[&row.grantee, &self.work_label(&row.work), &row.work, t(badge_key(badge)), &format!("#{}", row.seq)])
                })
                .filter(|row| crate::when::within(entry(&row.id).and_then(|r| r.anchored_at), &range.0, &range.1))
                .map(|g| {
                    let badge = g.badge(chain_now);
                    let lamp = rows.iter().find(|r| r.id == g.id).map(|r| r.lamp).unwrap_or(crate::ledgerx::Lamp::Queued);
                    let row = table::Row {
                        cells: vec![
                            table::Cell::Text(self.work_label(&g.work)),
                            table::Cell::Short(span_say(g.window)),
                            table::Cell::Mono(format!("#{}", g.seq)),
                            table::Cell::Pill(t(badge_key(badge)).to_string(), badge_tone(badge)),
                            table::Cell::Mark(lamp_mark(lamp)),
                        ],
                        click: true,
                        gone: false,
                        on: false,
                    };
                    (row, g.id.clone())
                })
                .collect();
            let migratable = all.map(|g| g.iter().filter(|r| r.migratable(chain_now)).count()).unwrap_or(0);
            (all.map(<[crate::grantx::Row]>::is_empty), listed, migratable)
        });
        let (read_empty, listed, migratable) = (view.0, &view.1, view.2);
        let mut open: Option<String> = None;
        let mut migrate = false;
        if migratable > 0 {
            stagger(ui, 1, |ui| {
                card::form(ui, |ui, f| {
                    migrate = f.go(ui, t(Key::MigrateTitle), &fill1(Key::MigrateGo, &migratable.to_string()), true, "").clicked();
                });
            });
        }
        if migrate {
            self.act(Action::MigrateGrants, now);
        }
        stagger(ui, 1, |ui| {
            let cols = [
                table::col(t(Key::U3Work), table::Col::Fr(1.0)),
                table::col(t(Key::U3Window), table::Col::Auto { min: 120.0, max: 320.0 }),
                table::col(t(Key::GrantNo), table::Col::Px(GRANT_NO_W)),
                table::col(t(Key::U3State), table::Col::Px(84.0)),
                table::MARK,
            ];
            let empty = match read_empty {
                None => t(Key::WbNotRead),
                Some(true) => t(Key::U3NoGrants),
                Some(false) => t(Key::SearchNone),
            };
            open = listed_table(ui, "grants", table::Spec::new(&cols, generation).head(true).empty(empty), listed);
        });
        self.ux.search.insert("grants", query);
        self.ux.range.insert("grants", range);
        if let Some(id) = open {
            self.push(Route::Grant(id), now);
        }
    }

    pub(super) fn grant_detail(&mut self, ui: &mut egui::Ui, row: &crate::ledgerx::Row, now: f64) {
        self.ensure_grants(now);
        let g = self.shell.grants.as_ref().and_then(|g| g.iter().find(|x| x.id == row.id).cloned());
        let badge = g.as_ref().map(|x| x.badge(self.chain_now()));
        let at = self.shell.remembered.as_ref().map(|r| r.at);
        let ready = self.opened.as_ref().is_some_and(|d| d.id == row.id);
        let record = g.as_ref().map(|x| self.work_label(&x.work)).unwrap_or_else(|| t(Key::WbNotRead).to_string());
        let window = g.as_ref().and_then(|x| x.window);
        let grantee = g.as_ref().map(|x| x.grantee.clone()).unwrap_or_default();
        let exclusive = g.as_ref().map(|x| t(exclusive_key(x.exclusive_from)).to_string()).unwrap_or_else(|| t(Key::WbNotRead).to_string());
        let doc = g.as_ref().and_then(|x| x.doc_name.clone());
        let lamp_words = lamp_label(row.lamp, true, at);
        let inq = self.shell.queue.items.iter().filter(|q| !q.step.in_flight()).position(|q| q.id == row.id);
        let export_phase = self.phase_of(crate::task::Kind::Gate);
        let (mut copied, mut copy, mut export, mut story, mut send, mut revoke) = (None, false, false, false, false, false);
        let mut pills = vec![lamp_hero(row.lamp, at)];
        pills.extend(badge.map(|b| card::HeroPill { words: t(badge_key(b)).to_string(), tone: badge_tone(b), live: false }));
        stagger(ui, 0, |ui| {
            card::hero(ui, &record, &format!("#{} \u{b7} {}", row.seq, t(Key::KindGrant)), false, &pills);
        });
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::BasicInfo), "", |ui| {
                card::form(ui, |ui, f| {
                    copied = if grantee.is_empty() { f.info(ui, t(Key::U3ToWhom), card::Show::Empty(t(Key::WbNotRead))) } else { f.info(ui, t(Key::U3ToWhom), card::Show::Short(&grantee)) };
                    f.with(ui, t(Key::U3Window), "", |ui| span_trail(ui, window));
                    match (row.anchored_at, inq) {
                        (_, Some(_)) => send = f.act(ui, t(Key::NavAnchoring), &lamp_words, key::Key::new(t(Key::U3SendNow), Role::Guide)).clicked(),
                        (Some(s), None) => f.with(ui, t(Key::NavAnchoring), "", |ui| {
                            ui.label(egui::RichText::new(crate::when::when(s)).font(Type::MonoSmall.font()).color(c(C::Ink)));
                            let (r, _) = ui.allocate_exact_size(egui::vec2(tk::MARK, tk::MARK), egui::Sense::hover());
                            mark::paint_mark(ui.ctx(), ui.painter(), r.center(), lamp_mark(row.lamp), tk::MARK);
                        }),
                        (None, None) => f.with(ui, t(Key::NavAnchoring), "", |ui| lamp_pill_ui(ui, row.lamp, at)),
                    }
                    let _ = f.info(ui, t(Key::U3Work), card::Show::Text(&record));
                    let _ = f.info(ui, t(Key::ExclusiveMark), card::Show::Text(&exclusive));
                    let _ = f.info(ui, t(Key::U3TermsDoc), doc.as_deref().map(card::Show::Text).unwrap_or(card::Show::Empty(t(Key::TermsDocNone))));
                });
            });
        });
        stagger(ui, 2, |ui| {
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), tk::KEY_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = tk::S2;
                copy = key::show(ui, key::Key::new(t(Key::V2CopyCode), Role::Primary).enabled(ready)).clicked();
                export = key::show(ui, key::Key::new(t(Key::V2ExportGrantFile), Role::Secondary).enabled(ready).phase(export_phase)).clicked();
            });
        });
        let mut kv_rows = vec![
            (t(Key::U3ToWhom), Val::mono(grantee.clone())),
            (t(Key::U3TermsHash), Val::mono(g.as_ref().map(|x| x.terms.clone()).unwrap_or_default())),
            (t(Key::DetailPick), Val::mono(row.id.clone())),
            (t(Key::U3RawType), Val::text(t(type_words(&row).0))),
            (t(Key::U3RawSeq), Val::figure(row.seq.to_string())),
            (t(Key::EntryAuthor), Val::mono(row.author.clone())),
            (t(Key::EntryPrev), row.prev.clone().map(Val::mono).unwrap_or_else(|| Val::text(t(Key::None_)))),
            (t(Key::CanonBytes), Val::figure(row.bytes.to_string())),
        ];
        if let Some((chain, tx)) = row.tx.clone() {
            kv_rows.push((t(Key::AnchorTx), Val::mono(format!("{chain} \u{b7} {tx}"))));
        }
        stagger(ui, 3, |ui| details_card(ui, "grant-details", &kv_rows));
        let history = self.shell.story.clone().filter(|(a, _, _)| a.eq_ignore_ascii_case(&row.id)).map(|(_, _, revs)| {
            if revs.is_empty() {
                return t(Key::StoryNone).to_string();
            }
            let rows_all: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
            revs.iter()
                .map(|(id, _)| {
                    let r = rows_all.iter().find(|x| x.id.eq_ignore_ascii_case(id));
                    let said = fill1(Key::StoryRevokedBy, &r.map(|x| x.seq.to_string()).unwrap_or_default());
                    format!("{said} \u{b7} {}", first_anchor_say(r.and_then(|x| x.anchored_at)))
                })
                .collect::<Vec<_>>()
                .join(t(Key::ListJoin))
        });
        let live = g.as_ref().map(|x| !x.revoked).unwrap_or(false);
        stagger(ui, 4, |ui| {
            card::form(ui, |ui, f| {
                story = f.go(ui, t(Key::DoReadStory), history.as_deref().unwrap_or(""), true, "").clicked();
                if live {
                    revoke = f.pen(ui, t(Key::PageRevoke)).clicked();
                }
            });
        });
        tell_copied(ui.ctx(), copied);
        if copy {
            if let Applied::GrantCode { text } = self.act(Action::CopyGrantCode { grant: row.id.clone() }, now) {
                ui.ctx().copy_text(text);
                self.toasts.say(t(Key::U3CopiedGrantText), Tone::Note, now);
            }
        }
        if export {
            self.out_open(OutSheet::GrantFile(row.id.clone()));
        }
        if story {
            self.act(Action::ReadStory { grant: row.id.clone() }, now);
        }
        if inq.is_some() && send {
            self.send_this(row.id.clone(), now);
        }
        if revoke {
            self.ux.u3.revoking = Some(format!("#{}", row.seq));
            self.u3_open_confirm(U3Confirm::Revoke { grant: row.id.clone() });
        }
    }

    pub(super) fn out_open(&mut self, s: OutSheet) {
        let kits = self.shell.home.as_ref().map(|h| h.dir(crate::home::Slot::Kits).display().to_string()).unwrap_or_else(|| self.shell.documents.clone());
        match &s {
            OutSheet::GrantFile(_) if self.typed.gf_out.trim().is_empty() => self.typed.gf_out = kits,
            OutSheet::Badge(_) if self.typed.badge_out.trim().is_empty() => self.typed.badge_out = kits,
            _ => {}
        }
        if let OutSheet::GrantFile(id) = &s {
            self.typed.gf_listed = self.shell.home.as_ref().and_then(|h| crate::grantfilex::listed_original(h, id)).unwrap_or_default();
            self.typed.gf_original = false;
            self.typed.gf_sealed = false;
        }
        self.ux.out_waiting = false;
        self.ux.out_sheet = Some(s);
    }

    pub(super) fn out_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let Some(s) = self.ux.out_sheet.take() else { return };
        if self.any_sheet() {
            return;
        }
        self.ux.out_sheet = Some(s.clone());
        let kind = match &s {
            OutSheet::GrantFile(_) => Some(crate::task::Kind::Gate),
            OutSheet::Badge(_) => Some(crate::task::Kind::Badge),
            OutSheet::Upstream(_) => None,
        };
        let running = kind.is_some_and(|k| self.shell.tasks.in_flight(k));
        if self.ux.out_waiting && !running {
            self.ux.out_waiting = false;
            self.ux.out_sheet = None;
            return;
        }
        let (title, row_key, at) = match &s {
            OutSheet::GrantFile(_) => (Key::V2ExportGrantFile, Key::IdStore, self.typed.gf_out.clone()),
            OutSheet::Badge(_) => (Key::V2MakeBadge, Key::IdStore, self.typed.badge_out.clone()),
            OutSheet::Upstream(_) => (Key::UpstreamDirName, Key::Folder, self.typed.vt_dir.clone()),
        };
        let ready = match &s {
            OutSheet::GrantFile(id) => self.opened.as_ref().is_some_and(|d| d.id == *id) && Self::landing_ok(&at),
            OutSheet::Badge(_) => Self::landing_ok(&at),
            OutSheet::Upstream(_) => true,
        };
        let phase = kind.map(|k| self.phase_of(k)).unwrap_or(Phase::Idle);
        let grant = match &s {
            OutSheet::GrantFile(id) => self.shell.rows.as_ref().and_then(|(r, _)| r.iter().find(|x| x.id.eq_ignore_ascii_case(id)).cloned()),
            _ => None,
        };
        let grant_say = grant.as_ref().map(|r| {
            let work = self.shell.grants.as_ref().and_then(|g| g.iter().find(|x| x.id.eq_ignore_ascii_case(&r.id)).map(|x| x.work.clone()));
            format!("#{} \u{b7} {}", r.seq, work.map(|w| self.work_label(&w)).unwrap_or_else(|| t(Key::WbNotRead).to_string()))
        });
        let (mut go, mut close, mut picked, mut cleared) = (false, false, None, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("grant-folder", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(title), "");
                card::form(ui, |ui, f| {
                    if let Some(g) = grant_say.as_deref() {
                        let _ = f.info(ui, t(Key::KindGrant), card::Show::Text(g));
                    }
                    let clears = matches!(s, OutSheet::Upstream(_)) && !at.trim().is_empty();
                    if clears {
                        cleared = f.act_path(ui, t(row_key), &at, key::Key::new(t(Key::PickClear), Role::Secondary)).clicked();
                    } else {
                        folder_row(f, ui, t(row_key), &at);
                    }
                    if let OutSheet::GrantFile(_) = &s {
                        let listed = !me.typed.gf_listed.is_empty();
                        let name = if listed { width::file_name(&me.typed.gf_listed) } else { String::new() };
                        if f.switch(ui, t(Key::GrantPkgOriginal), &name, me.typed.gf_original && listed, listed) {
                            me.typed.gf_original = !me.typed.gf_original;
                        }
                        if f.switch(ui, t(Key::GrantPkgSealed), "", me.typed.gf_sealed, true) {
                            me.typed.gf_sealed = !me.typed.gf_sealed;
                        }
                    }
                });
                if let Some(k) = kind {
                    me.stage_line(ui, k);
                }
            },
            |ui, _me| {
                let k = match &s {
                    OutSheet::GrantFile(_) => key::Key::new(t(Key::V2ExportGrantFile), Role::Primary),
                    OutSheet::Badge(_) => key::Key::new(t(Key::V2MakeBadge), Role::Primary),
                    OutSheet::Upstream(_) => key::Key::new(t(Key::DoSetUpstream), Role::Primary),
                };
                let why = if Self::landing_ok(&at) { t(Key::GrantFileUnread).to_string() } else { still_needs(&[(true, row_key)]) };
                go = key::show(ui, k.enabled(ready).phase(phase).why(&why)).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                picked = foot_pick(ui, "grant-folder", crate::platform::Pick::Folder);
            },
        );
        if let Some(p) = picked {
            match &s {
                OutSheet::GrantFile(_) => self.typed.gf_out = p,
                OutSheet::Badge(_) => self.typed.badge_out = p,
                OutSheet::Upstream(_) => self.typed.vt_dir = p,
            }
        }
        if cleared {
            if let OutSheet::Upstream(grant) = &s {
                self.typed.vt_dir.clear();
                self.act(Action::SetUpstream { grant: grant.clone(), dir: String::new() }, now);
                self.ux.out_sheet = None;
                return;
            }
        }
        if close || out.esc {
            self.ux.out_sheet = None;
            self.ux.out_waiting = false;
            return;
        }
        if !go || running {
            return;
        }
        let started = match s {
            OutSheet::GrantFile(id) => {
                let original = if self.typed.gf_original { self.typed.gf_listed.clone() } else { String::new() };
                let grantee = self.shell.grants.as_ref().and_then(|g| g.iter().find(|x| x.id.eq_ignore_ascii_case(&id)).map(|x| x.grantee.clone())).unwrap_or_default();
                let seal_to = if self.typed.gf_sealed { grantee } else { String::new() };
                let a = Action::ExportGrantFile { id, to: self.typed.gf_out.clone(), original, seal_to, key: None };
                matches!(self.act(a, now), Applied::Started(crate::task::Kind::Gate))
            }
            OutSheet::Badge(grant) => {
                let out = match self.shell.home.as_ref() {
                    Some(_) => {
                        let chosen = crate::home::choose(
                            &crate::home::Kind::Bundle { stem: format!("badge-{}", grant.trim_start_matches("0x").chars().take(10).collect::<String>()) },
                            std::path::Path::new(self.typed.badge_out.trim()),
                        );
                        self.remember_landing(Out::Badge, &chosen);
                        chosen.at.display().to_string()
                    }
                    None => String::new(),
                };
                matches!(self.act(Action::ExportBadge { grant, out }, now), Applied::Started(crate::task::Kind::Badge))
            }
            OutSheet::Upstream(grant) => {
                let a = Action::SetUpstream { grant, dir: self.typed.vt_dir.clone() };
                self.act(a, now);
                false
            }
        };
        if started {
            self.ux.out_waiting = true;
        } else {
            self.ux.out_sheet = None;
        }
    }

    pub(super) fn grant_signed(&mut self) {
        let (upstream, work) = (self.typed.g_upstream.clone(), self.typed.g_work.clone());
        self.grant_form_fresh();
        if !upstream.trim().is_empty() {
            self.typed.g_upstream = upstream;
            self.typed.g_work = work;
        }
    }

    pub(super) fn grant_form_fresh(&mut self) {
        self.typed.g_grantee.clear();
        self.typed.g_work.clear();
        self.typed.g_history.clear();
        self.typed.g_terms.clear();
        self.typed.g_upstream.clear();
        self.typed.g_scope.clear();
        self.typed.g_exclusive = false;
        self.typed.g_start_day.clear();
        self.typed.g_start_at = Default::default();
        self.typed.g_lasts = Default::default();
        self.ux.u3.terms = None;
        self.ux.u3.grant_days = 30;
        self.ux.u3.grant_days_set = true;
        self.ux.u3.chain_time_asked = false;
    }

    fn grant_today(&self) -> datepick::Day {
        today_of(self.chain_now().unwrap_or((self.shell.clock)()))
    }

    pub(super) fn grant_form_page(&mut self, ui: &mut egui::Ui, now: f64) {
        self.takes_drops();
        self.grant_form(ui, None, now);
    }

    pub(super) fn grant_form(&mut self, ui: &mut egui::Ui, upstream: Option<String>, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
        if !self.ux.u3.grant_days_set {
            self.ux.u3.grant_days = 30;
            self.ux.u3.grant_days_set = true;
        }
        if !self.ux.u3.chain_time_asked {
            self.ux.u3.chain_time_asked = true;
            self.auto(Action::ReadChainTime, now);
        }
        let key = format!("{}|{}|{}|{}", self.typed.g_work.trim(), self.typed.g_from.trim(), self.typed.g_to.trim(), self.shell.grants_gen);
        let whole = self.typed.g_from.trim().is_empty() == self.typed.g_to.trim().is_empty();
        if key != self.clash_key && self.shell.grants.is_some() {
            self.clash_key = key;
            if !self.typed.g_work.trim().is_empty() && whole {
                self.auto(Action::CheckClash { work: self.typed.g_work.clone(), from: self.typed.g_from.clone(), to: self.typed.g_to.clone() }, now);
            } else {
                self.shell.clash.clear();
            }
        }
        let chain_now = self.chain_now();
        if self.ux.u3.grant_days > 0 {
            match chain_now {
                Some(n) => {
                    let (f, to) = (n.to_string(), (n + self.ux.u3.grant_days as u64 * 86_400).to_string());
                    if self.typed.g_from != f || self.typed.g_to != to {
                        self.typed.g_from = f;
                        self.typed.g_to = to;
                    }
                }
                None => {
                    self.typed.g_from.clear();
                    self.typed.g_to.clear();
                }
            }
        } else {
            let (f, to) = match custom_window(&self.typed.g_start_day, &self.typed.g_start_at, &self.typed.g_lasts, self.grant_today()) {
                Some((a, b)) => (a.to_string(), b.to_string()),
                None => (String::new(), String::new()),
            };
            if self.typed.g_from != f || self.typed.g_to != to {
                self.typed.g_from = f;
                self.typed.g_to = to;
            }
        }
        let mixed = |x: &str| x.trim().strip_prefix("0x").map(|h| h.bytes().any(|b| b.is_ascii_uppercase())).unwrap_or(false);
        let (grantee_mixed, terms_mixed, upstream_mixed) = (mixed(&self.typed.g_grantee), mixed(&self.typed.g_terms), mixed(&self.typed.g_upstream));
        let grantee_shape = crate::grantx::grantee_shape_bad(&self.typed.g_grantee);
        let ready = crate::grantx::draft_ready(&self.typed.g_grantee, &self.typed.g_work, &self.typed.g_terms, &self.typed.g_upstream, self.ux.u3.grant_days, chain_now);
        let need: Vec<&str> = [(self.typed.g_grantee.trim().is_empty(), Key::U3ToWhom), (self.typed.g_work.trim().is_empty(), Key::U3Work), (self.typed.g_terms.trim().is_empty(), Key::U3TermsFile)]
            .into_iter()
            .filter(|(missing, _)| *missing)
            .map(|(_, k)| t(k))
            .collect();
        let why = if grantee_mixed || terms_mixed || upstream_mixed {
            t(Key::U3AddrMixedCase).to_string()
        } else if grantee_shape {
            t(Key::KitToWhyAddress).to_string()
        } else if !need.is_empty() {
            fill1(Key::GrantStillNeed, &need.join(t(Key::ListJoin)))
        } else if self.ux.u3.grant_days > 0 && chain_now.is_none() {
            t(Key::U3NoChainTimeForPreset).to_string()
        } else {
            String::new()
        };
        let window = match (self.typed.g_from.trim().parse::<u64>(), self.typed.g_to.trim().parse::<u64>()) {
            (Ok(a), Ok(b)) => Some((a, b)),
            _ => None,
        };
        let custom = self.ux.u3.grant_days == 0;
        let cross = window.is_some_and(|(a, b)| ymd(a).0 != ymd(b).0);
        let cell_words = |s: &str| s.trim().parse::<u64>().ok().map(|x| block_words(x, cross)).unwrap_or_else(|| (String::new(), "\u{2013}".to_string()));
        let (from_words, to_words) = (cell_words(&self.typed.g_from), cell_words(&self.typed.g_to));
        let between = window.map(|(a, b)| days_say(a, b)).unwrap_or_default();
        let no_span = match window {
            Some(_) => None,
            None if !custom && chain_now.is_none() => Some(t(Key::U3NoChainTimeForPreset)),
            None if !custom => Some(t(Key::CountdownNoWindow)),
            None => None,
        };
        let n = self.shell.clash.len();
        let (fs, ts) = (self.typed.g_from.trim(), self.typed.g_to.trim());
        let window_ok = (fs.is_empty() && ts.is_empty()) || (fs.parse::<u64>().is_ok() && ts.parse::<u64>().is_ok());
        let waiting = self.shell.clash_unsettled;
        let clash = if self.typed.g_work.trim().is_empty() || !window_ok {
            (Mark::Todo, t(Key::U3ClashNotYet).to_string())
        } else if n == 0 && waiting > 0 {
            (Mark::Warn, fill1(Key::U3ClashUnsettled, &waiting.to_string()))
        } else {
            (if n == 0 { Mark::Ok } else { Mark::Bad }, fill1(Key::U3ClashLine, &n.to_string()))
        };
        let more = [
            t(if self.typed.g_exclusive { Key::ExclusiveMark } else { Key::GrantSumNonExclusive }).to_string(),
            if self.typed.g_upstream.trim().is_empty() { t(Key::GrantSumNoUpstream).to_string() } else { head_tail(self.typed.g_upstream.trim()) },
            t(if self.typed.g_scope.trim().is_empty() { Key::GrantSumNoScope } else { Key::GrantSumScope }).to_string(),
        ]
        .join(" \u{b7} ");
        let from_card = self.ux.u3.terms.as_ref().map(|x| x.hex == self.typed.g_terms.trim()).unwrap_or(false);
        let adv = if from_card {
            t(Key::U3TermsFilled).to_string()
        } else if self.typed.g_terms.trim().is_empty() {
            t(Key::SetNone).to_string()
        } else {
            head_tail(self.typed.g_terms.trim())
        };
        let queued = (upstream.is_some() && self.ux.u4.relicense_signed && self.shell.queue.len() > 0).then(|| self.shell.queue.len());
        let reveal = std::mem::take(&mut self.ux.u4.relicense_reveal);
        let recent = self.recent_addresses();
        let (mut confirm, mut to_queue, mut flip, mut cancel) = (false, false, false, false);
        let stale = match (chain_now, self.shell.failed.contains_key(&crate::task::Kind::ChainTime), self.shell.tasks.in_flight(crate::task::Kind::ChainTime)) {
            (Some(at), true, false) => Some(fill1(Key::GrantChainTimeStale, &crate::when::day(at))),
            _ => None,
        };
        if let Some(n) = queued {
            let top = ui.cursor().top();
            stagger(ui, 0, |ui| {
                card::form(ui, |ui, f| {
                    to_queue = f.act(ui, &fill1(Key::U4RecentQueued, &n.to_string()), "", key::Key::new(t(Key::GoAnchor), Role::Secondary)).clicked();
                });
            });
            if reveal {
                let row = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top());
                ui.scroll_to_rect_animation(row, None, egui::style::ScrollAnimation::none());
                zikaron_ui::scroll::asked(ui.ctx());
            }
        }
        stagger(ui, 0, |ui| {
            fields::card(ui, |ui, st| {
                let check = if grantee_mixed {
                    Some((t(Key::U3AddrMixedCase), true))
                } else {
                    grantee_shape.then(|| (t(Key::KitToWhyAddress), true))
                };
                st.field(ui, t(Key::U3ToWhom), None, check, |ui, w| {
                    let heads: Vec<String> = recent.iter().map(|a| head_tail(a)).collect();
                    let items: Vec<menu::Item> = heads.iter().map(|h| menu::Item::Row(menu::Row { lead: t(Key::Address), label: h, mono: true, ..Default::default() })).collect();
                    let spec = pick::Spec { hint: t(Key::SearchAddress), empty: t(Key::SearchNone), w: PICK_W, dates: None };
                    let keep = |i: usize, q: &str, _: &str, _: &str| matches(q, &[&recent[i]]);
                    let mut picked = None;
                    let look = fields::Look { mono: true, bad: grantee_mixed || grantee_shape, ..Default::default() };
                    let _ = fields::joined(
                        ui,
                        &mut self.typed.g_grantee,
                        t(Key::U3ToWhomHint),
                        w,
                        look,
                        Some(|ui: &mut egui::Ui| picked = formparts::inner_pick(ui, "grant-recent", t(Key::U3RecentDots), true, false, !recent.is_empty(), &spec, &items, &keep)),
                    );
                    if let Some(i) = picked {
                        self.typed.g_grantee = recent[i].clone();
                    }
                });
                let file_look = fields::Look { icon: Some(Glyph::File), read_only: true, ..Default::default() };
                st.field(ui, t(Key::U3WhichWork), None, None, |ui, w| match &upstream {
                    Some(name) => {
                        let mut shown = name.clone();
                        let _ = fields::line(ui, &mut shown, "", w, file_look);
                    }
                    None => {
                        let lines = self.work_lines();
                        let current = lines.iter().find(|w| w.work.eq_ignore_ascii_case(self.typed.g_work.trim())).map(|w| format!("#{} \u{b7} {}", w.seq, w.name));
                        let label = t(if current.is_some() { Key::U3TermsSwap } else { Key::V2PickWorkBtn });
                        let hint = t(if lines.is_empty() { Key::U3NoWorksYet } else { Key::U3PickWorkHint });
                        let mut shown = current.clone().unwrap_or_default();
                        let mut open = false;
                        let _ = fields::joined(ui, &mut shown, hint, w, file_look, Some(|ui: &mut egui::Ui| open = formparts::inner_open(ui, label, !lines.is_empty()).clicked()));
                        if open {
                            self.ux.search.insert("grant-work", String::new());
                            self.ux.range.insert("grant-work", Default::default());
                            self.ux.u3.work_pick = true;
                        }
                    }
                });
                st.field(ui, t(Key::U3TermsFile), None, None, |ui, w| {
                    if self.ux.u3.terms.as_ref().map(|x| x.hex != self.typed.g_terms.trim()).unwrap_or(false) {
                        self.ux.u3.terms = None;
                    }
                    let mut pick = false;
                    match self.ux.u3.terms.as_ref().map(|x| (x.name.clone(), x.size.map(size_say).unwrap_or_default())) {
                        Some((name, size)) => {
                            let mut shown = if size.is_empty() { name } else { format!("{name} \u{b7} {size}") };
                            let _ = fields::joined(ui, &mut shown, "", w, file_look, Some(|ui: &mut egui::Ui| pick = fields::inner_face(ui, t(Key::U3TermsSwap)).clicked()));
                        }
                        None if !self.typed.g_terms.trim().is_empty() => {
                            let mut shown = t(Key::TermsTyped).to_string();
                            let _ = fields::line(ui, &mut shown, "", w, fields::Look { icon: Some(Glyph::Fingerprint), read_only: true, ..Default::default() });
                        }
                        None => {
                            let (d, key_hit) = formparts::drop_line(ui, "terms-file", t(Key::U3TermsDrop), t(Key::PickFile), true);
                            if let Some(p) = self.one_drop(&d, now) {
                                self.take_terms(&p, now);
                            } else if d.clicked || key_hit {
                                pick = true;
                            }
                        }
                    }
                    if let Some(p) = path_answer(ui.ctx(), egui::Id::new("zikaron-path-grant-terms"), pick, crate::platform::Pick::File) {
                        self.take_terms(&p, now);
                    }
                });
                let words = date_words();
                let today = self.grant_today();
                st.field(ui, t(Key::U3Window), None, None, |ui, w| {
                    let presets = [(7u32, t(Key::U3Days7)), (30, t(Key::U3Days30)), (90, t(Key::U3Days90)), (0, t(Key::U3Custom))];
                    let cells: Vec<seg::Cell> = presets.iter().map(|(_, l)| seg::Cell::from(*l)).collect();
                    let cur = presets.iter().position(|(d, _)| *d == self.ux.u3.grant_days).unwrap_or(3);
                    let need = Self::span_need(ui, &from_words, &between, &to_words, no_span);
                    let mut below = false;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        if let Some(i) = seg::seg(ui, "grant-days", &cells, cur) {
                            if presets[i].0 == 0 && self.ux.u3.grant_days > 0 {
                                if let Some(w) = window {
                                    (self.typed.g_start_day, self.typed.g_start_at, self.typed.g_lasts) = custom_of(w);
                                }
                            }
                            self.ux.u3.grant_days = presets[i].0;
                        }
                        if ui.available_width() >= need + 14.0 {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.span_blocks(ui, &from_words, &between, &to_words, no_span, custom, today, &words));
                        } else {
                            below = true;
                        }
                    });
                    if below {
                        let layout = if no_span.is_some() { egui::Layout::left_to_right(egui::Align::Center) } else { egui::Layout::right_to_left(egui::Align::Center) };
                        ui.allocate_ui_with_layout(egui::vec2(w, 0.0), layout, |ui| self.span_blocks(ui, &from_words, &between, &to_words, no_span, custom, today, &words));
                    }
                });
                if custom {
                    let over = |cells: &[String], table: &[(Key, u64, u64)]| -> (Vec<bool>, String) {
                        let parts: Vec<(u64, u64)> = table.iter().map(|(_, most, unit)| (*most, *unit)).collect();
                        let bad = match zikaron_ui::lasts::read(cells, &parts) {
                            zikaron_ui::lasts::Read::Over(o) => o,
                            _ => vec![false; table.len()],
                        };
                        let said: Vec<String> = table.iter().zip(&bad).filter(|(_, b)| **b).map(|((unit, most, _), _)| crate::lang::filln(Key::CliLastsAtMost, &[t(*unit), &most.to_string()])).collect();
                        (bad, said.join(t(Key::ListJoin)))
                    };
                    let (start_bad, start_said) = over(&self.typed.g_start_at, &GRANT_START);
                    let (lasts_bad, lasts_said) = over(&self.typed.g_lasts, &GRANT_LASTS);
                    let start_units: Vec<&str> = GRANT_START.iter().map(|(unit, _, _)| t(*unit)).collect();
                    let lasts_units: Vec<&str> = GRANT_LASTS.iter().map(|(unit, _, _)| t(*unit)).collect();
                    let typed = &mut self.typed;
                    st.pair(
                        ui,
                        |ui, st| {
                            let check = (!start_said.is_empty()).then_some((start_said.as_str(), true));
                            st.field(ui, t(Key::GrantStartAt), None, check, |ui, _| zikaron_ui::lasts::boxed(ui, "grant-start", &mut typed.g_start_at, &start_units, "0", &start_bad, true))
                        },
                        |ui, st| {
                            let check = (!lasts_said.is_empty()).then_some((lasts_said.as_str(), true));
                            st.field(ui, t(Key::GrantLasts), None, check, |ui, _| zikaron_ui::lasts::boxed(ui, "grant-lasts", &mut typed.g_lasts, &lasts_units, "0", &lasts_bad, true))
                        },
                    );
                }
            });
            ui.add_space(tk::S3 - ui.spacing().item_spacing.y);
            standing(ui, Mark::Warn, stale.as_deref());
            standing(ui, clash.0, Some(&clash.1));
        });
        stagger(ui, 1, |ui| {
            card::form(ui, |ui, f| {
                let _ = f.fold_from(ui, "grant-more", t(Key::U3GrantMore), &more, upstream.is_some(), |ui, f| {
                    flip = f.switch(ui, t(Key::GrantExclusive), t(Key::GrantExclusiveNote), self.typed.g_exclusive, true);
                    f.panel(ui, |ui| {
                        fields::stack(ui, |ui, st| {
                            let check = upstream_mixed.then(|| (t(Key::U3AddrMixedCase), true));
                            st.field(ui, t(Key::U3Upstream), None, check, |ui, w| {
                                fields::line(ui, &mut self.typed.g_upstream, "0x\u{2026}", w, fields::Look { mono: true, bad: upstream_mixed, ..Default::default() })
                            });
                            st.field(ui, t(Key::U3ScopeNote), Some(t(Key::U3Optional)), None, |ui, _| input::area_words(ui, &mut self.typed.g_scope, 2, ""));
                        });
                    });
                });
                let _ = f.fold(ui, "grant-adv", t(Key::U3MoreOptions), &adv, |ui, f| {
                    f.panel(ui, |ui| {
                        fields::stack(ui, |ui, st| {
                            let check = terms_mixed.then(|| (t(Key::U3AddrMixedCase), true));
                            st.field(ui, t(Key::U3TermsHash), None, check, |ui, w| {
                                fields::line(ui, &mut self.typed.g_terms, "0x\u{2026}", w, fields::Look { mono: true, bad: terms_mixed, ..Default::default() })
                            });
                        });
                    });
                });
            });
        });
        formparts::page_bar(ui, |ui| {
            confirm = key::show(ui, key::Key::new(t(Key::GrantConfirm), Role::Guide).enabled(ready).why(&why)).clicked();
            cancel = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
        });
        if flip {
            self.typed.g_exclusive = !self.typed.g_exclusive;
        }
        if confirm {
            self.u3_open_confirm(U3Confirm::Grant);
        }
        if to_queue {
            self.go(Place::View(crate::nav::View::Works, crate::nav::tab::WORKS_PENDING), now);
        }
        if cancel {
            self.back(now);
        }
    }

    fn span_need(ui: &egui::Ui, from: &(String, String), between: &str, to: &(String, String), no_span: Option<&str>) -> f32 {
        match no_span {
            Some(s) => paint::galley(ui, s, Type::Body, c(C::Ink3)).size().x,
            None => datepick::span_width(ui, datepick::Block { month: &from.0, day: &from.1 }, between, datepick::Block { month: &to.0, day: &to.1 }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn span_blocks(&mut self, ui: &mut egui::Ui, from: &(String, String), between: &str, to: &(String, String), no_span: Option<&str>, custom: bool, today: datepick::Day, words: &datepick::Words) {
        if let Some(s) = no_span {
            let room = ui.available_width();
            paint::line(ui, s, Type::Body, c(C::Ink3), room);
            return;
        }
        let start = custom.then(|| custom_start(&self.typed.g_start_day, &self.typed.g_start_at, today)).flatten();
        let [a, b] = datepick::span(ui, "grant-span", datepick::Block { month: &from.0, day: &from.1 }, between, datepick::Block { month: &to.0, day: &to.1 }, [custom, start.is_some()]);
        let Some(start) = start else { return };
        let shown = if self.typed.g_start_day.is_empty() { datepick::fmt(today) } else { self.typed.g_start_day.clone() };
        let mut day = shown.clone();
        if datepick::hang(ui, &datepick::block_salt("grant-span", 0), &a, &mut day, None, today, words) && day != shown {
            self.typed.g_start_day = day;
        }
        let mut end = self.typed.g_to.trim().parse::<u64>().map(|b| datepick::fmt(ymd(b))).unwrap_or_default();
        if datepick::hang(ui, &datepick::block_salt("grant-span", 1), &b, &mut end, Some(ymd(start)), today, words) {
            if let Some(lasts) = datepick::parse(&end).and_then(|e| lasts_to(&self.typed.g_start_day, &self.typed.g_start_at, e, today)) {
                self.typed.g_lasts = lasts;
            }
        }
    }

    pub(super) fn take_terms(&mut self, path: &str, now: f64) {
        match crate::anchorx::of_file(std::path::Path::new(path)) {
            Ok(x) => {
                let name = width::file_name(&x.subject);
                self.typed.g_terms = x.hex();
                self.ux.u3.terms = Some(TermsFile { path: path.to_string(), name, size: x.size, hex: x.hex() });
            }
            Err(f) => self.say_fault(&f, now),
        }
    }

    pub(super) fn anchor_key(&self) -> Key {
        if self.shell.settings.auto_anchor {
            Key::AnchorNowKey
        } else {
            Key::AddToLedgerKey
        }
    }
}

fn work_why(w: &WorkLine) -> Option<(&'static str, PillTone)> {
    use crate::ledgerx::Lamp;
    match (w.deleted, w.lamp) {
        (Some(_), _) => Some((t(Key::V2Deleted), PillTone::Grey)),
        (None, Lamp::Anchored) => None,
        (None, Lamp::Submitted | Lamp::Included) => Some((t(Key::V2OnChainWait), PillTone::Warn)),
        (None, Lamp::Remembered | Lamp::RememberedStale) => Some((t(Key::V2AwaitThisCheck), PillTone::Warn)),
        (None, _) => Some((t(Key::V2PendingGrey), PillTone::Warn)),
    }
}

const WORK_LIST_H: f32 = 300.0;

impl Win {
    pub(super) fn grant_work_pick_sheet(&mut self, ctx: &egui::Context) {
        if !self.ux.u3.work_pick {
            return;
        }
        let lines = self.work_lines();
        let mut typed = (self.ux.search.get("grant-work").cloned().unwrap_or_default(), self.ux.range.get("grant-work").cloned().unwrap_or_default());
        let today = (self.shell.clock)();
        let current = self.typed.g_work.trim().to_string();
        let (mut chosen, mut close) = (None, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("grant-work-pick", tk::SHEET_WIDE),
            self,
            |ui, _me| {
                sheet::title(ui, t(Key::V2PickWorkBtn), "");
                search_row(ui, "grant-work", &mut typed.0, t(Key::SearchWorks), &mut typed.1, today);
                let seq = |w: &WorkLine| format!("#{}", w.seq);
                let kept: Vec<&WorkLine> = lines.iter().filter(|w| matches(&typed.0, &[&w.name, &w.work, &seq(w)]) && crate::when::within(w.row.anchored_at, &typed.1 .0, &typed.1 .1)).collect();
                if kept.is_empty() {
                    paint::text(ui, t(if lines.is_empty() { Key::U3NoWorksYet } else { Key::SearchNone }), Type::Note, c(C::Ink3));
                    return;
                }
                zikaron_ui::scroll::nested(ui, egui::ScrollArea::vertical().id_salt("grant-work-list").max_height(WORK_LIST_H), |ui| {
                    card::form(ui, |ui, f| {
                        for w in kept {
                            let why = work_why(w);
                            let at = w.row.anchored_at.map(crate::when::when).unwrap_or_else(|| t(Key::V2StateLanded).to_string());
                            let (resp, _) = f.choice(
                                ui,
                                why.is_none(),
                                w.work.eq_ignore_ascii_case(&current),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    paint::text(ui, &seq(w), Type::MonoSmall, c(C::Ink3));
                                    let room = (ui.available_width() - 8.0).max(0.0);
                                    let r = paint::line(ui, &w.name, Type::Body, if why.is_some() { c(C::Ink3) } else { c(C::Ink) }, room);
                                    if w.deleted.is_some() {
                                        ui.painter().hline(r.rect.x_range(), r.rect.center().y, egui::Stroke::new(1.0_f32, c(C::Ink3)));
                                    }
                                },
                                |ui| {
                                    paint::text(ui, &at, Type::Small, c(C::Ink3));
                                    if let Some((s, tone)) = why {
                                        let _ = mark::pill(ui, s, tone);
                                    }
                                },
                            );
                            if why.is_none() && resp.clicked() {
                                chosen = Some((w.work.clone(), w.id.clone()));
                            }
                        }
                    });
                });
            },
            |ui, _me| {
                if key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked() {
                    close = true;
                }
            },
        );
        self.ux.search.insert("grant-work", typed.0);
        self.ux.range.insert("grant-work", typed.1);
        if let Some((work, id)) = chosen {
            self.typed.g_work = work;
            self.typed.g_history = id;
            close = true;
        }
        if out.esc || close {
            self.ux.u3.work_pick = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{custom_of, custom_window, lasts_of, lasts_to};

    fn s3(a: &str, b: &str, c: &str) -> [String; 3] {
        [a.to_string(), b.to_string(), c.to_string()]
    }

    #[test]
    fn an_end_day_writes_the_length_the_window_reads_back() {
        crate::when::set(crate::when::Zone::Utc);
        let at = ["10".to_string(), "30".to_string()];
        let day = 86_400;
        let l = lasts_to("2026-10-03", &at, (2026, 11, 5), (2026, 1, 1)).expect("a later day");
        assert_eq!(l, s3("33", "0", "0"));
        let (a, b) = custom_window("2026-10-03", &at, &l, (2026, 1, 1)).expect("a window");
        assert_eq!((b - a, crate::when::when(b)), (33 * day, "2026-11-05 10:30:00".to_string()));
        assert_eq!(lasts_to("2026-10-03", &at, (2026, 10, 4), (2026, 1, 1)), Some(s3("1", "0", "0")), "the day after");
        assert_eq!(lasts_to("2026-10-03", &at, (2026, 10, 3), (2026, 1, 1)), None, "the start's own day");
        assert_eq!(lasts_to("2026-10-03", &at, (2026, 9, 30), (2026, 1, 1)), None, "a day before the start");
        let far = lasts_to("2026-10-03", &at, (2029, 7, 1), (2026, 1, 1)).expect("a later day");
        assert_eq!(far, s3("1002", "0", "0"));
        assert_eq!(custom_window("2026-10-03", &at, &far, (2026, 1, 1)), None, "past the days the length holds");
        assert_eq!(lasts_to("2026-10-03", &["24".to_string(), String::new()], (2026, 11, 5), (2026, 1, 1)), None, "the start does not read");
        assert_eq!(lasts_to("", &at, (2026, 1, 3), (2026, 1, 1)), Some(s3("2", "0", "0")));
    }

    #[test]
    fn a_length_is_its_days_hours_and_minutes() {
        crate::when::set(crate::when::Zone::Utc);
        assert_eq!(lasts_of(0), s3("0", "0", "0"));
        assert_eq!(lasts_of(86_400 + 3_600 + 61), s3("1", "1", "1"));
        assert_eq!(lasts_of(1_000 * 86_400), s3("1000", "0", "0"));
        let w = (1_790_000_000, 1_790_000_000 + 30 * 86_400);
        let (day, at, lasts) = custom_of(w);
        let back = custom_window(&day, &at, &lasts, (2026, 1, 1)).expect("a window");
        assert_eq!(back.1 - back.0, w.1 - w.0);
    }
}
