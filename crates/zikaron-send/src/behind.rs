use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(pub [u8; 32]);

impl Key {
    pub fn of(root: &[u8], basis: &[u8], pile: &[u8]) -> Key {
        let mut b: Vec<u8> = b"background-audit/1".to_vec();
        for part in [root, basis, pile] {
            b.extend_from_slice(&(part.len() as u64).to_be_bytes());
            b.extend_from_slice(part);
        }
        Key(zikaron::cryptox::sha256(&b))
    }

    pub fn hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum State<R, F> {
    Unaudited,
    Auditing,
    Before(&'static str),
    Done(R),
    Failed(F),
}

pub trait Store<R>: Send + Sync {
    fn load(&self, key: &Key) -> Option<R>;
    fn save(&self, key: &Key, result: &R);
}

pub struct Prelude<F> {
    pub state: &'static str,
    pub step: Box<dyn Fn(&Key) -> Result<(), F> + Send + Sync>,
}

pub struct Job<R, F> {
    pub keeper: Option<Arc<dyn zikaron_anchor::settled::Keep>>,
    pub work: Box<dyn FnOnce() -> Result<R, F> + Send>,
}

enum Ask<R, F> {
    Recall(Key),
    Audit(Key, Job<R, F>),
}

struct Inner<R, F> {
    queue: VecDeque<Ask<R, F>>,
    states: BTreeMap<Key, State<R, F>>,
}

pub struct Audits<R, F> {
    inner: Mutex<Inner<R, F>>,
    store: Option<Arc<dyn Store<R>>>,
    prelude: Option<Prelude<F>>,
}

impl<R: Clone + Send + 'static, F: Clone + Send + 'static> Audits<R, F> {
    pub fn new(store: Option<Arc<dyn Store<R>>>) -> Audits<R, F> {
        Audits { inner: Mutex::new(Inner { queue: VecDeque::new(), states: BTreeMap::new() }), store, prelude: None }
    }

    pub fn with_prelude(mut self, p: Prelude<F>) -> Audits<R, F> {
        self.prelude = Some(p);
        self
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner<R, F>> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn state(&self, key: &Key) -> State<R, F> {
        self.lock().states.get(key).cloned().unwrap_or(State::Unaudited)
    }

    pub fn ask(&self, key: Key, job: Job<R, F>) -> bool {
        let mut i = self.lock();
        if matches!(i.states.get(&key), Some(State::Auditing | State::Before(_))) {
            return false;
        }
        i.states.insert(key, State::Auditing);
        i.queue.push_back(Ask::Audit(key, job));
        true
    }

    pub fn recall(&self, key: Key) -> bool {
        let mut i = self.lock();
        if i.states.contains_key(&key) || i.queue.iter().any(|a| matches!(a, Ask::Recall(k) if *k == key)) {
            return false;
        }
        i.queue.push_back(Ask::Recall(key));
        true
    }

    pub fn waiting(&self) -> bool {
        !self.lock().queue.is_empty()
    }

    pub fn run(&self) -> usize {
        self.run_each(|| {})
    }

    pub fn run_each(&self, each: impl Fn()) -> usize {
        let mut taken = 0;
        loop {
            let Some(next) = self.lock().queue.pop_front() else { return taken };
            if taken > 0 {
                each();
            }
            taken += 1;
            match next {
                Ask::Recall(key) => {
                    if self.lock().states.contains_key(&key) {
                        continue;
                    }
                    if let Some(r) = self.store.as_ref().and_then(|s| s.load(&key)) {
                        self.lock().states.entry(key).or_insert(State::Done(r));
                    }
                }
                Ask::Audit(key, job) => {
                    let got = {
                        let _behind = zikaron_anchor::action::open_behind(job.keeper.clone());
                        let before = match &self.prelude {
                            Some(p) => {
                                self.lock().states.insert(key, State::Before(p.state));
                                let r = (p.step)(&key);
                                self.lock().states.insert(key, State::Auditing);
                                r
                            }
                            None => Ok(()),
                        };
                        before.and_then(|()| (job.work)())
                    };
                    match got {
                        Ok(r) => {
                            if let Some(s) = &self.store {
                                s.save(&key, &r);
                            }
                            self.lock().states.insert(key, State::Done(r));
                        }
                        Err(f) => {
                            self.lock().states.insert(key, State::Failed(f));
                        }
                    }
                }
            }
        }
    }
}
