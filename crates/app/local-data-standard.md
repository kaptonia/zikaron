# Local data standard, third version

The family's standard for files an app seals on its own machine (ZIKARON first; the others take the same shape with
their own magic). This text says only what ZIKARON's code does today; the code is `crates/app/src/local.rs`, the
test vectors are `crates/app/tests/local_standard.rs`.

## 1 · A file's identity is its owner and its place

- **Owner.** A file in the machine directory belongs to the machine (`machine`). A file in a home belongs to that
  home, by the home's number (`home:<32 hex>`).
- **Home number.** Drawn at random when the home is born and kept in the home's own sealed label
  (`settings/home-label.json`, kind `home-label`). The label also says whose the home is: an identity's seat
  (`{"identity": …, "seat": …}`) or nobody's (`"none"`). The label itself is sealed with the owner `home-label`
  (read before the home's number is known).
- **Place.** The file's kind and its logical name inside its home or the machine directory (which slot, which
  name). Never its path or its name on disk: moving the data folder, renaming everything under a new master key, or
  restoring onto another machine leaves every identity as it was, and nothing is sealed again for it. A kind whose
  name on disk is keyed takes its logical name from what it holds (an entry by its id, a held grant by its id, a
  verdict by its grant, a kept grant file by its kit, a terms document by its digest, an issuance record by its
  grant).
- **Opening a home** reads its label first and checks its owner against the identity and seat it is opened as; a
  label of another identity or seat is refused as another file. A home of an older version (no label) gets one the
  first time a writer opens it or writes into it; a reader never makes one.

## 2 · The envelope

| bytes | what |
|---|---|
| 16 | magic `zikaron-local/2\n` (each product its own, in its family literal register) |
| 1 | algorithm: `2` = XChaCha20-Poly1305 with the place's half keyed (the third version, the one written); `1` = the same with the place's half unkeyed (the second version, read only); a later algorithm takes another number and keeps the magic |
| 1 + n | kind tag length, kind tag (ASCII) |
| 2 | the kind's format version, big-endian |
| 16 | identity digest: `sha256("zikaron-local/owner" ‖ 0 ‖ owner)[..8]` ‖ the place's half (the kind is the head's own field). Algorithm `2`: `HMAC-SHA256(K, "zikaron-local/place" ‖ 0 ‖ logical)[..8]`, `K` = HKDF-SHA256 (no salt) of the local data key under `zikaron-local/place-key`, so a locked disk's heads cannot be checked against public logical names (entry and grant ids) one by one. Algorithm `1`: `sha256("zikaron-local/place" ‖ 0 ‖ logical)[..8]` |
| 24 | nonce, fresh for every seal |
| rest | ciphertext and 16-byte tag |

- The additional data is the whole head, nonce included: any byte of the head changed, or the file put in another
  place or another home, does not open.
- The plaintext carries the owner and the logical name again: `u16 length ‖ owner text ‖ u16 length ‖ logical
  name ‖ the file's own bytes` (lengths big-endian). Once opened they are checked: the owner must be the one asked
  for, the digest must be theirs, and a keyed kind's logical name must be the one its content gives and the one
  its name on disk stands for.

## 3 · Reading

- Both envelopes are read. The first (`zikaron-local/1\n`: kind and version bound as additional data, no identity)
  is read forever; nothing makes it rewrite itself.
- A file that does not read is refused with one member (`LOCAL_SEAL`). Its evidence names one of six reasons, a
  closed table; the words people see and the way out are the same for all six:

| reason | when |
|---|---|
| `not-sealed` | no magic (a plain file an older version left, or anything else) |
| `truncated` | the magic is there but the head, or the ciphertext's tag, is cut short |
| `other-kind` | sealed as another kind |
| `newer-version` | a format version or an algorithm past this version's |
| `swapped` | another file: the digest is not the one asked for, the home has no label, or the checks inside fail |
| `unopenable` | another key, or a byte altered (the digest matched) |

- The owner's half is compared before any key is tried; so is the place's half of the second version. The third
  version's place half is keyed: one that differs is told by trying the key (a file that does not open is under
  another key, or altered; one that opens is another file), so another file and a wrong key are still told apart.

## 4 · Writing

- Only the third version is written. There is no migration step: nothing is sealed again in bulk. A file that is
  written anyway becomes the third version then; a ledger entry once written is never written again.
- A file that is there but does not read is never written over: the write is refused by name (the reason, the
  file) with the way out (move it aside; the next write makes it anew). Write-once files (held grants, kept grant
  files, terms documents and records, ledger entries) are refused when one is there in any case.
- The write gate also asks the writing task's ticket: a background task that started for a home no longer in use
  writes nothing and ends with `HOME_UNREACHABLE`.

## 5 · Known limits

- An older copy of the same file put back in its place is not told. Stopping it would need a counter nobody can put
  back with it; the ledger has its own signature chain.
- Two whole homes swapped label and all are not told by the envelope; opening a home checks its label's owner
  against the identity opening it.
- Labels are alike in every home (owner `home-label`, the same place): a label moved between two homes of nobody
  is not told.
- There is no way back: a file this version writes does not read in a version before it. There it is refused by
  name and left untouched; it opens again here.

## 6 · Test vectors

Key: bytes `00 01 … 1f`. Nonce: bytes `40 41 … 57`. Each vector gives the owner, kind, logical name, plaintext,
the identity digest and the whole envelope in hex; `crates/app/tests/local_standard.rs` holds them and checks them
byte for byte.

### `settings`

- owner: `home:000102030405060708090a0b0c0d0e0f`
- logical name: `settings/settings.json`
- plaintext: `{"capBytes":0}`
- digest: `20b1f7730575bd1dd0ff9d4f6d2cea6d`
- envelope:

```
7a696b61726f6e2d6c6f63616c2f320a010873657474696e6773000120b1f7730575bd1dd0ff9d4f6d2cea6d404142434445464748494a4b4c4d4e4f5051525354555657d41c6d1fbd854326bfc4b68e9dac56a2a68a98f4256964aa5201c475683441a075c9665e30a2e210e9c191387eb3b496e8440e8d1bf1ddf2d18f269e60dd222f7848990256ca31adf6218acfa13526d4d66a24d2d006f1a12e5e80bc4d
```

### `registry`

- owner: `machine`
- logical name: `identities-anchor.json`
- plaintext: `{"rows":[]}`
- digest: `fd4681f3485cef8e1a9ffa7714f7158a`
- envelope:

```
7a696b61726f6e2d6c6f63616c2f320a010872656769737472790001fd4681f3485cef8e1a9ffa7714f7158a404142434445464748494a4b4c4d4e4f5051525354555657d43e6811b3881078eaf491d7cbf90be6fbcec4a1607432f409599237276e50ff7882201c3ae5f732c5e9a93117a6e37bec286d645d91e097df530d8d
```

### `home-label`

- owner: `home-label`
- logical name: `settings/home-label.json`
- plaintext: `{"home":"000102030405060708090a0b0c0d0e0f","owner":"none"}`
- digest: `59b8b374c2f7a75282e463918289ae77`
- envelope:

```
7a696b61726f6e2d6c6f63616c2f320a010a686f6d652d6c6162656c000159b8b374c2f7a75282e463918289ae77404142434445464748494a4b4c4d4e4f5051525354555657d4336d1fbd85547aee96e2d2af8416f7e6cec4aa742a7cf2055c9868656541f57ad7681d3afcff3297dd992928e0f8c1ab5b4cd85db587ac82cc3dc425827b64621bc142758327f8e633d4cfb949facddae5695c465816556a8169fbab0011581b631b2e4eaa653bac08f2ba63a187e7
```

