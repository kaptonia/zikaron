#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Column {
    App,
    Edit,
    Window,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Act {
    About,
    Settings,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Quit,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Minimize,
    Zoom,
    Close,
}

impl Act {
    pub fn own(self) -> bool {
        matches!(self, Act::About | Act::Settings)
    }

    pub fn selector(self) -> &'static str {
        match self {
            Act::About => "zikaronAbout:",
            Act::Settings => "zikaronSettings:",
            Act::Services => "",
            Act::Hide => "hide:",
            Act::HideOthers => "hideOtherApplications:",
            Act::ShowAll => "unhideAllApplications:",
            Act::Quit => "terminate:",
            Act::Cut => "cut:",
            Act::Copy => "copy:",
            Act::Paste => "paste:",
            Act::SelectAll => "selectAll:",
            Act::Minimize => "performMiniaturize:",
            Act::Zoom => "performZoom:",
            Act::Close => "performClose:",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Item { act: Act, key: &'static str, option: bool },
    Sep,
}

const fn item(act: Act, key: &'static str) -> Entry {
    Entry::Item { act, key, option: false }
}

pub const BAR: [(Column, &[Entry]); 3] = [
    (
        Column::App,
        &[
            item(Act::About, ""),
            Entry::Sep,
            item(Act::Settings, ","),
            Entry::Sep,
            item(Act::Services, ""),
            Entry::Sep,
            item(Act::Hide, "h"),
            Entry::Item { act: Act::HideOthers, key: "h", option: true },
            item(Act::ShowAll, ""),
            Entry::Sep,
            item(Act::Quit, "q"),
        ],
    ),
    (Column::Edit, &[item(Act::Cut, "x"), item(Act::Copy, "c"), item(Act::Paste, "v"), item(Act::SelectAll, "a")]),
    (Column::Window, &[item(Act::Minimize, "m"), item(Act::Zoom, ""), Entry::Sep, item(Act::Close, "w")]),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Words {
    pub columns: Vec<(Column, String)>,
    pub items: Vec<(Act, String)>,
}

impl Words {
    pub fn column(&self, c: Column) -> &str {
        self.columns.iter().find(|(x, _)| *x == c).map(|(_, w)| w.as_str()).unwrap_or("")
    }

    pub fn item(&self, a: Act) -> &str {
        self.items.iter().find(|(x, _)| *x == a).map(|(_, w)| w.as_str()).unwrap_or("")
    }
}

pub fn acts() -> Vec<Act> {
    BAR.iter().flat_map(|(_, es)| es.iter()).filter_map(|e| if let Entry::Item { act, .. } = e { Some(*act) } else { None }).collect()
}

static ASKED: std::sync::Mutex<Vec<Act>> = std::sync::Mutex::new(Vec::new());

static SETTINGS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

static WAKE: std::sync::Mutex<Option<Box<dyn Fn() + Send>>> = std::sync::Mutex::new(None);

#[cfg(any(target_os = "macos", test))]
pub(super) fn ask(a: Act) {
    ASKED.lock().unwrap_or_else(|e| e.into_inner()).push(a);
    if let Some(w) = WAKE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        w();
    }
}

pub fn asked() -> Vec<Act> {
    std::mem::take(&mut *ASKED.lock().unwrap_or_else(|e| e.into_inner()))
}

pub fn on_ask(wake: Box<dyn Fn() + Send>) {
    *WAKE.lock().unwrap_or_else(|e| e.into_inner()) = Some(wake);
}

pub fn set_settings_reachable(on: bool) {
    SETTINGS.store(on, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(any(target_os = "macos", test))]
pub(super) fn enabled(a: Act) -> bool {
    match a {
        Act::Settings => SETTINGS.load(std::sync::atomic::Ordering::Relaxed),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_is_one_closed_table_its_keys_each_once() {
        assert_eq!(BAR.iter().map(|(c, _)| *c).collect::<Vec<_>>(), vec![Column::App, Column::Edit, Column::Window]);
        let acts = acts();
        let mut seen = acts.clone();
        seen.dedup();
        assert_eq!(seen.len(), 14, "fourteen items, each once: {acts:?}");
        let keys: Vec<(&str, bool)> = BAR.iter().flat_map(|(_, es)| es.iter()).filter_map(|e| match e {
            Entry::Item { key, option, .. } if !key.is_empty() => Some((*key, *option)),
            _ => None,
        }).collect();
        let mut once = keys.clone();
        once.sort();
        once.dedup();
        assert_eq!(once.len(), keys.len(), "a key twice: {keys:?}");
        assert_eq!(acts.iter().filter(|a| a.own()).copied().collect::<Vec<_>>(), vec![Act::About, Act::Settings]);
        assert!(acts.iter().all(|a| *a == Act::Services || !a.selector().is_empty()), "every item names its action");
    }

    #[test]
    fn settings_follows_the_window_and_a_press_is_taken_once() {
        set_settings_reachable(false);
        assert!(!enabled(Act::Settings) && enabled(Act::About));
        set_settings_reachable(true);
        assert!(enabled(Act::Settings));
        let _ = asked();
        ask(Act::Settings);
        ask(Act::About);
        assert_eq!(asked(), vec![Act::Settings, Act::About]);
        assert!(asked().is_empty(), "taken once");
    }
}
