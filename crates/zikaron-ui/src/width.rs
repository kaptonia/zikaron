//! The width contract: the page body's width comes from the viewport and nothing extends past it. Long
//! strings in cells elide by measured width; inline inputs leave room for trailing keys first.

/// Page body: its width is the available width on entry and wrapping is always on. Every page draws inside
/// it. Widths and elision keep content within the body; the clip stays the page's viewport, so a card's
/// edge and shadow in the right column are drawn whole.
pub fn body<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let w = ui.available_width().max(0.0);
    ui.set_width(w);
    ui.set_max_width(w);
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    add(ui)
}

/// Narrow to at most `max`, never widen.
pub fn narrow_to(ui: &mut egui::Ui, max: f32) {
    let w = max.min(ui.available_width()).max(0.0);
    ui.set_max_width(w);
}

/// Elide at the end by measured width: a string that fits stays; otherwise as many characters as fit, then
/// "…". Measured with the width this face actually lays out, never estimated from a character count.
pub fn elide_to(ui: &egui::Ui, s: &str, font: egui::FontId, max_w: f32) -> String {
    let width = |t: &str| ui.painter().layout_no_wrap(t.to_string(), font.clone(), egui::Color32::BLACK).size().x;
    if max_w <= 0.0 {
        return String::new();
    }
    if width(s) <= max_w {
        return s.to_string();
    }
    let cs: Vec<char> = s.chars().collect();
    let cut = |keep: usize| {
        let mut t: String = cs[..keep].iter().collect();
        t.push('…');
        t
    };
    let (mut lo, mut hi) = (0usize, cs.len().saturating_sub(1));
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if width(&cut(mid)) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if lo == 0 {
        "…".to_string()
    } else {
        cut(lo)
    }
}

/// Elide in the middle (head and tail kept): paths and digests, whose ends tell them apart.
pub fn elide_mid(ui: &egui::Ui, s: &str, font: egui::FontId, max_w: f32) -> String {
    let width = |t: &str| ui.painter().layout_no_wrap(t.to_string(), font.clone(), egui::Color32::BLACK).size().x;
    if max_w <= 0.0 {
        return String::new();
    }
    if width(s) <= max_w {
        return s.to_string();
    }
    let cs: Vec<char> = s.chars().collect();
    let cut = |keep: usize| {
        let head = keep.div_ceil(2);
        let tail = keep - head;
        let mut t: String = cs[..head].iter().collect();
        t.push('…');
        t.extend(cs[cs.len() - tail..].iter());
        t
    };
    let (mut lo, mut hi) = (0usize, cs.len().saturating_sub(1));
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if width(&cut(mid)) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if lo == 0 {
        "…".to_string()
    } else {
        cut(lo)
    }
}

/// A row whose trailing part (keys) is laid right to left first; the leading part takes what remains. Rows
/// are as tall as a field.
pub fn then<R, L>(ui: &mut egui::Ui, tail: impl FnOnce(&mut egui::Ui) -> R, lead: impl FnOnce(&mut egui::Ui, f32) -> L) -> (L, R) {
    then_at(ui, crate::tokens::INPUT_H, tail, lead)
}

/// The same row for words and a link: as tall as a line of body text.
pub fn then_line<R, L>(ui: &mut egui::Ui, tail: impl FnOnce(&mut egui::Ui) -> R, lead: impl FnOnce(&mut egui::Ui, f32) -> L) -> (L, R) {
    then_at(ui, crate::tokens::Type::Body.line(), tail, lead)
}

/// The same row at a given height (a card title with a pill beside it is as tall as the title's line).
pub fn then_at<R, L>(ui: &mut egui::Ui, h: f32, tail: impl FnOnce(&mut egui::Ui) -> R, lead: impl FnOnce(&mut egui::Ui, f32) -> L) -> (L, R) {
    let w = ui.available_width();
    ui.allocate_ui_with_layout(egui::vec2(w, h), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = crate::tokens::S2;
        let r = tail(ui);
        let room = ui.available_width().max(40.0);
        let l = ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| lead(ui, room)).inner;
        (l, r)
    })
    .inner
}

/// An input then trailing keys: the keys first, the input takes the rest.
pub fn line_then<R>(ui: &mut egui::Ui, text: &mut String, hint: &str, mono: bool, tail: impl FnOnce(&mut egui::Ui) -> R) -> (egui::Response, R) {
    then(ui, tail, |ui, room| crate::input::field(ui, text, hint, room, crate::input::Look { mono, ..Default::default() }))
}

/// The last part of a path (a file or folder name), without a trailing separator. Both separators count,
/// `/` and `\`, whichever system the path was written on (a path a Windows machine wrote, read here, names
/// its file the same); a path that is only separators, or empty, gives the empty string.
pub fn file_name(s: &str) -> String {
    let t = s.trim().trim_end_matches(['/', '\\']);
    t.rsplit(['/', '\\']).next().unwrap_or(t).to_string()
}

/// Cut to at most `max` characters with "…" in the middle (sentences that carry an id).
pub fn elide_chars(s: &str, max: usize) -> String {
    let cs: Vec<char> = s.chars().collect();
    if cs.len() <= max || max < 3 {
        return s.to_string();
    }
    let keep = max - 1;
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let mut out: String = cs[..head].iter().collect();
    out.push('…');
    out.extend(cs[cs.len() - tail..].iter());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_the_last_part_of_a_path() {
        assert_eq!(file_name("/a/b/c.txt"), "c.txt");
        assert_eq!(file_name("/a/b/dir/"), "dir");
        assert_eq!(file_name("plain"), "plain");
    }

    #[test]
    fn eliding_by_characters_keeps_both_ends_and_never_splits_a_character() {
        assert_eq!(elide_chars("abcdef", 10), "abcdef");
        let e = elide_chars("\u{e5}\u{e4}\u{f6}\u{e5}\u{e4}\u{f6}\u{e5}\u{e4}\u{f6}\u{e5}\u{e4}\u{f6}", 7);
        assert_eq!(e.chars().count(), 7);
        assert!(e.contains('…'));
    }
}
