# ZIKARON face grammar v1: review record

The record of how `zikaron.face/1` (docs/zikaron-face-v1.md) was written and
converged: what was asked, what the judge settled, what each round found,
what was folded and what was left, and the naming. The law's text holds what
the law says now; this file holds how it came to say it.

## The making of 2026-11-13

**Why the law.** A seat earns trust under many names: platform accounts,
role addresses, payout addresses. The ZIKARON seat asked (package
`zikaron.face-1-修法包`, 2026-11-13, files 01 to 04 with the product's facts
and source excerpts) for a way to write those names into the seat's own
ledger so that a stranger, holding nothing but chains, reads from the root
which accounts and addresses are the seat's now and since when: each such
name a *face*. The request asked for a new reader's law over `zikaron/2`,
with `zikaron/2`, `zikaron.kit/2` and the three contracts (registry,
revocation, succession book) untouched, and for the law's text, core, corpus
and instruments, a witness of the registry's behaviour with bytes after its
word, and a recomputed base lock. The base handed over with the request is
the base Court issued on 2026-11-11 (lock source `9580b3f9`), byte for byte.

**What the judge settled before the drafting (2026-11-13, the request's
seven).** A law of its own, `zikaron.face/1`, the one literal serving as law
name, entry type and the consent's domain; faces in their own entry type,
borrowing neither `history` nor `retraction`; who holds the office read from
the succession book alone, the ledger being one source of entry bytes among
others; face entries are no records (no work, no grant, no count among
records); a key face's signature given by the party holding that key, over
one line of `zikaron/2` §5.2's shape; the party holding the key has no act
of its own that withdraws; a face entry's bytes travel on chain with its
anchoring transaction.

**What the judge settled in the reading-back (2026-11-13).** Before a line
was written the mechanism was read back to the judge in plain words, and one
consequence of the request's draft was put: the draft read each chain's book
for the anchors of that chain ("per chain"), the books are one per chain and
know nothing of each other, the product changes a key in the books of the
seat's own chains, and the product's plan has readers read every chain; so
on a chain a seat never entered, the root key stays in office for ever, and
whoever holds a root key that was retired elsewhere binds and releases there
in the seat's name, which is the very thing a change of key is for. The
judge ruled, in four exchanges:

1. *One home.* A root's lineage is read on one chain at a time, its
   **home**; faces are not anchored on several chains at once. Bindings and
   releases count on the home alone, and the office is read from the home's
   book alone. This replaces "the book of the chain the anchor is on" in the
   request's third settled point; "the book alone" stands.
2. *The home is declared by an entry of fixed form*, a kind of face entry of
   its own with a root and a chain id and no prose, so that a desktop and a
   command line can each change the home in one act.
3. *Single-signed.* The home moves by one declaration anchored on the new
   chain by the key in office. A departure declared on the old chain was
   considered and dropped: it would freeze a seat whose old chain has
   stopped, refuses its transactions or prices them out, and a move changes
   no key, so the key in office proves itself.
4. *A key that has handed its seat on opens no home*, and, the books being
   separate contracts that stop a second hand-over only inside one chain, *a
   second hand-over by that key on another chain counts for nothing*. Before
   a first home the books are therefore read together, the earliest
   hand-over of each key deciding.

Also read back and left as they stand: a root is the signer of the genesis
of the seat's first ledger, a continued ledger keeping the root of the
ledger it continues (`zikaron/2` §7.3); no binding stands before a home;
changing the key in the home's book is the seat's own step; a consent has no
term, and after a release the seat may bind again with the same signature; a
contract's address gives no consent and is no key face; a reader reads every
chain or reads an older table; whoever holds the key in office holds the
seat.

**What the request left to Court, and what the drafting took.** The
request's fourth section listed twelve points of writing. Taken as follows,
each for the first round's readers to test:

1. One type, `zikaron.face/1`, and an `act` member (`home`, `bind`,
   `release`), `bind` divided by `kind` (`account`, `key`).
2. The reading of the book is written in this law in full; `zikaron.kit/2`
   is not cited, and this law depends on `zikaron/2` and on the accession
   form alone.
3. A key in two roots' lineages: no sentence. A face entry and a `home`
   entry each name their root in the body, so an entry serves one root; the
   book is read by `holder` and `to`, and topic 3 of the accession form is
   read by no rule.
4. `handle` compares bytewise, with no normalization.
5. An unknown `kind` or `act` is a body without a shape (`SHAPE`).
6. Bodies are closed: `zikaron/2` §6.10 reaches no body of §2.2.
7. `note_md` stays, on `account`, `key` and `release`; a prose string,
   unbounded by the law. A `home` entry carries none.
8. A release whose moment precedes its binding's: the span begins and ends
   at the binding's moment.
9. Inside one chain the office is read by block: an accession moves nothing
   for the anchors of its block.
10. Several chains: superseded by the judge's ruling above (one home; the
    books read together before the first).
11. Tokens and member names: §7.6 and §7.8.
12. A corpus, a core and a harness of their own (`face-corpus/`, `face-py/`,
    `HARNESS-FACE.md`); `corpus2/` and `kit-corpus2/` untouched.

**Where the drafting went beyond the request's draft.** Listed for the judge
with the round's report:

- *`handle` is ASCII.* The request's draft gave `handle` as 1 to 128 bytes
  of UTF-8. A face entry is a `zikaron/2` entry, and `zikaron/2` §3.3 holds
  every string value outside a prose subtree within U+0020 through U+007E; a
  `handle` above U+007E makes the bytes no entry at all (`E_VALUE_CHARSET`).
  The law says so in §1 and adds only the two ends (neither a space).
  Carrying other scalar values would need a member whose name ends in `_md`,
  a prose subtree, and with it the question of normalization the request's
  fourth point leaves to writers; the drafting kept the plain member.
- *One walk in one order.* The judge's four rulings are written as a single
  walk (§7.2) over steps ordered by time, then chain id, then block: before
  a home an accession on any declared book moves the office when its holder
  is the office; after, the home's book alone; a `home` entry opens when the
  office anchors it on the chain it names. "The earliest hand-over decides"
  and "a second hand-over counts for nothing" both follow from the one rule
  that a hand-over moves the office only when the office makes it. Two
  chains at one second go by chain id.
- *The home takes effect from the next step*, as an accession does for
  anchors: an opening and a binding in one block leave the binding unseated,
  and an accession in the opening's block is read under the home the block
  found.
- *The author is a key of the line so far.* An entry signed by a key that
  has held the seat is seated when the key now in office anchors it; one
  signed by a key yet to hold it is not.
- *`AUTHOR` among the reasons, and `UNSEATED`* for the request's "no counted
  anchor", which here also covers a record off the home and a `home` entry
  anchored on a chain it does not name.
- *The accession record carries no `root`*: the walk reads `holder` and
  `to`. The record carries `tx` and `blockTimestamp`, which the request
  asked for and left to Court to place.
- *Input validity is closed*, and an input that keeps two clocks on one
  chain is no input (`NO_TABLE`).
- *`unread` counts hashes the office anchored*, on any chain, since an
  unread hash may be a `home` entry.

**Not taken.** A reading in which every chain's book is merged for every
anchor, with no home ("the earliest hand-over of each key decides
everywhere"), was put to the judge beside the home and would also have
closed the opening; the judge chose the home, with the merged reading kept
for the time before the first one.

### Round 0: the checkpoint

Written in one pass on branch `zikaron-face-1-2026-11-13` from Court
`9580b3f9`: the law's text (ten sections), the core `face-py/` (three files
over `impl2-py/`), `HARNESS-FACE.md` (four commands: `entry`, `declare`,
`carried`, `table`), the generator `face-corpus/gen.py` with its own
predictor, the comparator `compare-face.py`, `criterion-digest-face.sh`, and
`bcov.py face`. §10.5 names no digest.

The corpus of a seed holds 656 cases: 293 `entry` (the four shapes, every
member missing, added and in another form, the bounds of labels and handles,
thirty-odd consents that do and do not hold, entries of other types and
bytes that are no entries, 72 random bodies), 27 `declare`, 49 `carried`,
256 `table` (some seventy named scenarios of the walk, a hundred-odd inputs
on either side of §6's bounds and of the transport's reader, 90 random
worlds) and 31 misuse cases. The predictor in `gen.py` is the author's own
reading written a second time; the criterion agrees with it on every case at
seeds 1 to 6, zero divergences and zero disagreements, `unwitnessed` empty
at each, and `gen.py --seed 1` rebuilds the committed tree byte for byte.

| seed | manifest sha256 | bytes read |
|---|---|---|
| 1 | `0x9b8a30c0f7fd6bc63acea3ea8bb7ba4b48d0ec99f18fd65251cc7c9d8a26cfe0` | `0x8cb6910655f88aca0ad781d4852be99473739492ffb78526d86a484370a9f214` |
| 2 | `0x438ccb1cd89f65282e2fb3a10ac50cb9e7ff81fa634cc042421cf7261409bfba` | `0xddcc141f5e2a143a9496bc8a70733c1ea25f8d9f2140e0ef86bd335a0240b05c` |
| 3 | `0x4118161f3279884a73ca1032d7d29c9fdf685a9dae94b70f242f54e641929a66` | `0xfb4b413e576b927c8d4d5040dbf0d05249df82844c4ec992258cbedd5f60d641` |
| 4 | `0x22e597542da4fab991d0c7af715e39880352131125dd13aef2c399cd07cdb632` | `0x746e7d1bf3cd2a56dce3a6c07d3a12f2f9a7ad4cb4fa50fb39ee77304f87daf7` |
| 5 | `0xe397910340750ce6c66c4b39414edb540239aa764ea21316fd68582dc7d37fd8` | `0x07c59c325dae2903e020de190cf2b0458b86e6757952df141f65731a2e12dd8c` |
| 6 | `0x50ce2e6aa1c92bf8c632cc4fd6024c1b709b347ead18aab81346189cf88c5be5` | `0x32e30df982f5c9482c1a027444e97406ec9f5c079844a94086e708f9c611faba` |

`bcov.py face` at seed 1: 656 cases, 171 branch arms, one not both-sided,
the `if __name__ == "__main__"` of `zkf.py`, which an in-process run never
enters. `criterion-digest-face.sh` computes
`0x01c1fb248d119839a702ddace737b56da6dd5b4fa6dda003cd129dc552ee9c8b` over
the three files as they stand and the `zikaron/2` core digest `0x2a65…567a`;
it is a candidate and no naming. `criterion-digest2.sh` and
`criterion-digest-kit2.sh` print what they printed, and no file of
`impl2-py/`, `impl-py/`, `kit2-py/`, `corpus2/` or `kit-corpus2/` moved.

The registry's suite gained `ZikaronRegistryCarried.t.sol`, four tests on
the pinned build: `anchor` with a face entry's bytes after its word succeeds
and emits the one log it emits without them; the same under fuzzing for any
caller, hash and tail; a word riding in the tail is not anchored; 35 bytes
of calldata revert. `forge test`: 75 passed (71 before), the two codeHashes
as `CODEHASH.md` has them, no contract byte moved. The family literal table
gained the entry type and the consent's domain (one literal in two roles)
and the marker's prefix.

### Round 1: seven seats, one fold

Seven seats over `f5a16068`: four readers (A the law's text, D the corpus
and the comparator, E the tooling, the record and the family, F the law
against the chains) and three implementers (Rust, Node, Python), each
implementer building on its own `zikaron/2` core, the program that closed
the revision of 2026-10-30 and was measured again on 2026-11-11. Seat D's
session stopped on an API error and was resumed from the files it had left;
nothing it had measured was redone.

**The three implementations.** Each was written from the text, the harness
and the corpus files alone, and each read zero divergences from the
criterion at seed 1 at its first measurement, before reading the criterion's
answers; seeds 2 to 6 read zero for all three, and no program changed for
the criterion. Probes outside the corpus against the criterion as a black
box, byte for byte: Rust 45,039, Node 41,000, Python 44,000, with no
disagreement, panic or hang.

**The readers.** E reproduced every number of round 0 but one: `bcov.py
face` reports one arm not both-sided or two, by the string-hash order a set
of releases is iterated in. D listed 77 closed boundaries and found 22 with
a side no case of some seed witnessed, and 16 of 66 core mutants that every
seed let through, one of them equivalent, with two more that some seeds let
through; it planted three defects the comparator let pass (a `declare` case
whose arguments ran `entry`, a misuse case that predicted an answer, a stray
file in the corpus tree). A tested 38 reading-mutants, of which the corpus
caught 31; the seven others each wanted one case. F built four forge tests
over the reference book (every `Acceded` log matches the accession form byte
for byte, `Declared` never does, and after every transaction of random
scripts the walk's office equals the book's `current(R)`), ran a model of
the book through 1,148 cuts with the same result, and probed the reading as
each kind of outside party. No finding of any seat is a difference between
the criterion and the text: each lies in the text, the corpus or the
instruments, and the criterion did what the text says on every input any
seat built.

**Folded, load-bearing, beyond the request.** Two rules, each changing
answers:

- *Who adds to the misread list* (F). Under round 0 every face entry of the
  pile that did not stand was listed, and §5.2 had a reader collect the
  bytes of every transaction to a declared registry: anyone, for the gas,
  could add rows to every root's table, entries claiming that root under
  handles of their choosing, and two readers that collected differently read
  different tables of the same chains. §7.6 now lists a face entry only
  where a key of the line anchored it, whatever the verdict, and §5.2 has
  the reader collect from the transactions of the anchor records it holds; a
  party holding no key of the line adds nothing. The request's attacker
  table had such entries "not stand", and they still do not; they are now
  also absent. What a thief of a retired key of the line anchors is listed,
  as the request's table has it (`UNSEATED`).
- *The unproven count* (F). An anchor record whose verdict is `UNPROVEN`
  counted for nothing and left no trace: a reader whose node could not
  answer a state read a root with no home, or a stolen account still bound
  because its release was the unproven record. §7.7 now counts the office's
  `UNPROVEN` records and §7.8 carries the count as `unproven`, beside
  `unread`.

The old criterion against the folded one, over the folded corpus at seed 1:
532 cases answer alike; the 185 tables read differ by the new member, and 81
of them also list fewer misread entries; no other member moved. By §5.1 item
7 of the standard the next round is a round before a closing round.

**Folded, load-bearing, the corpus.** Seat D's 52 cases (`build_boundaries`:
the label alphabet at its edges, every form and key of §6.1 and §6.2 on its
failing side alone, the upper bounds of the two ranges each as the input's
only fault, txIndex before logIndex in a block where the two disagree, a
hand-over to the office itself, home entries by a stranger and by an earlier
key, chain before block in a moment, the entry ties of homes and spans, a
release of a release, unread before a home, an entry opened twice, a tx tie
that held only by hash order), and the fold's own 9 (`build_round1`): a
release in its binding's block with the lesser tx (A1), bytes in the pile
that `accept` rejects (A2), the later of two bindings released (A5), spans
of one block ordered by tx before entry (A7), three of the unproven count,
and two of the misread list's scope. A3, A4 and A6, Python's self hand-over
and the accession below `fromBlock` that Rust and Node named are among D's.
`reasons_first_applies` was rebuilt so that each of its five entries is
anchored by the root and listed under the new scope. Seed 1 holds 717 cases:
309 `entry`, 28 `declare`, 49 `carried`, 297 `table`, 34 misuse.

**Folded, the instruments.** The boundary list of §10.1 now exists as data
(D): the manifest publishes 83 boundaries, each with the cases of each side
by name; the comparator carries the test of each side, requires the
manifest's list to equal its own, and fails any named case whose answer
misses its side. Of the 83 entries, 69 are closed boundaries, each
both-sided at every seed, and 14 are rules of order, ties and precedence,
each with its named witnesses. The comparator checks every case's command
against its first argument, requires exit 2 of a misuse case and of no
other, and accounts for every file of the corpus tree (D's three planted
defects each now fail it). `bcov.py` runs under one string-hash order (E-1),
and the criterion iterates its releases in order. The digest script's
comment says what its guard reads.

**Folded, the text.** §2.2 says how its closure stands beside `zikaron/2`
§6.9 and §6.10 (E-8). §2.4 names no project's roles (F4). §3.4 says what a
key signs beside its transactions and that a wallet's key consents through a
tool (A B3, E-2). §5.3 reads a registry by its form: a registry that emits
the form for further words of its calldata anchors them, and a producer
carries entries only to a registry whose code it has read (F5). §6.1: a
book's range is read apart from the registries' and may begin below
`fromBlock` (Rust, Node, A3, D). §6.2: an element given twice is two records
(A8, E-4, Python F2). §6.3: `logIndex` counted over the block; the keys in
reach are a closure from `R` through holders; records beyond them move
nothing and add only bytes (F, Rust F2, A). §7.2: the anchors of a step are
read in no order among themselves (A); a hand-over to the office itself
appends it again (Python F1, Node F3); the gloss on later hand-overs holds
while the office stands elsewhere (Python F5). §7.3: seated for a face entry
with a shape (Python F6, D T4). §7.5: which release ends a span, the lesser
entry id first, the comparison with `tx` included, and a standing release
that ends no span appears nowhere (A B2, Python F4, D T2, Node F4). §7.6:
the list's scope, and the line item 3 reads against the line §7.3 reads
(Python doubt 4). §7.8: `basis` the same values in the same order (Node F5);
one element of `homes` per opening record (A4). §9: the reader's chains say
that the walk sets chains side by side by their producers' block times and
that the basis is the reader's trust in them (F1, Python F3); a home on a
chain the basis gives no book; the stolen key's anchors in the hand-over's
own block, and a move read before the hand-over (A9, F6); unread and
unproven as hints; the paragraph on what this law leaves as it found
prefixes its `zikaron/2` sections and says where `MISMATCHED` applies (A B1,
E-3, E-5). §10.1 says how the boundary list is published and checked (D F5).
The harness contract says how a path's bytes are handed on, that `consent`
reads the body's own members, how `table` misuse stands beside the argument
count, and lists `unproven` (E, Rust, Node).

**For the judge: the clocks** (F1, Python F3, A9). The walk orders two
chains by the block times their producers write. A holder of a key once in
office who produces a chain's blocks, or who acts on a chain whose clock
lags, can date a hand-over or an opening there inside that key's tenure and
take the seat in every table whose basis holds that chain (F's witness: four
blocks on a chain of its own, dated inside the retired root's tenure, give
it the home, the office and both rows). On honest clocks the same acts gain
nothing, and the remaining race is the one a stolen key in office already
runs. The text now makes the basis the reader's statement of the chains
whose times it accepts. A move by two signatures, one on the home it leaves,
would order moves on one chain's blocks, at the cost the judge weighed (a
seat frozen when the old home stops); it would not remove the comparison
before a first home, where the judge's fourth ruling reads every book
together by time.

**Not taken.** F's mechanism for the clocks (a move counts only when its
sender is the office of the home it leaves as that chain stands at
`toBlock`): a wall, passed by a thief who dates its move before the seat's
own, and it reads one book's final state into the past. Python's inverted
form (a key that has handed over in any book opens nothing): it voids the
earlier opening of every key that later hands over on another chain, which a
seat rotating its key in every book of its basis does. A's reading that an
author must be the office at its step: an entry signed before a change of
key and anchored after it is the seat's own, and a thief of a retired key
needs the office to anchor what it signed; "a key of the line" stays, as the
request has it. F's doubt on accession logs of legacy or unsigned
transactions: they replay the seat's own hand-overs, which another book
records as the same hand-over or ignores, and give an outside party nothing.
E-7(a): the forge test's bytes are shaped as a face entry, and its fuzz test
covers every tail. D T1: the bound "longer than 36 bytes" is implied by
`accept` and stays as a reader's aid. F's own inverted form for unproven
records (a conforming input carries none) gave way to the count, which a
table can show. E-6: the three places the family table names are the desk's
to create, and the package README says so. E-7(b), the conformance README's
sentence on the naming, waited for round 2. The cost of a sixteen-mebibyte
input (about 20 ms a key entry, E and Python) lies outside the corpus and
the comparator's timeout, and the harness lets a candidate refuse above the
bound.

**Measured after the fold.** The criterion agrees with the generator's
predictions on every case at seeds 1 to 6, zero divergences and zero
disagreements, every boundary both-sided and `unwitnessed` empty at each;
`gen.py --seed 1` rebuilds the committed tree byte for byte. `bcov.py face`
at seed 1: 717 cases, 173 arms, one not both-sided (the `if __name__ ==
"__main__"` of `zkf.py`). The candidate digest is
`0x105ff340fbe48a5af121c1485b7bde7561adc6a2cccf5256663b8c8f6c5a2f36`, no
naming. `forge test`: 75 passed, the two codeHashes as `CODEHASH.md` has
them.

| seed | manifest sha256 | bytes read |
|---|---|---|
| 1 | `0xa7c51947ccf9faa6c50b3c7a693ad7926da16491e8195c13f687264e88c7f805` | `0xeaded6bf73769fc0085236ae3faae35e7d51dd5e9aaee24281adb572a9233a6d` |
| 2 | `0x21a0b7d28990970b293d4018ce94fe68754b32bbb3bf95b62905477096679792` | `0x948610e584e81f8995ba54bbe975856900fa9b8557da10de4a075eb2027c1076` |
| 3 | `0x83b69d3360a39fd4abb0f6c3d1f49b46fc4ff16fd0f9463618a3c22d37bf0d55` | `0x7a0205cf9fc8d9dcb911a42a11e4d2d5831121b9ebcc373de9b6e2b521559a64` |
| 4 | `0xa5557fd7931c56efa0240c4d017f6789ee90f3cfb8b3c6abad2696a8b74c606a` | `0x349bc8a73d185f66c6eea88f1fc6b4e1932b2414b5892ed08a8c1eda742420e7` |
| 5 | `0x4f13df347d34c71e9cfb9e7aaa2517d5dedc79cbb8e88e000195c9947c50be21` | `0x25b3a0ee43b4d30a7ef303ee38b3997950318635b831b2214cf1b00abf9722fe` |
| 6 | `0xda8e7e0dcf6af526d3a6daf7acc3ade9a92b4ea474c269a10caf5fc9e42e9507` | `0x9c2ffe7d40bc80c358f59242d346a1812fa032dfcaeb4539e0df5484a6669d88` |

### Round 2: seven seats, one fold

Seven seats over `6d60786c`: four readers new to the law (A2 the text, D2
the corpus and the comparator, E2 the tooling, the record and the family, F2
the law against the chains) and the three implementers of round 1, each
continuing on its own program.

**The three implementations.** Each ran its round-1 program unchanged on the
folded corpus at seed 1 first: Rust, Node and Python each read 185
divergences, all tables, all from round 1's two rules (104 by the `unproven`
member alone, 81 also by the misread scope), the count the record gave for
the old criterion against the new. Each brought its program to the folded
text, quoting §7.6, §7.7 and §7.8 for every change that moved an answer
(Node and Python also seated only entries with a shape, §7.3, which moves
none), and kept the round-1 program beside it; seeds 1 to 6 then read zero
for all three. Probes, byte for byte against the criterion: Rust 46,046,
Node 40,000, Python 44,000, with no disagreement, panic or hang.

**The readers.** E2 reproduced every number of round 1 but two, the mutant
count of D and the boundary list's 14 one-sided rules; it found one
statement wider than the findings, on where they lie, and the dispositions
short by four items; all are corrected above. D2 tested 58 core mutants: 9
survived every seed and 4 some; its patch kills all but one, which no input
can tell (the bound of §5.1, implied by `accept`). A2 tested 31
reading-mutants, of which the corpus let 5 through, each caught by one of
its probes. F2 built four forge tests of its own (79 in all), a reader model
that builds face inputs from receipts as §6.3 says, and a probe of 1,800
partial readers whose node refuses a random fifth of the states: all 1,228
partial tables that differ from the full reader's show `unproven` above
zero, and none differs while showing zero. No finding is a difference
between the criterion and the text, and no finding changes an answer of the
criterion.

**Folded, load-bearing, the chain-facing rules** (F2; neither changes an
answer):

- *One record per anchor.* `anchorMany([h, h])` emits two logs of one hash
  in one transaction. A reader writing one record per log builds an input
  §6.2 refuses, so any key in reach, the holder of a retired key among them,
  could make the seat's table `NO_TABLE` for that reader at the cost of one
  transaction. §6.3 now has the conforming input hold one record per
  `(chainId, blockNumber, tx, hash)`, as `zikaron/2` §9.4 holds an anchor
  set.
- *One moment.* Each chain of the basis carries its own `toBlock`. A reader
  that narrows one chain, or reads chains to finality depths that differ,
  reads a later chain's acts against an office and a home the earlier chain
  has since moved: F2's witness gives a retired root, on honest clocks, the
  home, the office, a release of the seat's payout face and a binding of its
  own, in the table of a reader whose home chain stops at an earlier block.
  §6.3 now reads a face input to one moment, every chain ended at its last
  block at or before one time every chain has passed, and §9 says what a
  reader that reads chains to different times sees. This goes beyond the
  request and needs no ruling on the clocks.

**Folded, load-bearing, the corpus.** D2's 20 cases (`build_round2`: numeric
orders across a digit count in chain ids, blocks and seconds; an anchor's
upper bound as its input's only fault; a clock that runs back from an
accession to a later anchor; JSON booleans in int members; an opening by an
entry of another type with a home body, and by a home body beyond its row; a
release under another root; ROOT before SIGNATURE and SUBJECT on one input;
ties whose order no seed's keys decide; the unproven count off the home and
of a hash nobody holds; key rows by address before role) and the readers' 13
(`build_round2_readers`: the two counts inside a hand-over block, for the
outgoing office and for the incoming key; one hash in two UNPROVEN records
counting two; an UNPROVEN record of another type; an UNPROVEN move of the
home; a release off the home; a release on a home the seat moved to; a
release before and one after a binding; a moment decided by block before tx
in one second; three member forms each the only fault). Seed 1 holds 750
cases: 309 `entry`, 28 `declare`, 49 `carried`, 330 `table`, 34 misuse.

**Folded, the instruments.** Every side of §7 in the boundary list now tests
what its boundary names (D2): none passes on "a table" alone, the valid
sides of §6 and of the transport reader being a table by what they name, and
two ties are tested against the case's input. The list holds 104 entries, 79
closed boundaries each both-sided at every seed and 25 rules of order, ties
and precedence each with its named witnesses, and the manifest's new
`unobservable` member names the three whose two sides answer alike on every
input, with the reason (§5.1's bound, the consent at s = (n − 1)/2, the
transport's depth 128). A slug names one case per command. The comparator
walks the corpus tree entry by entry, refuses links and special files, and
admits as files only the paths of file cases and the generator's three files
(E2's three plants and D2's each now fail it). `bcov.py` restarts itself
with its own interpreter flags. The generator's header and the corpus README
say which cases were chosen by seats that had read `face-py/`; the
conformance README describes the digest script and the comparator as they
stand.

**Folded, the text.** The rule classes are marked on predicates and
chain-facing rules, and [X] points at §5.2 (A2). §6.2: the order of three
arrays reaches no output (A2). §6.3: accession records bring no bytes (A2
T4); narrowing lowers a `toBlock` or leaves a chain out (A2 T5); topic 3
enters no member (F2 T3). §7.2: an opening is a record item 1 found in
office (A2). §7.5: the least release ends the span wherever its moment falls
(A2 L4). §7.6: whatever the record's verdict and wherever it stands in the
walk (Python, Node). §7.7: unread at each record's step; unproven on every
chain, each record once; and a table with unproven above zero may differ in
any member (A2, Rust, Node, E2, Python). §9: the reader's chains name clocks
that disagree in either direction and chains read to different times (F2 T1,
L2); a reader that leaves out the new home reads the seat as if the home had
stayed; before a home, the bar on a key that handed over holds while the
office stands elsewhere (A2 T1); a seat whose outgoing key may be stolen
hands over first in its home's book (A2 T2, whose witness lost the seat in
the order round 1's sentence listed the books); its anchor records in the
hand-over block are in office; the reader's books (F2 T2); what the office
anchors, a hash handed to it included (A2). §10.1: boundaries whose sides
answer alike are marked with the reason (D2).

**Not taken.** F2's [R] alternative for one moment (each chain's `toBlock`
time carried in the input and the walk stopped at the least): it changes the
form of §6.1, and the [X] rule closes the class. F2's producer line for book
transactions without a chain id: they replay the seat's own acts and gain an
outside party nothing. The corpus's role spellings in random worlds, in two
named cases and in three declare cases name family roles: general rule 5
binds law texts, and changing them moves digests for nothing. F2's doubt
that a retired key can keep handing on in books the seat never entered, and
so grow a reader's scan: recorded, and named in §9 in round 3. F2's doubt on
a VOID release by a holder of an authorization the office key once signed:
`zikaron/2` §5.7 forbids the authorization and the table lists the release,
so recorded. A2's doubt on the one-sided entries of the list: answered by
naming them as rules of order, ties and precedence in round 3. E2-10's
keying of answers by category and slug: the slug check of one case per
command serves, recorded.

**Measured after the fold.** The criterion, unchanged by this fold (its
candidate digest stays
`0x105ff340fbe48a5af121c1485b7bde7561adc6a2cccf5256663b8c8f6c5a2f36`),
agrees with the generator's predictions on every case at seeds 1 to 6, zero
divergences and zero disagreements, every boundary passing its sides and
`unwitnessed` empty at each; `gen.py --seed 1` rebuilds the committed tree
byte for byte. `bcov.py face` at seed 1: 750 cases, 173 arms, one not
both-sided (the `if __name__ == "__main__"` of `zkf.py`). By §5.1 item 7 of
the standard the next round is the closing round: the fold moved no answer,
so the three programs of this round are measured unchanged on the folded
corpus.

| seed | manifest sha256 | bytes read |
|---|---|---|
| 1 | `0x6857bb2b0003c8f53fd90d442b6208f5a21946de6f09130d218c9bbc26d9443b` | `0xcca447865f8b36252752630ad80a46ae5ac89dcba9dd98170fe0f52aa2061933` |
| 2 | `0xb9ec3302220a87a47c711dffc5c3dce65bc515f11713b367d3bbf59f4e866ed4` | `0x4a8e30c5f5b7758fa4cf87d7ddbab5dc99de82d94020ae596ddc8736de7c1601` |
| 3 | `0x98f5427494fd10e0e0f9975a0ce96be2b8c112a04a09dd7d8470f31be9d90778` | `0x9634a37ca484e0f8063018379e0fabd81f2e720344d5a518a2d553d2e14f119e` |
| 4 | `0xfa399f649db93a4b6a39120f1a1d5c8a7c0b4c2710dacd945f68decccd9db055` | `0x8933a17c33dcbfd6876dfe30a3cab92a1e613dbf5daa02293bfc2f4a3268c7bb` |
| 5 | `0x01c6cc10ebfa62d842f903e42fe5ec4a688326defa25a4d74c15e0cbebae819a` | `0x8772518e20899afbaded41317a56b5b50868dca52acdfaa9ee7f0ef8e33e8179` |
| 6 | `0x020031b94b814d2f1c338af00a703500dfd4fd8ff44beddd2c0c548c4df8c3ad` | `0x996e8d5db978821e0bb6e862fce1cdfb164f4b52126e39ad2fd9aebbfff1720e` |

### Round 3: the closing round

Seven seats over `6c7ccaad`: four readers new to the law (A3 the text, D3
the corpus and the comparator, E3 the tooling, the record and the family, F3
the law against the chains) and the three implementers, each continuing on
its program of round 2.

**The closing measurement.** Each implementer ran its round-2 program
unchanged on the corpus of `6c7ccaad` at seeds 1 to 6 first: Rust, Node and
Python each read zero divergences and zero disagreements at every seed, the
bytes read equal to the six digests of round 2. No program changed. Probes,
byte for byte against the criterion with those programs: Rust 106,043, Node
40,000, Python 44,000, with no disagreement, panic or hang. Rust's probes
over its round-2 program, measured unchanged in this round, meet §10.1's
hundred thousand fuzz samples for one independent implementation.

**The readers.** No finding is a difference between the criterion and the
text, and no finding asks to change an answer of the criterion. A3 tested 51
reading-mutants, of which the corpus let five through and one more held by a
single random world; F3 built five forge tests of its own (80 in all), a
reader model that builds every face input from receipts, transactions,
headers and states as §6.3 now says, and 800 random worlds read seven ways
each, in which on honest clocks a holder of a retired key gains nothing in
any reading of one moment (2,000 tables), and every refused-states table
that differs from the full one shows `unproven` above zero (476 of 476); D3
tested 94 core mutants, of which 10 survived every seed and 2 some, and
planted three corpora the comparator passed; E3 reproduced every number of
round 2 and of the corrections it made, re-ran the 124 mutants of D2, A2 and
E2 over the committed seeds (only the equivalent one survives), and found
the record's statements and dispositions short in seven places (corrected
above).

**Folded, the corpus.** D3's patch (`build_round3`, 203 cases a seed: every
printable character as a one-character `platform` and as a one-character
`role`, each on its side of the label alphabet; record keys against members
outside them, an anchor differing in `sender` alone and an accession
differing in `tx` or `holder` alone; a release whose subject is a seated
entry that does not read; an opening by a key that joins the line later; hex
members in uppercase each as the only fault; a chain id as a string on a
chain no record names; private keys of 63 and 65 digits; a lone surrogate in
a member name), its corrections to the boundary list (the transport reader
split into five, one of them marked unobservable; a misfiled side moved),
and A3's 7 (`build_round3_readers`: the author of an entry, and of a `home`
entry before a home, against the line the hand-over block found; a move back
to an earlier home, by a new entry and by the first one anchored again; two
releases of one binding and two openings, each in one block by two
transactions, ordered by tx; an author in reach who never joined the line).
Seed 1 holds 960 cases: 499 `entry`, 28 `declare`, 49 `carried`, 347
`table`, 37 misuse. The boundary list holds 118 entries, 89 closed
boundaries each both-sided at every seed, three of them marked unobservable,
and 29 rules of order, ties and precedence each with its named witnesses;
one case that answers alike under both readings of the side it stood on
(`opening_and_accession_first_block`) left that side.

**Folded, the instruments.** The comparator requires every file case's path
to lie in its category's directory with its name form (D3's misplaced case
now fails it). `bcov.py` no longer pins the string-hash order: the
criterion's sorted releases made the pin unneeded, and under `-I` it pinned
nothing (E3). The family table's marker entry names the place the desk
creates (E3). The generator's header, the corpus README and the conformance
README say which cases review seats chose, how the boundary list carries its
rules of order, and what the comparator admits (E3).

**Folded, the text** (all wording; no answer moves). §1: a handle's length
is that of its decoded bytes (A3 L3). §5.3 carries the mark [X] (A3). §6.3:
one record per anchor key holds for every anchor record whoever sent it, so
no party can make a reader's table `NO_TABLE` with `anchorMany([h, h])` (A3
L2); an anchor is one under `zikaron/2` §9.1; topic 3 beyond its form; a
chain with no block at or before the moment is left out (F3-5). §7.6: at
whatever step the record sits (A3). §7.7: unread counts what the reader
holds no entry for, rejected bytes included; an unproven count above zero
may change any member the walk decides (A3). §9: the reader's chains say
that one moment is no later than the least advanced chain and what a reader
that names no book sees (A3 D1, F3-3, F3-4); the stolen key's counted
records (A3); before a home, a hand-over to another key on another chain
moves nothing (A3 T1); a move back to an earlier home, which the old text's
sentence on moving the home had denied (A3 L1); book transactions signed for
no chain, whose replay can give a withdrawn key the office, and the producer
sentence that closes it (F3-1); what a retired key costs a reader (F3-2);
what the office anchors, on the home and on the chain a `home` entry names
(A3 T2); unread nonzero for a reader holding only the chain (A3 T3). §10.1:
rules of order name their witnesses, unobservable means on every input that
can be built, and the comparator checks the ties no answer shows by their
input (D3, E3).

**Not taken.** D3's input tests for every named side (F8), so that a case
moved off its edge in the generator fails the comparator: information level,
left to a later round of the instruments. D3's survivor T69 (a key face
whose address is the root never stands): a rule the text never names, and no
case closes the class of arbitrary special cases; `key_face_is_root`
witnesses the root's own key face under `entry`. A3's optional wording on
narrowing with `book: null`: §9 now says what naming no book reads.

**Measured after the fold.** The criterion, unchanged (candidate digest
`0x105ff340fbe48a5af121c1485b7bde7561adc6a2cccf5256663b8c8f6c5a2f36`),
agrees with the generator's predictions on every case at seeds 1 to 6, every
boundary passing its sides and `unwitnessed` empty; `gen.py --seed 1`
rebuilds the committed tree byte for byte; `bcov.py face` at seed 1 counts
960 cases, 173 arms, one not both-sided (the `if __name__ == "__main__"` of
`zkf.py`). The fold changes text and adds cases alone, so by §5.1 item 7 of
the standard the three programs of this round were run unchanged on the
folded corpus at seeds 1 to 6: Rust, Node and Python each read zero
divergences and zero disagreements at every seed, the bytes read equal to
the six digests below. The closing round closes.

| seed | manifest sha256 | bytes read |
|---|---|---|
| 1 | `0x5a34ba5fce5879785f249b26b2bbaa162c45a86d433abc0d1a8f34ecc518e0a7` | `0xcabdc43051dfa880c1accbaf8c393a454d963ec360800266a58e3d6ec7a5c4c4` |
| 2 | `0x185bf04db3422dabac331fbf14f86d740201b0118e9be15e312633f4eff00108` | `0x62ffb0497f291ebee7c9f3819df1c152656290d450d38700b7c8c3e55f041bf8` |
| 3 | `0x90c3a2f5c25b951b4ef70ae7cd1a7138dd203284157c9b6603b50f9da73a27d7` | `0x37698946e9652befc0d0829a7f9ffbf78364b425b90fcf144ad62aad2a13dafe` |
| 4 | `0x5f69cfec5892dbbec762eeb1e8b2465c06e404b1b1547dda2d47084037fbccc7` | `0x5071483470c2e9ae5205aa842fa66614396c6143017fc58ee93c8a6eb8d2963e` |
| 5 | `0xe6092a4178cbc82b323cbfd57ba4b77fb63c24b16c4b7a26155e3a83b6cc5ec7` | `0xefd16489b0a51db0e436712c3c4305e43395af0372127eaa707faccc4ee2a2ef` |
| 6 | `0x84d4a063ef53cc7fa14d9c575ba5e6cb9a605170b5d866f5714e156c0a7bb945` | `0x223138d5c85c779f32e592d39208fe6df736aa2099dfc85870c127cfad9d7c01` |

**The check seat.** One check seat read every sentence the closing fold
added or changed, against the rest of the text and the `zikaron/2` sections
it cites, building inputs for the criterion as a black box. Twelve sentences
have one reading. Five had two: one record per anchor key, which could be
read as asking for every sender's anchors; the conforming input, whose "the
form above" had lost its antecedent once the anchor's form was cited as
"under `zikaron/2` §9.1"; "any member the walk decides", which a narrow
reading of "the walk" confines to the home, the office and the line; "the
office that stood when the home opened", which a move back makes the first
or the last opening; and "produces no face input", which read absolutely
would deny the narrowing §6.3 allows. Three said more than the walk does: a
replayed declaration takes the office only where the home is nothing or that
book's chain; the misread list grows only with entries that do not stand;
unread is nonzero only where ordinary entries travel as hashes and their
anchors count. The rule classes now list §5.3 among the chain-facing rules.
Each was worded as the seat proposed, with the optional wordings on the move
back, on what the office anchors and on §10.1 taken too; no answer moved and
no case was needed. Read once more, the seat found three of the new wordings
wanting: what the office seats had grown to every face entry with a shape,
which §7.3 denies a `home` entry off the chain it names; "though it has
since retired" took the retirement as given; and §10.1 left the witnesses of
a rule of order unchecked by its letter. Each was worded as the seat
proposed. A third reading found the sentence on what the office seats open
to records of any verdict, which §7.2 does not seat; it now seats through
counted records alone.

### Naming (2026-11-13)

§10.5 names the release digest
`0x105ff340fbe48a5af121c1485b7bde7561adc6a2cccf5256663b8c8f6c5a2f36`,
computed by `criterion-digest-face.sh` over the three files of `face-py/`
(`zkf.py` sha256
`e0eac652ae5290a5cabf18f2faa1401846e4e523daa89755aed14e4ee03e99e4`,
`zkfentry.py`
`c3050efc9e70db99eeb932fac0ab23090b5f8cbdca2869ec2359206812df7b18`,
`zkftable.py`
`fac135bb952154ace4ee0b1c9cfb8f071abf9f8f9215bc4e43dbec724338b767`) and the
`zikaron/2` core digest
`0x2a6564e126c632c70b51c602b92bdee47ab2a86118b955d32e2dd5c0f1af567a` of
§10.4; it is the first digest this law names, and nothing is superseded. The
text so named has sha256
`7fd183e44a4fdccfd88679ed31e9284b5273850a2b12b052a97f16636bd079a8`.

The freeze rests on: the closing round, in which the three implementers'
round-2 programs read zero divergences at seeds 1 to 6 on the corpus of
round 2 and again on the corpus the closing fold grew; the criterion against
the generator's predictions at seeds 1 to 6, every boundary passing its
sides; probes outside the corpus against the criterion as a black box over
the four rounds (Rust 197,128, Node 121,000, Python 132,000), with no
disagreement, panic, hang or nondeterministic answer, the Rust program
measured unchanged in the closing round over 106,043 of them, which meets
§10.1's hundred thousand for one independent implementation; and the check
seat's three readings of the closing fold, the last of which found every
folded sentence of one reading. The text, the criterion and the corpus are
the ones measured: the naming moves no byte of `face-py/` or of the corpus.

## The revision of 2026-11-13, after the naming: the law brought to the family's

**Why.** The package's fifth item for the judge said that an entry signed by
any key of the line is seated once the key in office anchors it, as the
request's draft had it, at the cost that a hash another party hands the
office seats what a retired key signed. The judge asked whether the family's
laws do not already forbid a change of key while entries stand unanchored.
They do. `zikaron/2` §5.7 has a seat anchor every entry the ceding key
signed before it anchors the succession, and its §7.4 admits what a key
signed only where a record no later than that key's retirement bounds it,
whoever sends a later record. `zikaron.kit/2` §7.2 check 3 passes a grant
only where a standing record's `sender` is the grant's `author`, and fails
one that "the key in office anchored" and "a key not then in office signed".
The product's plan sends a face entry only when the book's key in office is
the desk's own. Round 1 had declined seat A's reading (the author must be
the office at its step) on the ground that an entry signed before a change
of key and anchored after it is the seat's own: that entry is one the parent
law obliges a seat never to leave, and the rule kept for it gave the holder
of a retired key what no sibling law gives, closed by a producer's sentence
alone. The judge ordered the change (2026-11-13), and then that the law hold
no fork from the family's existing laws in what they already settle, and no
contradiction or half-made joint with the other ZIKARON laws.

**The law read against the family.** Before the seats, the text was set
beside `zikaron/2`, `zikaron.kit/2`, the request's draft and the product's
facts, subject by subject. Three forks were found and are folded here; the
rest is listed with what it rests on.

1. *The author* (above). §7.2 item 3 and §7.3 now ask that the author of an
   opening and of a seated entry be the office as the step found it: a
   seated record is one the entry's own author sent while it held the
   office, which is `zikaron.kit/2` §7.2 check 3 in this law's subject.
   `AUTHOR` stays the reason of an entry no key of the line signed (that
   check's "a key that neither the ledger's successions nor the book the
   reader read ever seated"); an entry a key of the line signed and another
   key anchored is `UNSEATED`.
2. *The book was read by `holder` alone.* The request's draft (2.6, 2.7)
   asked for `zikaron.kit/2` §6.3's book record with its `root`, the records
   limited to those whose `root` is `R`, and for a reading synonymous with
   that law's §6.4. Round 0 dropped the member and chained the records by
   `holder` from `R` ("the accession record carries no `root`", listed then
   among the places the drafting went beyond the draft). That reading admits
   a hand-over made in a book that holds the handing key under another root,
   a book the seat never entered: `zikaron.kit/2` reads such a hand-over as
   another seat's (its §7.1 takes the root from the book's own statement,
   and its §9 has it that "a seat enters the book under its root key or not
   at all"), the product's plan gates its writing on the book's key in
   office, and the product's reader of the book reads by root. One book
   would have been read two ways by two ZIKARON laws, and the desk would
   have carried two readers of it. The accession record now carries `root`
   (§6.1; the third topic, §6.3), a hand-over moves the office only under
   `R` (§7.2 item 2), a conforming reader collects the logs whose third
   topic is `R` in place of a closure by `holder`, and the keys in reach are
   `R` and the `to` of those records. The judge's rulings on the home stand
   untouched: a hand-over still moves the office only when the office makes
   it, the books are read together before a first home, and the home's book
   alone after it. §9 gains "A book the seat has not entered" and says what
   a retired key costs a reader now that only the root key adds records
   under `R` in a book the seat never entered. No rule was added that a home
   open only on a chain whose book holds the office under `R`: opening
   elsewhere is a seat's own step, the product's gate refuses it, and §9
   says what follows.
3. *The anchor record was closed, and one given twice refused.* §6.3 already
   cited `zikaron/2` §9.4 for one record per `(chainId, blockNumber, tx,
   hash)`, while §6.2 refused what that section reads: an anchor record
   carrying a member beyond its seven, and one record given twice, which
   round 2 then had to keep from any party's reach by a rule for readers.
   §6.1 and §6.2 now read an anchor record as `zikaron/2` §9.2 and §9.4 hold
   it: members beyond the seven are read by nothing, and two elements that
   agree on the seven are one record. The face input, its chain objects and
   its accession records stay closed, as `zikaron.kit/2` §6.3 closes its
   notice input and its book records.

Set beside the family and found at one with it, or the law's own because its
subject is new: the clocks (`zikaron/2` §7.4 sets records of all chains side
by side by `blockTimestamp`, and its §11 has a retirement across chains as
good as the clocks of the chains a basis admits); the `unproven` count (that
law's `UNPROVEN` list, §8.6); the misread list confined to what keys of the
line anchored (that law's §8.1 admits lineage keys' records alone); `handle`
within printable ASCII (that law's §3.3); the office read by block, a
hand-over moving nothing for the anchors of its block (`zikaron.kit/2`
§6.4); an accession read as the acceding account's call whatever code it
carries, and no test of the chain a book transaction was signed for
(`zikaron.kit/2` §6.3 reads the accession form so, while `zikaron/2` §9.1
refuses a transaction signed for no chain as an anchor, which this law
inherits for its anchor records). The home, the books read together before
it, the holder's test and the order of chains inside one second are the
judge's rulings and have no counterpart to fork from.

**The fold.** Text: §5.2 (a face entry is sent from the key that signed it),
§6.1, §6.2, §6.3, §7.2 items 1 to 3, §7.3, §7.6, and in §9 the two rails,
moving the home, a book the seat has not entered, the reader's books, what a
retired key costs a reader, and what the office anchors (the producer's
sentence "a seat anchors the hashes of bytes it has read" leaves with the
opening it closed); §10.5 stands unnamed until the naming below. Core:
`zkftable.py` alone, the author's test, the ninth member and the root's
test, the anchor record's reading. Corpus: `build_author_is_office` (13
cases a seed), `build_book_root` (10) and `build_anchor_records` (5), the
random worlds drawing each record's root from a stream of their own so that
their other draws stand; `author_old_and_future` and `home_by_earlier_key`
change sides, the two cases of an anchor record with a member too many and
of one given twice become valid inputs, and the case whose accession record
carried `root` as its member too many now carries `emitter`. Seed 1 holds
988 cases: 499 `entry`, 28 `declare`, 49 `carried`, 375 `table`, 37 misuse.
The boundary list holds 124 entries, 90 closed boundaries each both-sided
and 34 rules of order, ties and precedence: the author's four rows renamed
and re-sided, the accession's members nine, the anchor's rows re-sided, and
six new rows.

**Measured before the seats.** The core before the revision and the core
after it, over the corpus as it stood at the naming (2,082 `table` cases at
seeds 1 to 6, the member `root: R` added to each accession record, which is
what a reader that reads the book under `R` hands over): 1,990 answers
byte-equal; 80 differ, each on an input where the old walk admits a record
in office of an entry another key of the line signed; 12, two a seed, are
the anchor record with a member beyond its seven and the one given twice;
none is unexplained. The criterion agrees with the generator's predictions
on every case at seeds 1 to 6, every boundary passing its sides,
`unwitnessed` empty, and `gen.py --seed 1` rebuilds the committed tree byte
for byte.

**The first measurement.** The three implementers' round-3 programs,
unchanged, on the corpus of `74a71051` at seeds 1 to 6: Rust, Node and
Python each diverge on 154, 163, 164, 160, 159 and 159 cases, at every seed
exactly the cases on which the core before and the core after differ, each
program answering as the core before did.

### The revision round: eight seats, one fold

Eight seats over `74a71051`: five readers (K the check seat of the fold; G
the law against `zikaron/2`; H the law against `zikaron.kit/2` and the
succession book; J the joints with the product and the package; F the law
against the chains) and the three implementers, each continuing on its
round-3 program and bringing it to the text from the text alone.

**The implementers.** Rust, Node and Python each changed one file from the
diff of the law and then read zero divergences and zero disagreements at
seeds 1 to 6, every changed sentence of one reading. Probes against the
criterion as a black box: Rust 104,043, Node 30,000, Python 32,000, with no
disagreement, panic or hang.

**What the readers found.** No seat found a difference between the criterion
and the text. K tested 40 reading-mutants of the three changed rules, of
which the corpus let two through (a record given twice that differs in
`blockTimestamp` alone; members beyond the seven of every kind of value),
both now witnessed. The load-bearing findings were joints, and four seats
came to one of them by four roads:

- *On its home this law and `zikaron.kit/2` named two keys in office* (H,
  the Rust seat, the Python seat, and F from the chains). The fold of
  `74a71051` read the book under `R` and left a home free to open on a chain
  whose book holds another key under `R`; there the seat's hand-overs carry
  another root and move nothing, the key that opened stays in office for as
  long as the home stands, and the book's own `current`, `zikaron.kit/2`
  §6.4 and the product's write gate name another key. F showed the holder of
  a retired key bringing a book to that state on purpose, by a race for the
  root's first declaration in a book the seat had not entered or by handing
  on first in a book the seat had left behind, and 241 of its 600 random
  worlds held a hand-over of the seat under another root. H showed the other
  half (its D4): an opening and a hand-over on the new home in one block
  left the office behind for good, the hand-over having been read under the
  home it left.
- *The product's order of rotation* (J): declare in every book, write the
  succession, anchor it, accede in every book. Between the succession's
  anchor and the accession in the home's book the outgoing key holds the
  office, and where another chain's book acceded first its holder carried
  the home there and kept the seat.
- *"Two rails" claimed an agreement `zikaron/2` does not bear out* (G, by
  the core): a seat that accedes, signs under the new key and only then
  writes the succession holds an `AUTHORITY_MISMATCH` in its ledger for
  good.
- *A ledger that continues a `zikaron/1` ledger* (G): the office starts at
  `R` here, while `zikaron/2` seeds that ledger's lineage with the
  continuation's author; whoever holds the old root key takes the seat by
  handing on first, until the seat has entered a book.

**The judge's supplement, mid-round (2026-11-13).** The ZIKARON seat asked
for a display name a root gives itself (`05-增补案-显示名.md`), written as an
account face on a platform label. The judge ruled: the word is `persona`; a
shape of its own, `kind` `persona` with the name in `name_md`, so that a
name may be written in any script, `handle` staying within printable ASCII;
the name is plain text, one unbroken run on one line, of bounded length,
with every space and format character barred. It stands, ends and spans as
any binding does, and the persona of a root is the name of the open span
that began last.

**Folded, the rules** (each moves answers):

1. *The head of a chain's book, and the opening's test of it* (§7.2, §7.3).
   The walk keeps for each chain the head of its book, the `to` of its last
   record under `R` and `R` where it has none, as `zikaron.kit/2` §6.4 reads
   a book; an opening counts only where that head, as the step found it, is
   the office. From an opening on, in a book that records a hand-over only
   from the key it holds, this law's office is the head of the home's book
   at every step, which is the key `zikaron.kit/2` reads, and the key the
   book's `current` gives wherever the book still names `R` as the root of
   `R`. The class F and H built is closed in the reading: no home opens on a
   book that holds another key, so no hand-over of the seat on its home is
   lost. A chain whose basis names no book keeps `R` as its head, which is
   the request's draft (2.7) again. The record of `74a71051` said that no
   such rule was added; it is added here, the four seats' witnesses and the
   order against forks having outweighed the reason given then.
2. *The opening block's accessions are read under the home it opens* (§7.2:
   anchors, then the home, then the accessions). H's D4 is closed.
3. *The persona* (§1 the name and its barred scalar values, §2.2 the fifth
   shape, §2.3, §7.4, §7.5, §7.8 the row and the table's `persona`, §8, §9).
   The barred table holds every scalar value Unicode 15.0 puts in the
   categories Cc, Cf, Zs, Zl, Zp and Co, with the fillers that show as
   blank, the variation selectors, the combining grapheme joiner and the
   noncharacters; a name is 1 to 32 scalar values.

**Folded, the text** (no answer moves): §1, this `accept` reads `zikaron/2`
alone (G6); §3.4, an address that answers applications' signing requests
(G5); §6.3, one record whichever declared registry emitted it (G4); §7.8,
the record's `root` enters no element of `lineage` (Node); and in §9: the
two rails rewritten with their four partings and the order that keeps them
together, the succession first and the accession at once after it (G1); the
home's book; before a home; moving the home; a reader that names no book on
a chain; a key in office that is stolen; a book the seat has not entered, by
the book's state and with what the holder of a retired key can do to a book
(F-1, F-3, H F2, J5, K F4); a ledger that continues an earlier one (G2, by
the sentence: the office is read from the book alone, as the judge settled
and as `zikaron.kit/2` reads); the kit's reading of the same book (H F5, the
Rust and Python seats); the reader's books, whose hand-over is recorded at
the acceding key's call (K F3, H F3, J6); book transactions signed for no
chain (H F4); what a retired key costs a reader (K F5, F-2); what the office
anchors, `zikaron/2` §5.7 asking for the time and this law for the sender
(G3); and a seat that moves to a later ledger law (G).

**Folded, the corpus and the instruments.** `build_anchor_records_checked`
(K's two witnesses), `build_home_book` (11 cases a seed: the opening's test
of the head on its four sides, the head's two, the opening block, the move
back) and `build_persona` (the shape's entries, every barred range at both
ends with its free neighbours, and 20 tables); four earlier cases rebuilt on
the side the gate leaves them. Seed 1 holds 1,176 cases: 654 `entry`, 28
`declare`, 49 `carried`, 408 `table`, 37 misuse. The boundary list holds 173
entries, 132 closed boundaries each both-sided and 41 rules of order, ties
and precedence. `HARNESS-FACE.md` names the fifth shape, the table's
`persona` and the open anchor record. K's weak test of the record given
twice reads the input alone now.

**Not taken.** G2's first form (start the walk at the key `zikaron/2` §7.3
names for a continued ledger): it reads the ledger where the judge settled
that the office is read from the book alone, and parts from `zikaron.kit/2`
§6.4. J's order of rotation (accede in the home's book before the succession
is written): the order is the family's, `zikaron.kit/2` §9 and the product
keep the succession first, and a change of it is the judge's to make for
every law that reads the book; it goes to the judge with the package. H's
and the Python seat's lesser forms (a §9 sentence naming the two answers)
and F's (advice to the seat): the rule above closes what they name. H's
option to count `VOID` with `UNPROVEN`: a `VOID` record of the office is
decided against, and the misread list shows its entry. A test of the chain a
book transaction was signed for: `zikaron.kit/2` reads the accession form
without one.

**For the judge, beyond this law.** The book admits the root's first
declaration from whoever holds the root key, so the holder of a retired root
keeps a seat out of every book it has not yet entered (F,
`test_BookNotEntered`, `test_EntryRace`); its full remedy lies in the book's
contract. `zikaron.kit/2` §9's sentence on the two rails has the flaw G
found in this law's.

**Measured after the fold.** The core of `74a71051` and the core after the
fold, over the 2,250 `table` cases of the corpus of `74a71051` at seeds 1 to
6: `persona` is null on every one, and beside it 2,111 answers are
byte-equal, 133 differ on inputs holding an opening whose chain's book has
another head than the office, 6 (one a seed) on the opening block's
accession, none unexplained. The criterion agrees with the generator's
predictions on every case at seeds 1 to 6, every boundary passing its sides,
`unwitnessed` empty, and `gen.py --seed 1` rebuilds the committed tree byte
for byte.

**The first measurement of the second fold.** The three implementers'
programs of the first revision round, unchanged, on the corpus of `01bfa541`
at seeds 1 to 6: the core of `74a71051` differs from the new core on 351
cases at every seed (every table that is read, which now carries `persona`,
and the persona entries), and Rust, Node and Python each diverge on exactly
those cases, answering as the core of `74a71051` did.

### The second revision round: seven seats, one fold

Seven seats over `01bfa541`: four readers (A the text; D the corpus and the
comparator; F the law against the chains; U the name against Unicode's data)
and the three implementers, each continuing on its program and bringing it
to the text from the text alone.

**The implementers.** Rust, Node and Python each read zero divergences and
zero disagreements at seeds 1 to 6 on the first run of the program they had
brought to the text, and none read a sentence of the fold two ways. Probes
against the criterion as a black box: Rust 104,987, Node 34,047, Python
32,785, with no disagreement, panic or hang; each swept every end of every
barred range with its neighbours and the ends of all seventeen planes.

**The readers.** No seat found a difference between the criterion and the
text, or a rule of the second fold with two readings. F set this law's
office beside `zikaron.kit/2`'s `in_office_at_block` on every home chain of
600 random worlds built with the reference book's rules, at 1,716,454 block
points from each opening on, and found no difference; it re-read the first
round's witnesses under the new core (the race for a book's entry, the stale
book, H's D1, D3 and D4, J's seizure of the seat), and each loss is gone or
is a denial §9 names; no outside party sets, changes or blanks a root's
persona, and two readers of one input read one name. A held §9's sentence on
the home's head against 4,000 random worlds without a violation, ran the
four partings of the two rails through the `zikaron/2` core as written, and
tested 40 reading-mutants, four of which the corpus let through. D tested 51
core mutants over seeds 1 to 6: nine lived for want of a witness and four
are equivalent; it also showed the comparator accepting two corpora whose
cases had been moved off the edge they were named for. U read the barred
table against Unicode 15.0 and 17.0: the table, the text and the core agree
at every end and neighbour; the table is the union of the general categories
Cc, Cf, Zs, Zl, Zp and Co and of the properties White_Space,
Default_Ignorable_Code_Point and Noncharacter_Code_Point, with U+2800,
U+FFFC and U+FFFD beside them; versions 16.0 and 17.0 added nothing to those
classes.

**Folded, one rule** (it moves answers): four rows of the barred table (U):
U+13441 and U+13442, the Egyptian hieroglyph blanks; U+16FE4, the Khitan
filler; U+1BC9D, the Duployan thick letter selector; U+1D159, the musical
null notehead. None of the categories and properties §1 names holds them,
and each is what the judge's standard bars: a blank, a filler, a selector of
form.

**Folded, the text** (no answer moves): §1, a name's length in decoded
scalar values and the two escapes its canonical spelling can hold, the ends
of the seventeen planes written out, "without a space", and the table's
sentence stated by Unicode 17.0's categories and properties with the blanks,
fillers and replacement characters beside them (U, the Python and Node
seats); §2.2, the `persona` shape carries no `note_md` (A); §2.4, a writer
spells a name in Normalization Form C (U); and in §9: the two rails agree on
what the seat sends, and part in five places, the fifth a record of the new
key in the block of its accession (A); an opening that the head's test
refuses leaves no mark but the table, and a seat reads its table before it
hands over (F); what brings a book in step (A); a reader that names no book
on a home the root opened (A); a book in which the root acceded under
another key holds no record under `R` again, and a book's own statement of
the key that acts for `R` is that key only where the book still names `R`
its root (F, `test_RootAccedesUnderAnother`); the persona among what the
office anchors (A, D). The sentence of the round before that set this law's
office beside the book's `current` is corrected above to say where the two
are one key.

**Folded, the corpus and the instruments.** D's patch
(`build_corpus_reader`, 46 cases a seed: the ends of every plane from 2 to
14; lengths in scalar values that graphemes, normalization and bytes each
count otherwise; persona rows that UTF-16 order would turn; names that
normalization or case folding would join; the persona's order key by key; an
anchor's `blockNumber` as the input's only fault) and its tightening of the
comparator, which now reads an entry case's input and passes a barred or
free name only on the name the case is named for, and three cases of the
opening's rules only where their input holds the step the rule reads.
`build_second_round` (21 cases: the four new rows at their ends and
neighbours, barred values inside a range, thirty-two and thirty-three
regional indicators, A's three witnesses of the head and of the rows'
order). The four rows' cases stand after every earlier case, so that every
earlier case keeps its bytes. Seed 1 holds 1,243 cases: 712 `entry`, 28
`declare`, 49 `carried`, 417 `table`, 37 misuse. The boundary list holds 190
entries, 140 closed boundaries each both-sided and 50 rules of order, ties
and precedence.

**Not taken.** A rule that a name begin with no combining mark, and an
allowance for the joiners after a virama (U): the judge's standard bars
every format character, and the cost goes to the judge with the package (the
zero-width joiners that Sinhala, Devanagari, Malayalam and Persian spelling
use, the variation selectors that pick a Mongolian form, an emoji
presentation or a variant ideograph, and every emoji sequence joined by
U+200D). Naming the reference book's functions in the law (F): the law names
no function of a contract, and the package tells the desk which calls to
read. D's sentence for §7.8 on names above U+FFFF: the corpus says it.

**For the judge, beyond this law.** In the reference book any address may
declare the root, and a holder of the root key that accedes to such a
declaration takes the root's record under another root for good (F); as with
the race for a book's entry, the remedy lies in the book's contract.

**Measured after the fold.** The fold changes one thing of the core, the
four rows of `zkfentry.py`'s table, and no file of the corpus of `01bfa541`
at seeds 1 to 6 (6,666 files) holds one of the five values they bar, so the
core before and the core after answer every case of that corpus alike. The
criterion agrees with the generator's predictions on every case at seeds 1
to 6, every boundary passing its sides, `unwitnessed` empty; `gen.py --seed
1` rebuilds the committed tree byte for byte, and every case of `01bfa541`
keeps its bytes. The text's table equals the core's row for row (37 rows).

**The first measurement of the third fold.** The three implementers'
programs of the second revision round, unchanged, on the corpus of
`75c52676` at seeds 1 to 6: the core of `01bfa541` differs from the new core
on five cases at every seed, the names that hold one of the five values the
four rows bar, and Rust, Node and Python each diverge on exactly those
cases, answering as the core of `01bfa541` did.

### The third revision round: four seats, and the closing fold

Four seats over `75c52676`: the check seat K, and the three implementers,
each continuing on its program.

**The implementers.** The one change that moved answers was the four rows of
§1's table; each program gained them from the text and read zero divergences
and zero disagreements at seeds 1 to 6 on its first run. Probes against the
criterion as a black box with the programs so brought: Rust 105,367, Node
20,508, Python 21,657, with no disagreement, panic or hang; Rust's, over its
final program, meet §10.1's hundred thousand fuzz samples for one
independent implementation. No seat read a sentence two ways.

**The check seat.** K read every sentence the third fold added or changed
against the core, the `zikaron/2` core and the reference book (seven forge
tests of its own): each is true of them. It read the table against Unicode
17.0 and 15.0: every scalar value of the classes §1 names is barred, and
exactly eight barred values lie outside them. It ran 25 reading-mutants of
the name rules, of which the corpus let two through, and checked this
record's counts and measurements against the manifest, the logs and the
seats' reports.

**Folded, the corpus** (no answer moves): `build_third_round`, five cases:
names of thirty-two and thirty-three scalar values that hold a quotation
mark or backslashes, whose canonical spelling is longer than their count,
and a name that begins with a combining mark. Seed 1 holds 1,248 cases: 717
`entry`, 28 `declare`, 49 `carried`, 417 `table`, 37 misuse. The boundary
list holds 192 entries, 141 closed boundaries each both-sided and 51
one-sided rules. `face-table-unicode.js` reads the table out of the law's
text and sets it beside the Unicode data of the `node` that runs it (the
Node seat asked that the extraction be kept).

**Folded, the text** (no answer moves): §1 names the eight values the table
bars beside Unicode's classes; §9, the two rails: the bullet states how each
law holds a key (this one by accessions alone, `zikaron/2` by the ledger's
successions and their anchors) and what may follow for a record or an entry,
each consequence borne out by a probe of the check seat, says what a seat
does to narrow the difference, and claims no order under which the two read
alike. The check seat's four readings had broken, one after another, each
sentence that spoke of every input: a record in the block of the accession,
an entry signed past the succession and anchored in the anchor's second, a
record whose block time another chain's clock sets after the accession, an
accession off the home; §9, a book is kept from the seat once another
party's key has acceded there, a declaration alone keeping nothing. Two
statements of this record's section before were corrected in place: the
Python seat's probes number 32,785, and the four rows hold values that none
of the categories and properties §1 names holds.

**The closing measurement.** The closing fold changes text and adds cases
alone, so by §5.1 item 7 of the standard the three programs of this round
were run unchanged on the folded corpus (`23190633`, which no later commit
moves) at seeds 1 to 6: Rust, Node and Python each read zero divergences and
zero disagreements at every seed, eighteen runs, the bytes read equal to the
six digests below. The criterion, unchanged since `75c52676` (candidate
digest
`0xe66ebad9ab4ef88238ba88f7cab00de5cf396a7e83a19b55c34d692ef6e30e1c`),
agrees with the generator's predictions on every case at those seeds, every
boundary passing its sides and `unwitnessed` empty, and `gen.py --seed 1`
rebuilds the committed tree byte for byte.

| seed | manifest sha256 | bytes read |
|---|---|---|
| 1 | `0x7aae59190c2fd4fae42ebd005478533a62ed96285553d322c51a9fca48fc6ee6` | `0xf1b8905d7aa725dd80a20c4402037d09e3cb06b881ea59e042f63854401f4d60` |
| 2 | `0xa1d4f601c5b866ac95d20e4f51e1ce0bcc17c8c4dada96d1173c4f84ac7a5972` | `0x062c71846c0a49444e10c1dbbc3bf39b955a5e7f9c2d3d13ebbb1398bb2f4c1c` |
| 3 | `0xe4c3ae03392c8a470a70da3d4e91ce005eca48072bc77ea3e15a2e9f99e92742` | `0x64e2b7b9fb46531faab70520b3ecfc845059354cb5e00f33d3b703597e50b95a` |
| 4 | `0x9e73bb62afed1585946eb424b6af0f2eb990fa12ea0a9f21cf8c5f309ebb52df` | `0xd9b6d19f888f6cecbe0c10ccd9c52bbc3b57bc7ab0c5f82790dbf5f9976fd057` |
| 5 | `0xe8ce8760154d2849e302577b48957ad7e8158c032f52ad08f3821f130d7bef74` | `0x6768e9a01c5d31abdf143881482648264f654b0b78138a74d9e9c06c51390913` |
| 6 | `0xdfcab7d171d0d98626421a27fde5be6592a5c568d764ee7f65b23cee2984e5ff` | `0xc7502516698b9cbed2db58ef953166242bc746d895bb0cf580f585cdec487470` |

**The check seat on the closing fold.** The seat read the folded sentences
against the two cores and the reference book, found its two mutants of the
name rules killed and each new boundary row passing for its own reason, and
checked this section against the manifest and the seats' reports. Every
folded sentence stood but those of the two rails: there each sentence that
spoke of every input fell to one input of the seat's making, in four
readings (`d1ac9db0`, `4e41ff6f`, `1fc29bd4` hold the rewordings). The
bullet as it stands states the two rules by which each law holds a key,
names what may follow with a probe behind each consequence, and gives a
seat's order as guidance. At `1fc29bd4` the seat found every sentence of one
reading, none that one input refutes, and the text fit to be named.

### Naming (2026-11-14)

§10.5 names the release digest
`0xe66ebad9ab4ef88238ba88f7cab00de5cf396a7e83a19b55c34d692ef6e30e1c`,
computed by `criterion-digest-face.sh` over the three files of `face-py/`
(`zkf.py` sha256
`e0eac652ae5290a5cabf18f2faa1401846e4e523daa89755aed14e4ee03e99e4`,
`zkfentry.py`
`b983984bade336bca0b5a7b38f3abfac07aa3babb07bc5cf4c2f86b09c8969b8`,
`zkftable.py`
`721bf8c7a005003b2e0a4a6dea9adee02c0a236f9e85250e33b76241de999eae`) and the
`zikaron/2` core digest
`0x2a6564e126c632c70b51c602b92bdee47ab2a86118b955d32e2dd5c0f1af567a` of
§10.4. It supersedes
`0x105ff340fbe48a5af121c1485b7bde7561adc6a2cccf5256663b8c8f6c5a2f36`, named
on 2026-11-13 over the law as it stood before this revision: a program that
agrees with that core reads another law. The text so named has sha256
`85ee126d730a4edb1b2137cb46ce1d80ee48dde6cc476997d2bd8f81c209c8df`.

The freeze rests on: the three folds of this revision, after each of which
the three implementers' programs, unchanged, parted from the new core on
exactly the cases the fold moved, and, brought to the text from the text
alone, read zero divergences at seeds 1 to 6; the closing fold of text and
cases, after which the three programs unchanged read zero divergences at
seeds 1 to 6; the criterion against the generator's predictions at seeds 1
to 6, every boundary passing its sides; probes outside the corpus against
the criterion as a black box over every round since the law was first
drafted (Rust 511,525, Node 205,555, Python 218,442), with no disagreement,
panic, hang or nondeterministic answer, the Rust program measured unchanged
in the closing measurement over 105,367 of them, which meets §10.1's hundred
thousand for one independent implementation; this law's office set beside
`zikaron.kit/2`'s on every home chain of 600 random worlds at 1,716,454
block points without a difference; and the check seat's readings of the
closing fold, the last of which found every folded sentence of one reading.
The text, the criterion and the corpus are the ones measured: the naming
moves no byte of `face-py/` or of the corpus.


## The revision of 2026-11-14, after the naming: a seat moves to another root

**Why.** Under the law as named on 2026-11-14 a home opens only on a chain
whose book holds the key in office under `R` (§7.2), and a book comes to
hold it by the root's own hand-over or by the hand-over of an earlier key
the book holds. A seat whose root key is lost, or in other hands, enters no
further book: it keeps its home where a book already holds its key, and no
chain it had not entered is open to it, whatever becomes of the chains it
had. The root key is the first key a seat retires, so the longer a seat
lives the likelier it stands there. The ZIKARON seat asked on 2026-11-14 for
a declaration by which a seat moves to a fresh root, the first such
declaration counting; the judge approved the request and then changed it in
talk with the Court, and the changes bind.

**What the judge settled.**

1. A reroot moves faces and nothing else. Grants are bound to the ledger
   that published them; a reroot says that the identity is found on another
   ledger, which is a matter of the persona and the faces. `zikaron/2` and
   `zikaron.kit/2` stay as they are and learn of no reroot.
2. The request carried the new root's consent as a signature in the `reroot`
   entry, under a second declaration of three lines, tested by recovery to
   the address the entry names as `to`. That address is the new root's own:
   the signature is its root key's, and the reading of the old root holds no
   record of the new root's book, so it cannot tell whether that key still
   holds office there. A party holding the leaked root key of a third seat,
   long retired in that seat's books, could reroot a seat of its own to that
   seat's root and sign the consent, and the table would show a reroot with
   both halves. The judge dropped the signature. A `reroot` entry is the old
   seat's word and an `origin` entry the new seat's, each seated by its own
   seat's key in office as every entry of this law is; the two seats are set
   together where the two tables name each other, which the law defines as
   **joined** (§7.9). The one cost is that a reader starting from the old
   root reads the new root's table as well, which a product does in any case
   to show the new root's name.
3. The seal covers faces alone. From the step after its reroot a seat holds
   no record in office; the accessions go on moving the office, the line and
   the heads. The request had the office stand still at the seal; the seat
   will go on handing its old key on, since the old ledger and its grants
   stay with the old root, and an office that stood still here would part
   from what `zikaron.kit/2` reads on the same book.
4. The first seated reroot counts, and no act undoes it; the origin
   likewise.
5. The table's members are `reroot` and `origin`. The request's `successor`,
   and the word "continues", are `zikaron/2`'s words for a ledger that
   continues another (§12.3, §12.6), with the reading together that carries;
   the law uses neither for a reroot and says in «What this law leaves as it
   found» that a reroot is no continuation.
6. The law does not test that the root a seat moves to is fresh. A seat that
   reroots to a root whose key entered a book or signed before is a seat's
   own matter; the reason for a fresh root is that every book holds it under
   itself.
7. The revision is made in the law's development period, before any release
   carries `zikaron.face/1`: the type and domain literal stay, and §10.5
   names the core anew.

**What changed.** §2.2: seven shapes, the two added being `reroot` (`act`,
`root`, `to`, optional `note_md`) and `origin` (`act`, `root`, `from`,
optional `note_md`), `to` and `from` byte-unequal to `root`; §2.3 says what
each says. §7.2: the walk keeps the seal and closes it at a step that holds
a record seated for a `reroot` entry that reads; a record is in office only
where its step found the seal open. §7.4: `reroot` and `origin` entries
stand as bindings stand and are no bindings; the reroot of the reading is
the standing one least in moment, then in `entry_id`, and the origin
likewise. §7.7: the unproven count takes only records whose step found the
seal open (the unread count follows from "in office"). §7.8: the table's
`reroot` and `origin`. §7.9: joined. §8: seven shapes. §9: four bullets (a
reroot is half a proof, and so is an origin; a sealed seat; what a reroot
leaves with the old root; the root a seat moves to), a clause each in «A key
in office that is stolen», «A book the seat has not entered» and «What the
office anchors», and a sentence in «What this law leaves as it found». §10.1
names the pairs of §7.9 among the corpus's inputs. `HARNESS-FACE.md`: the
two shapes in `entry`'s answer, the two members in `table`'s, and `zkf
joined <pair.json>`, a file holding an array of exactly two face inputs. The
misread reasons stay six: a release that names a reroot or an origin is
`SUBJECT`, and a face entry anchored after the seal by a key of the line is
`UNSEATED`.

### Round 0: the checkpoint

The core: `zkfentry.py` gains the two rows and the test of `to` and `from`
against `root`; `zkftable.py` gains the seal, the choice of the reroot and
the origin, the two members and `joined`, and reads a pair; `zkf.py` gains
the command. The generator's predictor was written to the same clauses. The
corpus at seed 1 grows from 1,248 cases to 1,451: 50 entries of the two
shapes and their near misses, 57 hand-built tables (the seal and its own
block; reroots that do not read or are not seated, by each condition of
§7.3; a sealed seat's unread, unproven, releases, personas and openings;
hand-overs after the seal; the choice among several reroots and several
origins; releases that name them), 60 random worlds with reroots and origins
drawn from a stream of their own, 32 pairs for `joined`, and 4 misuse cases;
the boundary list grows by 47 rows. No earlier case moved a byte of its
file. Of the 1,248 earlier cases 965 keep their predicted answer byte for
byte, and the 283 whose answer is a table differ by the two members alone,
`null` in each: the revision moves no answer the law gave before it. The
criterion against the generator's predictions at seeds 1 to 6: zero
divergences and zero disagreements, every boundary passing its sides and
`unwitnessed` empty; `gen.py --seed 1` rebuilds the committed tree byte for
byte; `bcov.py face` finds every arm of the three files witnessed but the
guard of `__main__`. The predictor and the core were also set side by side
in process over 600 random worlds and 300 pairs weighted toward reroots,
origins and equal moments (111 tables with a reroot): no difference.

| seed | manifest sha256 | bytes read |
|---|---|---|
| 1 | `0xa52f440c8001ec85b38ad46476ca0856222dfce9d24902f172457222da50fe3d` | `0x5c06c2637737f29a948304a34b7afaa06880ad22b42dfd67c20d699e797acdbd` |
| 2 | `0x3ad8d776c1de18622cdc5cbdf96c587d9565a7d47560459ea799d1ffcd46f7eb` | `0x119a265c6aaca8c756ccf64ddcb6209acb71378d90c3088760855c7a3bc27e7f` |
| 3 | `0x52a6462de5afbf7d86c1cb18526c4cc320fac35f8969097ee3f23a7a76fff108` | `0x503db377c5bb1abd4e7191cfb8c17e36af2e1270e26cbad1b68c36515e5ce0a6` |
| 4 | `0xf3186b8861edf7b61bb3dc4bb18a142bc97ef3aa03f82e6ddf67e7b4f071f3aa` | `0xfc5d3da1c7e76b487860302e3fabd51a9ffd8ca439776951c49a46b947984aa2` |
| 5 | `0x545142b05c7a4aea47aacbb0d1e68aaffdab934c377a4a0843fd41d5c3492585` | `0x8ec6a90e14df6447a8f6b7d5d307c3f83e358b1d19279bdcfa8b425b41093176` |
| 6 | `0xc63d6b19db6858519b8eb80b80468a36473669bf429d3bfcbb0f2bd230f01538` | `0xdd0ad1852dca0afea507cf3fd74ae4fd21ce2cbb4b1f6d5e50e34b18fb13798c` |


### The first round: seven seats, one fold

Seven seats read `803fb109`: three implementers continuing their programs of
the last round, a reader of the text (A), a reader of the corpus and the
comparator (D), a seat that set the revision against the family's laws, the
books and the product (F), and a seat that attacked it (V).

**The first measurement.** The three programs as they stood at the naming of
2026-11-14, and the core of that naming, were run unchanged on the new
corpus at seeds 1 to 6. At every seed each of the four differs from the new
core on the same 444 cases with the same answers: 12 entries of the two new
shapes, read as shapeless; 32 pairs, the command unknown; and 400 tables, of
which at seed 1 289 differ by the two members alone and 111 hold a reroot or
an origin the old reading does not know. Nothing else moved.

**The implementers.** Each brought its program to the text from the diff of
the law and the harness alone, and each read zero divergences at seeds 1 to
6. Probes outside the corpus against the criterion as a black box, byte for
byte, across the five commands: Rust 66,058 (30,000 tables, of which 3,376
read a reroot and 2,317 carry records of the office after the seal; 16,000
pairs), Node 23,400, Python 22,400 and 49 hand-built tables; no
disagreement, no panic, no hang. No implementer found a sentence of two
readings in the new mechanism. Two of them, and seat A, found that a face
input whose anchor record carries a member nested to the transport's depth
128 has no spelling as an element of a pair, the pair's array being one
level more: the harness now says so, and the corpus witnesses the bound on
both sides for `table` and for `joined`. The Node seat asked for a plainer
gloss of the reroot's own step and a sentence on a reroot anchored after the
seal; both are taken.

**Seat A, the text.** No sentence of the new mechanism has two readings that
answer differently, and the core follows the text on each case the brief
named. One load-bearing finding and five of wording. The load-bearing one is
a sentence §9 lacked: a reader whose basis leaves out the chain a seat's
home moved to reads the seat on the home it left, where a key since retired
is still in office, and may read there that key's reroot, joined to a root
its holder keeps. The openings by which some reader holds a retired key in
office were named before this revision for bindings and releases, which a
seat undoes once it holds the office again in that reader's table; a reroot
it does not undo. The rule stands, since the defence is the reader's choice
of basis as it was; §9 now says it in «A reroot is half a proof» and names
the reroot wherever it lists what such a key does. Three sentences fell to
one input each, as the judge's standing order has a sentence of §9 tested:
what a key in office binds and seats (false of a sealed seat), the unread
count of a seat that anchors hashes alone (zero past the seal), and "a seat
whose home's chain takes its transactions no longer has no reroot to make"
(it moves its home first, to a chain whose head is its key in office, and
reroots there). Each is rewritten; «A sealed seat» carries one sentence that
scopes every other bullet's words on a key in office to a seat whose seal is
open.

**Seat V, the attack.** Thirty hand-built inputs, 800 reorderings and
duplications of the corpus's tables (every table the same), 128 runs
injecting records after the seal by every key, shape and verdict (every
table the same but for `misread`, which grew 79 times, as «What a retired
key costs a reader» names), and five forge tests against the reference book
and registry, 256 fuzz runs among them: an outside party cannot make a fresh
root count as entered in any book, nor take it. Nothing is left of the
opening the dropped signature had: a party holding the retired root key of a
third seat writes no origin there, and no table joins. The seat's one
load-bearing finding is seat A's seen from the attacker's side, over each
opening that holds a retired key in office in some reader's table (a book
left behind, a chain that runs behind or that its holder produces, a book
the reader names, no book named): there the key reroots, and the reroot
stands. It is folded as one general sentence in «A key in office that is
stolen». The list of readers who read no seal from a record is completed.

**Seat F, the family's laws and the product.** No fork from `zikaron/2`,
`zikaron.kit/2`, the reference book or the registry, and no contradiction:
every sentence of the revision that speaks of them is borne out by their
text, and those that speak of every input held under their cores. On every
home chain of 600 random worlds built by the reference book's rules, with
reroots at random points, hand-overs after the seal and outside parties
acting, read at three moments each, this law's office equals
`zikaron.kit/2`'s key in office at a block at 157,468 block points before
the seal and 188,395 after it, and the lineage in 1,971 comparisons, without
a difference. Under the two other cores a ledger holding a reroot audits as
it does with any face entry at that place; a grant of the old root is
`GREEN` before and after; the old key's holder writes grants that check
`GREEN` and may withdraw the seat's grants by a notice; a licence granted to
the old root is sub-granted by the old key and not by the new root; a grant
of the new root over a work the old ledger recorded checks `GREEN` under the
new root. All of it is as «What a reroot leaves with the old root» says. The
seat's load-bearing finding is a joint with the product: the request had the
old root turn read-only and its key in office free to discard, while the law
leaves the old ledger, its grants and the licences it holds with that key.
The law now states the cost in that bullet; the product's side goes to the
ZIKARON seat with the package. Wordings taken: "the seat moves its faces"
for "the seat moves", since in the family's laws a seat is a ledger's office
and a reroot leaves it where it is; the root a seat moves its faces to is
one whose key has acceded to no other key's declaration, which is what a
fresh root buys, and the bullet no longer asks that it have signed nothing,
a new identity signing its genesis before any reroot; the new root's marker
on each account's page.

**Seat D, the corpus.** The seed rebuilds byte for byte; seeds 1 to 6 are
clean; every arm of the core is witnessed; the predictor reads the text as
the seat does. Of fifty mutants of the revision's rules, forty-five die at
seed 1, one is equivalent (the heads of a sealed seat, which nothing reads:
now an unobservable row with its reason), and four survive: the `entry_id`
set before the moment in the choice of the reroot (the case had the lesser
`tx` on the lesser `entry_id` by the chance of the seed), an origin's
`since` inside one block, an origin sent by a retired key of the line, and a
seal left open where the reroot's step holds an origin. Each has its witness
now, the first by construction at every seed; the comparator's tests that
would have passed a wrong answer are tightened. The boundary list grew by 47
rows in round 0, where this record and the brief said 49.

**The fold.** Prose of the law: the [R] class reads one input, or two for
§7.9; §2.2 "have no shape"; §2.3 "moves its faces" and what the table takes
no more of; §7.2's gloss of the reroot's own step; §7.4 "it has a reroot
exactly where the walk closed the seal"; in §9, «A reroot is half a proof»
(the key each table holds in office, one origin, joined under two bases), «A
sealed seat» (the scope of the other bullets, a reroot anchored after the
seal, the marker, the readers who read no seal), «What a reroot leaves with
the old root» (what the key's holder may do, and handing the old seat on to
a key the seat keeps), «The root a seat moves its faces to», «A key in
office that is stolen» (the origin, and the general sentence), a word each
in «The reader's chains», «The home's book is the seat's to keep», «Unread
and unproven are hints», «What the office anchors» and «A book the seat has
not entered», and the sentence on continuation, now affirmative. The
harness: the depth bound of a pair. The corpus: seventeen cases added (the
origin under each test of §7.3 and its `since` inside one block; a retired
key rerooting on a chain that runs behind; three pairs read under two bases;
the depth bound for `table` and `joined`), five changed (the two ties by
`tx`, with the lesser `tx` on the greater `entry_id`; the two cases that set
something in the reroot's own step, which now show the seal closed after it;
the pair read over two bases, whose bases now share no chain), and the
boundary list six rows longer, with the transport's depth 128 witnessed on
both sides and no longer unobservable. No byte of `face-py` moved. The cases
the law had before this revision keep their bytes and their answers; the
cases of round 0 from the first changed one on carry entries of later
sequence numbers, so their files and the entry ids in their answers moved
with them, and no reading moved but in the five changed cases, whose inputs
changed, and in `two_origins_one_tx_entry_decides`, whose tie by `entry_id`
falls on the other origin as the ids moved.

Not taken: counting the depth bound from each element of a pair, which would
be a second reader beside the table's. Given to the ZIKARON seat with the
package and outside the law: the old root's read-only state covering faces
alone, the old key kept or handed on and never discarded, the new root's
consent step and its signing port removed, each step read back from the
table before the next, a reroot that is not joined shown as a claim, and the
marker.

**After the fold.** The corpus at seed 1 holds 1,468 cases (entries 767,
declare 28, carried 49, tables 546, joined 37, misuse 41) and the boundary
list 245 rows. The criterion against the generator's predictions at seeds 1
to 6: zero divergences and zero disagreements, every boundary passing its
sides, `unwitnessed` empty, `gen.py --seed 1` rebuilding the committed tree
byte for byte. The four mutants that survived are killed at seed 1. The
predictor and the core set side by side in process over 9,600 random worlds
and 4,800 pairs: no difference.

**The closing measurement.** The three programs of this round, unchanged
since their seats left them, on the folded corpus at seeds 1 to 6: eighteen
runs, zero divergences and zero disagreements in each, the bytes-read
digests those of the table below. The fold moved prose and cases alone, so
by the standard's fifth section, part one, item 7, this measurement closes
the round, the check seat's reading of the folded sentences outstanding.

| seed | manifest sha256 | bytes read |
|---|---|---|
| 1 | `0xe76e716649ae95d7656fbe2d6b38b7f60a9486a3872ab50d90773cda1097057c` | `0x4829970206f67cf02d816cbd6e081ddc5d0eee712d2c363a9724719813e368a0` |
| 2 | `0xfe45cbab61e9f2064b29d4b764a38bffa96a38ee1059715831bb9839bd34e5a7` | `0x6328ac3c50fcd93aff468132832b93c31d85c8c29a375733e4e506b954da2f42` |
| 3 | `0xc7a7d030ce2d4e0ef270b6753f4b4b42ebbca2422e17458b45738ad112607c6f` | `0xc9a4576eb8b052935951eb75d7834e8b20fcb75434d3f030537916aee85b2863` |
| 4 | `0x97e281724413858bae386efb9f1737ca411d6120eb8896df6a396bd1bfa67b76` | `0x24c91758abd8835894156723fe23588002ac82f0e6a2c1c626cfa98a41c2d554` |
| 5 | `0x727ba3e2c077623a24133c8f866fa0a0845727d49dd3d8b56270c1e2b39f5684` | `0xb58203380f31a7c1f53a0c02e5bd662637b01769db7122829c5c7b792675e75b` |
| 6 | `0xd7017b6e0eb75edda75777cc4bd97829c03a78dcd6dea4c3ee430a6742728f25` | `0x3149492396d883260c4fadf67f561edc1d1481159e701cdba9cb42a1f001e1c7` |

**The check seat on the fold.** The seat read every folded sentence against
the rest of the text, the two other cores and the reference book (three
forge tests, one fuzzed over 256 runs: a key that acceded to no declaration
stands under itself in the book whatever anyone declares to it), re-ran seat
D's four surviving mutants, each killed now, and nine reading-mutants of its
own on what the fold touched, all killed, set the walk's seal beside the
table's `reroot` on 457 tables of the corpus, the same on each, and checked
this section against the manifest, the logs and the seven reports. No answer
of the core was in question. Six wordings fell or fell short, five to an
input of the seat's making and one to the bullet beside it, and are
rewritten: one origin is said of one input of a root, another basis reading
another; the seat whose home's chain is gone is told what a reroot needs and
how it moves its home, with no sentence on when it cannot; the origin joins
what a sealed seat takes no more of, in §2.3 and in the sentence that scopes
the other bullets; the general sentence of «A key in office that is stolen»
says what such a key does wherever this section has a reader's table hold it
in office, and lists neither the ways nor the bullets, a list being read as
closed (the seat's second reading found one bullet missing from a list of
three); a reroot joins two tables with the origin that answers it; and the
root that every book holds under itself is said of books of the family's
code. This section's sentence on what the fold moved now names the tie by
`entry_id` whose winner changed with the ids. At its second reading the seat
found every other folded sentence of one reading and true of the core, the
two other cores and the book, its probes byte for byte as before. At its
third reading, of that general sentence as it stands with no list in it, the
seat set its seven inputs and the continued-ledger shape against it, found
it of one reading and borne out by each, and found the text fit to be named
at `844b8680`.

### Naming (2026-11-14), the reroot revision

§10.5 names the release digest
`0xc82c356c829fd0f88b543fc15a49deeb8088bc57635523372822e18b8ae53e8d`,
computed by `criterion-digest-face.sh` over the three files of `face-py/`
(`zkf.py` sha256
`256ade16b336585b805fd8c7474c6e6e1dc8eba6fc7fc0ea7686b501061d0d33`,
`zkfentry.py`
`ac98a79abbc6a0d219b23739bd647349646fccce556fec20c82fb4c806c7da22`,
`zkftable.py`
`96087b094d2156c761518be3024b38de72c6f8e9ef252c5036b677bfa5917379`) and the
`zikaron/2` core digest
`0x2a6564e126c632c70b51c602b92bdee47ab2a86118b955d32e2dd5c0f1af567a` of
§10.4. It supersedes
`0xe66ebad9ab4ef88238ba88f7cab00de5cf396a7e83a19b55c34d692ef6e30e1c`, named
earlier on 2026-11-14 over the law as it stood before this revision: a
program that agrees with that core reads five shapes, no seal and no pair.
The text so named has sha256
`9251c8ffebd80530e226299012f9ebf6da7b8c384eb11ad6572953d23104d3c6`.

The freeze rests on: the criterion against the generator's predictions at
seeds 1 to 6, every boundary passing its sides, at round 0 and after the
fold; the first measurement, in which the three implementers' programs and
the superseded core, unchanged, parted from the new core on the same 444
cases at every seed with the same answers; the three programs brought to the
text from the text alone, each reading zero divergences at seeds 1 to 6; the
fold of prose and cases, after which the three programs unchanged read zero
divergences at seeds 1 to 6 in eighteen runs; probes outside the corpus
against the criterion as a black box in this revision (Rust 112,058, Node
23,400, Python 22,400), with no disagreement, panic, hang or
nondeterministic answer, the Rust figure being its seat's 66,058 and a
further 46,000 of the same generator at another seed (8,000 entries, 2,000
declarations, 2,000 carried, 22,000 tables, 12,000 pairs), run by the Court
with the seat's program and driver unchanged, which meets §10.1's hundred
thousand for one independent implementation against the core named here;
this law's office set beside `zikaron.kit/2`'s on every home chain of 600
random worlds at 345,863 block points, before and after the seal, without a
difference; fifty mutants of the revision's rules and nine of the fold's,
every one killed at seed 1 but the one that is equivalent, which the
boundary list names as unobservable with its reason; and the check seat's
three readings of the fold, the last of which found every folded sentence of
one reading. The text, the criterion and the corpus are the ones measured:
the naming moves no byte of `face-py/` or of the corpus.
