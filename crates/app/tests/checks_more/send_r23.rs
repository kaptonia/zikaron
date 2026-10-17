//! What the shared sender asks before it signs, and how it keeps a batch in flight, end to end over a home
//! (`Action::SendBatch`, after `Action::EstimateGas`): the chain asked whether it already holds these anchors from
//! this key (`zikaron_send::batch::on_chain`), a batch landed as submitted before its bytes go, a batch this home
//! already has out asked about by hash and never signed again, and the desktop reading the queue file again when
//! only it changed (`Shell::follow_disk`). Each test names the forms of its boundary it holds.
//!
//! Nodes are fakes on 127.0.0.1 in this process (`a8_send`), so what was asked and broadcast is read where it
//! arrived; each test runs alone in its own process ([`super::alone_in`]). No network.

use super::a8_send::{bench, chain, first_param, hash_of, homes, node, nothing_broadcast, send, tidy, Node, Reply, PLENTY};
use app::action::{apply, Action, Applied};
use app::fault::Known;
use app::queue::{Kept as _, Step};
use app::task::Done;

/// The queue on disk of the bench home named `form`.
fn disk(form: &str) -> app::queue::Queue {
    app::queue::Queue::read(&app::home::Home::bare(homes().join(form).join("home"))).expect("the queue reads")
}

/// The question of `body` is the chain's "does it already hold these" (`send::on_chain`): a log question with a
/// third topic, the hashes asked about. The exit gate's scan asks with two.
fn asks_held(body: &str) -> bool {
    let v = zikaron::json::parse(body.as_bytes()).ok();
    let topics = v.as_ref().and_then(|v| v.member("params")).and_then(|p| match p {
        zikaron::json::Value::Arr(a) => a.first().cloned(),
        _ => None,
    });
    matches!(topics.as_ref().and_then(|f| f.member("topics")), Some(zikaron::json::Value::Arr(t)) if t.len() == 3)
}

/// One `Anchored` log of the registry `registry` from `from` for `hash` (`0x` hex), in transaction `tx`, at block 9.
fn log_of(registry: [u8; 20], from: [u8; 20], hash: &str, tx: u8) -> String {
    format!(
        "{{\"address\":\"{}\",\"blockNumber\":\"0x9\",\"data\":\"0x\",\"logIndex\":\"0x0\",\"topics\":[\"{}\",\"0x{}{}\",\"{}\"],\"transactionHash\":\"0x{}\"}}",
        zikaron::hexfmt::encode(&registry),
        zikaron::hexfmt::encode(&zikaron_anchor::scan::topic0()),
        "00".repeat(12),
        zikaron::hexfmt::encode(&from).trim_start_matches("0x"),
        hash,
        format!("{tx:02x}").repeat(32)
    )
}

/// The entry the bench's batch of one sends, and the account it sends from.
fn batch_of(shell: &app::shell::Shell) -> (String, [u8; 20]) {
    (shell.queue.take_ids(1).first().cloned().expect("a queued entry"), shell.anchor.expect("an anchor key").0)
}

/// Forms of the question asked before signing: the hash held by this key through this registry (refused
/// `ON_CHAIN`, nothing signed or broadcast, the batch recorded refused with that code); held by another sender,
/// or through another contract (not this key's anchor: sent); the logs question refused by the only node
/// (`UNREACHABLE`, nothing sent on a guess); two nodes, one refusing and one holding it (`ON_CHAIN`); two nodes,
/// one refusing and one holding nothing (one answer is enough: sent).
#[test]
fn what_the_chain_holds_from_this_key_is_refused_before_anything_is_signed() {
    if super::alone_in(module_path!(), "what_the_chain_holds_from_this_key_is_refused_before_anything_is_signed") {
        return;
    }
    let mut kept = Vec::new();
    for form in ["heldHere", "otherSender", "otherContract", "logsRefused", "oneRefusedOneHeld", "oneRefusedOneEmpty"] {
        // The entry and account are known once the bench is laid; the nodes read them from here.
        let said: std::sync::Arc<std::sync::Mutex<Option<(String, [u8; 20])>>> = std::sync::Arc::new(std::sync::Mutex::new(None));
        let holding = |said: std::sync::Arc<std::sync::Mutex<Option<(String, [u8; 20])>>>, registry: Option<[u8; 20]>, sender: Option<[u8; 20]>| {
            node(move |m, b| match m {
                "eth_getLogs" if asks_held(b) => {
                    let Some((hash, from)) = said.lock().expect("said").clone() else { return Reply::Result("[]".into()) };
                    Reply::Result(format!("[{}]", log_of(registry.unwrap_or([0x11; 20]), sender.unwrap_or(from), &hash, 0x9a)))
                }
                _ => chain(m, b, PLENTY),
            })
        };
        let refusing = || {
            node(|m, b| match m {
                "eth_getLogs" if asks_held(b) => Reply::Error("{\"code\":-32000,\"message\":\"logs unavailable\"}".into()),
                _ => chain(m, b, PLENTY),
            })
        };
        let nodes: Vec<Node> = match form {
            "heldHere" => vec![holding(said.clone(), None, None)],
            "otherSender" => vec![holding(said.clone(), None, Some([0x22; 20]))],
            "otherContract" => vec![holding(said.clone(), Some([0x33; 20]), None)],
            "logsRefused" => vec![refusing()],
            "oneRefusedOneHeld" => vec![refusing(), holding(said.clone(), None, None)],
            "oneRefusedOneEmpty" => vec![refusing(), node(|m, b| chain(m, b, PLENTY))],
            other => panic!("no form {other}"),
        };
        let refs: Vec<&Node> = nodes.iter().collect();
        let mut shell = bench(form, &refs);
        let (id, from) = batch_of(&shell);
        *said.lock().expect("said") = Some((id.clone(), from));
        let got = send(&mut shell);
        kept.push(shell);
        match form {
            "heldHere" | "oneRefusedOneHeld" => {
                let f = got.expect_err("held already: refused");
                assert_eq!(f.which(), Some(Known::OnChain), "{form}: {f:?}");
                assert!(f.tail().contains(&id), "{form}: the hash held is named: {}", f.tail());
                nothing_broadcast(form, &refs);
                assert_eq!(disk(form).step_of(&id), Some(&Step::Refused { said: Known::OnChain.as_str().into() }), "{form}: recorded refused with the code");
            }
            "logsRefused" => {
                let f = got.expect_err("no answer: refused");
                assert_eq!(f.which(), Some(Known::Unreachable), "{form}: {f:?}");
                nothing_broadcast(form, &refs);
            }
            _ => assert!(matches!(got, Ok(Done::Submitted { .. })), "{form}: not this key's anchor here, sent: {got:?}"),
        }
    }
    tidy();
}

/// Forms of the broadcast, each read at the moment the bytes reach the node: taken (the batch is already
/// recorded submitted with that very hash, and stays so); refused by the node's own verdict (recorded submitted
/// at that moment, then back to refused with the verdict's code); the connection closed with no answer and the
/// node not knowing the hash (recorded submitted at that moment, and left in flight for the receipt wait to ask
/// by hash); the queue file not writable when the batch is to be landed (nothing broadcast, the queue as it was).
#[test]
fn a_batch_is_landed_in_flight_before_its_bytes_go() {
    if super::alone_in(module_path!(), "a_batch_is_landed_in_flight_before_its_bytes_go") {
        return;
    }
    let mut kept = Vec::new();
    for form in ["taken", "refused", "closed"] {
        let at_cast: std::sync::Arc<std::sync::Mutex<Option<(String, app::queue::Queue)>>> = std::sync::Arc::new(std::sync::Mutex::new(None));
        let (seen, home) = (at_cast.clone(), homes().join(form).join("home"));
        let n = node(move |m, b| match m {
            "eth_sendRawTransaction" => {
                let q = app::queue::Queue::read(&app::home::Home::bare(home.clone())).expect("the queue reads at the broadcast");
                *seen.lock().expect("seen") = Some((hash_of(&first_param(b)), q));
                match form {
                    "refused" => Reply::Error("{\"code\":-32000,\"message\":\"insufficient funds for gas * price + value\"}".into()),
                    "closed" => Reply::Close,
                    _ => chain(m, b, PLENTY),
                }
            }
            _ => chain(m, b, PLENTY),
        });
        let mut shell = bench(form, &[&n]);
        let (id, _) = batch_of(&shell);
        let got = send(&mut shell);
        kept.push(shell);
        let (hash, then) = at_cast.lock().expect("seen").clone().expect("the bytes reached the node");
        match then.step_of(&id) {
            Some(Step::Submitted { tx, nonce, .. }) => assert_eq!((tx.as_str(), *nonce), (hash.as_str(), Some(5)), "{form}: landed with the hash and nonce signed"),
            other => panic!("{form}: not landed as submitted when the bytes went: {other:?}"),
        }
        let now = disk(form);
        match form {
            "taken" => {
                assert!(matches!(got, Ok(Done::Submitted { ref tx, .. }) if *tx == hash), "{form}: {got:?}");
                assert_eq!(now.step_of(&id).and_then(Step::awaited).map(|(t, _)| t), Some(vec![hash.clone()]), "{form}: in flight");
            }
            "refused" => {
                let f = got.expect_err("refused");
                assert_eq!(f.which(), Some(Known::InsufficientFunds), "{form}");
                assert_eq!(now.step_of(&id), Some(&Step::Refused { said: Known::InsufficientFunds.as_str().into() }), "{form}: back, refused with the code");
                assert_eq!(f.out(), None, "{form}: nothing out");
                assert!(matches!(app::door::refused(&f), zikaron_glue::door::Reply::Refused { .. }), "{form}: the door answers a refusal");
            }
            _ => {
                let f = got.expect_err("unknown: refused");
                assert_eq!(f.which(), Some(Known::Unreachable), "{form}");
                assert_eq!(now.step_of(&id).and_then(Step::awaited).map(|(t, _)| t), Some(vec![hash.clone()]), "{form}: left in flight");
                // The batch is out: the refusal carries its transaction, and the door answers no node heard it, never
                // that nothing was sent.
                assert_eq!(f.out(), Some(hash.as_str()), "{form}: the transaction out");
                assert!(matches!(app::door::refused(&f), zikaron_glue::door::Reply::Unheard { ref tx, .. } if *tx == hash), "{form}: the door answers it unheard");
            }
        }
    }
    // The queue file cannot be written when the batch is to be landed (its room taken from this user): nothing is
    // broadcast, the refusal is the system's, and the queue stays as it was.
    #[cfg(unix)]
    {
        let form = "unwritable";
        let n = node(|m, b| chain(m, b, PLENTY));
        let mut shell = bench(form, &[&n]);
        let (id, _) = batch_of(&shell);
        let room = app::home::Home::bare(homes().join(form).join("home")).dir(app::home::Slot::Settings);
        // The portable call (the system's own permission words stay in the platform piece).
        let mut p = std::fs::metadata(&room).expect("the room").permissions();
        p.set_readonly(true);
        std::fs::set_permissions(&room, p.clone()).expect("the room made read-only");
        let got = send(&mut shell);
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(false);
        std::fs::set_permissions(&room, p).expect("the room given back");
        let f = got.expect_err("not landed: not sent");
        assert!(f.which() != Some(Known::OnChain) && f.which() != Some(Known::InsufficientFunds), "{form}: the write's own refusal: {f:?}");
        assert!(n.raws().is_empty(), "{form}: nothing broadcast");
        assert_eq!(disk(form).step_of(&id), Some(&Step::Queued), "{form}: the queue as it was");
        kept.push(shell);
    }
    tidy();
}

/// A batch whose entry this home already has out (the queue on disk records its transaction; this window's copy
/// does not know yet) is handed to the receipt wait by that transaction's hash: no nonce asked, nothing signed or
/// broadcast.
#[test]
fn a_batch_this_home_has_out_is_asked_about_by_hash_and_not_signed_again() {
    if super::alone_in(module_path!(), "a_batch_this_home_has_out_is_asked_about_by_hash_and_not_signed_again") {
        return;
    }
    let form = "earlier";
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench(form, &[&n]);
    let (id, _) = batch_of(&shell);
    let out = format!("0x{}", "5e".repeat(32));
    let home = app::home::Home::bare(homes().join(form).join("home"));
    let (marked, _) = app::queue::amend(&home, |q| q.mark(std::slice::from_ref(&id), Step::Submitted { tx: out.clone(), chain: 31337, nonce: Some(4) })).expect("written behind the window");
    assert_eq!(marked, 1);
    assert_eq!(shell.queue.step_of(&id), Some(&Step::Queued), "the window's copy does not know yet");
    let got = send(&mut shell);
    assert!(matches!(got, Ok(Done::Submitted { ref tx, ref ids, .. }) if *tx == out && *ids == vec![id.clone()]), "{form}: that batch, by its hash: {got:?}");
    nothing_broadcast(form, &[&n]);
    drop(shell);
    tidy();
}

/// What the disk held when the home opened is what was read: another writer's change to the queue file landing
/// between the opening and the desktop's first look at the disk is read at that look, never taken as seen.
#[test]
fn a_queue_changed_before_the_first_look_is_read_at_it() {
    if super::alone_in(module_path!(), "a_queue_changed_before_the_first_look_is_read_at_it") {
        return;
    }
    let form = "firstlook";
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench(form, &[&n]);
    let (id, _) = batch_of(&shell);
    let root = homes().join(form).join("home");
    let home = app::home::Home::bare(root.clone());
    // The command line writes alone and holds the home's lock as the desktop opens it again: no turn of the
    // desktop's own reads the disk before its first look.
    assert!(matches!(app::action::apply_settled(&mut shell, Action::SetCliAlone { lasts: app::machine::Lasts::Never, pin: "27618394".into() }), Applied::CliAloneSet(_)));
    let held = zikaron_seal::writer::take(&root, &app::home::machine_dir().expect("machine")).expect("the command line's turn");
    assert!(matches!(held.took, zikaron_seal::writer::Took::Writer), "the command line holds the lock");
    let _ = apply(&mut shell, Action::OpenHome { root: root.display().to_string() });
    let sub = Step::Submitted { tx: format!("0x{}", "7a".repeat(32)), chain: 31337, nonce: Some(0) };
    app::queue::amend(&home, |q| q.mark(std::slice::from_ref(&id), sub.clone())).expect("written behind the window");
    assert_eq!(shell.queue.step_of(&id), Some(&Step::Queued), "not looked yet");
    let at = if shell.followed_at.is_finite() { shell.followed_at.max(0.0) } else { 0.0 } + 100.0;
    shell.follow_disk(at);
    assert_eq!(shell.queue.step_of(&id), Some(&sub), "the first look reads what changed since the opening");
    drop(held);
    drop(shell);
    tidy();
}

/// The desktop's look at the disk reads the queue again when only the queue file changed (another writer marked an
/// entry submitted, no entry added), leaves it when nothing changed, and reads it again when the file was removed
/// (no file: an empty queue).
#[test]
fn the_desktop_reads_the_queue_file_again_when_only_it_changed() {
    if super::alone_in(module_path!(), "the_desktop_reads_the_queue_file_again_when_only_it_changed") {
        return;
    }
    let form = "follows";
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench(form, &[&n]);
    let (id, _) = batch_of(&shell);
    let base = if shell.followed_at.is_finite() { shell.followed_at.max(0.0) } else { 0.0 } + 100.0;
    let every = app::shell::FOLLOW_EVERY;
    shell.follow_disk(base);
    let seen = shell.seen_entries;
    let home = app::home::Home::bare(homes().join(form).join("home"));
    let sub = Step::Submitted { tx: format!("0x{}", "6f".repeat(32)), chain: 31337, nonce: Some(0) };
    app::queue::amend(&home, |q| q.mark(std::slice::from_ref(&id), sub.clone())).expect("written behind the window");
    assert_eq!(shell.queue.step_of(&id), Some(&Step::Queued), "not looked yet");
    shell.follow_disk(base + every + 1.0);
    assert_eq!(shell.seen_entries, seen, "no entry was added");
    assert_eq!(shell.queue.step_of(&id), Some(&sub), "the queue file alone changed: read again");
    let before = shell.queue.clone();
    shell.follow_disk(base + 2.0 * (every + 1.0));
    assert_eq!(shell.queue, before, "nothing changed: as it was");
    // The queue file removed by another writer: read again, no file is an empty queue.
    std::fs::remove_file(home.dir(app::home::Slot::Settings).join(app::queue::FILE)).expect("the file removed");
    shell.follow_disk(base + 3.0 * (every + 1.0));
    assert!(shell.queue.items.is_empty(), "no file: an empty queue: {:?}", shell.queue);
    drop(shell);
    tidy();
}

/// The sender's closed codes are answered by both ends in one spelling: each member as the desktop's closed table
/// spells the refusal it becomes (the command line's `token` on its credential road says `Code::as_str`).
#[test]
fn the_senders_codes_are_spelled_as_the_desktop_spells_them() {
    for c in zikaron_send::Code::ALL {
        assert_eq!(app::fault::known_of_sent(c).as_str(), c.as_str(), "{c:?}");
    }
}

/// A send's receipt is settled under the home's writer lock (the lock's second form): with the lock in the shell's
/// hand, under it, the hand kept; with the lock let go, taken for that one write and let go after; with the shell's
/// lock gone, taken the same way. A receipt wait in flight holds no writer's turn, while any other local write does.
#[test]
fn a_receipt_settles_under_the_lock_it_finds_or_takes() {
    super::vault_open();
    use zikaron_seal::writer::{take, Took};
    let dir = std::env::temp_dir().join(format!("zk-test-settle-lock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("home");
    let machine = app::home::machine_dir().expect("machine");
    let busy = || matches!(take(home.root(), &machine).map(|t| t.took), Ok(Took::Busy { .. }));
    let mut lock = app::lock::take(&home).expect("lock");
    let hand = lock.hand();
    assert_eq!(app::lock::settling(Some(&hand), home.root(), busy).ok(), Some(true), "under the shell's lock");
    assert!(lock.held(), "the shell keeps it");
    lock.release();
    assert_eq!(app::lock::settling(Some(&hand), home.root(), busy).ok(), Some(true), "taken for the one write");
    assert!(!lock.held() && !busy(), "and let go after");
    drop(lock);
    assert_eq!(app::lock::settling(Some(&hand), home.root(), busy).ok(), Some(true), "the shell's lock gone: taken the same way");
    assert!(!busy());
    // The shell left the home and came back while the wait ran: its lock now is another, holding the kernel lock;
    // the wait settles under that one at once (not taking the lock beside it, which this process already holds).
    let again = app::lock::take(&home).expect("the lock taken again");
    let started = std::time::Instant::now();
    assert_eq!(app::lock::settling(Some(&hand), home.root(), busy).ok(), Some(true), "under the lock taken again");
    assert!(started.elapsed() < app::lock::SETTLE_STEP * 5, "at once, not after asking again and again");
    assert!(again.held(), "the shell keeps it");
    drop(again);
    // The home's mark names another machine: this instance holds the kernel lock as no writer of it, and a receipt
    // is not settled under it (refused as read-only, nothing written), as taking the lock afresh refuses too.
    std::fs::write(zikaron_seal::writer::mark_path(home.root()), format!("{}\n", "ab".repeat(16))).expect("another machine's mark");
    let other = app::lock::take(&home).expect("the lock as another machine's home");
    assert_eq!(other.mode(), app::lock::Mode::OtherMachine);
    let mut ran = false;
    let got = app::lock::settling(Some(&other.hand()), home.root(), || ran = true);
    assert!(matches!(&got, Err(f) if f.which() == Some(app::fault::Known::ReadOnly)) && !ran, "not settled under another machine's home: {got:?}");
    drop(other);
    let go = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let wait = |go: std::sync::Arc<std::sync::atomic::AtomicBool>| {
        move || {
            while !go.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(app::fault::Fault::unknown("done".to_string()))
        }
    };
    let mut tasks = app::task::Tasks::new();
    assert_eq!(tasks.spawn_turnless(app::task::Kind::Anchor, wait(go.clone())), app::task::Spawned::Started);
    assert!(!tasks.holds_turn(), "a receipt wait holds no turn");
    assert_eq!(tasks.spawn(app::task::Kind::Record, wait(go.clone())), app::task::Spawned::Started);
    assert!(tasks.holds_turn(), "a record does");
    go.store(true, std::sync::atomic::Ordering::SeqCst);
    let bound = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !tasks.flying().is_empty() && std::time::Instant::now() < bound {
        let _ = tasks.drain();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(tasks.flying().is_empty() && !tasks.holds_turn());
    let _ = std::fs::remove_dir_all(&dir);
}
