# ZIKARON Desk User Manual

ZIKARON Desk is a desktop application for your own machine. It records your works or files in an append-only ledger, anchors that ledger to Ethereum, and lets you issue grants from it. Whoever receives a grant can verify it independently, on any machine.

This manual describes version 0.1.0. Words that appear in the interface are quoted exactly as the window shows them.

---

## 1 · Concepts

**Identity.** An identity is a set of signing keys. There are two kinds:

- **Recovery-phrase identity**: derived from 12 recovery words. It has two keys, one for the Recorder role and one for the User role.
- **Local-key identity**: an imported private key or key file. It fills only the one role chosen when it was imported.

**Two roles.** Every identity has a "Recorder" role and a "User" role. You switch between them at the top of the sidebar. Each role has its own key, address and ledger (data folder).

- **Recorder**: records your own works and issues grants.
- **User**: receives other people's grants, verifies them, and can sublicense them.

A recovery-phrase identity derives its two keys at `m/44'/60'/0'/0/0` (Recorder) and `m/44'/60'/0'/0/1` (User). The two addresses differ, and each needs a little gas.

**Ledger.** Each role has its own ledger: a chain of signed entries that can only be appended to. Old entries are never changed or removed. The first entry is "Create the ledger", the ledger's one and only root. Entry types:

| Type | What it is |
|---|---|
| Create the ledger | The root of the ledger |
| Anchored record | The content fingerprint of one record |
| Grant | Lets an address use a record for a period of time |
| Revocation | Revokes a grant |
| Note on entry | Adds a note to an earlier entry |
| Adoption | Brings anchors this key put on chain earlier into the ledger |
| Key change or handover | Hands the ledger to a new key |
| Deletion | Marks a record as deleted (see Appendix C) |

**Putting on chain (anchoring).** New entries wait in the "Pending" queue until an Ethereum transaction carries them; after that they are on chain. The whole queue can go out at once for the cost of a single transaction. Once an entry is on chain, anyone can check against the chain that the ledger held it at that moment.

**Grant.** A grant entry names the grantee, the record, the validity period and the terms file fingerprint. There are two ways to hand one over:

- **Grant code**: a line of text starting with `zikaron-grant:`. It lets the receiver verify only the signature and the validity period.
- **Grant file**: a `.zkgrant` file that carries the issuer's ledger, so the receiver can run the full check.

A User can also write a **sublicense** in their own ledger, passing an upstream grant on to someone else.

**Record kit.** A folder exported from a ledger. It holds the chosen entries, the original files and the on-chain proofs, so someone else can verify them offline.

**Local encryption.** All local data is encrypted under one master key, and the master key is opened by an 8-character passcode. While the app is locked, none of the local data can be read.

**Primary identity.** The first identity created on a machine is its primary identity by default; another can be made primary later in Settings > Identity key. Only the primary identity's recovery phrase or key file can reset a forgotten passcode (see section 5).

---

## 2 · Installation

Version 0.1.0 comes as packages for macOS with Apple silicon. There are no prebuilt packages for x86_64 Linux; build it from source.

**macOS**

- `ZIKARON-0.1.0-macos-arm64.dmg`: open it and drag `ZIKARON.app` to Applications. The `zikaron` command line is inside the app bundle at `ZIKARON.app/Contents/MacOS/zikaron`.
- `ZIKARON-0.1.0-macos-arm64.pkg`: an installer that puts the app in `/Applications` and the command line at `/usr/local/bin/zikaron`.
- The app is signed with a self-signed certificate (Kaptonia) and is not notarized by Apple, so macOS blocks it the first time you open the app or the installer. Try to open it once, then go to System Settings > Privacy & Security, scroll to the bottom and click "Open Anyway".

**Linux**

- Build from source as described under "Build from source" in the repository's README. On Linux, `packaging/linux/build.sh` also makes a `.deb` and an AppImage; once installed, the window program is `zikaron-desk` and the command line is `zikaron`.
- File dialogs go through the desktop portal, so install `xdg-desktop-portal` and a backend such as `xdg-desktop-portal-gtk`. Without a portal, clicking to choose a file says "The system file dialog can't open on this machine."; drag the file into the window instead.
- On Linux, a drop zone that takes either a file or a folder (such as New record) opens a dialog that picks files only; to give a folder, drag the folder in. "Choose folder…" picks folders as usual.

Building from source on macOS is also described in the repository's README.

---

## 3 · Getting set up

### Getting started

The wizard opens by itself when the app starts and something is missing: no passcode yet, no signing key, or you are a Recorder whose ledger has not been created. It opens at the first unfinished step.

The six steps are listed down the left; the current step fills the right. A finished step has a check mark, a step you may leave for later has a dashed mark, and you can click back to finished steps and to steps before the current one. Whether a step is done depends on the actual state, not on having pressed "Next".

| # | Step | Required? |
|---|---|---|
| 1 | Set a passcode | Required |
| 2 | Create an identity | Required |
| 3 | Network | Required |
| 4 | Create the ledger | Required for a Recorder; a User may choose "Later" |
| 5 | Add gas | May choose "Later" |
| 6 | Whole-machine backup | May choose "Do it later" |

Without a passcode, the wizard stays on step 1. With a passcode but no identity, it stays on step 2.

**Bottom buttons**

- On the left: "Back", and "Later" on steps that may be skipped. On the last step this key reads "Do it later" and closes the wizard.
- On the right: the step's own action key until the step is done, then a blue "Next". The last step has "Finish", which shows "First run complete; remaining items are in Settings > About".

**Leaving the wizard.** "Exit setup" at the bottom of the step list, or Esc, returns you to where you were before the wizard opened. Finished steps stay finished. At the next start the wizard opens again only if the passcode or identity is still missing, or a Recorder's ledger has not been created; for the other unfinished steps, use "Run wizard again" in Settings > About.

- If fresh recovery words have not been checked yet, it asks "Exit setup?" first: "These words haven't been checked; exiting discards them."
- A true first run (no passcode and no identity) has no way out.

**Other ways to open it**

| From | Opens at |
|---|---|
| "Run wizard again" in Settings > About | The first unfinished step; step 1 if all are done |
| "Go set one" in Settings > Identity key | Step 1 |
| "Create key…" on the sublicense page | Step 2 |
| "Set up ledger…" on the sublicense page | Step 4 |

Whenever there is no passcode yet, it opens at step 1.

#### Step 1 · Set a passcode

1. Type 8 letters or digits into the eight boxes. Case matters, and the characters show as dots.
2. When the row is full it asks you to "Enter it again". If both entries match, the passcode is set; if not, the row shakes and you start over.

- A passcode that is too simple is refused on the spot: eight identical characters, or digits that run in sequence or read as a date. The message is "Too simple. Choose another."
- Under the boxes: "8 letters or digits, case-sensitive." and "Entered at launch. If you forget it, recover with your phrase."

The passcode comes before the identity because keys are stored in a key store that the passcode opens. The app never reads or writes the system keychain; keys live only in its own key store.

#### Step 2 · Create an identity

You may first type a name in "Label" (optional, just for you to recognise it). Then choose one of three ways.

**Generate recovery words (a new identity)**

1. Press "Generate recovery words".
2. Make sure no one can see your screen, then click the cover ("Make sure no one can see your screen, then click to show"). The 12 words appear in three rows of four. "Hide recovery phrase" covers them again at any time.
3. Write them down in order on paper and press "I have written them down". It only works after the words have been shown.
4. Fill in the three words it asks for ("Word N") and press "Confirm and create". If they match, the identity is created.

**Import an existing key**: press "Import an existing key…". See "Import key" under "Identity and keys".

**Restore from a backup**: press "Restore from backup…", choose a whole-machine backup file, enter the "Backup password" and press "Recover". Identities, keys, ledgers and settings all come back, re-encrypted under the passcode you just set, and the wizard closes.

The first identity created on a machine is its primary identity.

#### Step 3 · Network

There are two choices:

- "Ethereum mainnet", marked "Recommended" and selected by default.
- "Custom": "Fill in the chain, contract and nodes yourself".

Pressing "Next" makes the choice, and the machine remembers it. With mainnet, if the current data folder has no chain settings yet, the app fills in the chain id, registry contract, start block and two public nodes. With "Custom" nothing is filled in; set things up yourself in Settings > Network.

Footnote: "Public nodes can see your queries. You can change them in Settings."

Sepolia testnet is not listed here. To use it, choose "Custom" and fill in the values from Appendix A. The one exception: if the network this machine has on record is already Sepolia (for example, restored from such a backup), it appears between "Ethereum mainnet" and "Custom".

Anchoring on mainnet costs real ETH in gas.

#### Step 4 · Create the ledger

1. Describe what the ledger is for in "Ledger description (optional)".
2. Press "Create the ledger". A confirmation card shows "Ledger description" and "Cost" ("Free (local only)"), with the note "Cannot be changed after creation". The ledger's location is under "Details".
3. Press "Create the ledger" on the card to write it.

#### Step 5 · Add gas

The page shows the "Key address", the "Gas balance" and a payment QR code ("Scan with a wallet to pay").

1. Send a little ETH to this address from your wallet. You can select and copy the address.
2. Press "I have sent it". The wizard sends no transaction; it reads the balance on chain once. Above zero, the step is done and the wizard moves on. Still zero, it says "The chain does not show the transfer yet; press “I have sent it” again in a moment."

Reading the balance needs nodes. If you chose "Custom" in step 3, set up nodes in Settings > Network first.

#### Step 6 · Whole-machine backup

The page reads "One backup file restores everything; with only the recovery words, only data readable on chain comes back."

Press "Export backup…" and fill it in as described under "Whole-machine backup" in "Storage". Once the export succeeds, the step is done.

---

## 4 · The window and common controls

#### 4.1 Window layout

The sidebar is on the left and the page is on the right. Across the top of the page is the toolbar: back and forward arrows and the page title on the left, the page's own buttons and filters on the right.

**The sidebar, top to bottom**

1. **Identity chip**: shows the current identity's name ("Unnamed" if it has none) and its kind. Click it to open the identity menu:
   - Under "Switch identity", each identity takes two lines. The primary is marked "Primary" and the current one "In use". Use ↑↓ and Enter, or click another identity to switch to it and land on Home.
   - Below that: "New identity…", "Import key…" and "Identity key" (the Identity key page in Settings).
2. **Role switch**: the segmented control "Recorder" | "User". Switching replaces the whole sidebar with that role's pages, lands on Home and shows "Switched to …".
3. **Pages**:

   | Recorder | User |
   |---|---|
   | Home | Home |
   | **Record**: Records, Grant | **Grant**: My grants |
   | **Look up**: Verify, Ledger, Alerts | **Look up**: Verify, Alerts |

   - The number next to "Alerts" counts the amber and red rows on the Alerts page.
   - Clicking the page you are already on takes it back to its list. Each page remembers where you left it.
4. **Bottom**:
   - "Settings".
   - "Lock", present only when a passcode is set.
   - The status line. The small icon at its right end is "Sync".

**The status line** shows the most pressing of these:

| Shows | Meaning |
|---|---|
| "In progress: …" | Background work you started is running; click it to return to the page where it started |
| "Syncing…" | A sync is running |
| "Sync failed" | The last chain read failed; the full message is under "Details" on the Alerts page |
| "Last synced HH:MM" | When the chain was last read successfully |
| "Not synced yet" | The chain has not been read |

**Sync.** Click the sync icon at the right end of the status line to read the chain in the background for the current role:

- Both roles read the signing key's balance once.
- A Recorder whose ledger exists also runs a ledger check.
- A User also re-verifies the grants in My grants.

When everything has come back, one summary toast appears. With no nodes set it says "No nodes configured: add them in Settings > Network".

**Keyboard shortcuts** (⌘ on macOS, Ctrl on Linux):

| Keys | Action |
|---|---|
| ⌘1 to ⌘6 | Open the sidebar pages in order |
| ⌘[ / ⌘] | Back / forward |
| ⌘, | Settings |
| ⌘L | Lock |

Shortcuts do nothing while the passcode gate, the wizard or a card is open.

#### 4.2 Lists and detail pages

List pages (Records, Pending, Ledger, Grant, My grants) have a search field at the top. Click a row to open its detail page; use the toolbar's back arrow or ⌘[ to return. As you scroll down a detail page, its title moves up into the toolbar.

On a narrow window, pages with a side column (New grant, Export record kit) move the side column below the form.

#### 4.3 Bars at the top of the page

In these cases a bar appears at the top of the page, one at a time:

| Bar | When |
|---|---|
| "Ledger chain broken; read-only" | The ledger check found a broken chain. "Go to Restore" opens Settings > Local data, where you can restore from a whole-machine backup or reconcile under "Advanced options" |
| "Ledger handed over to a new key; read-only" | This ledger has been handed over |
| "Read-only: …" or "Read-only: No lock" | Another ZIKARON window holds this data folder, so this window can only read |
| "Old data · date · view only" | You are viewing data that was set aside; "Return" goes back to the current data |

#### 4.4 Buttons and confirmation cards

**Button colours show their role**

- **Blue**: the main action on the screen. At most one per screen.
- **White**: every other action.
- **White with red text**: opens a confirmation card or the next step. It does not write anything itself.
- **Solid red**: writes to the ledger or sends to the chain the moment it is pressed. It appears only on a final confirmation card.

**How writing works.** Every action that writes to the ledger or the chain first shows a confirmation card. The card lists what will be written, where, and at what cost, and ends with a sentence saying it cannot be undone. Only the solid red key on that card writes; "Cancel" or Esc writes nothing.

**Cards in general**

- Only one card is open at a time, and clicking outside it does nothing.
- Machine values (entry IDs, transaction hashes, raw verdicts and so on) sit in the "Details" fold. Options you rarely need sit in "Advanced options".

**Long actions** show progress inside the button and a check mark when done. On failure the button turns red and shakes.

#### 4.5 Toasts

Results appear as a small card at the top centre of the page. Only one shows at a time; a new one replaces the old.

- **Ordinary toasts** stay for about 3 seconds.
- **Error toasts** say what happened on the first line and what to do next on the second, and stay for 6 seconds. They have "Close", and "Show details" when there is raw text; once details are open the toast stays until you press "Close" or "Hide details" (after hiding them it stays another 6 seconds).
- **Alert toasts** start with "Alert:" and stay for 6 seconds.
- If a task finishes after you have left its page, its toast has "Look up", which takes you back there.

**What raises a toast**

- An action you pressed raises one toast when it finishes, whether it succeeded or failed.
- Reads the app does by itself toast only on failure. Network failures raise no toast at all; they show as "Sync failed" in the status line.
- Errors on a card are written on the card, with the raw message under "Error details".

#### 4.6 Dropping and choosing files

**Drop zones.** You can drop a file in, or click the zone to open the system file dialog. Cancelling the dialog changes nothing.

- A drop zone takes one item at a time. Dropping several files shows "Drop one item at a time. Put multiple files in a folder first." and takes none.
- Two exceptions take several files: the drop zone on the "New record" card (a batch of records), and the attachments zone of a record kit.

**Dropping onto the window.** On pages without their own drop zone, drop a single file anywhere on the window:

- As a Recorder, it opens "New record".
- As a User, it goes to "Verify record" and checks it straight away.

**Choosing a path.** Where a folder or file is needed, one line shows the chosen path ("None selected" if nothing is chosen yet), with "Choose a file…" or "Choose folder…" beside it. Output locations are chosen with "Choose folder…" too: record kits, grant files, credentials and snapshots default to the `kits` folder in the data folder; backups and mirrors default to the location used last time (the first time you choose); key files have no default and need a folder chosen first.

---

## 5 · Passcode, lock and recovery

#### 5.1 The passcode gate

On a machine with a passcode, ZIKARON opens with the passcode gate: "Enter passcode" and eight boxes.

- The eighth character submits the passcode, and the card shows "Verifying…".
- A wrong passcode shakes the row and shows "Wrong passcode. N tries left".
- After 5 wrong tries the card becomes "Locked", and only recovery remains.

The count of wrong tries is kept on disk and survives restarting the app. This limit stops someone sitting at the machine from guessing. Against someone who copies the key store file away, the protection is the passcode itself and the encryption work every try has to repeat.

Two other cases:

- If the key store's encryption settings are below this machine's standard (for example, someone lowered them), the gate says "Encryption settings are below this Mac's standard." and "Enter your passcode to re-encrypt at this Mac's standard." That try is not counted as wrong; the correct passcode re-encrypts the store at this machine's standard.
- If the key store file turns out to be damaged when unlocking or setting a passcode, the app says "The key store file is damaged." and does not touch the file.

#### 5.2 Locking and auto-lock

"Lock" in the sidebar, or ⌘L, locks at once:

- The master key is wiped from memory, the data folder is closed and everything shown from it is cleared.
- Background work stops: ledger checks, grant re-verification, revocation watch, sending the pending queue and waiting for receipts.
- While locked, actions that need a key are refused: "The key store is locked." and "Unlock with your passcode, then try again."

After you unlock, the data folder reopens and the app catches up once: any ledger check or grant re-verification that is due runs, and the pending queue goes back to waiting for its receipts.

**Auto-lock** is in Settings > Identity key and is on out of the box. After the "Idle time" passes (1, 5, 15, 30 or 60 minutes; 15 by default) with no activity, the app locks itself. Any key press or mouse movement restarts the clock. With auto-lock off, the app locks only when you press "Lock" or quit. Transactions already sent are not affected by locking.

#### 5.3 Forgotten passcode

On the passcode gate, press "Forgot passcode" to reach "Recover". Only the **primary identity's** credentials can reset the passcode:

- **The primary is a recovery-phrase identity**: the card reads "Enter the primary identity's recovery words to reset the passcode." Fill in the 12 words.
  - You can paste the whole phrase into any box; it spreads out from that box onward.
  - Every box shows dots, and a word not on the word list is marked red.
- **The primary is an imported local-key identity**: the card reads "Use the primary identity's key file to reset the passcode." Press "Choose a file…" to pick the key file exported from the primary identity, and enter the "File password".

Then set a new passcode, entering it twice. On success the wrong-try count is cleared, the old passcode no longer works, and the toast reads "Recovered, passcode reset".

- A secondary identity's words or key file are refused: "This belongs to a secondary identity." and "Only the primary identity recovers the passcode; use the primary's, or restore from a backup."
- If the key store has not yet recorded which identity is primary, both ways are offered, and the primary is recorded at this unlock.
- "Cancel" returns to the passcode card.

#### 5.4 Restoring from a backup at the gate

At the bottom of the recovery card is "Restore from backup…":

1. Choose a whole-machine backup file, enter the "Backup password" and press "Recover".
2. "Replace this machine with the backup?" shows the backup's name and contents, with the note "The backup will replace this machine's identities and data; anything here that isn't in the backup will be lost." Press "Replace".
3. "Set a new passcode", entering it twice.

The machine's identities, keys and data are replaced by the backup's, re-encrypted under the new passcode, and the wrong-try count is cleared. Common refusals:

| Message | When |
|---|---|
| "Wrong backup password." | The backup password is wrong (not counted as a wrong passcode try) |
| "Not a ZIKARON backup." | The file is not a backup |
| "The backup comes from a newer version." | Update the app first |
| "The backup contents are malformed." | The backup file is damaged |

#### 5.5 Resetting an empty key store

If a passcode was set but no identity was ever created, and then 5 wrong tries lock the gate, there is nothing in the store to recover. The card says so and offers only "Reset key store":

- It deletes the empty key store and returns to wizard step 1.
- Setting a new passcode then creates a new master key. Any local data sealed under the old key, which can no longer be opened, is moved unchanged into a `set-aside` folder in the machine folder. Nothing is deleted.
- While the store still holds any identity, this button does not appear.

---

## 6 · Recorder: a full walkthrough

**Recorder seat** (choose "Recorder" at the top of the sidebar):

1. **First run.** Go through the wizard in section 3: set a passcode, create an identity, choose "Ethereum mainnet", create the ledger, send a little ETH to the Recorder address and export a whole-machine backup.
2. **Record a work.** Drop the file onto the drop zone on Home, fill in "Record name", press "Add to ledger", then press "Add to ledger" again on the confirmation card.
3. **Put it on chain.** Click the "Pending" card on Home (or press "Put all on chain" under the "Pending" segment of Records), then press "Confirm and send" on the "Put on chain" card.
4. **Grant it.** Open "Grant" and press "New grant":
   1. Fill in the grantee's address.
   2. Press "Choose a record" and pick a record that is on chain.
   3. Drop in the terms file and choose a validity period.
   4. Press "Add to ledger", then again on the confirmation card.
   5. Put it on chain as in step 3.
5. **Hand it over.** On the grant's detail page, press "Copy grant code" or "Export grant file…" and send it to the grantee. The grant file carries your ledger, so they can run the full check.
6. **Day to day.** Watch the Alerts page. To take a grant back, press "Revoke grant…" on its detail page.

---

## 7 · Recorder pages

#### Home (Recorder)

**Top left: the drop zone** "Drop a record to register it". Drop one item or click to choose one, and "New record" opens.

**Top right: the "Pending" card** shows how many entries are queued. It has three states:

- "Click the card to put them on chain": click it to open the "Put on chain" card straight away.
- "Waiting to go on chain": the transaction has been sent and is waiting for confirmation.
- "All on chain".

**Three tiles in the middle.** The whole tile is clickable:

| Tile | Shows | Click to open |
|---|---|---|
| "Ledger status" | The last ledger check's result, "OK" or "Problems", with the total entry count below | Ledger |
| "Records" | How many records are not deleted, with the newest record's name below | Records |
| "Expiring within 30 days" | How many unrevoked grants end within 30 days, by chain time | Grant |

- Before anything has been read, a tile shows "Not read yet".
- "Expiring within 30 days" shows "No chain time" when no chain time is known yet.

**"Recent"**: the last five ledger entries, with number, type, summary and on-chain state. Click a row to open it. With no entries it shows "No entries yet".

### Record on chain

"Records" in the sidebar. On the right of the toolbar is the segmented control "All" | "Pending N".

The "All" segment, top to bottom:

- The drop zone "New record".
- The search field "Find by record name or content fingerprint", with "Check a file" beside it.
- The record table, with the columns "Record", "On-chain state", "First anchor block time" and "Entry number", newest first. Deleted records are struck through.

**Check a file.** Choose a file. The app computes its SHA-256, looks for a record with the same fingerprint in the current ledger, and answers under the search field:

- "Entry N · on chain, block time …"
- "Entry N · on chain, block time not read"
- "Entry N · not confirmed on chain yet"
- "Entry N · confirmed · last checked …": this session's check has not come back yet, so the answer is from the last one.
- "This ledger has no entry with this digest"

If the same content was recorded more than once, the answer is the entry that is on chain; a deleted one gets " (deleted)". If this machine signed it, one more line, "Signed from …", gives the file name used at the time.

#### New record

The "New record" card has two steps.

**Step one: fill it in**

1. Drop a file, a folder or a Git repository into the card's drop zone, or click to choose one. The content fingerprint is computed this way:

   | Kind | How |
   |---|---|
   | File | SHA-256 of the file's bytes |
   | Folder | Over every file name and its content |
   | Git repository | Over the current commit |

   The zone then shows "Selected: …"; click it again to pick something else ("Click to replace"). If "Record name" is empty, it is filled in with the name (a file's name without its extension; a folder's or repository's full name).
2. Fill in "Record name".
3. Optionally, open "Recorded for (optional)". It has four fields: "App", "Their identity (0x…)", "Their reference" and "Their role (optional)".
   - If you fill in any of them, the first three are required, and "Their identity (0x…)" must be `0x` followed by 40 hexadecimal digits.
   - These words are written into the record entry as they are; the app itself does not read them.
4. Optionally, under "Advanced options", choose a "Linked Git repository (optional)".
   - The reading below: "No repository linked" until one is saved; "Linked, not checked yet" after "Add repository"; "N commits since the last record" after "Check commits".
   - "Add repository" saves the chosen repository in settings.
   - "Check commits" counts the new commits of the saved repository again.
5. Press "Add to ledger" (it reads "Send to chain now" when "Auto put on chain" is on). The note says "Cannot be changed once recorded, only annotated."

**Step two: confirm**

- The card lists "File", "Recorded for (optional)" and "Cost"; the content fingerprint and ledger location are under "Details".
- The solid red "Add to ledger" writes the entry and queues it, with the toast "#N added to pending, N in total".
- "Back" returns to step one.
- With "Auto put on chain" on, the app estimates gas right after writing and opens the "Put on chain" card. The card covers every entry in the queue that can be sent.

**Batches.** Drop two or more files into the card's drop zone at once to make a batch:

- The zone reads "N files chosen · signed one by one" and lists the file names. "Clear" empties it.
- On confirming, each file becomes one record, with the SHA-256 of its bytes as the fingerprint. The one "Record name" is written into every entry.
- The batch stops at the first failure. Entries already signed stay signed, and the toast reads "Signed N, stopped at file K: file name · reason". The remaining files stay in the list; fix the problem and press again.
- A batch cannot contain folders.

For each file signed (a single file, or each file of a batch), this machine remembers the file name and where the file was. "Check a file" and record kit export use this to find originals. Folders and Git repositories are not remembered.

#### Record detail

Click a row in the record table. The title is the record's name, with "#N · Anchored record" and the on-chain state below it.

**"Basic information"**

- "On-chain state", "First anchor block time", "Entry number".
- "File": the file name signed on this machine.

"Details" holds "Content fingerprint", "Entry ID", "Previous entry", "Size (bytes)" and "On-chain transaction".

**Buttons**

- Still in the pending queue: "Put on chain now".
- Not in the queue: "Grant to someone". It works only once the state is "Confirmed"; until then the hint reads "Put it on chain before granting it", or "Waiting for this check".
- "Export record kit".
- At the bottom: "Delete record…".

#### Deleting a record

"Delete record…" opens the card "Delete this record?", with the note "The record will be marked as deleted. Grants already issued are not affected." Press "Delete".

- A "Deletion" entry pointing at the record is added to the ledger. The original entry is not changed by a single byte.
- From then on the record is struck through, its page says "Deleted by entry #N. It can no longer be granted.", its page no longer offers the grant and export buttons, and New grant no longer offers it.

Whether the deletion itself goes on chain depends on whether the record was ever made public:

| The deleted record | Result |
|---|---|
| Still queued and not sent, or never on chain | The record leaves the queue and the deletion stays local. The two rows show "Deleted" and "Deleted locally", and the toast reads "Deleted (entry #N). Not put on chain." |
| Already sent, in a block or on chain | The deletion entry is queued and goes on chain like any other entry |

With a network set up, no ledger check yet, and the record not in the pending queue, deleting is refused: "The ledger has not been checked." and "Click Sync at the bottom of the sidebar."

For the entry's format and how it is read, see Appendix C.

### Pending queue

**The pending segment** ("Pending N" in the Records toolbar):

- The table lists number, type, summary and the date each entry was queued. When empty it reads "Nothing pending".
- Below the table is the "This batch" card: "N entries · gas not estimated" before an estimate and "N entries · gas estimate …" after, with "Put all on chain" and "Sync". After a batch has been sent, a "Last batch" line appears ("N put on chain · N removed").
- Click a row for its detail: "Summary", "Queued at" and "Sent together" ("First N"), with the button "Put on chain now".

**Putting entries on chain**

1. Press "Put all on chain" or "Put on chain now". The "Put on chain" card opens, showing "Entry count" ("First N") and the "Gas cap"; the gas estimate is under "Details". The note says "Once on chain it cannot be withdrawn; it usually confirms within minutes".
2. Once the estimate is in, press "Confirm and send". The app signs, sends the transaction and waits for the receipt.

All entries in a batch travel in one transaction.

**The gas cap** is worked out from the base fee of the latest block: (base fee × 2 + tip) × gas limit. If the base fee cannot be read, it is taken as 0.0006 ETH.

**Checks before sending**

- If the balance cannot cover the cap, nothing is sent. The message is "Insufficient balance." and "Needs N ETH, have N ETH.", with "Copy address" so you can top it up.
- If the estimate fails, the card becomes "Cannot estimate gas" and gives the reason; "Retry" tries again.
- Nothing can be sent while the ledger is locked or its chain is broken.

**After sending**

- In a block within 30 seconds: the toast reads "Put N on chain".
- Not yet in a block after that: the toast reads "Submitted N, waiting for confirmation", and the app asks for the receipt again every 15 seconds.
- If you quit and reopen the app, it keeps waiting for that transaction's receipt and never sends it twice.
- If the receipt shows the transaction failed: "The transaction was sent but failed on chain." The entries stay in the queue and can be sent again.

**When a node refuses**

- The transaction itself was refused: "Transaction nonce already used.", "Transaction already submitted, waiting for confirmation.", "Gas price too low.", "Gas limit too low.", "The registry contract rejected the transaction."
- The node's limits or setup: "The node is rate-limiting requests.", "This node does not accept transactions.", "This node requires credentials.", "The node is on a different network."
- Connection problems: "Cannot reach any node.", "The node did not answer in time.", "The node's answer was too long.", "The node's answer could not be read.", "Could not open a secure connection to the node."
- Anything else: "The node rejected the transaction.", with the node's own words under details.

**On-chain states** (the same on every page):

| State | Meaning |
|---|---|
| "Pending" | Queued, not sent yet |
| "Waiting to go on chain" | Sent, waiting to get into a block |
| "Confirming" | In a block, waiting for a ledger check to confirm it |
| "Confirmed" | A ledger check has confirmed it is anchored |
| "Confirmed · last checked MM-DD HH:MM" (shortened in lists to "Confirmed · HH:MM") | This session's check has not come back yet, so this is the last result; it turns grey after a day |
| "Reverted" | The transaction failed on chain; the entry stays queued and can be sent again |
| "Not sent" | Refused before sending (for example, not enough balance); the entry stays queued |
| "Not on chain" | Not queued and not on chain |
| "Deleted", "Deleted locally" | See "Deleting a record" under "Record on chain"; on detail pages the "On-chain state" row reads "Deleted, not put on chain" for the deleted record |

Anything that requires "Confirmed" (for example, choosing a record for a grant) accepts only this session's check.

**Auto put on chain.** The "Auto put on chain" switch in Settings > Network is off by default:

- Off: the write button reads "Add to ledger", and new entries wait in the queue until you send them.
- On: the button reads "Send to chain now", and after writing the app estimates gas and opens the "Put on chain" card.

The switch applies to every kind of write: records, grants, revocations and so on.

### Ledger contents

"Ledger" in the sidebar. On the right of the toolbar:

- The "More" menu: "Import existing records…", "Change key or hand over…", "Add note…" and "Sign a claim for someone…".
- The segmented control "All", "Pending" and "Grant". "Pending" lists every entry that is not yet confirmed.

**The ledger status line** at the top of the page: "Ledger status · OK/Problems · N total".

- If the root entry is not yet confirmed on chain, red "Root not confirmed on chain" is added.
- If some entries can be sent, "Put all on chain" appears on the right.
- Click the line to open the ledger check card.

**The entry table**, newest first:

- The columns are number, type, summary, first anchor block time and on-chain state.
- A deletion's summary reads "Deleted record · record name". One that does not follow the convention reads "Invalid deletion: …" (see Appendix C).

**Entry detail.** Click a row:

- An anchored record shows "Record", "On-chain state" and "First anchor block time", with the buttons "Export record kit" and "Use for grant" (only when confirmed).
- A grant: see "Grant detail" under "Grant list".
- Other entries show "Summary", "On-chain state" and "First anchor block time".
- Buttons any entry may have:
  - Still queued: "Put on chain now".
  - Neither on chain nor queued: "Add to pending". An entry that has been on chain before cannot be queued again: "This entry is already on chain." and "No need to queue it again. The on-chain state comes from the last sync; sync again if unsure."
  - Confirmed entries other than records: "Note…".
- "Details": "Entry ID", "Type", "Sequence", "Author", "Previous entry", "Size (bytes)" and "On-chain transaction".

### Ledger check

The ledger check compares every local entry with the anchors on chain. It runs:

- On the interval set in Settings > Notifications (300 seconds by default). With the interval at 0 it does not run on a timer, only when you press Sync or in the case below.
- Straight away when this ledger has no check result yet, or when the ledger has changed (a new entry written, a batch put on chain).
- Only when nodes, chain ID and registry contract are all set. Otherwise it shows "Never checked: no network set yet".

The check card is titled "Ledger check · Passed/Has gaps/Failed" and gives a light and a count for each item:

- "On chain", "Not on chain yet"
- "Mismatched on chain", "Findings", "Excluded"
- "Imports unproven", "Unrecognized types", "Malformed entries"
- "Unavailable", "Unproven", "Void", "Discarded"

Below: "N entries · checked every N s". The raw result is under "Details".

- **When the ledger is "OK"**: there is a check result, the chain is not broken, and every problem item is zero. Entries not yet on chain do not count as problems.
- A check counts deletion entries as unrecognized types. When that count equals exactly the number of deletion entries, the item shows as a green "N convention entries".
- If the result is "Failed" (a broken chain), the whole ledger becomes read-only and the broken-chain bar appears at the top.

#### Notes

"Add note…", or "Note…" on an entry's detail page, opens "Add note to entry":

1. Write the "Note text" (required).
2. Pick the entry to annotate in the "Target entry" list, or under "Advanced options" use "Or paste an entry id".
3. Press "Add note" to write it.

A note is saved as a new entry; the original entry is not changed.

### Import anchor proofs

This brings anchors that a key put on chain earlier, but that are not in the ledger, into the ledger. Open the "Import existing records" card:

- **With "Key address" empty**, it lists anchors this machine's key sent earlier that are not in the ledger. The list comes from the last ledger check, so it needs the network set up and one check done; otherwise it shows "Chain not read".
- **With an address filled in** (pick one from "Address book" if you like), it scans the chain for the anchors that key sent.

**The table** has a switch on each row and the columns "Time" and "State"; block number and digest are under "Details". The states are:

- "Passed": switched on by default. Only these rows can be imported.
- "No such transaction", "Wrong sender", "Digest does not match"
- "Already in the ledger"

**When importing another person's address**, a "Their signature" section appears below:

1. Copy the "Text" and send it to the holder of that key ("Send this to the holder of that key; paste what they sign back.").
2. When they sign it and send it back, copy their signature and press "Paste" beside "Signature". It is checked on the spot, and its light shows "Waiting", "Passed" or "Mismatch".

You can import without their signature; those anchors are then recorded as unproven. If the ledger gets new entries, they need to sign again.

**By hand.** The "By hand" fold takes anchors typed one per line; press "Check these rows".

Finally, press "Import N". This writes one adoption entry and queues it.

**Signing a claim for someone.** When you hold the key someone else wants to claim, choose "Sign a claim for someone…" from "More":

1. Paste in the text they sent. The card shows how many anchors; who is claiming and the head of their ledger are under "Details".
2. Enter this machine's passcode to sign.
3. Copy the signature and send it back to them.

Text the app cannot read gives "This is not a claim text."

### Hand over ledger

"Change key or hand over…" opens a card with two steps:

1. Fill in three fields and press "Hand over":
   - "New key address": checked as soon as it is complete. The new key must have no anchor proofs on chain. The reading shows "The new key has N anchor proofs"; you can only continue at zero.
   - "Kind": "Key change only" or "Hand over to someone".
   - "Statement" (optional).
2. Check the details and press the solid red "Hand over" to write it.

The note says "The new key must have no ledger, and this cannot be undone." From then on the new key continues the ledger. This machine becomes read-only, with "Ledger handed over to a new key; read-only" at the top of the page.

### Grant list

"Grant" in the sidebar. On the right of the toolbar is the blue "New grant".

**The grant list**

- Search: "Find by grantee, record or state".
- The table columns are "Record", "Validity", "Grant no." and "State", plus the on-chain state.
- The state is judged by chain time: "Active", "Expired", "Not yet active", "Revoked" or "Unknown".
- With no grants it reads "No grants yet".

### Draft a grant

Ways in:

- "New grant" on the grant list starts an empty form.
- "Grant to someone" on a record's detail page, or "Use for grant" on an entry's detail page, fills in that record.

The form's fields:

- **"Grantee"**: paste a `0x…` address. The "Recent addresses" menu on the right lists the address book and earlier grantees.
- **"Which record"**: press "Choose a record" and pick one. Only records on chain can be chosen; the rest are greyed out with the reason:
  - "Grant after it is on chain"
  - "Going on chain, please wait"
  - "Waiting for this check"
  - "Deleted"
- **"Terms file"** (required): drop the terms file in, or click to choose it.
  - Once chosen, the file's name and size are shown; "Replace…" picks another.
  - The file's SHA-256 is filled into "Terms file fingerprint" under "Advanced options", marked "Filled from the terms file".
  - When the grant is signed, a copy of the terms file is kept in the data folder. It later goes to the grantee with the grant file and record kits, so they can recompute the fingerprint themselves.
- **"Validity"**: "7 days", "30 days" (the default), "90 days" or "Custom".
  - The presets count from chain time. With no chain time yet, it says "No chain time for presets: sync first or choose Custom"; sync first, or use Custom.
  - "Custom" takes "Start (seconds)" and "End (seconds)" as Unix seconds.
- **The overlap light**: "Overlapping live grants: N". It counts grants on the same record that are marked exclusive on this machine, overlap in time and are not revoked.
- **The "Exclusivity, sublicense and scope" fold**:
  - The "Exclusive grant" switch: "Recorded on this Mac only. Cannot be changed after signing." The exclusive mark is kept only on this machine and is not written into the grant; put the exclusivity terms in the terms file.
  - "Upstream grant (sublicense only)".
  - "Scope".

Addresses and hashes in the grantee, terms file fingerprint and upstream fields must be all lowercase. With a capital letter, the form says "The address has capital letters. Change it to lowercase before signing." and the sign button is disabled.

**Signing**

1. Press "Add to ledger" (or "Send to chain now"). It works once the grantee, the record and the terms file fingerprint are all filled in, and, with a preset validity, once there is chain time.
2. The confirmation card "Sign grant" shows "Record", "Validity", "Terms file" and "Cost"; the grantee and the fingerprints are under "Details". The note says "After signing it can only be revoked".
3. Press it again on the card to sign. The toast reads "Issued #N. N pending". Then put it on chain as described under "Pending queue".

If the same record already has an exclusive grant with an overlapping period, signing is refused and the card "Exclusive grant conflict" lists the grants that clash; "Cancel" closes it.

### Pre-signing checklist

The "Before signing" side column of the New grant page is a two-step checklist:

- The two steps are "Fill in terms" and "Draft your first grant", with the key's balance shown below.
- You can type a note in "Hash or note" and press "Mark done: …" to tick the next step. "Reset checklist" starts over.
- The checklist is kept in the data folder.

#### Grant detail

Click a row in the grant list. The title is "#N · Grant".

**"Basic information"**

- "Record", "Validity", "State", "On-chain state", "First anchor block time".
- "Exclusive": "Yes · recorded on this Mac only", "Yes · recorded on this Mac only · no terms file" (an exclusive mark from an earlier version), or "No".
- "Attached terms file": the file name, or "No terms file".

"Details" holds the grantee, the terms file fingerprint and the entry's machine values, plus "Show revocations".

**Buttons**

- **"Copy grant code"** puts the `zikaron-grant:…` code on the clipboard.
- **"Export grant file…"** writes a `grant-<first ten hex characters of the grant's entry ID>.zkgrant` file to the "Saved to" location, numbering it if the name is taken.
  - The file holds the entries of every hop in the grant chain, the issuer's ledger, the terms files and the grant code.
  - It reads the chain once before exporting and refuses if the chain cannot be read.
  - The toast gives the location, how many hops, and how many terms files.
- **"Put on chain now"** appears while the grant is still queued.
- **"Revoke grant…"** at the bottom appears while the grant is not revoked.

### Revoke grant

"Revoke grant…" opens the card "Revoke this grant":

- The card shows "Record" and "Original validity". "Details" holds the grantee and a field "Ruling file fingerprint (optional)".
- The note says "This cannot be undone. The grantee will be alerted".

Press "Revoke" to write the revocation entry and queue it. Once it is on chain, the grantee's revocation watch will find it.

### Record kit

Open it with "Export record kit" on a record's or entry's detail page; that entry is already selected.

**Entry range.** The "Entry range" line shows "All N" (choosing nothing means the whole ledger) or "N selected".

- "Choose…" opens "Choose entries", with a switch on each entry so you can pick any set. There are also "All" and "Clear selection".
- Revocations of the chosen grants, and the ledger's creation and handover entries, come along automatically; the receiver needs them to tell whose ledger this is.
- If some chosen entries are not on chain yet, red text says "N selected entries are not on chain yet: the package holds no on-chain proof for them", with "Put on chain now" beside it.

**Originals of the selected records.** This lists the files this machine signed for the chosen records. You can "Remove" any of them.

- A file no longer where it was signed is marked "No longer at its registered location".
- A file that has been changed is marked "Not an original of a selected record; left out of the package".

**Attachments (optional).** Drop files or folders here. Each item shows the name it will have in the kit ("Exported as files/…").

- File names inside a kit may use only lowercase letters, digits, `.`, `_` and `-`. Other names are rewritten automatically: lowercased, each run of other characters replaced by one `-`, leading and trailing `-` and `.` trimmed, and `-` plus the first eight characters of the original name's digest inserted before the extension (for example `name-1a2b3c4d.pdf`).
- When names were rewritten, the kit carries a table of original names, `files/zikaron-names.json`.
- A row whose name cannot be rewritten is marked red "This file name cannot be converted. Rename the file.", and the kit cannot be exported until it is fixed.
- Attachments must be originals of the chosen records; any other file is left out.

**Other settings.** "Note" adds a note to the kit. "Saved to" defaults to `kits` in the data folder.

**Exporting.** Press "Verify and export":

1. The app reads the chain once.
2. It creates a folder `kit-<first eight characters of the earliest chosen record's fingerprint>` at the save location (plain `kit` if no record is chosen), numbering it if the name is taken.
3. The kit checks itself and is written only if it passes. The toast reads "Record kit exported to …".

If it cannot be written, the reason is given:

| Message | Reason |
|---|---|
| "Cannot write to this location." | The save location is not writable |
| "Something with the same name is already at the save location." | A name clash at the save location |
| "A file name cannot be used inside the package." | A file name could not be rewritten |
| "Two files inside the package have the same name." | Two files with the same name in the kit |
| "The package failed its own check and was not saved." | The self-check failed |

**The side column**

- "How recipients verify": the receiver drops the kit into Verify > Verify record.
- "Readings and reference (not in the package)": "Calculate" computes three readings for the first chosen record: "First recorded", "Max depth" and "Continuity". They are shown on the page only and do not go into the kit.
- "Record packages exported on this Mac": one section for each kit this machine has exported from this ledger.
  - You can set a "Fetch address", the https address where the kit can be fetched online. Left empty, it is the publish address plus the kit's path.
  - "Delete local copy", pressed twice, deletes the local kit folder. The ledger is not touched.

### Depth

Depth readings describe how long a record has been in the ledger and how continuously it has been on chain. There are three: "First recorded", "Max depth" and "Continuity" (on chain / span). They appear in these places:

- Recorder: "Readings and reference (not in the package)" in the side column of the Record kit page; press "Calculate".
- User: in the Record verifier, when "Record hash (optional)" is filled in.
- Due diligence: on the Other ledgers page with "Record hash (optional)" filled in, "Max depth" is under "Details".

The command line's `depth` verb uses the same calculation.

---

## 8 · User

#### A full walkthrough

**Grantee seat** (choose "User" at the top of the sidebar):

1. The first time the app opens, the wizard runs as a Recorder: set a passcode, create an identity, choose a network and create the ledger (free, local only). The rest can wait.
2. Switch to "User" at the top of the sidebar. A User ledger is needed only to sublicense; when you get there, press "Set up ledger…" on the sublicense page.
3. Paste the grant code you received into the "Verify a grant" card on Home, press "Start check" and read the result.
4. Open "My grants", press "Add grant", paste the grant code or drop in the grant file, and press "Verify and add" to keep it.
5. When you receive the work itself, open Verify > Verify grant, put in the grant, the file and the terms file, and press "Verify" once to check all three together.
6. Watch the Alerts page. Grants about to expire, revoked grants and issuer ledgers that were handed over all show up there.

#### Home (User)

- **Top left, "Verify a grant"**: paste a grant code and press "Start check" to see the result on Verify grant.
- **Top right, the drop zone "Drop a record kit to check its records"**: drop in or choose a record kit to go to Verify record and check it straight away.
- **Three tiles**. The whole tile is clickable:

  | Tile | Shows | Click to open |
  |---|---|---|
  | "Needs attention" | How many re-verified grants did not pass all checks | My grants |
  | "Active grants" | How many grants are within their validity, with "N issuers" (issuers of all grants held) below | My grants |
  | "Check a received record" | Whether the file in the last grant check was a "Match" | Verify grant |

  - Until this session's re-verification has run: "Needs attention" shows the last results, marked "last checked MM-DD HH:MM"; "Active grants" shows how many grants you hold, marked "Not verified".
  - "Check a received record" reflects only grant checks made in this session: until one has been run with a file, it shows "--".
- **"Recent"**: check results, revocation-watch alerts and the pending count from this session. Click a row to open its page. With nothing to show it reads "No recent activity".

### Grant vault

"My grants" in the sidebar. The toolbar has the segmented control "All" | "By issuer", "Verify again" and the blue "Add grant".

#### Add grant

The "Add grant" dialog:

- Paste a grant code into "Paste grant code", or drop a grant file onto "Drop a grant file". If the code carries a sublicense chain, every hop in it is added.
- A dropped grant file is checked on the spot: "… verified · N hops in the chain · M terms files".
- A pasted grant code gets a note: a grant code carries no issuer ledger, so only the signature and validity can be verified. For the full check, ask the issuer to export a grant file.
- "Grant name" and "Issuer name" are optional and kept only on this machine.

Press "Verify and add":

- Signatures and format are checked first. If any grant fails, none are added; the dialog stays open and gives the reason.
- Grants already held, byte for byte, are skipped. If all of them are already held, it says "This grant is already in My grants." and the dialog stays open.
- A copy of the grant file is kept in the data folder, and the issuer ledger inside it is used for later checks.

#### Grant cards

Each grant has a card with its verdict, record name, issuer, validity and last time on chain.

- The verdict is "All passed", "Has gaps", "Failed" or "Revoked by issuer".
- Until this session's re-verification reaches it, a card shows the last verdict marked "last checked …", turning grey after a day.
- A grant never re-verified shows "Not verified".
- If a file in the store cannot be read, the top of the page says "File rejected: …" for each one.

Click a card for its detail page:

- **"Basic information"**
  - "Issuer", "Validity", "First anchor block time".
  - "Remaining": for example "Expires in N days", by chain time.
  - "Upstream": "None (issuer is the author)", or the state of the upstream ledger.
- **Status notes**
  - Revoked: "The issuer has revoked this grant." and "Sublicenses based on it are void too."
  - Has gaps: one sentence per gap, for example that the issuer ledger is missing or that the grant is not yet confirmed on chain.
  - Issuer ledger handed over: "The issuer's ledger was handed over to a new key".
- **The credential card**: "Saved to" and "Create credential". When this session's re-verification has passed all six checks, "Sublicense to others" appears too.
- **"Advanced options"**: "Upstream ledger folder (optional)"; "Save upstream" remembers it, and saving it empty clears it.
- **"Details"**: the entry ID, terms file fingerprint, content fingerprint, verdict and the raw result of each of the six checks.

#### Verify again

"Verify again" re-verifies every grant you hold:

1. It looks for the issuer's ledger, first on this machine and then in the vault. The vault is the ledgers carried by grant files you added, plus folders saved with "Save upstream".
2. It scans the chain, updates each card's verdict and records the result.

Re-verification also runs on the interval in Settings > Notifications (600 seconds by default), once nodes, chain ID and registry contract are set.

### Upstream ledgers

The "By issuer" segment in the My grants toolbar groups the cards by issuer, with problem groups first. Each group's header gives the state of the issuer's ledger. Grants not yet re-verified are grouped under "Not verified".

### Credential

A credential is proof of a grant for a third party, such as your customers: a text file and a QR code. Press "Create credential" on the credential card:

- It needs the network set up, and it reads the chain once.
- The app follows the grant chain back to its root using the grants you hold, and creates a `badge-<first ten characters of the grant ID>` folder at the save location containing `badge.txt` and `badge.svg` (the QR code).
- The detail page then shows "Credential created" and the QR code.

The other party verifies it by pasting the text into Verify > Verify grant.

### Draft sublicense

**Starting.** A grant that this session's re-verification has passed on all six checks has "Sublicense to others" on its detail page:

1. A confirmation card "Sublicense" shows "Record", "Issuer" and "Upstream validity", with the note "The sublicense ends when the upstream grant is revoked or expires."
2. "Draft…" opens the sublicense page. Nothing is written at this point.

**The sublicense page.** The upper half shows the upstream grant; the lower half is the same form as "Draft a grant":

- The record is fixed to the upstream grant's record.
- The validity is not automatically capped at the upstream validity. Watch it yourself.
- Press "Add to ledger" (or "Send to chain now" when "Auto put on chain" is on). The confirmation card is titled "Sign sublicense"; press again to sign.

The sublicense is written to your own (User) ledger and goes into your own pending queue. After signing, "Go put them on chain" and "N pending" appear under the form.

**What it needs.** Sublicensing needs a User signing key and ledger:

- No key: the page says "No key has been created." Press "Create key…".
- No ledger: the page says "No ledger yet." and "Open your ledger before sublicensing." Press "Set up ledger…".

### Revocation watch

After each re-verification of all your grants, the app checks two things:

- Has an issuer revoked a grant you hold?
- Has an issuer's ledger been handed over to a new key?

If either has happened:

- The window asks the system for attention once (on macOS the Dock icon bounces), and a toast appears: "Alert: Held grant revoked" or "Alert: The issuer's ledger was handed over to a new key".
- A red "Revocation watch" row is added at the end of the Alerts page, linking to My grants.

Each event alerts only once. No toast appears while the first-run wizard is open.

---

## 9 · Verify

"Verify" in the sidebar. Both roles have "Verify grant" and "Look up a ledger"; the User also has "Verify record".

Verifying does not use the key store or send transactions, and results are not saved. Only two things are written on this machine: changes to the address book, and snapshot files written by "Save snapshot".

### Grant check

Put in all three things, then press "Verify" once:

1. **"Grant"**: paste a grant code, or press "Choose a file…" to pick a grant file.
2. **"File" (optional)**: drop in or choose the work you received.
3. **"Terms" (optional)**: drop in or choose the terms file.

Files you put in show their name and size; "Remove" takes them out.

**"Advanced options"**

- "Ledger folders (one per hop)": point each hop of the grant chain at the issuer's ledger folder by hand.
- "Nodes (one per line, optional)", "Registry contract (optional)", "From block": empty means the values in Settings.
- "Check as of (seconds, optional)": empty means chain time.

**Where the issuer's ledger comes from.** The app looks in this order:

1. This machine: the issuer's ledger on this same machine.
2. The vault: ledgers carried by grant files you added, and upstream folders you saved.
3. A record kit: the kit or grant file you point to, and the ledger carried by the grant file being checked.
4. A publish address: the issuer's https address. Each file is fetched and the kit must pass its check before it is used.

In the result card, below "Work", "Validity" and "First anchor block time", one line per hop, "Ledger source: …", names where it was found; if nowhere, it shows "None".

- "Replace…" or "Add…" lets you give an https address, record kit, ledger folder or grant file yourself; then press "Start check".
- A ledger from a publish address shows "From https://… · N files · verified", with "Fetch again".
- If a source cannot be read, the reason is given for each one.

**The result card** starts with three lines:

| Line | Shows |
|---|---|
| "Grant" | "All six checks passed", "Has gaps" or "Failed", with the tag "Active", "Undecided" or "Invalid" |
| "File" | The file name, tagged "Match" or "Mismatch" against the work hash the grant points to (for several hops, the last hop) |
| "Terms" | The file name, tagged "Match" or "Mismatch" against the terms fingerprint in the grant |

Anything not given reads "Not provided"; anything that could not be read reads "Unreadable". Below come "Work", "Validity" and "First anchor block time".

**The "Six checks" fold**

- "Grant signature", "Issuer ledger intact", "In the issuer's ledger"
- "Confirmed on chain", "Within validity", "Not revoked"

**Grey lights.** A check that could not be completed is grey, with the reason beside it:

- "Issuer ledger missing", "Issuer ledger unreadable"
- "No nodes configured", with a "Go to Settings" link
- "Chain unreadable right now", "Not yet confirmed on chain", "Chain time unavailable"

Below, each kind of gap gets one sentence saying what to do next, for example:

- "The issuer ledger is missing. Drop in a record kit or a grant file, or enter an https address, then verify again."
- "This grant is not yet confirmed on chain. Check again in a few minutes."
- "No nodes are set. Set up the network in Settings, then check again."

**Sublicenses (several hops).** Each hop gets its own six lights, and between hops a light "Hop N links to hop N+1".

**The summary at the bottom**

- "All match" (green box): the grant passed all six checks, and the file and terms, where given, match. Anything not given does not affect this.
- "N do not match" (red box): the grant failed, or the file or terms do not match; each counts as one.
- "Not everything could be checked": nothing mismatches, but the grant has gaps or a file could not be read.

"Has gaps" never counts as passing.

### Check a received record

When you receive the work itself, put it into the "File" box on the Verify grant page and press "Verify" together with the grant. The app computes the SHA-256 of the file's bytes and compares it with the work hash the grant points to (for several hops, the last hop). The result card's "File" line reads "Match" or "Mismatch"; "Details" shows the byte count, the computed digest and the expected digest side by side. After changing the file, press "Verify" again. The User home tile "Check a received record" shows the last result.

### Record verifier

The "Verify record" tab under "Verify" in the sidebar (User only).

Drop a record kit folder, a grant file or a single record file onto "Drop a record kit or record file", or click to choose one. It is checked as soon as it is in; "Verify now" checks again.

**"Advanced options"**

- "Record content (optional)": a local path or an `https://` publish address. An `http://` address gets "Only https addresses are supported."
- "Record hash (optional)".

**The "Verification result" card** has three lights:

| Light | What it can show |
|---|---|
| Record kit | "Record kit intact, signatures valid", "Record kit check failed: …", or "Verified as a single file" for a single file |
| Anchor proofs on chain | "N match the chain", "N not on chain", or "Not compared with the chain" when the chain could not be read (including no network) |
| Issuer ledger | The ledger check result; "This is part of the ledger" when a kit carries only part of it; "Issuer ledger not read" if it could not be read |

**Record by record.** For a kit, a grant file or a publish address, a "Record by record" table follows, one row per record:

- "Original in kit": the original's fingerprint is recomputed and compared, giving ✓, ✗ or missing.
- "On-chain state".
- "First anchor block time".

**The conclusion**

- No mismatch: a green "All match".
- Any mismatch: "N mismatches." and "See the error details and confirm with the sender before accepting."

With a record hash filled in, three readings for that record are shown as well: "First recorded", "Max depth" and "Continuity".

### Other ledgers

The "Look up a ledger" tab under "Verify" in the sidebar.

Fill in "Author address" (or pick from "Address book" on its right) and press "Read". The app scans the chain for that address's anchor proofs and looks for its ledger in the same four places as described under "Grant check".

- "Record content (optional)" under "Advanced options" can point to a record kit, grant file, ledger folder or https publish address.
- "Add to address book" and "Remove from address book" change only this machine's address book.

**"Overview"** has four tiles:

| Tile | Shows |
|---|---|
| "Ledger status" | The check result; with anchor proofs but no ledger, "Anchor proofs only, no ledger" |
| "Record count" | Not counting deleted records |
| "Grant history" | With "N records on chain" below |
| "Last on chain" | When something was last put on chain |

- If none of the four places has the ledger, it says "No ledger content yet: provide a record package or publish address", and the record and grant counts read "Not obtained" rather than 0.
- Below the tiles is a table of grants: work, validity, first anchor block time.

**"Entry list"** lists the other ledger entry by entry; click a row for a read-only detail. Deletions are read the same way as in your own ledger (Appendix C).

### Due diligence

A User does due diligence on the "Look up a ledger" page, which has more under "Advanced options":

- "Record hash (optional)".
- "Validity needed (optional)": "From (seconds)" and "To (seconds)".
- "Snapshot location" and "Save snapshot".

"Overview" gains a "Grant conflict check", which tells you whether the period you need overlaps active grants the author has already issued. It checks only when both "Record hash (optional)" and "Validity needed (optional)" are filled in; otherwise it shows "No record entered; not checked" or "No period entered; not checked":

- Overlap, in red: "N active grants overlap this period".
- No overlap, in green: "No active grants overlap this period".

"Save snapshot" writes this due-diligence result to `snapshot-<first ten characters of the address>.json`. A snapshot is not proof.

---

## 10 · Alerts

### Alert list

"Alerts" in the sidebar. Each row of the table is one matter: a light, the item and its current state. A row that needs action has an "Open …" link at its right; clicking the row goes to the page where it can be dealt with and opens the relevant part (for example the ledger check card, or the grouping by issuer). A row with nothing to do reads "All clear".

| Recorder | User |
|---|---|
| "Pending" | "My grants expiring" |
| "Queue backlog" | "Grant revoked" |
| "Grants expiring" | "Whole-machine backup" |
| "Whole-machine backup" | "Ledger handover" |
| "Ledger check result" | "Upstream issue" |

- When "Pending" and "Queue backlog" say the same thing, they are merged into one row.
- **"Whole-machine backup"** reads "Not backed up yet" if no backup was ever exported, and "N not backed up" when ledger entries or received grants have been added since the last one.
- **Revocation watch** alerts come at the end of the table, one row each. They are listed only in the session that found them (the "Grant revoked" and "Ledger handover" rows count only these); after a restart they are gone, though a revoked grant still shows "Revoked by issuer" in My grants.
- The number next to "Alerts" in the sidebar counts the amber and red rows.
- The full message from the last failed chain read is under "Details".

**How alerts reach you.** Alerts travel as a window attention request plus a toast. When one of these newly appears, the window asks the system for attention once and an alert toast appears: for the Recorder, "Grants expiring", "Whole-machine backup", and "Ledger check result" when the chain is broken; for the User, "My grants expiring" (expired ones included), "Whole-machine backup", "Upstream issue", and revocation watch. Other amber and red rows (for example "Pending" and "Queue backlog") show only on this page and in the sidebar count, with no alert.

- Each matter alerts only once; the alerts already given are remembered in the data folder's settings. If the situation changes (a new deadline, the ledger broken somewhere else, new entries in an upstream ledger), it counts as a new matter and alerts again.
- Network errors never alert; they only show "Sync failed" in the status line.
- There is no setting to turn alerts off.

---

## 11 · Settings

"Settings" in the sidebar opens a list of seven sections; click one to open it. Both roles have the same sections.

| Section | Contains |
|---|---|
| "Language and time" | Interface language and time zone |
| "Appearance" | Light, dark or follow the system |
| "Identity key" | Identities, passcode and key backup |
| "Network" | Nodes, registry contract and publish address |
| "Notifications" | How often the ledger is checked, or grants re-verified |
| "Local data" | Encryption, backup and data folder |
| "About" | Version and setup check |

**Where settings are kept**

- Appearance, auto-lock, the last language chosen, the chosen network and the record of the last whole-machine backup are kept on this machine. They apply to every identity and are used on the passcode gate too.
- Everything else is kept in the current data folder. Without a data folder, or when it is read-only, these cannot be changed: the page stays as it was and a toast gives the reason.

#### 11.1 Language and time

- **"Language"**: "中文" or "English". The passcode gate uses the language chosen last.
- **"Time zone"**: "UTC" (the default) or "Follow the system".
  - Every time in the window is shown in it. The full form is `2026-09-24 23:46:12`; narrow places show only the date or `MM-DD HH:MM`.
  - If the system time zone cannot be read: "The system time zone could not be read; moments show in UTC".

#### 11.2 Appearance

"Light" (the default), "Dark" or "Follow the system". It takes effect at once.

### Identity and keys

Settings > Identity key.

A card at the top shows the current identity's name and its role and kind. "Copy address" copies the current role's signing address.

**The "Identity" group**

| Row | Meaning |
|---|---|
| "Current identity" | Role, kind and name |
| "Created" | The date this identity was created |
| "Gas balance" | The balance of the current role's signing address, or "Not read" |
| "Backup status" | "Recovery phrase confirmed, key file exported", "Recovery phrase confirmed", "Key file exported" or "Not backed up" |
| "Backup file" | Whether the exported key file is still on disk right now (checked against the real file) |
| "Saved to" | "This Mac's key store (passcode encrypted)" |

If the current role's key cannot be found, an extra "Key address" row explains why, for example "No signing key yet" or "Key not in this Mac's key store". For the empty role of a local-key identity it reads "Not in use. Import another key, or use a mnemonic identity."

**The "Primary" group** ("Only the primary identity can recover the passcode.")

- The current identity is primary: it reads "This identity" and says whether the passcode is recovered with the recovery phrase or with the key file.
- The current identity is not primary: it shows the primary's name, with "Set as primary…" below.
  - On the "Set as primary" card, enter this machine's passcode and press "Set as primary".
  - The old primary becomes a secondary identity. Every key and all local data are re-encrypted under a new master key; the passcode stays the same. The change happens completely or not at all.
  - A local-key identity imported from a raw private key must export its key file before it can become primary.
  - It is refused while a data folder is not where it should be (an external drive not connected) or while background writes are running. Try again later.

**The "Passcode" group**

- **"Auto-lock"** and **"Idle time"** (see 5.2).
- **"Change passcode…"**: enter the current passcode, then the new one twice. The toast reads "Passcode updated". A wrong current passcode counts as a wrong try.
- If the current passcode is digits only: "Your passcode is digits only. Adding letters is recommended."
- With no passcode yet, the group has only "Go set one".

**The "Export" group**

- **"Export key file…"**: a standard keystore file for use in other wallets. For a local-key identity that is primary, it is also how the passcode is recovered. Steps:
  1. Enter this machine's passcode.
  2. Choose a file password of at least 8 characters. The strength bar is only a guide and never blocks you.
  3. Choose where to save it and press "Export key file".
  - The file can be read only by you. The app reads it back to check it; only then does it say "Key backed up to …" and update the backup status.
- **"Show recovery phrase…"**: after you enter the passcode, the 12 words are shown. "Hide and close" clears them. A local-key identity has no recovery phrase, so this row is unavailable.

**The "Change identity" group**

- **"Switch identity…"**: lists every identity. Click a row to expand it and see both roles' full addresses; only "Switch" actually switches.
- **"New identity…"**: an optional "Label", then generate, write down and check 12 words, as in wizard step 2.
- **"Import key…"**: three tabs, "Recovery phrase", "Private key" and "Key file".
  - Recovery phrase: 12 boxes; the whole phrase can be pasted.
  - Private key: 64 hexadecimal digits, with or without 0x.
  - Key file: drop in the keystore file and enter the "Key file password".
  - For a private key or key file, choose "Import as which identity" ("Recorder" or "User"). The key fills only that role; the other stays empty.
  - An optional "Label". Press "Import identity".
  - An imported identity is secondary and cannot recover this machine's passcode. An identity already on this machine is not imported twice, but if its keys are missing, importing it puts them back.
- **"Rename…"**: change an identity's label. It only helps you tell identities apart and affects no verdict.

New and imported identities are secondary. The exception is a machine with no identity yet, where the first one created becomes primary.

**Deleting an identity.** "Delete identity…" at the bottom:

- **The primary identity cannot be deleted directly.** First make another identity primary ("Set primary"). If it is the only identity, create or import another one first.
- **Deleting a secondary identity**:
  - The card states the consequence ("The ledger can no longer be continued" for a Recorder, "Grants under this identity will no longer be yours" for a User) and lists which roles' ledgers already have entries.
  - Enter this machine's passcode and press "Delete identity".
  - Only the keys in the key store and the identity's registration are removed. The data folders are kept.
- **An identity never backed up cannot be deleted**: "This identity is not backed up and cannot be deleted." and "Back up the key or confirm the recovery phrase first. A recorder can also record a handover."
- A Recorder should record a handover first (see "Hand over ledger"); otherwise a new key cannot continue the ledger.

**"Details"**

- Both roles' full addresses.
- Derivation paths, for recovery-phrase identities only.
- The identity list file.
- The signing domains this identity can sign: a Recorder signs entries and co-signatures and sends anchor transactions; a User signs entries in its own ledger and sends anchor transactions.

#### 11.4 Network

**Readings**

- **"Network"**: where this data folder's network settings came from.
  - "Ethereum mainnet (taken from the network this Mac chose)"
  - "Custom": set by hand.
  - An amber "No network set yet".
- **"Chain reading"**:
  - "N nodes agree"
  - "Only 1 node responded; no cross-check"
  - "Not read"
- Nodes, registry contract, chain ID and start block are under "Details".

**"Auto put on chain"**: a switch, off by default, kept separately for each data folder (see "Pending queue").

**Buttons**

- **"Use this Mac's default network"**: appears only when all three are empty. It fills them in from the network this machine chose, with the toast "Network set: Ethereum mainnet". It is refused if this machine chose "Custom".
- **"Edit nodes…"** opens the editor:
  - Write nodes as `chain-id=node-url`, separated by spaces, and press "Save nodes".
  - Fill in "Chain ID", "Registry contract" and "Start block" and press "Save chain settings".
  - Once you have saved your own values, "Network" reads "Custom".
- **"Read chain"**: reads the signing key's balance once.

**Node connections.** A node address starts with `https://` or `http://`. For https nodes, the certificate and host name are checked on every connection; if the certificate fails, the chain cannot be read. When a node refuses or a connection fails, the message names the cause (rate limit, credentials needed, wrong network, certificate, timeout and so on), with the node's own words under details.

**Publish address.** The https address where you intend to put record kits online:

- Only `https://` is accepted; anything else gives "Only https addresses are supported."
- The app does not upload anything. Put the record kits on that static host yourself.
- Enter the address and press "Save published address". Once an address is saved, "Record kit to compare" and "Check publication" appear. "Check publication" fetches the kit from the publish address file by file and compares it with your local copy:
  - "Published": "All N files match."
  - "Publication incomplete": "N files missing, M do not match.", listing the missing and mismatched files.
  - "Cannot reach the published address", with the reason.

#### 11.5 Notifications

- Recorder: the "Ledger check" interval, "Every N seconds" or "Manual only". The default is 300 seconds.
- User: the "Grant verification interval". The default is 600 seconds.

"Change interval…" takes a whole number of seconds. An interval of 0 turns the automatic run off.

How alerts reach you is covered in section 10.

### Storage

Settings > Local data.

**Readings at the top**

- "Contents": "N identities · N ledger entries · N records · settings"
- "Protection": "Passcode-encrypted"
- "Usage": "… of … used", red when over the limit

#### Whole-machine backup

One backup file restores everything: every identity and key on this machine, all local data and all settings. It does not contain the master key, the passcode or the wrong-try count.

- "Last backup": the time, or "Never".
- "Not backed up": how many entries have been added since.
- "Export backup…":
  1. Enter this machine's passcode.
  2. Choose a password in "Backup password, at least 8 characters" and repeat it in "Enter the backup password again".
  3. Choose where to save it and press "Export".
  - The file is named `zikaron-backup-YYYY-MM-DD.zikaron`. A second export on the same day gets a number; nothing is overwritten.
  - The app reads the file back and opens it to check it; only then does it say "Backup exported: …".
  - If you forget the backup password, the backup cannot be restored.
- "Restore from backup…":
  - Choose the backup file, enter the "Backup password" and this machine's passcode, and press "Recover".
  - The machine's identities, keys and data are replaced by the backup's. The passcode stays the same. Anything on this machine that is not in the backup is lost.
  - The restore happens completely or not at all.
  - Old files that still cannot be opened afterwards are moved unchanged into the `set-aside` folder in the machine folder. Nothing is deleted.

With only a recovery phrase and no whole-machine backup, only data readable on chain can come back.

#### Fetching the ledger (restored identities)

When you import an identity again from its recovery phrase, private key or key file, its ledger does not come with it. A red box appears at the top of this page, "This identity was restored: its ledger has not been fetched". Until the ledger is fetched, this identity can read but cannot write to its ledger.

To fetch it:

1. Choose a whole-machine backup, enter the "Backup password" and press "Fetch ledger".
2. The app takes this identity's ledger from the backup and compares it with this key's anchors on chain:
   - **Everything matches**: "Fetched N entries; all N records on chain are in the ledger: writing is open". Writing is allowed again.
   - **The chain has anchors the ledger lacks**: the box changes to "Newer entries exist elsewhere: N records on chain are not in the fetched ledger" and stays read-only. Find the newer backup and fetch again.
   - **Entries on this machine conflict with the fetched ones**: a conflict card appears. After "Fetch and replace", the machine's previous data is kept as "Old data" and can be opened read-only with "Look up" in the "Old data" group on this page.

### Ledger mirror

Only on a Recorder's Local data page.

"Export ledger mirror…":

- Exports the ledger to a folder you choose, laid out as `ZIKARON-backup/<address>/<role>`.
- If the folder already holds an older mirror of this identity, only the new entries are added.
- A mirror is an export for other tools to read. It cannot be used to restore this machine; use a whole-machine backup for that.

#### Data folder

Each role of each identity has its own data folder.

- "Change data folder…": choose another folder and press "Open data folder". This lasts only until you quit; the next launch returns to this identity's own data folder.
- "Measure usage": counts the space used and the number of entries.
- "Import grants folder…" (User): choose a folder and press "Import folder" to import the grant entry files (`<64 hexadecimal digits>.entry`) in it in one go.
- The empty role of a local-key identity has no data folder, so these buttons are unavailable.

**"Advanced options"**

- "Usage limit (bytes)", then "Save limit".
- "Move to an empty folder":
  - Choose a folder and press "Move". The data folder moves there and becomes this identity's data folder.
  - If the chosen folder is not empty, the data goes into a new `ZIKARON` subfolder inside it.
  - The old location is left untouched; delete it yourself afterwards.
- "Adopt existing ledger folder", then "Adopt in place": takes over an existing ledger folder, re-checking every entry before connecting it.
- "Writing": "Writable", or "Paused: reconcile after restore".
- "Last reconciled" and "Reconcile now": checks the local ledger offline and, if it passes, allows writing again.

"Details" gives the data folder's path, the encryption used, the path of the last whole-machine backup and the instance lock.

Only one window can write to a data folder. A window opened later can only read, and shows the read-only bar at the top.

### About

- **"Version"**: `ZIKARON Desk 0.1.0`.
- **"Setup check"**: a light for each item, with unfinished ones marked "To set up".
  - Recorder: "Signing key", "Passcode", "Gas", "Create the ledger", "Whole-machine backup".
  - User: "Signing key", "Passcode", "Node setup", "Ready", "Whole-machine backup".
- **"Run wizard again"**: see section 3. It is unavailable when the current identity is a local-key identity and the current role is its empty one.

### Diagnostics

The "Details" fold at the bottom of the About page: "Cores and signing domains", "Build type", "Fonts", "Log output", "Running tasks", "Last self-check", and "Run self-check" (runs a self-check in the background and writes the result to "Last self-check"). The fonts row says where each face comes from: the Latin and monospace fonts are built into the app; Chinese comes from the system's PingFang on macOS and from Noto Sans SC built into the app on Linux.

---

## 12 · Command line (`zikaron`)

The command line reads and writes the ledger folder you give it.

A ledger in ZIKARON Desk's own data folder is encrypted, and the command line has no passcode, so it refuses to read one: `E_UNREADABLE` "已锁定:这是 ZIKARON Desk 封存的本机数据,命令行不读" ("locked: this is ZIKARON Desk's sealed local data; the command line does not read it"), exit code 2. To work with the command line, export a "Ledger mirror" first. Each file in the mirror's `entries/` folder is one entry's bytes, named with 64 hexadecimal digits and no suffix. Single files can be read with `show --path` and similar flags. To use them as a `--ledger` folder, copy them into an empty folder and add `.entry` to each file name.

| Verb | What it does |
|---|---|
| `keygen` | Generate a key and print its address and private key |
| `init` | Write the ledger's creation entry into an empty ledger |
| `history` | Append an anchored record |
| `grant` | Sign a grant and write it to the ledger |
| `revoke` | Sign a revocation and write it to the ledger (ruling file digest optional) |
| `adopt` | Write an adoption entry |
| `attest` | Produce a co-signature with an outside key |
| `succeed` | Write a key change or handover entry |
| `annotate` | Write a note |
| `retract` | Write a deletion entry (the target must be a record in this ledger that is not already deleted) |
| `anchor` | Anchor a hash on chain (through the registry contract, or as a bare self-transfer) |
| `scan` | Scan for anchors using the chain settings |
| `audit` | Audit a ledger against scan results and print a report and verdict |
| `check-grant` | Run the six checks on a grant |
| `chain-check` | Check a sublicense chain hop by hop (`--hop <grant file>[=<audit input file>]`, repeatable) |
| `depth` | Depth readings for a record |
| `fpm-sign` | Sign a fingerprint manifest |
| `ack-sign` | Sign an acknowledgement |
| `badge` | Encode or decode a credential (exactly one of `--encode` and `--decode`) |
| `kit-export` | Export a record kit (written only if it passes its self-check) |
| `show` | Show an entry's author, ID, type, previous entry, sequence and content |

**Exit codes**

| Code | Meaning |
|---|---|
| 0 | Affirmative |
| 1 | Negative (the entry was refused, or the verdict is FAIL, BROKEN_CHAIN or NO_LABEL) |
| 2 | Misuse (nothing on standard output) |
| 3 | Partial (PARTIAL, GAPS, UNAVAILABLE) |
| 4 | No answer (node unreachable, readings disagree, and so on) |

Standard output is one canonical JSON value with no trailing newline. `--now` injects the moment to judge at; without it, only chain time is used. For output shapes and every flag, see `CLI-SCHEMA.md` in the repository.

---

## Appendix A · Built-in networks

Each network comes with two public nodes from different providers, so their answers can be cross-checked.

| | Ethereum mainnet (default) | Sepolia testnet |
|---|---|---|
| Chain ID | 1 | 11155111 |
| Registry contract | `0x36Ea8A857a5FE813429d4D9947000C644A88809A` | `0xC29410B882c4C3b77e33659d2f06ac563e7B08a3` |
| Start block | 26087229 | 11715660 |
| Nodes | `https://mainnet.gateway.tenderly.co`<br>`https://rpc.flashbots.net` | `https://sepolia.gateway.tenderly.co`<br>`https://rpc.sepolia.ethpandaops.io` |

To use the testnet:

1. Choose "Custom" in wizard step 3.
2. In Settings > Network, press "Edit nodes…", enter `11155111=https://sepolia.gateway.tenderly.co 11155111=https://rpc.sepolia.ethpandaops.io` and save the nodes.
3. Fill in the chain ID, registry contract and start block, and save the chain settings.

## Appendix B · Files on this machine and encryption

**The machine folder** holds the key store, the identity registry, the machine settings `machine.json`, the record kit index and so on.

- Its location is given by the pointer file `~/.zikaron-desk` (one line holding an absolute path). Without a pointer it is `~/.zikaron-desk.d/`.
- Earlier versions kept the machine folder at `~/Library/Application Support/ZIKARON`. If there is no pointer and that folder exists, the app keeps using it.

**The data folders**, one for each role of each identity, hold the ledger, settings, pending queue, received grants and so on.

**Local encryption.** Ledger entries, settings, the pending queue, the identity registry, the record index, and received grants and terms files are all encrypted with a local data key derived from the master key (XChaCha20-Poly1305). While the app is locked none of it can be read, and someone who copies this machine's files cannot read it without the passcode.

**Files that are not encrypted this way**

- Files read before unlocking: the key store itself (encrypted separately), `machine.json`, and the data folder pointer.
- Exports meant for others: record kits, the record kit index, grant files, credentials, ledger mirrors, key files and whole-machine backups. A whole-machine backup is encrypted with its backup password.
- Files other people send you.

**Whole-machine backups** derive their key from the backup password with scrypt (standard parameters N=262144, r=8, p=1) and are encrypted with XChaCha20-Poly1305.

## Appendix C · How deletion entries are read

"Delete record…" writes an entry of type `retraction`, signed and put on chain like any other:

| Field | Value |
|---|---|
| `entryType` | `retraction` |
| Body `subject` | Required: the entry ID of a record in this ledger, `0x` followed by 64 lowercase hexadecimal digits |
| Body `note_md` | Optional: a note |

This is ZIKARON Desk's reading convention and is not part of the `zikaron/1` protocol text. Other tools list such an entry as an unknown type (`UNKNOWN_TYPE`), and the ledger's verdict does not change because of it.

How this app reads it:

1. A well-formed entry whose `subject` points at a record in this ledger that is not yet deleted: the record reads as deleted. It is struck through everywhere, no new grant can be signed for it, and its detail page no longer offers the grant and export buttons.
2. If the deleted record was never made public, the deletion stays local; if it was already sent or on chain, the deletion entry goes on chain as usual (see "Record on chain").
3. After a record on chain is deleted, the chain record stays as it is, and it still counts towards depth.
4. Grants already issued are not affected.
5. A second deletion of the same record reads as "Invalid deletion: target was already deleted".
6. A `subject` that is not a record reads as "Invalid deletion: target is not a record".
7. A `subject` not found in this ledger reads as "Invalid deletion: target entry is not in this ledger".
8. A missing or malformed `subject`, or extra fields, reads as "Invalid deletion: its content does not fit the convention".
9. An invalid deletion affects no record and never stops the ledger from being read.

Other people's ledgers in "Look up a ledger" are read by the same rules.
