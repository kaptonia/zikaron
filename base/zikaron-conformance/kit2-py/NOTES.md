# zkk2 (kit2-py): the readings this implementation chose

A pure Python 3 implementation of `zikaron.kit/2` (`docs/zikaron-kit-v2.md`),
written from that text and the two parent laws, exposing the command line of
`HARNESS-KIT2.md`, and embedding the `zikaron/2` core of `impl2-py` (which
embeds the frozen `zikaron/1` core) for every parent predicate.

    zkk2.py       the command line
    zkk2doc.py    §1 accept by spec, §3 badge encode and decode
    zkk2kit.py    §4 the walk, the one-file form, kit paths, the manifest,
                  verification with the directory manifest; §5 grant packages
    zkk2read.py   §6 reachability and bounds over both segments, §7 the grant
                  check, the ledger link, the chain check

## 1. `accept` dispatches on the envelope's `spec`

A byte string that passes `zikaron/2` §4.3 steps 1 to 3 (canonical, an
object, exactly the seven keys) and whose `spec` is `zikaron/1` goes to the
`zikaron/1` core's §4.3; everything else goes to `zikaron/2` §4.3. The steps
and the token set are the same in both laws, so the dispatch changes no token.

## 2. The ledger of a continued input

`audit(I)` yields the new segment's ledger entries from the `zikaron/2` core
and, where `continues` is not `null`, the old segment's ledger entries, read
as the `entry_id`s the nested `zikaron/1` report lists in `anchored` and
`unanchored`, matched against the old-segment byte strings. The findings that
name an entry are read from its own segment's report; the counted and
`UNPROVEN` records of the old segment are those the `zikaron/1` trim keeps
(sender in that segment's lineage), computed with the `zikaron/1` core's own
lineage function; the new segment's are those the `zikaron/2` trim keeps.

## 3. Reachability through `continues`

`F → F'` holds along `prev`, and from every ledger entry at `seq` 0 of the
new segment whose `continues.entry_id` is the report's `continues.entry_id`
(the selected genesis where it is timely, and any timely twin naming the
same close) through that `entry_id`; the selection of §12.6 reads no
timeliness, so the selected genesis may be `RETIRED` while a twin is the
ledger entry, and the edge belongs to the value the report carries. A member named `continues` on any other entry, the
genesis of an uncontinued root and an extra member of either law's §6.10
among them, is data and no edge, as the parent law reads it.

## 4. Check 3, timeliness, and the law that holds the grant

Check 3 reads the report's lists: a `zikaron/1` grant (its `spec` member)
under a report whose `continues` is `null` is `UNKNOWN`, and leaves checks 4
and 6 `UNKNOWN` (§9); a grant whose `entry_id` is in `retired` is `FAIL`
where the label is `COMPLETE` and `UNKNOWN` otherwise, leaving 4 and 6
`UNKNOWN` either way; a grant in neither `have` nor `retired` is `FAIL`,
leaving 4 and 6 `UNKNOWN`. The core decides "no ledger entry" by `have`
rather than by the `EXCLUDED` and `MALFORMED` lists the text names: the two
agree because each segment's §8.1 partitions its pile completely (a byte
string passing `accept` is a ledger entry or stands in `EXCLUDED`,
`RETIRED` or `UNKNOWN_TYPE`, a grant never in the last; a `zikaron/1` byte
string outside the old segment fails this law's §4.3 and stands in
`MALFORMED`), so the lists and `have` name the same byte strings.

## 5. Coverage over both lineages

Check 4's covering test asks `senders` to hold every key of the new segment's
lineage and, for a continued ledger, every key of the old segment's
`zikaron/1` lineage.

## 6. The one-file form

`kit-verify` and `grant-package` take a directory or a regular file, a
symbolic link to either resolved as the platform resolves it; a regular file
that exists and cannot be read in full is `E_KIT_UNREADABLE` with subject
`.` (§4.1a), an answer and not misuse. The layout parser reads the magic,
then items; the subject of `E_KIT_UNREADABLE`
is the path of the item being read where its path line was read in full and
is a kit path, and `.` otherwise (the magic, a path line that is no kit path
or not UTF-8, an empty layout). "Out of order or repeat a path" is decided
over the items after the manifest: each path must be bytewise greater than
the one before it; `manifest.json` repeated later is a repeat.

## 7. The directory manifest

For a `contents` row whose path is a directory original, the names are each
listed file's `name_md` where present and its kit path below the directory
otherwise; §4.3's `contents` rule refuses a manifest under which two rows of
one directory original take one name, a name has an empty segment or a
segment `.` or `..`, a name followed by `/` is a prefix of another, or one
directory original lies under another, before any digest is computed; the
rows sort by the name's UTF-8 bytes; the object is
`{"files":[{"path","sha256"}...]}` serialized by `zikaron/2` §3.4 with the
names as prose strings (no §3.5 test applies to a value the verifier
computes) and its `sha256` is compared to `content`. A file original's row
is decided by the manifest rules (its listed `sha256` equals `content`) and
never by step 5. A `contents` row's optional `name_md`, the directory's own
name under §4.3, is tested for form (a prose string) with the row's other
members and read by nothing else: step 5 recomputes the directory manifest
from the `files` rows alone, and the `GP_OK` result of §5.3 carries each
row's `content` and `path`; distinctness of `contents` rows reads the pair
(`content`, `path`), so two rows of one pair that differ in `name_md` fail
`contents`.

## 8. Grant package subjects

A kit failure's subject is the kit verdict's own (`null` for
`E_KIT_MANIFEST_ABSENT`); a badge failure's subject is the index the badge
token carries, or `null` for `E_BADGE_PREFIX`, `E_BADGE_CAP`,
`E_BADGE_INCOMPLETE`; `E_GP_TERMS` carries the path below `files/terms/`.
`GP_OK` lists each hop's terms paths without that prefix, in bytewise order.

## Harness misuse, by name

A file that cannot be read; a `kit-verify` or `grant-package` argument that
is neither a directory nor a regular file; `badge-encode` with no path; a
`--now` outside the integer universe; a `hops.json` that the audit-input reader
(`zk2audit.load_input`) refuses or that is not an array of `{grant, audit,
notices}` objects; a `--notices` path, or a hop path, that
cannot be opened, a path holding U+0000 or a lone surrogate among them.

## 9. The notice input, and a grant carrying a revocation member

`load_notices` reads a notices file with the parent's `load_input` (UTF-8,
no repeated member name, §3.2 integers, no fractions) and then decides §6.3:
exactly `basis`, `revocations`, `accessions`; the basis's six members and
two range rules; each record's closed member set, forms, chain equal to the
basis's, block inside its range, and a position `(blockNumber, txIndex,
logIndex)` spelled at most once per array. Any failure is `NoNotice`, the
invalid notice input, read as `null` by every check and written as `null` in
`notices`. The records are kept sorted by position.

`revocation_member` reads the grant's body: the key present is the member
carried, whatever its value; well-formed is an object of exactly `chain`
(an int, which the canonical bytes already bound to §3.2's universe) and
`at` (hex20). `applies` compares the basis's `chainId` and
`revocationContract` to the member. The root the book is read under
(`book_root`) is, in order, the `root` of the least-positioned book record
whose `to` is the author, where the notice input applies and holds one; the
view's root where the audit input is valid; the grant's `ledger` where
hex20; its author. `in_office_at_position` and
`in_office_at_block` walk the sorted book records of that root and take the
`to` of the last one strictly before, the root where none.

`_member_checks` decides checks 2, 3, 4 and 6 for such a grant and
`grant_check` runs check 5 as for any grant: check 2 passes where some byte string of the new segment passing §4.3
(`View.seg_entries`) is a `genesis` whose author is the segment's seed
(`View.seed`), timely or not, is `UNKNOWN` otherwise, `FAIL` on
`BROKEN_CHAIN`; the standing records are read from `view.anchors`, the
anchor set as the audit received it (`Result.anchors` of the embedded core,
before its §8.1 trim), on the member's chain, of the grant's `entry_id`,
counted, at or below the notice basis's `toBlock`, whose sender is in
office at their block; check 3 first convicts (`FAIL`) a grant that check 2 passed, that the
table's check 3 fails (neither a ledger entry nor `RETIRED`, and not a
`zikaron/1` entry under an uncontinued audit: `EXCLUDED` of either
segment, or `MALFORMED` outside the old segment) and whose author no book
lineage seats (no book read where the notice input does not apply), then
is `UNKNOWN` where the notice input does not apply, `FAIL` where the
view's root differs from the root `book_root` derives, `PASS` where some
standing record's sender is the author, `FAIL` where standing records exist
and none is, `UNKNOWN` otherwise; check 4 is `PASS` iff a standing record exists;
check 6 walks the notice records naming the grant in position order,
skipping those whose sender is out of office at their position, `FAIL` at
the first counted one, `UNKNOWN` where only `UNPROVEN` or `VOID` ones by a
key in office remain, `PASS` where none. Check 3's conviction is gated by
check 2 alone, the rest of checks 3 and 4 by check 2 and by the notice
input applying; check 6 by the latter alone.
`ledger_link` compares the root `book_root` derives for the downstream
grant, the book's for its author where the notice input names one, the
view's where one is valid, and the grant's `ledger` or author otherwise;
with no valid view it is unknown unless the grant carries the member and
the notice input applies to it. `read_file` in `zkk2.py` stats the path
first and treats anything but a regular file or a character device, and
every failure to open (`OSError`, `ValueError`, `UnicodeError`), as
misuse, exit 2.
