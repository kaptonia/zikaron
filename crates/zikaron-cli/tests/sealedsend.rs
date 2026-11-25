use zikaron::json::Value;
use zikaron_cli::codes::Field;
use zikaron_cli::entry;

fn key(n: u8) -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = n;
    k
}

fn pile(notes: &[&str]) -> Vec<Vec<u8>> {
    let k = key(7);
    let author = entry::address(&k).expect("address");
    let g = entry::seal(&author, "genesis", 0, None, entry::shape(vec![(Field::StatementMd, Value::Str("s".into()))]), &k).expect("genesis");
    let mut out = vec![g.bytes.clone()];
    for n in notes {
        let body = entry::shape(vec![(Field::Subject, Value::Str(g.entry.id_hex())), (Field::NoteMd, Value::Str(n.to_string()))]);
        out.push(entry::seal(&author, "annotation", 1, Some(&g.entry.id_hex()), body, &k).expect("annotation").bytes);
    }
    out
}

#[test]
fn a_ledger_breaks_by_its_own_hard_findings() {
    assert_eq!(zikaron_send::shut::breaks(&pile(&["a"])), Vec::<String>::new(), "one line");
    assert_eq!(zikaron_send::shut::breaks(&pile(&["a", "b"])), vec!["EQUIVOCATION".to_string()], "two at one place");
    let headless: Vec<Vec<u8>> = pile(&["a"]).into_iter().skip(1).collect();
    assert_eq!(zikaron_send::shut::breaks(&headless), Vec::<String>::new(), "no genesis");
    assert_eq!(zikaron_send::shut::breaks(&[]), Vec::<String>::new(), "empty");
}

#[test]
fn the_senders_evidence_is_said_in_the_command_lines_words() {
    use zikaron_send::Why;
    let said = zikaron_cli::sealedsend::said;
    assert_eq!(said(&Why::NonceUnread { node: "http://n:1".into() }), "http://n:1: nonce unreadable");
    assert_eq!(said(&Why::Funds { need: 10, have: 9 }), "need 10 wei, have 9 wei");
    assert_eq!(said(&Why::Each(vec![Why::Text("a".into()), Why::NoNodes])), "a; no nodes");
    assert_eq!(said(&Why::OnChain { names: vec!["h1".into(), "h2".into()], tx: "t".into(), block: 7 }), "h1 h2 t 7");
}

fn landed(receipt: zikaron_send::batch::Receipt, offer: Option<zikaron_send::batch::Offer>, voided: bool) -> zikaron_send::batch::Landed {
    use zikaron_anchor::send::Fees;
    let stuck = offer.map(|offer| zikaron_send::batch::Stuck {
        txs: vec!["0xaa".into()],
        chain: 31337,
        from: [1; 20],
        nonce: 4,
        to: [2; 20],
        input: vec![3],
        sent: Fees::fallback(),
        unpriced: (offer == zikaron_send::batch::Offer::PriceUnread).then_some(zikaron_send::batch::Unpriced::Unread),
        offer,
        left: Vec::new(),
    });
    zikaron_send::batch::Landed {
        tx: "0xaa".into(),
        chain: 31337,
        confirmed: false,
        receipt,
        sent: 1,
        dropped: 0,
        queue: Vec::new(),
        gas: None,
        calldata: String::new(),
        stuck,
        voided,
        waited: 0,
        kept: None,
    }
}

#[test]
fn a_batch_no_node_holds_is_signed_again_only_unused_and_only_on_send() {
    use zikaron_anchor::send::Fees;
    use zikaron_seal::machine::CliAnchor;
    use zikaron_send::batch::{Offer, Receipt};
    let fees = Fees { max_fee: 9, priority: 1, gas_limit: 50_000, from_chain: true };
    let unheld = || Some(Offer::Unheld { fees, base_now: 4 });
    let at = zikaron_send::batch::again_at;
    let got = at(&landed(Receipt::Unused, unheld(), false), CliAnchor::Send).expect("signed again");
    assert_eq!((got.0.nonce, got.1), (4, fees), "at its own nonce, at the fees offered");
    assert!(at(&landed(Receipt::Unused, unheld(), false), CliAnchor::Queue).is_none(), "queue only sends nothing");
    assert!(at(&landed(Receipt::Unused, Some(Offer::Spent), false), CliAnchor::Send).is_none(), "resent the most times");
    assert!(at(&landed(Receipt::Unused, Some(Offer::PriceUnread), false), CliAnchor::Send).is_none(), "price unread");
    assert!(at(&landed(Receipt::Void, None, true), CliAnchor::Send).is_none(), "void");
    assert!(at(&landed(Receipt::NotYet, Some(Offer::Resend { fees, base_now: 20 }), false), CliAnchor::Send).is_none(), "held by a node: a fee bump, the desktop's");
    assert!(at(&landed(Receipt::Included { status: 1, block: 3 }, None, false), CliAnchor::Send).is_none(), "included");
}

#[test]
fn entries_an_agreed_reading_holds_leave_unless_a_transaction_of_theirs_is_out() {
    use zikaron_seal::queue::{Queue, Queued, Step};
    let id = |n: u8| format!("0x{}", format!("{n:02x}").repeat(32));
    let q = Queue {
        items: vec![
            Queued { id: id(0xa1), at: 1, step: Step::Queued, chain: 31337 },
            Queued { id: id(0xb2), at: 2, step: Step::Refused { said: "ON_CHAIN".into() }, chain: 31337 },
            Queued { id: id(0xc3), at: 3, step: Step::Submitted { tx: "0xdd".into(), chain: 31337, nonce: Some(0) }, chain: 31337 },
            Queued { id: id(0xe4), at: 4, step: Step::Queued, chain: 31337 },
            Queued { id: id(0xa1), at: 5, step: Step::Queued, chain: 1 },
        ],
        chain: 31337,
        anchored: Vec::new(),
        blocks: Vec::new(),
        passed: Vec::new(),
    };
    let bare = |n: u8| format!("{n:02x}").repeat(32);
    let reading = vec![bare(0xa1), bare(0xb2), bare(0xc3)];
    assert_eq!(zikaron_cli::sealedsend::leaving(&q, &reading, 31337), vec![(id(0xa1), 31337), (id(0xb2), 31337)], "the chain read, and no other chain's row");
    assert_eq!(zikaron_cli::sealedsend::leaving(&q, &[], 31337), Vec::<(String, u64)>::new(), "a reading that holds none takes none");
}
