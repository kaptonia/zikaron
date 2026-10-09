# Changelog

[中文](CHANGELOG.md) | English

## 0.1.2

This release reworks how the app talks to nodes (proxies, rate limits, comparing several nodes), lets the command line write and put records on chain through the running desktop app, adds resending a stuck batch with higher fees, and moves local data to a new version of its encrypted format. The ledger law and the record kit format (zikaron/1) are unchanged: ledgers, record kits and data folders written by 0.1.1 read and write as before in 0.1.2. Encrypted local files written by 0.1.2 cannot be read by 0.1.1, so do not open the same data with 0.1.1 after upgrading.

### Network and nodes

- **Proxy**: Settings > Network has a new "Proxy" setting: System (the default), Off, or Custom proxy (`http://` or `socks5://`). Nodes on this machine are never reached through a proxy. The manual's "Network" section covers VPNs.
- **Steadier connections**: when a node's address resolves to several IP addresses, each is tried; every request has one overall deadline; a chain read cut off mid-connection is asked once more (a transaction is never sent twice); a rate-limited node is asked again after waits from one table; one request goes to every node at the same time.
- **Comparing nodes by what each request needs**: each kind of request compares only the facts its result depends on; the chain head is the lowest of the nodes', and a node too far behind is named; nodes serving another chain are left out.
- **Keys in node addresses stay private**: an access key inside a node address never appears in any message, detail or log, whatever the address's form (IPv6, no port given, the key in the path).
- **The main network's registry is checked when saved**: a registry you enter by hand whose code is not the build ZIKARON pins is not saved.
- IPv6 node addresses can be written in brackets (for example `http://[::1]:8545`).
- Every request's `User-Agent` names the product only.

### Putting records on chain

- **Resend with higher fees**: when a batch sits in the nodes' pools without being included, "Resend with higher fees" signs it again at the same nonce at the price now; the old and the new transaction are both watched and the first included counts. A batch is resent at most three times.
- **When no node holds a batch**: if its nonce is still unused, "Send again at the price now" sends it again; if another transaction used its nonce, the batch lapses and its entries go back to the queue. 0.1.1 kept waiting for a receipt forever.
- **The tip is read correctly**: 0.1.1 always read the tip as its 1 gwei cap; the tip the chain actually paid is read now.
- **The balance check before sending** asks every node at the lowest block they have all reached and compares them; a node that gives no answer is never read as zero.
- **The confirmation card says when fees fall back**: when the base fee cannot be read and the fallback is used, the card says so.
- **The command line tells two refusals apart**: a node that refuses the gas estimate because the transaction would fail (`E_GAS_REFUSED`), and one that refuses for its own reasons such as a rate limit (`E_UNREACHABLE`).

### Command line

- **Writing and putting on chain through the desktop**: with `--home <data folder>`, eleven writing verbs (`adopt` among them) are done by the running desktop app, the entry shows in its window at once, and the command line holds no key. `anchor` through the desktop follows "Putting on chain from the command line" in Settings > Network: send automatically, or queue only and wait for you to send from the desktop.
- **Enable command line**: a new switch in Settings > Local data makes `zikaron` available in any terminal (a link in `/usr/local/bin` on macOS and in `~/.local/bin` on Linux, this user's `PATH` on Windows).
- **Adopting anchor proofs judges a co-signature whole**: a co-signature with only one of its two halves is refused, on the page and on the command line alike.
- **A rerun never pays twice**: without `--home`, `anchor` records the hashes it signed; when the same command runs again it asks by hash first, answers an included transaction as included and signs no new nonce, and answers `E_TX_VOID` when what it sent lapsed.
- **`zikaron contract`** prints the command line's own contract (verbs, flags, exit codes, refusal reasons), made from the same tables in the code.
- A usage error no longer echoes the value you typed (for example a mistyped private key); it names only the flag and where it went wrong.
- Messages on standard error follow the system language, Chinese or English.
- New `--key-file` reads the private key from a file; key files derived with pbkdf2 are accepted too.
- `anchor --form registry` without `--registry` is a usage error (exit 2) before any node is asked.

### Local data

- **A new version of the encrypted format**: the whole file header is authenticated, and file names and the part of each header that says which file it is are computed with this machine's key, so someone holding the disk cannot tell from entry or grant numbers what this machine keeps. Older files are read, only the new version is written, and nothing is migrated in bulk.
- **Files that cannot be read are never overwritten**: a file written by a newer version or damaged is left as it is and named with one of six reasons; settings a newer version wrote are carried back as they are.
- **One writer per data folder across machines**: when a data folder on a synced or removable drive is open on two machines, the later one is read-only and can take over with "Write from this machine"; a writer mark that cannot be read leaves the data read-only and the mark untouched.
- **A data folder from elsewhere is refused before it opens**: one that belongs to another identity or another machine is refused without writing a byte.
- **Entries land on FAT and exFAT volumes** (0.1.1 failed on every entry there).
- **Whole-machine backups** carry the read-only network list; the backup lamp has four colours (up to date, entries added since, never backed up, last backup failed), and a failed backup is named under Alerts.
- **A backup never replaces a file**: when the name chosen for a backup is already taken, the next free number is used.
- **Saving on Windows waits out a busy file**: when another program (a virus scanner, for example) holds a file for a moment, the save tries again for up to a second instead of failing with "no permission".
- **Programs the app starts never hold its lock**: on macOS and Linux, a program started from the app no longer keeps the data folder locked after the app quits.
- A broken key file, or one with parameters out of bounds, is refused before any key derivation runs, so it cannot tie up the machine.
- Recovery words that were shown are wiped from memory on hide, lock and quit; a passcode is wiped as soon as it has been handed on.

### Interface

- File dialogs no longer freeze the window.
- Switching identity uses the identity menu again, with its title and the blue frame on the identity in use.
- The first layer of the English interface no longer carries Chinese node messages; what each node said is in folds such as "What each said".
- English in toasts and key-value tables wraps between words.
- The "Third-party licences" list scrolls smoothly and fills its frame.
- Long type labels are shortened with the full text on hover; the English for "存证" is now Record.
- Wording across the settings pages is shorter.

### Fixes

- A batch that no node held stayed waiting for its receipt forever.
- How far a backup is behind counts the entries added since the backup; 0.1.1 could show an old backup as current after a data folder was deleted and recording went on.
- Importing an identity that is already here answers by name (already on this machine, or the name is taken).
- A chain height a node returns in the wrong shape is refused by name, never read as 0.
- A record place given on more than one line is refused by name instead of reading only the first line.
- A grant code's shape is checked first, so a half-pasted code no longer says the root cannot be reached.
- A strict ledger read names a zero-byte entry file instead of skipping it.
- A grant file's length header is read in one spelling only.
- Closing a panel or overlay no longer flashes its text in the placeholder colour while it fades.
- Quitting no longer waits for network reads to finish: on every system an exchange in flight is cut within a quarter of a second.
- After the app quits, the command line with `--home` is told at once that the app is not running; on Windows it could be left waiting on a channel the app had already closed.
- A grant code pasted from a messenger or an email with line breaks, spaces or invisible marks (zero-width characters, a byte-order mark) is read as the code itself.

### Known limitations

- The Windows and Linux builds are built and tested automatically but have not yet been tried widely on real machines; please report problems on GitHub.
- When a data folder's writer mark cannot be read, the pop-up messages use general wording; the bar at the top of the page says what actually happened.

## 0.1.1

This version adds Windows and Linux clients; the preset networks grow to four and each identity uses its own network; the gas fee is worked out from the chain; reading other people's material can read several chains at once; slow operations in the window run in the background. The ledger law and the record kit format (zikaron/1) are unchanged: ledgers, record kits and data folders written by 0.1.0 are read and written by 0.1.1 as before.

### Platforms

- **Windows**: Windows 10 and 11 on x86_64, as a zip: unpack and run; the programs carry their own runtime. Machine data lives in `%LOCALAPPDATA%\ZIKARON\` by default. The programs are not signed; see section 2 of the manual for installing and the first start.
- **Linux**: `.deb` and AppImage for x86_64, needing glibc 2.31 or later (Ubuntu 20.04, Debian 11 and later).
- Every package carries the third-party licences; they are also under "About" in the app.

### Networks

- **Four preset networks**: Ethereum mainnet, Sepolia testnet, Arbitrum One, OP Mainnet. The registry contract has the same address and the same build on the three production networks.
- **Each identity uses its own network**: pick the network when you create or import an identity; identities on one machine may be on different chains. Nodes, chain id and contract filled in by hand belong to the identity that filled them in and do not change when the preset table does. A 0.1.0 identity is given this machine's former network once, on upgrade.
- **New mainnet preset nodes**: `rpc.flashbots.net`, preset in 0.1.0, keeps only the last ten to twenty thousand blocks of logs, so syncing reported "Nodes returned different results.". When a data folder opens with the 0.1.0 pair of nodes unchanged, they are replaced with the new pair; edited ones are left alone.
- **Read-only networks**: Settings > Network can add any number of read-only networks, used only to check others' material and never to send transactions. Before one is read, the code fingerprint of its registry contract is checked; on a mismatch it is not read.
- **Reading others' material reads several chains**: Verify record, Others' ledger, Due diligence and Verify grant read the main network and every read-only network, and say chain by chain whether it was read. A chain that was not read is marked "Chain not read", never taken as absent.
- **Verifying offers to add a network**: when a record kit names a network you have not added, the kit is still checked but its anchors are not; the page names the chain, and "Add" takes the chain id, registry contract and start block to Settings.

### Anchoring and fees

- **The gas fee comes from the chain**: the cap is (base fee × 2 + tip) × gas limit. The tip is the median of the last 20 blocks' paid tips, at most 1 gwei. 0.1.0 used fixed values.
- **The gas limit follows the estimate**: the node's estimate × 1.5 (rounded up), at most 200,000; an estimate above 200,000 is refused and nothing is sent. 0.1.0 always carried 200,000, so its balance threshold and the cap on the confirm card were about ten times too high. The balance check before sending, the cap on the confirm card and the transaction sent use the same number.
- **The estimate is made afresh every time**: every time the confirm card opens; changing the node, the network or the data folder voids it. 0.1.0 reused the last estimate and, after switching chains, could send with another chain's figure.
- **Amounts in significant digits**: small amounts no longer show as `0.0000 ETH`; caps round up, balances round down.
- **The receipt is awaited until the deadline**: after sending, the receipt is asked for until confirmed or timed out; a node briefly unreachable is not a failure.
- **The command line estimates before anchoring too**: if the node says the transaction would fail, or the estimate is above 200,000, nothing is sent and the refusal is `E_GAS_REFUSED` (exit 1). The 0.1.0 command line did not estimate and broadcast transactions that would fail.

### Syncing and reading the chain

- **Only the logs that matter are asked for**: queries carry the senders this machine cares about instead of pulling back everyone's records from the registry contract, so syncing no longer slows down as the chain's record count grows.
- **A refused query is asked again**: when a node refuses a query it is asked again as it was; only after three refusals is the range split by the limit the node gave. A limit written with thousands separators is understood.
- **What has been checked is not asked again**: records several nodes agreed on are kept on this machine and not asked for again; new records are asked for; a replaced block is asked again. Settings > Local data > Advanced options can recheck everything.
- **Nodes are compared only on the fields a verdict uses**: differences in unrelated fields are no longer a disagreement. With the two preset nodes, 0.1.0's self-check sometimes reported a disagreement, and importing on-chain records could be refused because of it.

### The window

- **Slow work runs in the background**: estimating gas (the primary key says "Estimating"), sending ("Sending"), fingerprinting a file, recording with a file ("Recording"), changing the data folder ("Moving"; meanwhile that data can be read but not written). The window no longer freezes.
- **Pickers with search**: "Choose a record" and "Recent addresses" in New grant, the address book, the target entry of a note, "Choose entries" when exporting a record kit, and switching identity open as floating cards with a search field.
- **Filter by date**: Records, Ledger, Grants and My grants can be filtered by on-chain date, picked from a calendar.
- **Verify pages**: both the Recorder and the User have Verify grant, Verify record and Others' ledger.
- **Grant cards** show when the grant went on chain.
- **The network page** shows only the network's name on its top line.
- **Chinese text on Windows** follows the system and uses Microsoft YaHei UI, with a real bold for titles; when it is not there, the embedded Noto Sans SC is used.
- **Hide entries deleted on this machine**: a new switch in Settings > Local data; when on, the Records and Ledger lists leave out entries deleted before they went on chain, which stay on this machine only; entries deleted after going on chain are still listed. Display only, off by default.

### Record kits and verification

- **A record kit says where it is anchored**: on export, a line `anchored-on: eip155:<chain id> · registry <contract> · from <start block>` is added at the end of the note.
- **Verification results are written down**: each record kit verified leaves a result file under `kits/verified/` (format `zikaron.kit-verification/1`) naming only the chains actually read, for other apps to read.

### Command line

- `--endpoint` accepts `https://` and checks the certificate.
- Writes are audited offline first: a write that would break the ledger is refused with `E_WOULD_BREAK`, and not a byte is written.
- `history` gains `--file`, recording a file by the same convention as the app; when the content does not qualify, standard error gets one more line saying why.
- `check-grant` reads `.zkgrant` grant files exported by the app; `--ledger` can point at a ledger mirror folder or a record kit.
- `audit --out` saves a copy of this audit's input.
- Exit codes, refusal reasons and what standard error says are set out in the repository's `CLI-SCHEMA.md`.

### Fixes

- Imported identities (recovery phrase, private key, key file) could not create a ledger. Now a ledger checked against the chain can be written, wherever it came from.
- Importing a private key as the primary identity at first start now requires exporting a key file at the same time, so it can be recovered later.
- A damaged key store no longer sends you into the first-start wizard; it says "The key store cannot be read", with the reason and the file's path.
- After a key change on the command line the old key can no longer write; with two roots in a ledger and no `--root`, writes are refused.
- "Change data folder…" accepts only an existing data folder or an empty folder, and no place inside the current one (0.1.0 copied the folder into itself, layer by layer).
- "Copy grant code" on a sublicense carries the whole chain up to the root; the issuer's note is kept on the last hop only.
- "Import grants folder…" checks the files one by one; a bad one does not hold up the rest.
- Exporting a record kit with an entry that is not in the ledger is an error instead of being skipped quietly.
- A passcode longer than the field can take is refused instead of cut short.
- Restoring a whole-machine backup checks every member's file name and refuses any that climbs out of its folder.

### Known limitations

- When anchoring from the command line, any node refusal of the estimate is reported as the transaction would fail, without telling rate limits and the like apart; nothing is sent, and the node's own words are in `detail`.
- When the base fee cannot be read and the fallback fee is used, the confirm card does not say it is the fallback.
- With only one node configured, logs and receipts are not cross-checked against each other; two or more nodes are not affected.
- The official OP Mainnet node sometimes answers that the range is limited; asking again usually gets through.
- The Windows and Linux clients have passed automated builds and tests but have not yet been tried widely on real machines; please report problems on GitHub.

## 0.1.0

First release: macOS (Apple silicon).
