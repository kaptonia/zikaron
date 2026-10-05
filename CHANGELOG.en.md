# Changelog

[中文](CHANGELOG.md) | English

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
