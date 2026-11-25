# ZIKARON face grammar v1 (`zikaron.face/1`)

The law of an identity's faces: the platform accounts and the chain
addresses a ledger's seat declares to be its own and the name it goes by,
and the table a stranger reads of them from chains alone. Where
[`zikaron/2`](zikaron-v2.md) says what an entry is and what an anchor is,
`zikaron.face/1` says what a face entry is, how a key consents to being a
face, where a seat's lineage is read, how a seat moves its faces to another
root, and what the face table of a root says. It consumes `zikaron/2` only
through that law's frozen core, and it changes no output of that law.

**Status.** The law is the set of total predicates in this text. A
conforming implementation embeds the `zikaron/2` core of the release digest
named in §10.4, and agrees with the core named in §10.5 of this text on
every input; until that core is named this text is the criterion. Evolution
goes through `zikaron.face/2`.

**Rule classes.** Three classes appear, and every predicate and every
chain-facing rule below is marked with its class:

- **[F]** face predicates: decided offline from the byte strings the
  predicate is given and, where stated, a root, and nothing else.
- **[R]** reading predicates: decided offline from a face input (§6), or for
  §7.9 from two. No predicate of this law consults a chain or a clock; what
  a chain said is inside the face input.
- **[X]** chain-facing rules: how a face input is read from chains, and how
  an entry's bytes ride with its anchor (§5.2, §5.3, §6.3). They are the
  work of the reader that builds the input and of the producer that sends
  the anchor, and the freeze of §10 reaches them only through the inputs
  they produce.

## 1. Notation

`zikaron/2` §1 applies: byte strings, `hex20`, `hex32`, `hex65`, tokens,
skeleton and prose strings, `int`, bytewise order, `sha256`, `keccak256`,
secp256k1 and its order `n`, UTF-8, Unix seconds. In addition:

- **`accept(b)`**: the acceptance of a byte string as an entry, `zikaron/2`
  §4.3 decided by the embedded core, yielding the entry or that law's
  rejection token. A byte string that passes it is an **entry**; its
  **`entry_id`** is `sha256(b)` (`zikaron/2` §2.1), and its `author`,
  `entryType` and `body` are the members of its envelope (`zikaron/2` §4.1).
  This `accept` reads `zikaron/2` alone: a `zikaron/1` byte string is no
  entry under this law, where `zikaron.kit/2`'s function of that name reads
  one.
- **`R`**: the root a reading is made under, a `hex20`: the address that
  names the seat (`zikaron/2` §7.3).
- **label**: a string of 1 to 32 characters, each one of `a` to `z`, `0` to
  `9`, or `-` (U+002D). Labels compare bytewise.
- **handle**: a string of 1 to 128 scalar values whose first and whose last
  scalar value is other than U+0020. A handle sits in an entry outside every
  prose subtree, so `zikaron/2` §3.3 already holds each of its scalar values
  within U+0020 through U+007E, and its length in scalar values is the
  length of its decoded UTF-8 bytes, each `\"` and `\\` of the entry's
  canonical spelling standing for one of them. Handles compare bytewise.
- **name**: a prose string (`zikaron/2` §3.3) of 1 to 32 scalar values, none
  of them **barred**: one unbroken run of text on one line, without a space.
  The barred scalar values are those of the table that closes this section
  and the two scalar values that end each of the seventeen planes, U+FFFE
  and U+FFFF, U+1FFFE and U+1FFFF, and so on through U+10FFFE and U+10FFFF.
  A name's length is that of its decoded scalar values, each `\"` and `\\`
  of the entry's canonical spelling standing for one of them; no other
  escape occurs in a name, U+0000 through U+001F being barred. Names compare
  bytewise, as the UTF-8 of their scalar values, and this law normalizes
  nothing.
- **face entry**: an entry whose `entryType` is byte-equal to
  `zikaron.face/1`.

The barred scalar values of a name, a closed table: the controls, every
space, the separators of lines and paragraphs, the format characters, those
that set a direction, join, or select a form among them, the fillers and
blanks that show as nothing, the replacement characters, the private-use
areas and the noncharacters. It holds every scalar value that Unicode 17.0
puts in its general categories Cc, Cf, Zs, Zl, Zp and Co or gives the
property White_Space, Default_Ignorable_Code_Point or
Noncharacter_Code_Point, and beside them eight values no such class holds:
U+2800, the braille blank; U+FFFC and U+FFFD, the replacement characters;
U+13441 and U+13442, the Egyptian hieroglyph blanks; U+16FE4, the Khitan
filler; U+1BC9D, the Duployan thick letter selector; and U+1D159, the
musical null notehead.

| from | to | what |
|---|---|---|
| U+0000 | U+0020 | controls, space |
| U+007F | U+00A0 | delete, controls, no-break space |
| U+00AD | U+00AD | soft hyphen |
| U+034F | U+034F | combining grapheme joiner |
| U+0600 | U+0605 | Arabic number signs |
| U+061C | U+061C | Arabic letter mark |
| U+06DD | U+06DD | Arabic end of ayah |
| U+070F | U+070F | Syriac abbreviation mark |
| U+0890 | U+0891 | Arabic currency marks above |
| U+08E2 | U+08E2 | Arabic disputed end of ayah |
| U+115F | U+1160 | Hangul fillers |
| U+1680 | U+1680 | Ogham space mark |
| U+17B4 | U+17B5 | Khmer inherent vowels |
| U+180B | U+180F | Mongolian variation selectors and vowel separator |
| U+2000 | U+200F | spaces, joiners without width, direction marks |
| U+2028 | U+202F | line and paragraph separators, embeddings and overrides, narrow no-break space |
| U+205F | U+206F | mathematical space, word joiner, invisible operators, isolates, deprecated format characters |
| U+2800 | U+2800 | braille blank |
| U+3000 | U+3000 | ideographic space |
| U+3164 | U+3164 | Hangul filler |
| U+E000 | U+F8FF | private use |
| U+FDD0 | U+FDEF | noncharacters |
| U+FE00 | U+FE0F | variation selectors |
| U+FEFF | U+FEFF | byte order mark |
| U+FFA0 | U+FFA0 | halfwidth Hangul filler |
| U+FFF0 | U+FFFF | specials |
| U+110BD | U+110BD | Kaithi number sign |
| U+110CD | U+110CD | Kaithi number sign above |
| U+13430 | U+1343F | Egyptian hieroglyph format controls |
| U+13441 | U+13442 | Egyptian hieroglyph blanks |
| U+16FE4 | U+16FE4 | Khitan small script filler |
| U+1BC9D | U+1BC9D | Duployan thick letter selector |
| U+1BCA0 | U+1BCA3 | shorthand format controls |
| U+1D159 | U+1D159 | musical null notehead |
| U+1D173 | U+1D17A | musical format controls |
| U+E0000 | U+E0FFF | tags, variation selectors |
| U+F0000 | U+10FFFF | private use |

## 2. Face entries

**[F] 2.1 The type.** A face entry is a `zikaron/2` entry: it passes
`zikaron/2` §4.3 in full, it is signed under that law's one domain by its
`author`, and it carries a `seq` and a `prev` like every entry.
`zikaron.face/1` is none of the eight types of `zikaron/2` §6, so that law
reads it by §6.9: an entry all the same, its body an object to which that
law attaches no meaning. The tables of this section are reading tests of
this law (`zikaron/2` §11) and never `zikaron/2` entry predicates: a byte
string of this type that passes `zikaron/2` §4.3 is an entry for every
verifier, this law's included, whatever §2.2 finds in its body.

**[F] 2.2 Seven shapes, closed.** The body of a face entry has one of seven
**shapes** or none. A body has a shape when it is an object with exactly the
members of that shape's row, each carrying the form named for it, the
optional member present or absent:

| shape | members |
|---|---|
| `home` | `act`: the string `home`; `root`: hex20; `chain`: int |
| `account` | `act`: the string `bind`; `root`: hex20; `kind`: the string `account`; `platform`: label; `handle`: handle; `note_md`: optional, a string |
| `key` | `act`: the string `bind`; `root`: hex20; `kind`: the string `key`; `address`: hex20; `role`: label; `sig`: hex65; `note_md`: optional, a string |
| `persona` | `act`: the string `bind`; `root`: hex20; `kind`: the string `persona`; `name_md`: name |
| `release` | `act`: the string `release`; `root`: hex20; `subject`: hex32; `note_md`: optional, a string |
| `reroot` | `act`: the string `reroot`; `root`: hex20; `to`: hex20, byte-unequal to `root`; `note_md`: optional, a string |
| `origin` | `act`: the string `origin`; `root`: hex20; `from`: hex20, byte-unequal to `root`; `note_md`: optional, a string |

The closure of this section is a reading test of this law (`zikaron/2`
§6.9), and `zikaron/2` §6.10 keeps every output of that law indifferent to
it: under this law a body bearing a member beyond its row, lacking a
required member, or carrying a member in another form has no shape, and so
has a body whose `act` or `kind` is any other value; a `reroot` body whose
`to` is byte-equal to its `root`, and an `origin` body whose `from` is
byte-equal to its `root`, have no shape either. `note_md` is a prose string
(`zikaron/2` §3.3), read by no predicate of this law, and this law sets no
bound on its length. The `persona` shape carries no `note_md`. The seven
rows share no body: `act` and `kind` decide the row.

**[F] 2.3 What a shape says.** A `home` entry says that the lineage of the
seat named `root` is read on the chain whose id is `chain` (§7). An
`account` entry says that the account `handle` on the platform `platform` is
a face of the seat named `root`; a `key` entry says that the address
`address`, in the role `role`, is a face of that seat, and carries in `sig`
that address's own consent (§3). A `persona` entry says that the seat named
`root` goes by the name `name_md`. A `release` entry says that the binding
made by the entry whose `entry_id` is `subject` ends. A `reroot` entry says
that the seat named `root` moves its faces to the root `to`: its faces are
from then on the faces bound under `to`, and from the step after the record
that seats the entry its own table takes no binding, no release, no home and
no origin (§7.2). An `origin` entry says that the seat named `root` took its
faces up from the root `from`. A `reroot` entry and an `origin` entry name
no face. The **face** an `account` entry names is the pair (`platform`,
`handle`); the face a `key` entry names is the pair (`address`, `role`); the
face a `persona` entry names is its `name_md`; two entries name one face
exactly when they have one shape and their pairs, or their names, are
byte-equal member by member, so `Alice` and `alice` are two handles and two
faces.

**2.4 Spellings in use.** `platform` and `role` are open: any label is a
legal spelling, and this law keeps no register. Spellings in use at this
law's writing, stated so that writers converge and binding no reader: for
`platform`, `x`, `telegram`, `github`, `youtube`, `wechat`, and `web`, whose
`handle` is a host name; for `role`, `payout`. A writer spells `handle` as
the platform spells the account's unique name; normalization is the writer's
work, and this law compares the bytes it is given. A writer spells `name_md`
in Unicode Normalization Form C: one name in two forms is two faces under
this law.

## 3. The key's consent

**[F] 3.1 Declaration.** For a root `r`, an address `a` and a label `l`, the
**declaration** `T(r, a, l)` is the byte string of four lines joined by
single line feed bytes `0x0A`, with no line feed after the last:

```
zikaron.face/1
root:<r>
address:<a>
role:<l>
```

where `<r>` and `<a>` are the `hex20` spellings, `0x` and forty lowercase
hexadecimal digits, and `<l>` is the label's bytes. The declaration is ASCII
throughout.

**[F] 3.2 Message and digest.** The message is `M = D || 0x0A ||
hex32(sha256(T))`, where `D` is the fourteen bytes `zikaron.face/1`: a
message of `zikaron/2` §5.2's shape under a domain literal that law does not
spell, 81 bytes long. The digest is `zikaron/2` §5.3's, computed over this
`M`, whose `decimal(len(M))` is `81`. By `zikaron/2` §5.6 no byte string is
a message under this domain and under that law's: a signature over a
declaration is no entry signature, and an entry signature is no consent.

**[F] 3.3 The consent holds.** For a body of the `key` shape, the consent
**holds** when `sig`, read as `zikaron/2` §5.4 reads a signature (65 bytes
`r || s || v`), passes that section's four tests (`v` in {27, 28}; `r` and
`s` in [1, n − 1]; `s` at most (n − 1) / 2; recovery succeeds) over the
digest of §3.2 for `T(root, address, role)`, built from the body's own three
members, and the address recovered as `zikaron/2` §5.5 recovers one is
byte-equal to `address`. Where any of these fails the consent does not hold,
and this law distinguishes no further among the failures. The declaration
names the root, so a consent given to one root holds under no other.

**3.4 Producer obligations.** The key that consents signs as `zikaron/2`
§5.7 obliges: RFC 6979 nonces, low-s by negation, so that one key consents
to one declaration with one `sig`. Beside its transactions, a key of this
ecosystem signs messages of `zikaron/2` §5.2's shape and no other message
(`zikaron/2` §5.7), which is why the consent has that shape. The tool that
signs builds `M` from the root, the address it holds the key of, and the
role, and takes no message from a caller; an address that serves a wallet
and answers applications' signing requests gives its consent through such a
tool alone: a request from an application to sign a message of §3.2's shape
is a request to bind the address to a root the application chose, and the
hash in it shows nothing of that root. A consent carries no term and no
counter, and the key that gave it has no act of this law that withdraws it
(§9).

## 4. The marker

**[F] 4.** The **marker** of a root `r` is the byte string `zikaron:`
followed by the `hex20` spelling of `r`. An account shows it in public on
its platform so that a reader who starts from the account finds the root.
This law fixes the spelling and nothing else about it: whether a platform's
page shows the marker is for the reader to see there (§9).

## 5. Bytes carried with the anchor

**[F] 5.1 Carried.** For a byte string `c`, read as a transaction's
calldata, `carried(c)` is the entry `c[36:]` when all of these hold, and
nothing otherwise: `c` is longer than 36 bytes; its first four bytes are
`0xeecdf927`, the selector of `anchor(bytes32)`; `sha256(c[36:])` is
byte-equal to `c[4:36]`; and `accept(c[36:])` yields an entry. The entry
carried is any entry, whatever its type.

**[X] 5.2 Sending and collecting.** A producer sends each face entry in a
transaction of its own, from the key that signed the entry, to a registry,
the calldata being the selector of `anchor(bytes32)`, the entry's
`entry_id`, and the entry's bytes, in that order: the entry travels with its
anchor, and a reader who holds nothing reads it from the chain. A reader
collects `carried(c)` from the transaction of each anchor record it holds,
`c` being that transaction's top-level calldata, and puts what it collects
in the pile. Calldata of another selector, `anchorMany(bytes32[])` among
them, carries nothing under this law.

**[X] 5.3 What the carried bytes change.** Nothing about the anchor. Whether
the transaction anchors its hash, and with which verdict, is `zikaron/2`
§9.1 and §9.3 alone, which read the hash at its aligned offset and the
registry's log. A registry whose code emits the form once, for the one word
`anchor(bytes32)` takes, anchors that word and nothing after it; a declared
registry whose code emits the form for further words of its calldata anchors
those words as `zikaron/2` §9.1 says, and a producer carries an entry only
to a registry whose code it has read. An entry counts by its `entry_id`
wherever its bytes came from: a ledger, a record package, another reader, or
the chain. A reader refuses no entry for having come from elsewhere than the
chain, and one entry in hand twice is one entry.

## 6. The face input

**[R] 6.1 Five members.** A **face input** is a value of `zikaron/2` §3's
universe that is an object with exactly the members `root`, `chains`,
`anchors`, `accessions` and `pile`:

- `root` is a hex20: `R`.
- `chains` is the **basis**: an array of objects, each with exactly the
  members `chainId` (int), `fromBlock` (int), `toBlock` (int), `registries`
  (an array of hex20), `book` (a hex20 or `null`) and `bookFromBlock` (an
  int or `null`), with `fromBlock` ≤ `toBlock`, `bookFromBlock` `null`
  exactly when `book` is `null`, and `bookFromBlock` ≤ `toBlock` where it is
  an int. The array is strictly ascending by `chainId`, compared
  numerically, so a chain has one object; `registries` is bytewise strictly
  ascending. An object declares, for one chain, the registries whose logs of
  `zikaron/2` §9.1's form were read over the blocks `fromBlock` to `toBlock`
  inclusive, and the succession book, where it names one, whose logs of the
  accession form (§6.3) were read over the blocks `bookFromBlock` to
  `toBlock` inclusive.
- `anchors` is an array of **anchor records**, each an object carrying the
  seven members of `zikaron/2` §9.2 in the forms that section names:
  `chainId`, `blockNumber`, `blockTimestamp` (ints), `tx` and `hash`
  (hex32), `sender` (hex20), `verdict` (`counted`, `UNPROVEN` or `VOID`). A
  record may carry members beyond the seven, as `zikaron/2` §9.4 lets it,
  and no predicate of this law reads them. Its `chainId` is a chain of the
  basis and its `blockNumber` lies in that chain's `fromBlock` to `toBlock`.
- `accessions` is an array of **accession records**, each an object with
  exactly the members `chainId`, `blockNumber`, `blockTimestamp`, `txIndex`,
  `logIndex` (ints), `tx` (hex32), `holder`, `to` and `root` (hex20). Its
  `chainId` is a chain of the basis whose `book` is a hex20, and its
  `blockNumber` lies in that chain's `bookFromBlock` to `toBlock`, a range
  read apart from the registries' and free to begin below `fromBlock`.
- `pile` is an array of strings, each spelling one byte string as the two
  bytes `0x` followed by an even number of hexadecimal digits of either
  case, the empty byte string being `0x`; this spelling is a transport's and
  none of `zikaron/2` §1's hex forms.

**[R] 6.2 Valid.** A face input is **valid** when it has the form of §6.1
and, further: no two elements of `anchors` agree on `chainId`,
`blockNumber`, `tx` and `hash` and differ in `blockTimestamp`, `sender` or
`verdict`, two elements that agree on the seven being one record, whatever
members either carries beyond them, as `zikaron/2` §9.4 holds an anchor set;
no two elements of `accessions`, an element given twice included, agree on
`chainId`, `blockNumber`, `txIndex` and `logIndex`; and the records keep one
clock per chain, so that for any two records among the anchor records and
the accession records together that share a `chainId`, equal `blockNumber`s
carry equal `blockTimestamp`s and the smaller `blockNumber` carries a
`blockTimestamp` no greater than the other's. `zikaron/2` §6.10 reaches no
object of this section, and the anchor record alone is open to members
beyond its own. On anything else, a value that is no object of §6.1's form
included, the reading of §7 yields the one answer `NO_TABLE`. The order of
`anchors`, `accessions` and `pile` reaches no output. A byte string of the
pile that `accept` rejects is no entry and reaches no output; the **entries
of the pile** are the byte strings of it that `accept` accepts, two that are
byte-equal being one.

**[X] 6.3 Reading the chains.** An anchor record records an anchor under
`zikaron/2` §9.1 on a chain of the basis, emitted by an address that chain's
object declares in `registries`, in a block of its range, with the members
and the verdict `zikaron/2` §9.2 and §9.3 give it. The anchors of one
`(chainId, blockNumber, tx, hash)` give one record whichever declared
registry emitted them and however often, as `zikaron/2` §9.4 holds an anchor
set: two logs of one transaction that carry one hash are one record. An
accession record records a log of the **accession form**: a log in the
receipt of a transaction with status 1, whose `address` field, read as
`zikaron/2` §9.1 reads it, is the `book` of that chain's object, in a block
of the book's range, carrying exactly four topics and empty data, topic 0
byte-equal to `keccak256("Acceded(address,address,address)")` and topics 1,
2 and 3 each 20 bytes preceded by 12 zero bytes; the record's `holder` is
topic 1, its `to` topic 2 and its `root` topic 3, its `txIndex` the
transaction's index in its block, its `logIndex` the log's index among all
the logs of its block, counted from zero over the block's receipts in
transaction order, its `tx` the transaction's hash, and its `blockTimestamp`
the timestamp of the block's header. Topic 3 is the book's own statement of
the seat a hand-over belongs to: the root under which the book holds the
record of the key that hands on. The **keys in reach** of an input are `R`
and the `to` of every accession record whose `root` is `R`. A conforming
input carries an accession record for every log of the accession form, in
every declared book's blocks, whose topic 3 is `R`, and an anchor record for
every anchor a declared registry emits in its chain's range whose
transaction sender is a key in reach. On a valid input the records beyond
these move no office, open no home, seat nothing and list nothing, since an
accession record under another root moves nothing (§7.2) and a key out of
reach never holds the office of §7.2 or joins the line; what the anchor
records among them add is the bytes their transactions carry (§5.2), which
reach the table only as entries of the pile (§5.3). The basis's `fromBlock`
is at or before the first block at whose end each declared registry carried
code, and `bookFromBlock` at or before the first block at whose end the book
carried code, a range from block 0 serving in every case. A reader that did
not obtain every such log has not read the basis and produces no face input
under it, as `zikaron/2` §9.4 says of a scan it could not complete; it
narrows the basis to what it read, lowering a `toBlock` or leaving a chain
out. A face input is read to one moment: the reader fixes a time that every
chain of the basis has passed, a block of each chain carrying a later
timestamp, and ends each chain's range at that chain's last block whose
timestamp is at or before that time, a chain with no such block being left
out, so that a reader that narrows one chain narrows every other chain of
the basis to the same time. The law reads the two forms and never a
contract's code: which addresses a reader names as registries and which as a
book are the reader's declarations.

## 7. The reading

Under a valid face input the reading is one walk over what the chains
recorded, in one order, followed by what the walk leaves standing.

**[R] 7.1 Steps.** A **step** is a pair (`chainId`, `blockNumber`) for which
the input carries at least one anchor record or accession record; its time
is the `blockTimestamp` its records share (§6.2). Steps are ordered by time,
then `chainId`, then `blockNumber`, each compared numerically and ascending:
a total order, in which the steps of one chain keep that chain's block
order, and steps of two chains at one second go by `chainId`.

**[R] 7.2 The walk.** The walk keeps five things: the **office**, a key, at
first `R`; the **home**, a chain id or nothing, at first nothing; the
**line**, a list of keys, at first `R` alone; for each chain of the basis,
the **head** of that chain's book, a key, at first `R`; and the **seal**,
open or closed, at first open. It visits the steps in order, and at each
step does four things in turn:

1. *Anchors.* Every anchor record of the step is read against the office,
   the home, the heads and the seal as the step found them, so the anchor
   records of a step are read in no order among themselves. A record is **in
   office** when the step found the seal open, its `verdict` is `counted`
   and its `sender` is the office.
2. *Home.* Where the step holds an **opening**, the home becomes the step's
   `chainId`. An opening is an anchor record of the step that item 1 found
   in office, whose `hash` is the `entry_id` of an entry of the pile that is
   a face entry of the `home` shape, with `root` byte-equal to `R`, `chain`
   equal to the step's `chainId`, and an `author` that is the office as the
   step found it, where the head of the step's chain, as the step found it,
   is the office too.
3. *Accessions.* The accession records of the step are taken in ascending
   order of `txIndex`, then `logIndex`, and a record whose `root` is other
   than `R` does nothing. A record whose `root` is `R` and whose `holder` is
   the office at that moment, where the home as item 2 left it is nothing or
   is the step's `chainId`, **moves the office**: the office becomes the
   record's `to`, and `to` is appended to the line with the record, also
   where `to` is the office itself. Every record whose `root` is `R` then
   makes its `to` the head of the step's chain, whoever its `holder`.
4. *Seal.* Where the step holds an anchor record seated (§7.3) for a
   `reroot` entry that reads (§7.4), the seal closes, and a closed seal
   stays closed: the seat is **sealed** from the next step on.

So an accession in a block moves nothing for the anchors of that block; a
`home` entry takes effect for the accessions of its own block, which are
read under the home it opens, and for anchors from the step after its own.
The head of a chain is what that chain's book alone says: the `to` of its
last record under `R`, and `R` where it has none, a chain whose basis names
no book keeping `R`. An opening needs that key to be the office: a home
opens on a chain whose book holds the key in office under `R`, so that from
the opening on, in a book that records a hand-over only from the key it
holds, the office is the head of the home's book at every step. Before any
opening the home is nothing and the accession records of every declared book
are read together in the one order: the first hand-over under `R` by the
office, on whatever chain, moves the office, and a later hand-over by a key
the office has left, on any chain, moves nothing while the office stands
elsewhere. Once a home stands, the accession records of the home chain alone
move the office. A reroot takes effect from the step after its own: the
records of its own step, before or after the reroot's record in it, are read
as that step found the seal. In a sealed seat no record is in office, so
none is seated and no home opens, while item 3 goes on as before: the
accession records move the office, the line and the heads of a sealed seat
as they move those of an open one.

**[R] 7.3 Seated.** An anchor record is **seated for** a face entry `e` of
the pile whose body has a shape when its `hash` is `e`'s `entry_id` and, at
its step: it is in office; `e`'s `author` is the office as the step found
it; and, where `e` has the `home` shape, `e`'s `chain` is the step's
`chainId` and the head of that chain as the step found it is the office, and
otherwise the home as the step found it is the step's `chainId`. An opening
is thus an anchor record seated for a `home` entry whose `root` is `R`, and
every seated record is one the entry's own author sent while it held the
office. Seated records of one entry are ordered by `blockTimestamp`,
`chainId`, `blockNumber`, each numerically, then `tx` bytewise; the least is
the entry's **moment**.

**[R] 7.4 Standing.** A face entry `e` of the pile **reads** when its body
has a shape, its `root` is byte-equal to `R`, and, for the `key` shape, its
consent holds (§3.3). An entry that reads and has a seated record **stands**
where its shape is `home`, `account`, `key`, `persona`, `reroot` or
`origin`; a `release` entry stands where it reads, has a seated record, and
its `subject` is the `entry_id` of a standing `account`, `key` or `persona`
entry. A standing `account`, `key` or `persona` entry is a **binding**.
Among the standing `reroot` entries, the one whose moment is least in §7.3's
order, the lesser `entry_id` where two moments are equal in it, is the
**reroot** of the reading; among the standing `origin` entries, the one
chosen in the same way is its **origin**. The reading has no reroot where no
`reroot` entry stands, and no origin where no `origin` entry stands; it has
a reroot exactly where the walk closed the seal (§7.2). A standing `reroot`
or `origin` entry that is not so chosen appears in no member of the table.

**[R] 7.5 Spans, rows, state.** Each binding gives one **span**. It begins
at the binding's moment. Among the standing `release` entries that name the
binding, the one whose moment is least in §7.3's order, wherever it falls
against the binding's moment, the lesser `entry_id` first where two moments
are equal, is the release that **ends** the span: the span ends at that
release's moment, or at the binding's own moment where the release's moment
precedes it in §7.3's order, `tx` included. Where no standing release names
the binding, the span is open. A standing release that ends no span appears
in no member of the table. The bindings that name one face (§2.3) form one
**row**, whose **state** is `bound` where at least one of its spans is open
and `released` otherwise. Binding one face again adds a span to its row, and
releasing one binding of several leaves the others as they stand. The
**persona** of the reading is read from the rows whose bindings have the
`persona` shape: among their open spans, the one whose beginning is greatest
in §7.3's order, the greater `entry_id` of its binding where two beginnings
are equal in it, **rules**, and the persona is the name of its binding;
where no such span is open the reading has no persona. So a seat changes its
name by binding another, falls back to the name it bound before by releasing
the later, and has none once every one is released.

**[R] 7.6 Misread.** A face entry of the pile that does not stand, and whose
`entry_id` is the `hash` of an anchor record whose `sender` is a key of the
line as the walk left it, is **misread**, whatever that record's verdict and
at whatever step it sits, with one reason, the first of these that applies:

1. `SHAPE`: its body has no shape.
2. `ROOT`: its `root` is other than `R`.
3. `AUTHOR`: its `author` is no key of the line as the walk left it.
4. `SIGNATURE`: it has the `key` shape and its consent does not hold.
5. `SUBJECT`: it has the `release` shape and its `subject` is the `entry_id`
   of no binding.
6. `UNSEATED`: it has no seated record.

Entries of other types, byte strings that are no entries, and face entries
that no key of the line anchored are outside this list, so a party that
holds no key of the line adds nothing to it. Item 3 reads the line the walk
left and §7.3 the office each step found: an entry a key of the line signed
that has no record its author sent in office, whatever a predecessor or a
successor of that key anchored, has no seated record, and where no earlier
reason applies it is `UNSEATED`.

**[R] 7.7 Unread and unproven.** The **unread** count is the number of
distinct `hash` values among the anchor records that are in office at their
step (§7.2) and whose `hash` is the `entry_id` of no entry of the pile. It
counts what the seat anchored and the reader holds no entry for, bytes that
`accept` rejects included, entries of every type together: ordinary entries
of a ledger travel as hashes, so the count is seldom zero, and it bounds
what the table may be missing where a producer anchored a face entry without
its bytes. The **unproven** count is the number of anchor records, on every
chain and each record once, so that two records of one `hash` count two,
whose `verdict` is `UNPROVEN`, whose `sender` is the office as their step
found it, and whose step found the seal open: records the reader could not
read either way (`zikaron/2` §9.3), so that a table whose count is above
zero may differ in every member but `root`, `marker` and `basis` from the
table a reader holding the states of those blocks reads.

**[R] 7.8 The table.** The reading yields the **face table**, an object with
exactly these members:

- `root`: `R`.
- `marker`: the marker of `R` (§4), a string.
- `basis`: the input's `chains`, the same values in the same order.
- `home`: the home as the walk left it, an int, or `null` for nothing.
- `office`: the office as the walk left it, a hex20.
- `persona`: `null` where the reading has no persona; otherwise an object
  with exactly `entry`, the `entry_id` of the binding whose span rules
  (§7.5), and `name_md`, that binding's name.
- `reroot`: `null` where the reading has no reroot; otherwise an object with
  exactly `entry`, the `entry_id` of the reroot (§7.4), `to`, that entry's
  `to`, and `since`, that entry's moment, written as an object with the
  `chainId`, `blockNumber`, `blockTimestamp` and `tx` of that seated record.
- `origin`: `null` where the reading has no origin; otherwise an object with
  exactly `entry`, the `entry_id` of the origin (§7.4), `from`, that entry's
  `from`, and `since`, that entry's moment, written as the `since` of
  `reroot` is.
- `lineage`: an array, the line in order. Its first element is
  `{"accession":null,"key":R}`; each later element is an object with `key`,
  the key appended, and `accession`, an object with the members `chainId`,
  `blockNumber`, `blockTimestamp`, `txIndex`, `logIndex`, `tx` and `holder`
  of the record that moved the office to it; the record's `root`, which is
  `R`, enters no element.
- `homes`: an array of the openings, one element for each opening record, an
  entry opened twice appearing twice, each an object with `entry` (the
  `home` entry's `entry_id`) and the opening record's `chainId`,
  `blockNumber`, `blockTimestamp`, `tx` and `sender`, in §7.3's order, the
  lesser `entry` first where two records are equal in it.
- `faces`: an array of rows. An account row has exactly `kind` (`account`),
  `platform`, `handle`, `state` and `spans`; a key row has exactly `kind`
  (`key`), `address`, `role`, `state` and `spans`; a persona row has exactly
  `kind` (`persona`), `name_md`, `state` and `spans`. Account rows come
  first, ordered bytewise by `platform` then `handle`; key rows follow,
  ordered bytewise by `address` then `role`; persona rows come last, ordered
  bytewise by the UTF-8 of `name_md`. `spans` is an array of objects with
  exactly `entry` (the binding's `entry_id`), `since`, `release` and
  `until`: `since` is the binding's moment, written as an object with the
  `chainId`, `blockNumber`, `blockTimestamp` and `tx` of that seated record;
  `release` is the `entry_id` of the release that ends the span, and `until`
  that release's moment in the same form, or the binding's moment where §7.5
  says the span ends there; both are `null` for an open span. Spans are
  ordered by `since` in §7.3's order, then by `entry` bytewise, the lesser
  first.
- `misread`: an array of objects with exactly `entry` and `reason`, ordered
  bytewise by `entry`.
- `unread`: the unread count, an int.
- `unproven`: the unproven count, an int.

On an input that is not valid the reading yields `NO_TABLE` and nothing
else.

**[R] 7.9 Joined.** Under two valid face inputs, the first is **joined** to
the second when the table of the first carries a `reroot` that is an object
whose `to` is byte-equal to the `root` of the table of the second, and the
table of the second carries an `origin` that is an object whose `from` is
byte-equal to the `root` of the table of the first. Each table is read under
its own input, and the two inputs need share nothing. Where either input is
not valid the answer is `NO_TABLE`.

## 8. Tokens and vocabularies

The closed vocabularies of this law: the seven shapes `home`, `account`,
`key`, `persona`, `release`, `reroot`, `origin`; the two states `bound`,
`released`; the six reasons `SHAPE`, `ROOT`, `AUTHOR`, `SIGNATURE`,
`SUBJECT`, `UNSEATED`; the answer `NO_TABLE`; the three verdicts of an
anchor record, which are `zikaron/2`'s. The rejection tokens of `accept` are
`zikaron/2` §10's. Labels are open (§2.4).

## 9. Outside the grammar, and the openings by name

The following are decided by readers and by the seats themselves, and the
openings are named so that nobody reads the table for more than it says.

- **An account row is half a proof.** The table says that the seat claims
  the account. Whether the account claims the seat is the marker on the
  platform's page, which the reader goes and sees; a seat can list an
  account it does not hold, and the row alone says nothing against that. A
  key row carries both halves: the seat's claim and the key's consent.
- **A persona is a name and no identity.** Any seat binds any name, and two
  roots may go by one name: a persona row is the seat's word about itself
  and says nothing of who else bears the name. A reader tells seats apart by
  their roots; whatever is kept as evidence names the root and the address,
  and the name is for showing beside them. The barred scalar values (§1)
  keep a name to one unbroken run on one line; letters that look alike,
  combining marks and scripts that run from right to left stay, so a reader
  shows a name as text set apart from what surrounds it, and what a name
  looks like is the reader's to show (`zikaron/2` §11). A persona has no
  platform and no marker.
- **The consent is the key's, the binding is the seat's.** A face stands and
  ends by the seat's entries alone. The key that consented has no act of
  this law that ends the binding, and its consent carries no term: after a
  release the seat may bind the same address again with the same `sig`, and
  the new binding stands. A key consents to a root it is willing to be bound
  by for as long as that seat chooses, and a tool that signs consents limits
  the roots it signs for.
- **Addresses that cannot sign.** A consent is a secp256k1 signature that
  recovers to the address. A contract's address, a company's or a shared
  wallet's among them, gives none, and is no key face under this law.
- **Two rails, read apart.** Who holds the office is read from the
  succession book (§7.2). `zikaron/2` §7.3 and §7.4 read authority and
  lineage from the ledger's own successions, their positions, and the block
  times of the records that anchor them, and that law's §11 leaves a
  disagreement between the two rails to a reader's law, which this one is.
  Each law reads its own rail. The office here moves by accessions alone
  (§7.2), an accession moving nothing for the anchors of its own block,
  whatever successions the ledger holds and wherever their anchors fall;
  `zikaron/2` holds a key in authority and in its lineage by the ledger's
  successions and their anchors, whatever a book records. So a record or an
  entry may count under one law and not under the other: a record of the
  outgoing key after its succession's anchor may be seated here and
  discarded there (its §8.1); one after the accession and before that anchor
  may seat nothing here and be counted there, as may a record of the
  incoming key in the block of its accession; an entry the incoming key
  signs at a ledger position before the succession that names it may be
  seated here and an `AUTHORITY_MISMATCH` there (its §8.2), which breaks the
  ledger; and a key the book seats may hold the office here while no
  succession of the ledger names it, its entries `EXCLUDED` there (its
  §8.6). A seat narrows the difference by its order: each succession written
  at its place in the ledger and anchored, the accession at once after it in
  the book of its home, nothing signed under the outgoing key past its
  succession and nothing sent under it from that anchor on, nothing sent
  under the new key before the block after its accession. This law reads the
  block times the chains wrote, whatever the order in which a seat sent
  (§7.1).
- **The home's book is the seat's to keep.** Once a home stands, an
  accession on another chain moves no office here and sets only the head of
  its own chain's book. A seat that changes its key in other books and
  leaves its home's book behind leaves its old key in office under this law,
  and whoever holds that key binds, releases and reroots in the seat's name.
  Changing the key in the home's book is the seat's own step, and no rule of
  this law takes it for the seat.
- **Before a home.** No binding stands before an opening, since a binding's
  record is seated only on the home (§7.3). Until the first opening the
  office is read against every declared book together, so a key that has
  handed its seat on, on any chain, opens nothing anywhere while the office
  stands elsewhere, and its hand-over to another key on another chain in
  that time moves no office; it does set the head of that chain's book, and
  the office opens no home there while that book names another key. A seat
  opens its home before it binds, on a chain whose book holds its key in
  office, and a root without a home has no faces.
- **Moving the home.** An opening (§7.2) on another chain, a `home` entry
  the key in office signed and anchors there while that chain's book holds
  that key under `R`, moves the home there: for the hand-overs of that block
  at once, for anchors from the next step. An opening that finds the chain's
  book holding another key under `R` opens nothing and leaves the home where
  it stood, and nothing but the table shows it; the book of that home still
  moves the office, so the holder of a key the seat goes on to retire
  elsewhere hands the seat on there. A seat reads its table after it opens
  or moves a home, and hands over in the book of the home the table shows.
  The entry is read against the office as the chains' clocks order it: the
  reader sets the block time of the opening beside the block times of the
  accessions in the home it leaves, and the home it leaves has no say and
  need not accept a transaction. A seat brings the book of the chain it
  moves to in step first, so that it holds the key in office under `R`, by
  the root's hand-over to that key or by the hand-over of an earlier key
  that book holds, and a seat that begins to anchor on a chain anchors there
  what `zikaron/2` §5.7 asks of a chain added to a basis. After the move the
  old home's records seat no binding and no release, and the bindings made
  there stand as they stood; an opening on the old home moves the home back
  there, once that chain's book holds the key in office.
- **One second.** Steps of two chains at one `blockTimestamp` go by
  `chainId`. The order is total and arbitrary there, as any order across two
  chains' clocks is; a seat that hands over and a holder of the outgoing key
  that acts in the same second on another chain are ordered by it, and
  before that second the outgoing key was in office in any case, as far as
  the clocks the reader accepts say.
- **The reader's chains.** A table is relative to its basis, which it
  carries, and the walk sets chains side by side by the block times their
  producers write (§7.1). A reader declares the chains whose blocks and
  block times it accepts as the seat's history. A chain whose blocks a
  holder of a key once in office produces lets that holder date a hand-over
  or an opening there inside the key's tenure; two chains of the basis whose
  clocks disagree let that holder act on the one that runs behind, within
  their difference after a hand-over on the other, and be read inside the
  key's tenure; either way the holder takes the seat in every table whose
  basis holds those chains, and a seat's own act sent within that difference
  reads under the key it handed on from. A reader that reads its chains to
  one moment (§6.3) reads the table of that moment, and that moment is no
  later than the least advanced chain of the basis: a chain that stops, or a
  node that withholds a chain's latest blocks, holds the whole table at an
  earlier time, and a reader that wants a later one leaves that chain out,
  with what this bullet says of a chain left out; a reader that reads one
  chain to an earlier time than another reads the later chain's acts against
  an office and a home the earlier chain has since moved, and a key retired
  there after its range opens homes, moves the office, binds, releases and
  reroots in that reader's table. A reader that leaves out the chain where a
  seat's home now stands reads the seat as if the home had stayed where it
  was, and a key retired since in the new home's book may still seat records
  on the old one in that reader's table; before a home a reader that leaves
  out a chain, or names no book on it, may read another lineage. A reader
  that names no book on a chain reads `R` as the head of that chain: an
  opening there by another key is none in that reader's table, the home
  stays where that reader last read it, and what the seat binds on the chain
  it moved to is unseated there. Where that chain is the home, opened by
  `R`, the office stands at `R` in that reader's table whatever that chain's
  book has recorded since, and the holder of the root key binds, releases
  and reroots there. The basis says what the reader read.
- **A key in office that is stolen.** Whoever holds the key in office holds
  the seat until it is handed on: it binds, releases, moves the home to a
  chain whose book holds that key, moves the seat's faces by a reroot to a
  root of its own choosing or names the seat's origin where it has none,
  neither of which any act of this law undoes, and hands the seat on, and
  its counted anchor records in the block of the hand-over are in office
  wherever they sit in that block. Every key a reader's table holds in
  office acts there with the whole of the office, whichever opening of this
  section put it there: where a key the seat retired holds the office in
  some reader's table, it may reroot the seat in that table, and the reroot
  stands in that table whatever the seat does after, and joins the seat
  there to a root that answers it (§7.9). A seat whose outgoing key may be
  in other hands hands over first in the book of its home: an opening read
  after that hand-over opens nothing, and until that hand-over the holder of
  the outgoing key may hand the seat on in that book before the seat does. A
  hand-over on a chain the home has left moves no office. This law reads
  what the office did and cannot tell whose hand moved it.
- **A book the seat has not entered.** A book names with each hand-over the
  root its holder stands under there, and this law reads the hand-overs
  under `R` alone (§7.2). `R` stands under `R` in a book until it accedes
  there to another key's declaration; from then on that book names another
  root for `R` and for every key that follows it, holds no record under `R`
  and never will: a seat whose office has left `R` opens no home there, and
  one whose office is `R` opens a home there whose book moves the office no
  more. A book holds a key under `R` when its last accession whose third
  topic is `R` names that key as `to` or, where it has none, when the key is
  `R`; a book's own statement of the key that acts for `R` gives that key
  only where the book still names `R` as the root of `R`, and a seat that
  tests a book reads both. A key other than `R` that a book never seated
  under `R` stands there under another root, its own address where the book
  seats a newcomer under itself, so a hand-over such a key makes in that
  book is a record of that other root: it moves no office and sets no head,
  and the seat opens no home on that chain while its book holds another key
  than the office under `R`. A book comes to hold the key in office by the
  root's own hand-over there or, where it holds an earlier key of the seat,
  by that key's hand-over to the key in office; a book of the family's code
  takes an accession only from a key that has entered nothing in it, and a
  declaration only from the key that holds the record. It comes to hold it
  only while the root key, or that earlier key, is the seat's alone: whoever
  else holds it declares there and accedes with a key of its own before the
  seat's key accedes, or accedes there with the root key to a declaration of
  its own, and keeps that book from the seat for good. A seat therefore
  makes its root's first hand-over in every book it may use and keeps every
  such book in step with its home's; a seat whose root key is gone or in
  other hands enters no further book, and keeps its home where a book holds
  its key in office, or moves its faces by a reroot (§7.2) to a root that
  every book of the family's code holds under itself.
- **A reroot is half a proof, and so is an origin.** A table's `reroot` says
  that the seat's key in office named another root as the one to which the
  seat moves its faces; a table's `origin` says that the seat's key in
  office named another root as the one its faces came from. Each is one
  seat's word about another, and the other root has no part in it: a seat
  can name as its origin a root that never named it, and reroot to a root
  that never answered. What sets the two together is §7.9: where the first
  is joined to the second, the key each table holds in office named the
  other, each read under its own input. A table carries one origin, so of
  two roots that reroot to one root, one at most is joined to it under one
  input of that root; under another basis that root's table may carry
  another origin. A reader shows a reroot or an origin that is not joined as
  a claim, and reads the other root's table before it shows more. Joined is
  read under two bases, as every table is read under one (see «The reader's
  chains»): a reader whose basis leaves out the chain where a seat's home
  now stands reads the seat on the home it left, and may read there a reroot
  that a key since retired signed, joined to a root that key's holder keeps,
  so a reader follows a reroot under a basis that holds the seat's home. A
  reader that follows reroots from root to root sets its own bound on their
  number and stops at a root it has read.
- **A sealed seat.** From the step after its reroot is seated, a seat binds,
  releases and opens nothing under this law, and no act of this law undoes
  the reroot: a `release` that names it names no binding (§7.4), and a
  `reroot` entry all of whose records fall after the seal closed has no
  seated record (§7.3). What this section says a key in office binds,
  releases, opens, seats, reroots or names as an origin, it says of a seat
  whose seal is open; the records of a sealed seat's key in office seat
  nothing, and its hand-overs in the book of the home move the office as
  before (§7.2). The rows of a sealed table say what stood when the seal
  closed, since no later record opens a span or ends one; what the
  identity's faces are now is read in the table of the root it moved its
  faces to, where the seat binds them anew and shows the new root's marker
  on each account's page (§4), each key that consented under the old root
  consenting again under the new, since a consent names its root (§3.3). The
  seal is read like every other act of the office. A reader in whose table
  the record seats no reroot reads no seal from it: one whose basis leaves
  out the home or ends before the record, one whose node withheld the
  record's log or could not prove its state (`UNPROVEN`), one that holds no
  bytes for the entry, and one whose basis holds another key in office at
  that step; the counts of §7.7 hint at the `UNPROVEN` record and at the
  missing bytes, and at none of the others.
- **What a reroot leaves with the old root.** A reroot moves faces and
  nothing else. `zikaron/2` and `zikaron.kit/2` read a `reroot` entry as
  they read any face entry (see «What this law leaves as it found») and
  attach no meaning to it: the old root's ledger, the grants it published
  and the grants that name it are read as before, under the old root and by
  the key those laws hold in office there, and the new root's ledger is
  another ledger. The office and the line of a sealed table therefore go on
  following the book of its home (§7.2; see «The kit's reading of the same
  book»). A seat keeps the old root's key in office, and hands it on in its
  books as before, for as long as that ledger and its grants matter to it;
  whoever comes to hold that key signs for that ledger and may withdraw
  under it the grants the seat published (`zikaron/2` §6.4), so a seat that
  lets that key go leaves the old ledger to whoever finds it, and a seat
  that is done with the key hands the old seat on to a key it keeps. The new
  root's table says nothing for what the old ledger takes on after the
  reroot.
- **The root a seat moves its faces to.** A seat reroots to a root made for
  the purpose, whose key has acceded in no book to another key's
  declaration, so that no book of the family's code holds that key under
  another root (see «A book the seat has not entered»): such a root, while
  it has handed nothing on, opens its home on the chain it chooses, and
  afterwards keeps its books in step as every seat does. This law tests none
  of that. A reroot and an origin are seated on the home like a binding
  (§7.3), so a seat without a home makes neither, and each is a record on
  the chain that is the home at its step. A seat whose home's chain takes
  its transactions no longer first moves its home (see «Moving the home») to
  a chain whose book holds its key in office, bringing that book in step
  where it still can, and reroots there.
- **A ledger that continues an earlier one.** A ledger that continues a
  `zikaron/1` ledger keeps that ledger's root as `R` (`zikaron/2` §7.3), and
  the office starts at `R` here, while `zikaron/2` seeds that ledger's
  lineage with the continuation's `author` and reads no record `R` sends as
  the seat's. Such a seat enters its books with the root key before anything
  else: until it has, whoever else holds that key, a key the seat handed on
  from under `zikaron/1` included, takes the seat here by handing on first,
  and a seat that has lost that key has no faces.
- **The kit's reading of the same book.** `zikaron.kit/2` §6.4 reads a book
  for its own chain alone, every record under a root moving the key in
  office there. On the home, from its opening on, this law's office is that
  key at every block, in a book that records a hand-over only from the key
  it holds: the opening asks for it (§7.2), and the home's hand-overs move
  both alike. Off the home the two part, each reading its own subject:
  before a home this law reads the books together and the first hand-over by
  the office decides, after it the home's book alone moves the office, and
  on any other chain `zikaron.kit/2` reads what that chain's book holds.
- **The reader's books.** A book is read by its form at the address the
  reader names. A reader that names on a chain an address other than the
  book the seat keeps there, another instance of the same code among them,
  reads the hand-overs that address records and none the seat made: where
  that chain is the seat's home, a key the seat retired in its own book
  stays in office in that reader's table. A reader names as a chain's book
  the instance the seat uses, at an address whose code records a hand-over
  only at the acceding key's own call, after a declaration the holder made
  in its own call, and names with it the root that holder stands under.
- **Book transactions signed for no chain.** A transaction whose signature
  names no chain is its signer's act on every chain whose book accepts it,
  at the time another party includes it there. A declaration a seat replaced
  in one book by a later one stands in a book where its transaction was so
  included, and the key it names takes the office when the party holding
  that key accedes there while the declarer holds the office, that book
  holds the declarer under `R`, and the home is nothing or that book's
  chain. A key of the seat signs every transaction it sends to a book for
  the chain it sends it on.
- **What a retired key costs a reader.** The holder of a key once in the
  line anchors under it at the price of gas. The holder of any key a book
  still holds in office under `R`, the root's in every book the seat never
  entered among them, hands on under `R` there to keys of its own, and they
  hand on in turn; each such key is in reach, so the input grows with all
  they anchor and the misread list with every face entry a key of the line
  anchors that does not stand. A reader that stops at a bound of its own
  produces no face input under that basis, as §6.3 says of a reading it
  could not complete, and narrows the basis as §6.3 says.
- **What the office anchors.** A record seats only an entry its own sender
  signed (§7.3): the key in office of a seat whose seal is open seats,
  through its counted anchor records, the `account`, `key`, `persona`,
  `release`, `reroot` and `origin` entries it signed itself and anchors on
  the home, and the `home` entries it signed itself and anchors on the chain
  each names. A hash another party hands the office seats nothing unless the
  entry is one the key in office signed, and a record sent while another key
  than the entry's author holds the office seats nothing for it, a
  successor's record and the author's own alike. A seat therefore anchors
  each face entry under the key that signed it before that key hands the
  seat on in the book: `zikaron/2` §5.7 has a seat anchor every entry of the
  ceding key before it anchors the succession, whichever key of the seat
  sends the record, and this law asks for the record from the signing key
  itself, as `zikaron.kit/2` §7.2 asks of a grant. What a seat left
  unanchored it signs anew under the key in office.
- **Accessions by accounts carrying code.** The accession form tests no code
  at the acceding address, the book's own contract deciding who may call it.
  An account that lets another party call in its name has handed that party
  its seat in every contract, the book among them.
- **Unread and unproven are hints.** The unread count (§7.7) is nonzero for
  a reader that holds only the chain of a seat that anchors ordinary entries
  as hashes alone, once those anchors are counted and while its seal is open
  (§7.7), and proves no face missing; the unproven count is zero for a
  reader whose node answers every state `zikaron/2` §9.3 asks, and a reader
  that sees it above zero reads the table again with such a node. A seat
  that anchors a face entry without its bytes, or through `anchorMany`,
  leaves readers who hold its ledger and readers who hold only the chain
  with different tables; the obligation of §5.2 is the producer's, and no
  predicate enforces it.
- **Public and permanent.** A face entry's bytes, its note included, sit in
  a transaction's calldata for as long as the chain is kept. A release ends
  a span and removes nothing.
- **Trust.** What a reader concludes from a table, a row, a state or a span
  is the reader's.

**What this law leaves as it found.** No output of `zikaron/2` changes. A
face entry of a ledger is an unknown type there: it appears in `zikaron/2`
§8.6's `UNKNOWN_TYPE` list, from which a reader of this law strikes it as
`zikaron/2` §6.9 allows, and it is anchored or unanchored like any entry. A
face entry is no `history`: a `history` reference or a retraction's
`subject` that names one resolves under `zikaron/2` §11 to `MISMATCHED`
where the ledger read holds it, and never to `RESOLVED`, so no grant reaches
it, and `zikaron.kit/2` reads no face. A record package that carries face
entries carries entries, checked by `zikaron/2` §4 and by nothing else
there. A seat that moves to a later ledger law (`zikaron/2` §6.6) writes its
faces as that law and a later face law have them (§10.3); a `zikaron/2` face
entry its key signs after the migration is seated here by the book all the
same, while `zikaron/2` reads no authority at its position. A reroot and the
origin that answers it join two roots' face tables (§7.9), and a reroot
leaves each ledger to its own root: `zikaron/2` §12.3 and §12.6 say when one
ledger continues another and how the two are read together, and each ledger
is read under its own root as those laws read it, whatever reroot or origin
stands among its entries.

## 10. Freeze and evolution

**10.1** The reference implementation freezes when an independently written
implementation, produced from this text and a conforming `zikaron/2` core
alone, agrees with it on every input of a generated corpus (every reading of
an entry, every declaration with its message and digest, every carried
entry, every face table, every pair of inputs read as §7.9 reads two),
across at least one hundred thousand fuzz samples, with zero panics, hangs,
nondeterministic answers, or wrong accepts under fuzzing, and with every
closed boundary in this text witnessed on both sides by at least one corpus
input under every seed the corpus is generated with. The boundary list is a
precondition on the corpus and never a property claimed of it afterwards:
the manifest publishes it, and in it each boundary names the cases that
witness each of its sides, each rule of order, tie or precedence names the
cases that witness it, and beside them stands the residue of the closed
vocabularies no case witnesses; a boundary whose two sides give one answer
on every input that can be built is marked with that reason, its cases
standing for the side the answer shows; and the comparator checks every
named case against the side or the rule it witnesses, by its answer and, for
the ties no answer shows, by its input.

**10.2** Once §10.5 names a release digest, agreement with the frozen core
on the corpus is the test anyone can run in seconds, and it is evidence of
conformance and never its definition: conformance is agreement on every
input, and an input outside the corpus on which a verifier differs from the
core is a non-conformance the corpus did not catch and a reason to grow it.

**10.3** `zikaron.face/2` is a new type literal and a new domain literal. An
entry of type `zikaron.face/1` is read by this law forever, and a later law
that reads faces says how the two tables meet.

**10.4** This law and `zikaron/2` are two laws: a change to one is never a
change to the other, and each names its own core. This law's core embeds the
`zikaron/2` core of release digest
`0x2a6564e126c632c70b51c602b92bdee47ab2a86118b955d32e2dd5c0f1af567a`, named
in that law's §12.5 on 2026-10-31, which itself embeds the `zikaron/1` core
of release digest
`0xbecfb6f0d0f8b71c314f1b2efef414abfb6df74b711ca8f81efdef685d0132fc`
(`zikaron/2` §12.6). A face core built on any other `zikaron/2` core is
another core.

**10.5 The core's identity.** Naming a release digest here is the freeze,
and it is the only act that performs it. The release digest is
`0xc82c356c829fd0f88b543fc15a49deeb8088bc57635523372822e18b8ae53e8d`,
computed by `zikaron-conformance/criterion-digest-face.sh` over the three
source files of the pure Python implementation in
`zikaron-conformance/face-py/` (`zkf.py`, `zkfentry.py`, `zkftable.py`; the
language's standard library and the `zikaron/2` core of §10.4, nothing else)
and the `zikaron/2` core digest of §10.4, named on 2026-11-14; the face
corpus, `HARNESS-FACE.md`, and the review record beside it say how it
converged. From the naming the core binds and this text is its textbook; a
dispute about what `zikaron.face/1` says on some input is settled by running
the core on it. A change to any of the three files is a new core and a new
digest named by the same act.
