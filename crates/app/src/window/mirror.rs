use super::*;

impl Win {
    pub(super) fn mirror_sheet(&mut self, ctx: &egui::Context, trouble: Option<crate::fault::Fault>, now: f64) -> (Option<Action>, bool, bool) {
        self.measure_if_due(now);
        let show_trouble = |ui: &mut egui::Ui| {
            if let Some(f) = &trouble {
                states::err_box(ui, "bk-trouble", f.human(), &f.next(), t(Key::U3RawError), &f.shown());
            }
        };
        let (mut go, mut close) = (None, false);
        let last = self.shell.settings.mirror.clone();
        let since = match self.shell.archive_now().map(|a| (a.items, a.mirror.clone())) {
            Some((n, crate::mirror::Mirrored::At { entries, .. })) => fill1(Key::U3EntriesCount, &n.saturating_sub(entries).to_string()),
            _ => "\u{2014}".to_string(),
        };
        let last_said = last.as_ref().map(|r| crate::when::when(r.at)).unwrap_or_else(|| t(Key::DataBackupNever).to_string());
        if self.typed.mirror_out.is_empty() {
            self.typed.mirror_out = match &last {
                Some(r) => crate::mirror::folder_of(std::path::Path::new(&r.path)).display().to_string(),
                None => mirror_default(&self.shell.documents),
            };
        }
        let mut picked = None;
        let out = sheet::show(
            ctx,
            sheet::Spec::new("bk-mirror", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::MirrorTitle), t(Key::MirrorNote));
                fields::stack(ui, |ui, _st| {
                    zikaron_ui::fields::rows(ui, |ui, f| {
                        let _ = f.info(ui, t(Key::MirrorLastOut), card::Show::Text(&last_said));
                        let _ = f.info(ui, t(Key::MirrorSince), card::Show::Text(&since));
                        f.folder(ui, t(Key::MirrorWhere), &me.typed.mirror_out, t(Key::PickNone));
                    });
                });
                show_trouble(ui);
            },
            |ui, me| {
                if page::Page::new().primary_with(ui, t(Key::DoExportMirrorGo), Self::landing_ok(&me.typed.mirror_out)).1.clicked() {
                    go = Some(Action::ExportMirror { to: me.typed.mirror_out.trim().to_string() });
                }
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                picked = foot_pick(ui, "bk-mirror", crate::platform::Pick::Folder);
            },
        );
        if let Some(p) = picked {
            self.typed.mirror_out = p;
        }
        (go, close, out.esc)
    }
}

fn mirror_default(docs: &str) -> String {
    if docs.is_empty() {
        return String::new();
    }
    std::path::Path::new(&docs).join("ledger-mirror").display().to_string()
}
