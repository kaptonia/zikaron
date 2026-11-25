use super::*;

impl Win {
    pub(super) fn set_network(&mut self, ui: &mut egui::Ui, now: f64) {
        let s = self.shell.settings.clone();
        let bare = s.chain_id.is_none() && s.registry.is_none() && s.endpoints.is_empty();
        let row = s.network.as_deref().and_then(crate::deploy::named).or_else(|| crate::deploy::same_as_row(s.chain_id, s.registry, s.from_block, &s.endpoints)).or_else(|| crate::deploy::of_chain(s.chain_id, s.registry));
        let title = match row {
            _ if bare => t(Key::SetNetworkNone).to_string(),
            Some(d) => network_label(d.name),
            None => t(Key::U3Custom).to_string(),
        };
        let (mark, said) = match self.chain_read() {
            Err(f) => (Mark::Bad, fill1(Key::SetReadFailedNow, f.human())),
            Ok(Some(Done::Chain { sources, single_source, unanswered, differed, .. })) => {
                let said = if *single_source { t(Key::SetSingleSource).to_string() } else { fill1(Key::SetAgreed, &sources.to_string()) };
                let said = if unanswered.is_empty() { said } else { format!("{said} · {}", fill1(Key::SetUnansweredCount, &unanswered.len().to_string())) };
                (if *single_source || !unanswered.is_empty() || !differed.is_empty() { Mark::Warn } else { Mark::Ok }, said)
            }
            _ => (Mark::Todo, t(Key::SetNotRead).to_string()),
        };
        let unanswered: Vec<String> = match self.chain_read() {
            Ok(Some(Done::Chain { unanswered, .. })) => crate::chainx::plain_of(unanswered),
            _ => Vec::new(),
        };
        let differed: Vec<String> = match self.chain_read() {
            Ok(Some(Done::Chain { differed, .. })) => crate::chainx::plain_of(differed),
            _ => Vec::new(),
        };
        let unproven: Vec<String> = self.shell.audit.as_ref().map(|a| crate::chainx::plain_of(&a.unproven)).unwrap_or_default();
        let refused: Vec<String> = match self.chain_read() {
            Err(f) => crate::chainx::plain_of(f.nodes()),
            _ => Vec::new(),
        };
        let tint = match (bare, mark) {
            (true, _) => card::Tint::Warn,
            (false, Mark::Ok) => card::Tint::Ok,
            (false, Mark::Warn) => card::Tint::Warn,
            (false, Mark::Bad) => card::Tint::Bad,
            _ => card::Tint::Blue,
        };
        let head = match s.chain_id {
            Some(ch) if !bare => format!("{title} \u{b7} {}", fill1(Key::ChainNumbered, &ch.to_string())),
            _ => title.clone(),
        };
        let phase = self.phase_of(self.own_net_task());
        let mut read_chain = false;
        let own_overview = self.own_net_overview(ui, now);
        stagger(ui, 0, |ui| {
            if own_overview {
                return;
            }
            card::form(ui, |ui, f| {
                read_chain = f.summary(ui, Glyph::Globe, tint, &head, None, &said, |ui| own_net_key(ui, bare, phase));
                if !unanswered.is_empty() {
                    let _ = f.fold(ui, "chain-unanswered", t(Key::SetWhatEachSaid), &unanswered.len().to_string(), |ui, f| f.note(ui, &fill1(Key::SetUnanswered, &unanswered.join(" \u{b7} "))));
                }
                if !differed.is_empty() {
                    let _ = f.fold(ui, "chain-differed", t(Key::SetWhatEachSaid), &differed.len().to_string(), |ui, f| f.note(ui, &differed.join(" \u{b7} ")));
                }
                if !unproven.is_empty() {
                    let _ = f.fold(ui, "chain-unproven", t(Key::SetWhatEachSaid), &unproven.len().to_string(), |ui, f| f.note(ui, &fill1(Key::SetUnproven, &unproven.join(" \u{b7} "))));
                }
                if !refused.is_empty() {
                    let _ = f.fold(ui, "chain-refused", t(Key::SetWhatEachSaid), &refused.len().to_string(), |ui, f| {
                        for line in &refused {
                            f.note(ui, line);
                        }
                    });
                }
            });
            self.stage_line(ui, crate::task::Kind::Chain);
        });
        if read_chain {
            self.own_net_go(now);
        }
        stagger(ui, 1, |ui| self.set_rpc_nodes(ui, now));
        stagger(ui, 2, |ui| self.set_log_asks(ui, now));
        stagger(ui, 2, |ui| self.set_rest_switch(ui, now));
        if self.shell.settings.rest_on {
            stagger(ui, 3, |ui| self.set_rest_rows(ui, now));
        }
        self.own_after_rest(ui, now);
        stagger(ui, 7, |ui| self.set_proxy(ui, now));
        let none = || Val::text(t(Key::SetNone));
        let rows = self.own_net_details().unwrap_or_else(|| {
            vec![
                (t(Key::SetNodes), if s.endpoints.is_empty() { none() } else { Val::mono(s.endpoints.join("\n")) }),
                (t(Key::BasisChain), s.chain_id.map(|n| Val::figure(n.to_string())).unwrap_or_else(none)),
                (t(Key::SetRegistry), s.registry.map(|a| Val::mono(a.hex())).unwrap_or_else(none)),
                (t(Key::BasisFrom), Val::figure(s.from_block.to_string())),
            ]
        });
        stagger(ui, 8, |ui| details_card(ui, "network-details", &rows));
    }

    pub(super) fn set_rpc_nodes(&mut self, ui: &mut egui::Ui, now: f64) {
        let saved = self.shell.settings.endpoints.clone();
        let home_chain = self.own_home_chain();
        let (mut remove, mut add) = (None, false);
        let err = self.ux.rpc_err.clone();
        card::section(ui, &rpc_head(&self.shell.settings), "", |ui| {
            card::form(ui, |ui, f| {
                f.keep(ui);
                for (i, spec) in saved.iter().enumerate() {
                    let (chain, url) = spec_parts(spec);
                    f.key(("rpc-node", spec));
                    if let Some(c) = chain.filter(|c| Some(*c) != home_chain) {
                        f.tag_next(&fill1(Key::ChainNumbered, &c.to_string()));
                    }
                    let (row, gone) = f.line(ui, None, url, "", false, false, c(C::Ink), |ui| zikaron_ui::button::icon_key(ui, Glyph::No, 26.0, true, t(Key::KitRemove)).clicked());
                    let _ = zikaron_ui::layer::tip(row, url);
                    if gone {
                        remove = Some(i);
                    }
                }
                f.key("rpc-add");
                f.panel(ui, |ui| {
                    fields::stack(ui, |ui, st| {
                        let look = fields::Look { mono: true, bad: err.is_some(), ..Default::default() };
                        let line = err.as_deref().map(|e| (e, true));
                        let (resp, hit) = st.field(ui, t(Key::NetRpcAdd), None, line, |ui, w| {
                            fields::joined(ui, &mut self.typed.rpc_new, t(Key::NetRpcHint), w, look, Some(|ui: &mut egui::Ui| fields::inner_key(ui, t(Key::ReadNetAdd), None).clicked()))
                        });
                        if resp.changed() {
                            self.ux.rpc_err = None;
                        }
                        add = hit.unwrap_or(false) || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                    });
                });
            });
        });
        if let Some(i) = remove {
            let left: Vec<&str> = saved.iter().enumerate().filter(|(k, _)| *k != i).map(|(_, s)| s.as_str()).collect();
            self.ux.rpc_err = None;
            self.act(Action::SetEndpoints { specs: left.join(" ") }, now);
            return;
        }
        if !add {
            return;
        }
        if self.typed.rpc_new.trim().is_empty() {
            self.ux.rpc_err = Some(String::new());
            return;
        }
        match net_spec(&self.typed.rpc_new, home_chain, &saved) {
            Err(e) => self.ux.rpc_err = Some(e),
            Ok(spec) => {
                let mut all = saved;
                all.push(spec);
                if matches!(self.act(Action::SetEndpoints { specs: all.join(" ") }, now), Applied::Endpoints(_)) {
                    self.typed.rpc_new.clear();
                    self.ux.rpc_err = None;
                }
            }
        }
    }

    pub(super) fn set_log_asks(&mut self, ui: &mut egui::Ui, now: f64) {
        let stored = self.shell.settings.log_asks().to_string();
        let edit = &mut self.ux.asks_edit;
        let mut set = None;
        card::form(ui, |ui, f| {
            let words = card::SettingWords { save: t(Key::DoSetLogAsks), saved: t(Key::SettingSaved), unit: "", hint: "", key_w: 92.0 };
            let check = |x: &str| match x.parse::<u64>() {
                Ok(n) if zikaron_anchor::action::asks_within(n) => Ok(()),
                _ => Err(t(Key::LogAsksRange)),
            };
            set = f.setting(ui, t(Key::SetLogAsks), edit, &stored, 90.0, words, check);
        });
        if let Some(n) = set.and_then(|x| x.parse::<u64>().ok()) {
            if matches!(self.act(Action::SetLogAsks { n }, now), Applied::LogAsks(_)) {
                self.ux.asks_edit.land();
            }
        }
    }

    pub(super) fn set_rest_switch(&mut self, ui: &mut egui::Ui, now: f64) {
        let on = self.shell.settings.rest_on;
        let mut flip = false;
        card::form(ui, |ui, f| flip = f.switch(ui, t(Key::NetRestMode), "", on, true));
        if flip {
            self.act(Action::SetRestMode { on: !on }, now);
        }
    }

    pub(super) fn set_rest_rows(&mut self, ui: &mut egui::Ui, now: f64) {
        let rows: Vec<(String, String)> = self.shell.settings.rest.iter().map(|b| (b.clone(), self.shell.rest_key_said(b))).collect();
        let file_said = crate::chainx::keys_file_said();
        let err = self.ux.rest_err.clone();
        let (mut remove, mut add) = (None, false);
        card::section(ui, t(Key::NetRestTitle), "", |ui| {
            if let Some(e) = file_said.as_deref() {
                states::banner(ui, states::Banner::Bad, e, |_| ());
                ui.add_space(tk::S2);
            }
            card::form(ui, |ui, f| {
                f.keep(ui);
                for (base, sub) in &rows {
                    f.key(("rest-row", base));
                    f.tag_next(t(service_label(base)));
                    let (row, gone) = f.line(ui, None, base, sub, false, false, c(C::Ink), |ui| zikaron_ui::button::icon_key(ui, Glyph::No, 26.0, true, t(Key::KitRemove)).clicked());
                    let _ = zikaron_ui::layer::tip(row, base);
                    if gone {
                        remove = Some(base.clone());
                    }
                }
                f.key("rest-add");
                f.panel(ui, |ui| {
                    fields::stack(ui, |ui, st| {
                        let mono = fields::Look { mono: true, ..Default::default() };
                        let at = |key: bool| err.as_ref().filter(|(k, _)| *k == key).map(|(_, e)| (e.as_str(), true));
                        let base = st.field(ui, t(Key::NetRestBase), None, at(false), |ui, w| fields::line(ui, &mut self.typed.rest_base, REST_HINT, w, mono));
                        let key = st.field(ui, t(Key::NetRestKey), None, at(true), |ui, w| input::secret_line_w_marked(ui, &mut self.typed.rest_key, "", w, false));
                        if base.changed() || key.changed() {
                            self.ux.rest_err = None;
                        }
                        let enter = key.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        add = st.foot(ui, |ui| key::key(ui, t(Key::ReadNetAdd), Role::Secondary, true).clicked()) || enter;
                    });
                });
            });
        });
        if let Some(base) = remove {
            self.ux.rest_err = None;
            self.act(Action::RemoveRest { base }, now);
            return;
        }
        if !add {
            return;
        }
        let base = self.typed.rest_base.trim().to_string();
        if base.is_empty() {
            self.ux.rest_err = Some((false, String::new()));
            return;
        }
        if let Some(f) = crate::action::add_refused(&self.shell, &base, self.typed.rest_key.expose()) {
            let key = zikaron_anchor::rest::base_of(&base).is_ok() && f.which() == Some(crate::fault::Known::SettingsShape);
            let said = if key && self.typed.rest_key.expose().trim().is_empty() {
                ""
            } else if key {
                t(Key::NetRestKeySpaces)
            } else if f.human() == t(Key::FaultWhatSettingsShape) && !f.tail().is_empty() {
                f.tail()
            } else {
                f.human()
            };
            self.ux.rest_err = Some((key, said.to_string()));
            return;
        }
        if matches!(self.act(Action::AddRest { base, key: self.typed.rest_key.clone() }, now), Applied::RestAdded { .. } | Applied::RestKeyed { .. }) {
            self.typed.rest_base.clear();
            self.typed.rest_key.clear();
            self.ux.rest_err = None;
        }
    }

    pub(super) fn set_proxy(&mut self, ui: &mut egui::Ui, now: f64) {
        use crate::machine::proxy::{NONE, SYSTEM};
        use crate::proxycells::{self as pc, Bad};
        let written = self.shell.machine.proxy.clone();
        let cur = match written.as_deref().map(str::trim) {
            None | Some(SYSTEM) => 0,
            Some(NONE) => 1,
            Some(_) => 2,
        };
        let saved = written.as_deref().filter(|_| cur == 2).and_then(pc::of_saved);
        if cur == 2 && !self.ux.proxy_seeded {
            if let Some((cells, _)) = &saved {
                self.typed.proxy = cells.clone();
            }
            self.ux.proxy_seeded = true;
        }
        let shown = if self.ux.proxy_manual { 2 } else { cur };
        let choices: Vec<seg::Cell> = [Key::ProxySystem, Key::ProxyOff, Key::ProxyManual].iter().map(|k| seg::Cell::from(t(*k))).collect();
        let open = self.ux.proxy_manual || cur == 2;
        let c = &mut self.typed.proxy;
        let checked = pc::check(c);
        let is_bad = |b: Bad| matches!(&checked, Err(v) if v.contains(&b));
        let (mut pick, mut hit) = (None, false);
        let mut to_save = None;
        card::form(ui, |ui, f| {
            f.keep(ui);
            f.key("proxy");
            pick = f.with(ui, t(Key::SetProxy), "", |ui| seg::seg(ui, "set-proxy", &choices, shown));
            if !open {
                return;
            }
            f.key("cells");
            f.panel(ui, |ui| {
                fields::stack(ui, |ui, st| {
                    let kinds: Vec<seg::Cell> = pc::KINDS.iter().map(|(_, w)| seg::Cell::from(*w)).collect();
                    let at = pc::KINDS.iter().position(|(k, _)| *k == c.kind).unwrap_or(0);
                    if let Some(i) = st.field(ui, t(Key::U3RawType), None, None, |ui, _| seg::seg(ui, "proxy-kind", &kinds, at)) {
                        c.kind = pc::KINDS[i].0;
                    }
                    let host_line = (is_bad(Bad::Host) && !c.host.trim().is_empty()).then(|| (t(Key::ProxyBadHost), true));
                    let port_line = (is_bad(Bad::Port) && !c.port.trim().is_empty()).then(|| (t(Key::ProxyBadPort), true));
                    let (host, _) = st.pair_w(
                        ui,
                        128.0,
                        |ui, st| st.field(ui, t(Key::ProxyCellHost), None, host_line, |ui, w| fields::line(ui, &mut c.host, t(Key::ProxyHint), w, fields::Look { mono: true, ..Default::default() })),
                        |ui, st| st.field(ui, t(Key::ProxyCellPort), None, port_line, |ui, w| fields::line(ui, &mut c.port, "", w, fields::Look { mono: true, ..Default::default() })),
                    );
                    let pasted: Option<String> = ui.input(|i| {
                        let p: String = i.events.iter().filter_map(|e| if let egui::Event::Paste(p) = e { Some(p.as_str()) } else { None }).collect();
                        (!p.is_empty()).then_some(p)
                    });
                    if let Some(split) = pc::split_in(c, pasted.as_deref(), host.has_focus()) {
                        *c = split;
                    }
                    let user_shown = !(c.user.is_empty() && c.pass.is_empty());
                    let user_line = if is_bad(Bad::UserLong) && user_shown {
                        Some((t(Key::ProxyBadUserLong), true))
                    } else if is_bad(Bad::UserMissing) && user_shown {
                        Some(("", true))
                    } else {
                        None
                    };
                    let user_label = if pc::takes_password(c.kind) { Key::ProxyCellUser } else { Key::ProxyCellUserId };
                    let takes = pc::takes_password(c.kind);
                    let saved_proxy = written.as_deref().and_then(|w| zikaron_net::proxy_of(w).ok());
                    let typed_proxy = pc::check(c).ok().and_then(|a| zikaron_net::proxy_of(&a).ok());
                    let pass_line = if is_bad(Bad::PassLong) {
                        Some((t(Key::ProxyBadPassLong), true))
                    } else if takes && matches!((&saved_proxy, &typed_proxy), (Some(s), Some(t_)) if pc::password_stays(s, t_)) {
                        Some((t(Key::ProxyPasswordSet), false))
                    } else {
                        None
                    };
                    st.pair(
                        ui,
                        |ui, st| st.field(ui, t(user_label), Some(t(Key::ProxyCellOptional)), user_line, |ui, w| fields::line(ui, &mut c.user, "", w, fields::Look::default())),
                        |ui, st| {
                            if takes {
                                st.field(ui, t(Key::IdPassword), Some(t(Key::ProxyCellOptional)), pass_line, |ui, w| input::secret_line_w_marked(ui, &mut c.pass, "", w, false));
                            }
                        },
                    );
                    let typed_pass = !c.pass.is_empty() && takes;
                    let shown_written = written.as_deref().map(|w| zikaron_net::proxy_text_shown(w).0).unwrap_or_default();
                    to_save = pc::check(c).ok().filter(|a| cur != 2 || typed_pass || zikaron_net::proxy_text_shown(a).0 != shown_written);
                    st.foot(ui, |ui| hit = key::key(ui, t(Key::ReadNetSave), Role::Secondary, to_save.is_some()).clicked());
                });
            });
        });
        match pick {
            Some(2) => self.ux.proxy_manual = true,
            Some(i) if i == cur => self.ux.proxy_manual = false,
            Some(i) if i != shown => {
                self.ux.proxy_manual = false;
                self.act(Action::SetProxy { choice: if i == 0 { SYSTEM } else { NONE }.to_string() }, now);
            }
            _ => {}
        }
        if let (true, Some(address)) = (hit, to_save) {
            self.act(Action::SetProxy { choice: address }, now);
            if self.shell.machine.proxy.as_deref().is_some_and(|p| p != SYSTEM && p != NONE) {
                self.ux.proxy_manual = false;
                self.ux.proxy_seeded = false;
                self.typed.proxy.pass.clear();
            }
        }
    }
}

const REST_HINT: &str = "https://api.etherscan.io/v2/api";

fn rpc_head(s: &crate::settings::Settings) -> String {
    let by_hand = !s.endpoints.is_empty() && s.network.as_deref().and_then(crate::deploy::named).is_none() && crate::deploy::same_as_row(s.chain_id, s.registry, s.from_block, &s.endpoints).is_none();
    if by_hand {
        format!("{} · {}", t(Key::NetRpcTitle), t(Key::U3Custom))
    } else {
        t(Key::NetRpcTitle).to_string()
    }
}

fn chain_lead(spec: &str) -> Option<(&str, &str)> {
    let (c, u) = spec.trim().split_once('=')?;
    let c = c.trim();
    (!c.is_empty() && c.bytes().all(|b| b.is_ascii_digit())).then_some((c, u.trim()))
}

fn spec_parts(spec: &str) -> (Option<u64>, &str) {
    match chain_lead(spec) {
        Some((c, u)) => (c.parse().ok(), u),
        None => (None, spec.trim()),
    }
}

fn net_spec(text: &str, home: Option<u64>, saved: &[String]) -> Result<String, String> {
    let text = text.trim();
    if text.chars().any(char::is_whitespace) {
        return Err(t(Key::NetRpcSpace).to_string());
    }
    let spec = match (chain_lead(text), home) {
        (Some(_), _) => text.to_string(),
        (None, Some(c)) => format!("{c}={text}"),
        (None, None) => return Err(t(Key::NetRpcNoChain).to_string()),
    };
    let typed = crate::chainx::typed(&spec)?;
    if saved.iter().any(|s| crate::chainx::typed(s).is_ok_and(|e| e == typed)) {
        return Err(t(Key::NetRpcListed).to_string());
    }
    Ok(spec)
}

fn service_label(base: &str) -> Key {
    use zikaron_anchor::rest::Service;
    match zikaron_anchor::rest::base_of(base).map(|t| Service::of(&t.host)) {
        Ok(Service::Etherscan) => Key::NetSvcEtherscan,
        Ok(Service::Blockscout) => Key::NetSvcBlockscout,
        _ => Key::NetSvcLike,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nodes_source_is_said_in_the_nodes_own_title() {
        let row = crate::deploy::named("sepolia").expect("the sepolia row");
        let mut s = crate::settings::Settings { chain_id: Some(row.chain_id), registry: Some(row.registry_address()), from_block: row.from_block, network: Some("sepolia".into()), endpoints: row.endpoint_specs(), ..Default::default() };
        assert_eq!(rpc_head(&s), t(Key::NetRpcTitle), "a preset's own nodes");
        s.network = None;
        s.endpoints.push(format!("{}=https://node.example", row.chain_id));
        assert_eq!(rpc_head(&s), format!("{} · {}", t(Key::NetRpcTitle), t(Key::U3Custom)), "nodes set by hand");
        s.endpoints.clear();
        assert_eq!(rpc_head(&s), t(Key::NetRpcTitle), "no nodes, nothing to say of them");
    }
}
