#[derive(Clone, Debug)]
pub struct Gen<T> {
    held: Option<(u64, T)>,
    asked: u64,
}

impl<T> Default for Gen<T> {
    fn default() -> Self {
        Gen { held: None, asked: 0 }
    }
}

impl<T> Gen<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ask(&mut self) -> u64 {
        self.asked += 1;
        self.asked
    }

    pub fn asked(&self) -> u64 {
        self.asked
    }

    pub fn land(&mut self, at: u64, value: T) -> bool {
        if at != self.asked {
            return false;
        }
        self.held = Some((at, value));
        true
    }

    pub fn shown(&self) -> Option<&T> {
        self.held.as_ref().map(|(_, v)| v)
    }

    pub fn current(&self) -> bool {
        matches!(&self.held, Some((at, _)) if *at == self.asked)
    }

    pub fn forget(&mut self) {
        self.held = None;
        self.asked += 1;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Copies {
    Baseline,
    Settings,
    Queue,
    Machine,
    Register,
    Noticed,
}

impl Copies {
    const ALL: usize = 6;

    fn at(self) -> usize {
        match self {
            Copies::Baseline => 0,
            Copies::Settings => 1,
            Copies::Queue => 2,
            Copies::Machine => 3,
            Copies::Register => 4,
            Copies::Noticed => 5,
        }
    }
}

static TICKETS: [std::sync::atomic::AtomicU64; Copies::ALL] = [const { std::sync::atomic::AtomicU64::new(0) }; Copies::ALL];
static WROTE: [std::sync::atomic::AtomicU64; Copies::ALL] = [const { std::sync::atomic::AtomicU64::new(0) }; Copies::ALL];

pub fn ticket(c: Copies) -> u64 {
    TICKETS[c.at()].fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
}

pub fn wrote(c: Copies) {
    let t = ticket(c);
    WROTE[c.at()].fetch_max(t, std::sync::atomic::Ordering::SeqCst);
}

pub fn last_write(c: Copies) -> u64 {
    WROTE[c.at()].load(std::sync::atomic::Ordering::SeqCst)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Held([u64; Copies::ALL]);

impl Held {
    pub fn takes(&mut self, c: Copies, at: u64) -> bool {
        let wrote = WROTE[c.at()].load(std::sync::atomic::Ordering::SeqCst);
        if at <= self.0[c.at()] || at <= wrote {
            return false;
        }
        self.0[c.at()] = at;
        true
    }

    pub fn newer(&self, c: Copies, at: u64) -> bool {
        at > self.0[c.at()]
    }

    pub fn raise(&mut self, c: Copies, at: u64) {
        self.0[c.at()] = self.0[c.at()].max(at);
    }
}

#[derive(Debug)]
pub struct Cell<K, V> {
    held: std::sync::Mutex<Option<(K, V)>>,
}

impl<K, V> Default for Cell<K, V> {
    fn default() -> Self {
        Cell { held: std::sync::Mutex::new(None) }
    }
}

impl<K: PartialEq, V: Clone> Cell<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_or(&self, key: K, make: impl FnOnce() -> V) -> V {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((k, v)) = held.as_ref() {
            if *k == key {
                return v.clone();
            }
        }
        let v = make();
        *held = Some((key, v.clone()));
        v
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        held.as_ref().filter(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    pub fn held(&self) -> Option<V> {
        self.held.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|(_, v)| v.clone())
    }

    pub fn put(&self, key: K, value: V) {
        *self.held.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, value));
    }

    pub fn clear(&self) {
        *self.held.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

pub struct Road;

thread_local! {
    static ON_ROAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

static TOUCHED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub struct OnRoad(bool);

impl Road {
    pub fn begin() -> OnRoad {
        OnRoad(ON_ROAD.with(|r| r.replace(true)))
    }

    pub fn touched() -> u64 {
        TOUCHED.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Drop for OnRoad {
    fn drop(&mut self) {
        let was = self.0;
        ON_ROAD.with(|r| r.set(was));
    }
}

pub fn touch() {
    if ON_ROAD.with(|r| r.get()) {
        TOUCHED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

pub trait Cause: Copy + Eq + std::fmt::Debug + 'static {
    type Reading: Copy + Eq + std::fmt::Debug + 'static;
    const ALL: &'static [Self];
    fn stales(self) -> &'static [Self::Reading];
}

pub trait Watched: Copy + Eq + std::fmt::Debug + 'static {
    const ALL: &'static [Self];
    fn path(self, root: &std::path::Path) -> std::path::PathBuf;

    fn stamp(self, root: &std::path::Path) -> Option<Stamp> {
        let md = std::fs::metadata(self.path(root)).ok()?;
        Some(Stamp { len: md.len(), modified: md.modified().ok() })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub len: u64,
    pub modified: Option<std::time::SystemTime>,
}

pub fn changed<W: Watched>(before: &[(W, Option<Stamp>)], now: &[(W, Option<Stamp>)]) -> Vec<W> {
    now.iter().filter(|(w, s)| before.iter().find(|(b, _)| b == w).map(|(_, t)| t) != Some(s)).map(|(w, _)| *w).collect()
}

pub fn look<W: Watched>(root: &std::path::Path) -> Vec<(W, Option<Stamp>)> {
    W::ALL.iter().map(|w| (*w, w.stamp(root))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reading_lands_only_of_the_generation_last_asked_and_the_old_stays_shown() {
        let mut g: Gen<&str> = Gen::new();
        assert_eq!(g.shown(), None);
        let first = g.ask();
        assert!(g.land(first, "one"));
        assert!(g.current());
        let second = g.ask();
        assert!(!g.current());
        assert_eq!(g.shown(), Some(&"one"), "the old reading stays shown while the new one is on its way");
        let third = g.ask();
        assert!(!g.land(second, "late"), "an older generation landing late is dropped");
        assert!(g.land(third, "three"));
        assert_eq!(g.shown(), Some(&"three"));
        let fourth = g.ask();
        g.forget();
        assert_eq!(g.shown(), None, "the input changed: nothing read before answers it");
        assert!(!g.land(fourth, "in flight"), "a reading in flight when the input changed is dropped");
    }

    #[test]
    fn a_cell_derives_once_per_key() {
        let c: Cell<(u64, u64), usize> = Cell::new();
        let made = std::cell::Cell::new(0usize);
        let make = || {
            made.set(made.get() + 1);
            made.get()
        };
        assert_eq!(c.get(&(1, 1)), None);
        assert_eq!(c.get_or((1, 1), make), 1);
        assert_eq!(c.get_or((1, 1), make), 1, "the same key: held");
        assert_eq!(c.get(&(1, 1)), Some(1));
        assert_eq!(c.get_or((1, 2), make), 2, "another key: derived again");
        assert_eq!(c.get(&(1, 1)), None);
        c.clear();
        assert_eq!(c.get(&(1, 2)), None);
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Probe {
        Here,
        There,
    }

    impl Watched for Probe {
        const ALL: &'static [Probe] = &[Probe::Here, Probe::There];
        fn path(self, root: &std::path::Path) -> std::path::PathBuf {
            root.join(match self {
                Probe::Here => "here.json",
                Probe::There => "there.json",
            })
        }
    }

    #[test]
    fn a_watched_file_changed_from_outside_is_seen_and_no_other() {
        let root = std::env::temp_dir().join(format!("page-reads-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a folder");
        std::fs::write(Probe::There.path(&root), b"1").expect("written");
        let before = look::<Probe>(&root);
        assert_eq!(changed(&before, &look::<Probe>(&root)), Vec::<Probe>::new());
        std::fs::write(Probe::Here.path(&root), b"new").expect("written");
        assert_eq!(changed(&before, &look::<Probe>(&root)), vec![Probe::Here], "a file come");
        let before = look::<Probe>(&root);
        std::fs::write(Probe::There.path(&root), b"12").expect("written");
        assert_eq!(changed(&before, &look::<Probe>(&root)), vec![Probe::There], "a file changed");
        let before = look::<Probe>(&root);
        std::fs::remove_file(Probe::Here.path(&root)).expect("removed");
        assert_eq!(changed(&before, &look::<Probe>(&root)), vec![Probe::Here], "a file gone");
        let _ = std::fs::remove_dir_all(&root);
    }
}
