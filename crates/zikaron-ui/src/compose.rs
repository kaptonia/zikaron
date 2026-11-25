use egui::{Id, ImeEvent, Ui};

fn held_id(edit_id: Id) -> Id {
    edit_id.with("zikaron-composing")
}

fn held(ui: &Ui, edit_id: Id) -> Option<(usize, String)> {
    ui.data(|d| d.get_temp::<(usize, String)>(held_id(edit_id)))
}

fn byte_at(s: &str, at: usize) -> usize {
    s.char_indices().nth(at).map_or(s.len(), |(b, _)| b)
}

fn holds(text: &str, at: usize, words: &str) -> bool {
    text.chars().skip(at).take(words.chars().count()).eq(words.chars())
}

pub fn around<R>(ui: &mut Ui, edit_id: Id, value: &mut String, draw: impl FnOnce(&mut Ui, &mut String) -> R) -> R {
    let before = held(ui, edit_id);
    let base = value.clone();
    if let Some((at, words)) = &before {
        let b = byte_at(value, *at);
        value.insert_str(b, words);
    }
    let had = ui.memory(|m| m.has_focus(edit_id));
    let events: Vec<ImeEvent> = if had { ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::Ime(x) = e { Some(x.clone()) } else { None }).collect()) } else { Vec::new() };
    let out = draw(ui, value);
    let last = events.iter().rposition(|e| matches!(e, ImeEvent::Preedit(_) | ImeEvent::Commit(_)));
    let off = events[last.map_or(0, |i| i + 1)..].iter().any(|e| matches!(e, ImeEvent::Disabled));
    let laid = match last.map(|i| &events[i]) {
        None => before.filter(|(at, w)| holds(value, *at, w)),
        Some(ImeEvent::Commit(_)) => None,
        Some(ImeEvent::Preedit(w)) if w.is_empty() => None,
        Some(ImeEvent::Preedit(w)) => egui::text_edit::TextEditState::load(ui.ctx(), edit_id)
            .and_then(|st| st.cursor.char_range())
            .map(|r| r.sorted_cursors()[0].index)
            .filter(|at| holds(value, *at, w))
            .or_else(|| {
                let same = base.chars().zip(value.chars()).take_while(|(a, b)| a == b).count();
                (0..=same).rev().find(|at| holds(value, *at, w))
            })
            .map(|at| (at, w.clone())),
        Some(_) => None,
    };
    let composing = !off && ui.memory(|m| m.has_focus(edit_id));
    if let Some((at, words)) = &laid {
        if holds(value, *at, words) {
            let (a, b) = (byte_at(value, *at), byte_at(value, *at + words.chars().count()));
            value.replace_range(a..b, "");
        }
    }
    ui.data_mut(|d| match laid.filter(|_| composing) {
        Some(c) => {
            d.insert_temp(held_id(edit_id), c);
        }
        None => d.remove::<(usize, String)>(held_id(edit_id)),
    });
    out
}
