# ZIKARON Desk User Manual

ZIKARON Desk is a desktop application for your own machine. It records your works or files in an append-only ledger, anchors that ledger to Ethereum (or the layer-2 network you choose), and lets you issue grants from it. Whoever receives a grant can verify it independently, on any machine.

This manual describes version 0.1.4. Words that appear in the interface are quoted exactly as the window shows them.

---

## 1 · Concepts

**Identity.** An identity is a set of signing keys. There are two kinds:

- **Recovery-phrase identity**: derived from 12 recovery words. It has two keys, one for the Recorder role and one for the User role.
- **Local-key identity**: an imported private key or key file. It fills only the one role chosen when it was imported.

**Two roles.** Every identity has a "Recorder" role and a "User" role. You switch between them at the top of the sidebar. Each role has its own key, address and ledger (data folder).

- **Recorder**: records your own works and issues grants.
- **User**: receives other people's grants, verifies them, and can sublicense them.

A recovery-phrase identity derives its two keys at `m/44'/60'/0'/0/0` (Recorder) and `m/44'/60'/0'/0/1` (User). The two addresses differ, and each needs a little gas. Each identity records the network it chose, shared by both its roles; identities on one machine may use different chains.

**Ledger.** Each role has its own ledger: a chain of signed entries that can only be appended to. Old entries are never changed or removed. The first entry is "Create the ledger", the ledger's one and only root. Entry types:

| Type | What it is |
|---|---|
| Create the ledger | The root of the ledger |
| Record | The content fingerprint of one record |
| Grant | Lets an address use a record for a period of time |
| Revocation | Revokes a grant |
| Note on entry | Adds a note to an earlier entry |
| Adoption | Brings anchors this key put on chain earlier into the ledger |
| Key handover | Hands the ledger to a new key |
| Deletion | Marks a record as deleted (see Appendix C) |

**Putting on chain (anchoring).** New entries wait in the "Pending" queue until a transaction on the identity's network (Ethereum mainnet, or Arbitrum One, OP Mainnet and so on) carries them; after that they are on chain. The whole queue can go out at once for the cost of a single transaction. Once an entry is on chain, anyone can check against the chain that the ledger held it at that moment.

**Grant.** A grant entry names the grantee, the record, the validity period and the terms file fingerprint. There are two ways to hand one over:

- **Grant code**: a line of text starting with `zikaron-grant:`. It lets the receiver verify only the signature and the validity period.
- **Grant file**: a `.zkgrant` file that carries the issuer's ledger, so the receiver can run the full check.

A User can also write a **sublicense** in their own ledger, passing an upstream grant on to someone else.

**Record kit.** One file (`.zkit`) or a folder exported from a ledger. It holds the chosen entries, the original files and the on-chain proofs, so someone else can verify them offline.

**Local encryption.** All local data is encrypted under one master key, and the master key is opened by an 8-character passcode. While the app is locked, none of the local data can be read.

**Primary identity.** The first identity created on a machine is its primary identity by default; another can be made primary later in Settings > Identity key. Only the primary identity's recovery phrase or key file can reset a forgotten passcode (see section 5).

---

## 2 · Installation

Version 0.1.4 comes as packages for macOS with Apple silicon and for x86_64 Linux, and as a zip for x86_64 Windows. Every package carries the third-party licences (on macOS in `ZIKARON.app/Contents/Resources/THIRD-PARTY-LICENSES.txt`, on Linux in `/usr/share/doc/zikaron-desk/THIRD-PARTY-LICENSES.txt`, on Windows in the unpacked folder); in the app they are under "About".

**Upgrading from an earlier version.** From 0.1.3: install over it; the machine folder and data folders are used as they are, with nothing else to do. A file 0.1.2 or an earlier version left in an older form is not read: it is named as not encrypted in this version's form and left as it is. That holds for the machine folder those versions wrote (key store, identity registry, settings), so coming from them this version starts from an empty machine folder; ledgers kept by 0.1.0 and 0.1.1 also hold their entries in an older form. To bring a Recorder's ledger across:

1. In 0.1.2, "Export ledger mirror" (see "Ledger mirror") to a folder outside the machine folder, then quit.
2. Move the earlier version's machine folder aside, together with its pointer file (Appendix B says where both are), so that this version finds neither. Data folders kept inside the machine folder move with it; the mirror is what comes across.
3. Start this version: it begins with an empty machine folder. Set a passcode, then import the identity with the same recovery phrase.
4. When the wizard reaches "Create the ledger", do not create a new ledger: click "Adopt existing ledger folder", choose the mirror's `author` folder and press "Adopt in place" (see "Step 4 · Create the ledger" in section 3). Every entry is checked again, and the ledger is then checked against this key's records on chain; once adopted, the step is done and the wizard goes on. A home that already has its own ledger does not take another.
5. Run the ledger check on the Ledger page and see it come back "OK".

Encrypted local files written by this version cannot be read by 0.1.2 or earlier versions.

**macOS**

- `ZIKARON-0.1.4-macos-arm64.dmg`: open it and drag `ZIKARON.app` to Applications. The `zikaron` command line is inside the app bundle at `ZIKARON.app/Contents/MacOS/zikaron`; to type `zikaron` in a terminal, turn on "Enable the zikaron command" in Settings > Command line and AI control (see "Command line"): it places a link in `/usr/local/bin` pointing at the command line inside the app bundle, and when needed macOS shows its own administrator password dialog. An app run straight from the dmg cannot do this and says "Move the app to the Applications folder first".
- `ZIKARON-0.1.4-macos-arm64.pkg`: an installer that puts the app in `/Applications` and the command line at `/usr/local/bin/zikaron`, so `zikaron` works in a terminal right after installing and "Enable the zikaron command" is not needed (with the pkg's command line already in that place, the switch shows "Taken by another install").
- macOS 11 or later is required.
- The app is signed with a self-signed certificate (Kaptonia) and is not notarized by Apple. Files downloaded in a browser carry macOS's quarantine flag, and macOS refuses to open them; on recent macOS, "Open Anyway" in System Settings > Privacy & Security does not always work either. Check the SHA-256 with `shasum -a 256 <file>` first, then remove the quarantine flag in Terminal:

  - With the dmg: before opening the dmg, run the command below, then open the dmg and drag `ZIKARON.app` to Applications:

    ```
    xattr -d com.apple.quarantine ~/Downloads/ZIKARON-0.1.4-macos-arm64.dmg
    ```

    If you have already dragged the app in, run this instead:

    ```
    xattr -dr com.apple.quarantine /Applications/ZIKARON.app
    ```

  - With the pkg: run the command below, then double-click the pkg to install:

    ```
    xattr -d com.apple.quarantine ~/Downloads/ZIKARON-0.1.4-macos-arm64.pkg
    ```

  Change the paths to wherever you downloaded the files.

**Linux**

- `zikaron-desk_0.1.4_amd64.deb`: for Debian, Ubuntu and their derivatives. Install it with `sudo apt install ./zikaron-desk_0.1.4_amd64.deb`; the window program is `zikaron-desk` (also in the applications menu) and the command line is `zikaron`, both in `/usr/bin`, so `zikaron` works in a terminal right after installing ("Enable the zikaron command" in Settings > Command line and AI control shows "Provided by the installed package").
- `ZIKARON-0.1.4-x86_64.AppImage`: no installation; make it executable with `chmod +x` and run it. Running it opens the window program; the `zikaron` command line inside is not exposed, and "Enable the zikaron command" is not available in an AppImage ("Not supported for this install"), so install the `.deb` for the command line. It uses FUSE; on a system without FUSE, run it with `--appimage-extract-and-run`.
- Both need glibc 2.31 or later (Ubuntu 20.04, Debian 11 and later). The packages are not signed; check the SHA-256 with `sha256sum <file>` first.
- For other architectures or distributions, build from source as described under "Build from source" in the repository's README. In an app built from source, "Enable the zikaron command" places a link named `zikaron` in `~/.local/bin`, which must be on your `PATH`. On x86_64 Linux, `packaging/linux/build.sh` makes a `.deb` and an AppImage; on aarch64 it makes only the `.deb`, and the AppImage step stops with an error because no AppImage runtime is pinned for that architecture.
- File dialogs go through the desktop portal, so install `xdg-desktop-portal` and a backend such as `xdg-desktop-portal-gtk`. Without a portal, clicking to choose a file says "The system file dialog can't open on this machine."; drag the file into the window instead.
- Dropping files on the window needs X11: when there is an X display (including XWayland on a Wayland desktop) the window runs on X11; in a pure Wayland session with no X display it runs on Wayland and does not receive dropped files.
- On Linux, a drop zone that takes either a file or a folder (such as New record) opens a dialog that picks files only; to give a folder, drag the folder in. "Choose folder" picks folders as usual.

**Windows**

- `ZIKARON-0.1.4-windows-x86_64.zip`: for Windows 10 and 11 on x86_64. Nothing to install: unpack it into any folder and double-click `zikaron-desk.exe`; the command line is `zikaron.exe` in the same folder. The programs carry their own runtime; nothing else is needed.
- `zikaron.exe` is a command line and runs in a terminal: in its folder, hold Shift, right-click an empty spot and choose "Open in Terminal" ("Open PowerShell window here" on Windows 10), then type `.\zikaron.exe` with a verb and its flags. Double-clicked in Explorer, it opens a window, prints its usage and closes at once, which looks like a crash but is not one.
- Check the SHA-256 first in PowerShell with `Get-FileHash <file> -Algorithm SHA256`.
- The programs are not signed. On first start Windows says "Windows protected your PC"; click "More info", then "Run anyway". On Windows 11 with Smart App Control on, unsigned programs are blocked outright: there is no "Run anyway" and no way to allow this one program; to use it, first turn Smart App Control off in Windows Security › App & browser control › Smart App Control settings.
- To upgrade, replace the folder with the new version's. Machine data is not in the unpacked folder but in `%LOCALAPPDATA%\ZIKARON\` (see Appendix B); deleting the unpacked folder does not delete it.
- No Start menu entry or desktop shortcut is added. To type `zikaron` in any terminal, turn on "Enable the zikaron command" in Settings > Command line and AI control, which adds the command line's folder to this user's `PATH` (no administrator rights needed); terminals opened after that find `zikaron`. If you move the unpacked folder elsewhere, the switch shows off; turn it on again to add the new place. Replacing the folder in place, as when upgrading, needs nothing.
- On Windows, as on Linux, a drop zone that takes either a file or a folder opens a dialog that picks files only; to give a folder, drag the folder in.

The window program takes no arguments: started with any, it opens no window and only says why. To work with a file, open the window and drop the file on it; for the command line, use `zikaron`. If the window cannot open (usually the graphics driver does not support the graphics interface it needs, Metal on macOS and OpenGL on Windows and Linux, or it runs in a virtual machine or remote desktop without 3D acceleration), it says why as well; on Windows, started by double-clicking in Explorer, it says so in a system dialog.

Building from source on macOS and Windows is also described in the repository's README.

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

- On the left: "Back", and "Later" on steps that may be skipped. On the last step this key reads "Do it later" and closes the wizard with the same message as "Finish".
- "Back" goes to the page before: from the check of the copied words to the same twelve words (the check boxes emptied), and from a step's first page to the step before.
- At the bottom left of the sidebar, just above "Exit setup", "中文" | "English" switches the interface language from the first step on. The choice is kept on this machine and used on the passcode gate too (the gate does not switch it).
- On the right: the step's own action key until the step is done (step 1 has none; filling the boxes submits), then a blue "Next". The last step has "Finish", which shows "First run complete; remaining items are in Settings > About".

**Leaving the wizard.** "Exit setup" at the bottom left of the sidebar (the language switch just above it), or Esc, returns you to where you were before the wizard opened. It is there once this machine has a passcode and an identity: on the first start it appears as soon as both are set, and the same after a restart; before that there is nowhere to go back to. Leaving at "Create the ledger" keeps the identity and creates no ledger. Finished steps stay finished. At the next start the wizard opens again only if the passcode or identity is still missing, or a Recorder's ledger has not been created; for the other unfinished steps, use "Run wizard again" in Settings > About.

- If fresh recovery words have not been checked yet, it asks "Exit setup?" first: "Exit" discards the words and leaves; "Keep checking" or Esc stays in the wizard.
- While the passcode or the identity is still missing (on a true first run, both are), there is no way out and Esc does not leave.

**Other ways to open it**

| From | Opens at |
|---|---|
| "Run wizard again" in Settings > About | The first unfinished step; step 1 if all are done. Disabled while the current role has no identity |
| "Go set one" in Settings > Identity key | Step 1 |
| "Create key" on the "Sublicense" page (when there is no signing key yet) | Step 2 |
| "Create the ledger" on the "Sublicense" page (when there is no ledger yet) | Step 4 |

Whenever there is no passcode yet, it opens at step 1.

#### Step 1 · Set a passcode

1. Type 8 letters or digits into the eight boxes. Case matters, and the characters show as dots.
2. When the row is full it asks you to "Enter it again". If both entries match, the passcode is set; if not, the row shakes and you start over.

- A passcode that is too simple is refused on the spot: eight identical characters, or digits that run in sequence or read as a date. The message is "Too simple. Choose another."
- Under the boxes: "8 letters or digits, case-sensitive." A forgotten passcode is recovered with the recovery phrase or a key file exported from the primary identity (see 5.3).

The passcode comes before the identity because keys are stored in a key store that the passcode opens. The app never reads or writes the system keychain; keys live only in its own key store.

#### Step 2 · Create an identity

You may first type a name in "Label" (optional, just for you to recognise it). Then click one of the card's three rows: "Generate recovery words" (marked "Recommended"), "Import an existing key" or "Restore from backup".

**Generate recovery words (a new identity)**

1. Press "Generate recovery words".
2. Make sure no one can see your screen, then click the cover ("Make sure no one can see your screen, then click to show"). The 12 words appear in three rows of four. While they show, "Hide recovery phrase" below covers them again.
3. Write them down in order on paper and press "I have written them down". It only works after the words have been shown, and it covers them again.
4. Fill in the three words it asks for ("Word N") and press "Confirm and create". If they match, the identity is created.

**Import an existing key**: press "Import an existing key". See "Import key" under "Identity and keys". Imported here, it is the machine's first identity and becomes primary; a private key then exports its key file on the same form (the key file password twice and where to save it), and a forgotten passcode is recovered with that file.

An imported key may have written a ledger elsewhere, so it cannot write at first: once a network is chosen in step 3, the app checks this key's records on chain against the ledger here. If the chain holds no record of this key (a new key, say) or every record is in the ledger, writing opens; if the chain holds records the ledger lacks, see "Fetching the ledger" under "Storage".

**Restore from a backup**: press "Restore from backup", choose a whole-machine backup file, enter the "Backup password" and press "Recover". Identities, keys, ledgers and settings all come back, re-encrypted under the passcode you just set, and the wizard closes.

The first identity created on a machine is its primary identity.

#### Step 3 · Network

There are two choices:

- "Ethereum mainnet", marked "Recommended", on public nodes; selected by default until the current identity or this machine has chosen a network.
- "Custom": left empty, to fill in the chain, contract and nodes in Settings > Network (a preset can fill them).

Pressing "Next" makes the choice. What you choose here is the network of the identity made in step 2, and both its roles (recorder and user) use it: with mainnet, the current role's data folder gets the chain id, registry contract, start block and two public nodes at once, and the other role's data folder gets the same the first time it opens; with "Custom", a network a preset filled into the current data folder is cleared (one filled in by hand is left alone), and reading the chain or anchoring says no network is configured until you open "Edit nodes" in Settings > Network, fill it in by hand or pick a preset, and save. The machine also remembers this choice: the wizard and "New identity" select it first next time.

Sepolia testnet, Arbitrum One and OP Mainnet are not listed here. To use one, choose "Custom", then open "Edit nodes" in Settings > Network and pick it under "Preset" (see Appendix A). The one exception: if the current identity or this machine has already chosen one of them (for example, restored from such a backup), it appears between "Ethereum mainnet" and "Custom".

Anchoring on mainnet costs real ETH in gas.

#### Step 4 · Create the ledger

An identity imported in step 2 creates its ledger once it has been checked against the chain after step 3 (see step 2).

1. Describe what the ledger is for in "Ledger description (optional)".
2. Press "Create the ledger". A confirmation card shows "Ledger description" and "Cost" ("Free (local only)"). The ledger's location is under "Details".
3. Press "Create the ledger" on the card to write it.

**Adopting an existing ledger folder.** With a ledger already at hand (a ledger folder, or a mirror "Export ledger mirror" wrote), there is no need to create one: "Adopt existing ledger folder", beside "Create the ledger" (there while this data folder has no ledger), opens a card of the same name. The card says to choose a ledger folder, or the folder an earlier version's "Export ledger mirror" wrote: its entries are checked one by one and sealed into this data folder, and the folder itself is not changed.

1. Choose that folder under "Ledger folder".
2. Press "Adopt in place" (it can be pressed once a folder is chosen). "Cancel" or Esc closes the card and adopts nothing.

This is the same adoption as "Adopt existing ledger folder", then "Adopt in place" under "Advanced options" in Settings > Local data, with the same rules and the same words when it is refused (see "Data folder" under "Storage"). Once adopted, this data folder has its root entry and the step is done just as after creating the ledger: the right-hand key becomes "Next", and adding gas comes next. For an imported identity, the adopted ledger is then checked against this key's records on chain (see "Fetching the ledger").

#### Step 5 · Add gas

The page shows the "Key address", the "Gas balance" and a payment QR code ("Scan with a wallet to pay").

1. Send a little ETH to this address from your wallet. You can select and copy the address.
2. Press "I have sent it". The wizard sends no transaction; it reads the balance on chain once. Above zero, the step is done and the wizard moves on. Still zero, it says "The chain does not show the transfer yet; press “I have sent it” again in a moment."

Reading the balance needs nodes. If you chose "Custom" in step 3, set up nodes in Settings > Network first.

#### Step 6 · Whole-machine backup

The card's "Contents" row lists what the backup holds (identities, ledger entries, records, settings). One backup file restores everything; with only the recovery words, only data readable on chain comes back. "Do it later" skips the step.

Press "Export backup" and fill it in as described under "Whole-machine backup" in "Storage". Once the export succeeds, the step is done.

---

## 4 · The window and common controls

#### 4.1 Window layout

The sidebar is on the left and the page is on the right. Across the top of the page is the toolbar: back and forward arrows and the page title on the left, the page's own buttons and filters on the right. When an imported identity's chain holds records its ledger lacks, the toolbar of Home, of each sidebar page's own page and of the Settings home also shows an amber "read-only".

**The sidebar, top to bottom**

1. **Identity chip**: shows the current identity's name ("Unnamed" if it has none) and its kind ("No identity yet" when there is none). Click it to open the identity menu:
   - The title "Switch identity" sits on top; under it each identity takes two lines: its name, then its kind, with "Primary" after the primary one and "In use" after the current one, which sits in a blue frame. Click another identity to switch to it and land on Home.
   - After the identities: "New identity", "Import key" and "Identity key" (the Identity key page in Settings).
2. **Role switch**: the segmented control "Recorder" | "User". Switching replaces the whole sidebar with that role's pages, lands on Home and shows "Switched to …".
3. **Pages**:

   | Recorder | User |
   |---|---|
   | Home | Home |
   | **Entry**: Records, Grant | **Grant**: My grants |
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
| "In progress: …" | Background work you started is running; with several at once it reads "In progress: … and others (N)". Click it to return to the page where it started |
| "Syncing…" | A sync is running |
| "Sync failed" | The last chain read failed; the full message is under "Details" on the Alerts page |
| "Last synced HH:MM" | When the chain was last read successfully; followed by "UTC" when the time zone is UTC |
| "Not synced yet" | The chain has not been read |

**Sync.** Click the sync icon at the right end of the status line to read the chain in the background for the current role:

- Both roles read the signing key's balance once.
- A Recorder whose ledger exists also runs a ledger check.
- A User also re-verifies the grants in My grants.

When everything has come back, one summary toast appears. With no nodes set it says "No nodes configured: add them in Settings > Network"; when one of these reads is already running, it says "… is already running; request skipped".

**How the chain is read.**
- Only anchors sent by this ledger's own addresses (the keys that wrote in it) are asked for, not anyone else's; every one that comes back is still checked on this machine item by item. A range with no such address is not asked about.
- When a node declines, the same question is asked again; only after three refusals is the range split (at the limit the node names, or in half), and each smaller question has three tries too.
- For an anchor several nodes confirmed alike, this machine keeps its block time and verdict and does not ask about it one by one again; which anchors the ledger has, and whether the chain has new ones, is still asked at every sync, and if the block changed (a different block hash), or the log a node returns differs from what was kept, it is asked about again. A reading from one node, or an unproven verdict, is not kept. To ask about everything again, use "Check everything again" under "Advanced options" in Settings > Local data.

**Keyboard shortcuts** (⌘ in the table is the Command key on macOS and the Ctrl key on Linux and other systems; ⌘L, for example, is Ctrl+L on Linux):

| Keys | Action |
|---|---|
| ⌘1 onward | Open the sidebar pages in order (⌘1 to ⌘6 for a Recorder, ⌘1 to ⌘4 for a User) |
| ⌘[ / ⌘] | Back / forward |
| ⌘, | Settings |
| ⌘L | Lock |

Shortcuts do nothing while the passcode gate, the wizard or a card is open.

#### 4.2 Lists and detail pages

List pages (Records, Pending, Ledger, Grant, My grants; Pending is the second half of the "All" | "Pending" switch in the Records toolbar) have a search field at the top. On Records, Ledger, Grant and My grants, two more fields sit beside it, "Start date" and "End date": each opens a month calendar; pick a day to fill it in, with "Today" and "Clear" at the foot. With dates set (either one may be left empty), the list keeps only the rows whose anchor block time falls between the two days (both included), and rows not yet on chain are left out; days are counted in the time zone of Settings > Language and time. On a narrow window the two date fields move to a row under the search field.

Wherever you choose one item from a list ("Recent addresses" in a new grant, the address books, an annotation's target entry), a floating card opens with a search field; lists whose rows carry a time (the target entry) have the two date fields too. "Choose a record" in a new grant and "Choose entries" for a record kit open a sheet instead, with the search field and the two date fields as well. Click a row to open its detail page; use the toolbar's back arrow or ⌘[ (Ctrl+[ on Linux and other systems) to return. As you scroll down a detail page, its title moves up into the toolbar.

On a narrow window, pages with a side column (Export record kit) move the side column below the form.

#### 4.3 Bars at the top of the page

In these cases a bar appears at the top of the page, one at a time:

| Bar | When |
|---|---|
| "Ledger chain broken; read-only" | The ledger check found a broken chain. "Go to Restore" opens Settings > Local data, where you can restore from a whole-machine backup or reconcile under "Advanced options" |
| "Ledger handed over to a new key; read-only" | This ledger has been handed over |
| "Read-only: …" or "Read-only: No lock" | Another ZIKARON window holds this data folder, so this window can only read |
| "Another machine is writing to this data; read-only here" | The data folder is on a synced or removable drive and another machine is writing it. "Write from this machine" on the bar moves writing to this machine, and the other machine becomes read-only |
| "Writer mark unreadable by this version; read-only here" | This data's writer mark was written by a newer version or is damaged; nothing is changed. Once nothing else writes this data, "Write from this machine" takes it over |
| "Old data · date · view only" | You are viewing data that was set aside; "Return" goes back to the current data |

#### 4.4 Buttons and confirmation cards

**Button colours show their role**

- **Blue**: the main action on the screen. At most one per screen.
- **White**: every other action.
- **White with red text**: opens a confirmation card or the next step. It does not write anything itself.
- **Solid red**: writes to the ledger or sends to the chain the moment it is pressed. It appears only on the card that does the writing.
- **Frameless** (such as "Later" and "Exit setup"): a minor way out; it writes nothing.

**How writing works.** Every action that writes to the ledger or the chain writes only when you press the solid red key on its card; "Cancel" or Esc writes nothing. The card says what will be written and at what cost, with machine values under "Details". Cards such as "New record" and "Add note to entry" are filled in and written on the same card; two-step cards such as "Key handover" take "Next" first, and the solid red key comes on the second step after you check it; putting on chain, deleting a record and the like first show a confirmation card to check.

**Cards in general**

- Only one card is open at a time, and clicking outside it does nothing.
- Machine values (entry IDs, transaction hashes, raw verdicts and so on) sit in the "Details" fold. Options you rarely need sit in "Advanced options".

**Long actions** show progress inside the button and a check mark when done. On failure the button turns red and shakes.

#### 4.5 Toasts

Results appear as a small card at the top centre of the page. Only one shows at a time; a new one replaces the old.

- **Ordinary toasts** stay for about 3 seconds.
- **Error toasts** say what happened on the first line and what to do next on the second, and stay for 6 seconds. They have "Close", and "Show details" when there is raw text; once details are open the toast stays until you press "Close" or "Hide details" (after hiding them it stays another 6 seconds).
- **Alert toasts** start with "Alert:", have "Close", and stay for 6 seconds.
- If a task finishes after you have left its page, its toast has "Look up", which takes you back there.

**What raises a toast**

- An action you pressed raises one toast when it finishes, whether it succeeded or failed.
- Reads the app does by itself toast only on failure. Network failures raise no toast at all; they show as "Sync failed" in the status line.
- Errors on a card are written on the card, with the raw message under "Error details".

#### 4.6 Dropping and choosing files

**Drop zones.** You can drop a file in, or click the zone to open the system file dialog. Cancelling the dialog changes nothing.

- A drop zone takes one item at a time. Dropping several files shows "Drop one item at a time. Put multiple files in a folder first." and takes none.
- Exceptions: the attachments zone of a record kit takes several files; the "Add grant" card takes only the first of several dropped files. The "New record" card has no drop zone: while it is open, files dropped onto it are taken, several at once making a batch.

**Dropping onto the window.** On pages without their own drop zone, drop a single file anywhere on the window:

- As a Recorder, it opens "New record".
- As a User, it goes to "Verify record" and checks it straight away.

**Choosing a path.** Where a folder or file is needed, one line shows the chosen path or file name ("None selected" if nothing is chosen yet), with "Choose a file" or "Choose folder" beside that line or at the card's bottom left (as on "New record" and "Restore from backup"). Output locations are chosen with "Choose folder" too: record kits, grant files, credentials and snapshots default to the `kits` folder in the data folder; backups and mirrors default to the location used last time, else the Documents folder; key files have no default and need a folder chosen first.

---

## 5 · Passcode, lock and recovery

#### 5.1 The passcode gate

On a machine with a passcode, ZIKARON opens with the passcode gate: "Enter passcode" and eight boxes.

- The eighth character submits the passcode, and the card shows "Verifying…".
- A wrong passcode shakes the row and shows "Wrong passcode. N tries left".
- After 5 wrong tries the card becomes "Locked", and only recovery remains.

The count of wrong tries is kept on disk and survives restarting the app. This limit stops someone sitting at the machine from guessing. Against someone who copies the key store file away, the protection is the passcode itself and the encryption work every try has to repeat.

Three other cases:

- If the key store's encryption settings are below this machine's standard (for example, someone lowered them), the gate says "Encryption settings are below this machine's standard." and "Enter your passcode to re-encrypt at this machine's standard." That try is not counted as wrong; the correct passcode re-encrypts the store at this machine's standard.
- If the key store file turns out to be damaged when unlocking or setting a passcode, the app says "The key store file is damaged." and does not touch the file.
- If the key store file cannot be read at start, the passcode side shows "The key store cannot be read" with the reason and the file's path; the first-run wizard does not open and no new key store is made (the keys are still in that file). Repair or move the file, then reopen the app.

#### 5.2 Locking and auto-lock

"Lock" in the sidebar, or ⌘L (Ctrl+L on Linux and other systems), locks at once:

- The master key is wiped from memory, the data folder is closed and everything shown from it is cleared, including recovery words shown on screen.
- No new background work starts: ledger checks, grant re-verification, revocation watch, sending the pending queue and waiting for receipts. A send, receipt wait or write already under way finishes first, then the master key is wiped.
- While locked, actions that need a key are refused: "The key store is locked." and "Unlock with your passcode, then try again."

After you unlock, the data folder reopens and the app catches up once: any ledger check or grant re-verification that is due runs, and the pending queue goes back to waiting for its receipts.

**Auto-lock** is in Settings > Identity key and is on out of the box. After the "Idle time" passes (1, 5, 15, 30 or 60 minutes; 15 by default) with no activity, the app locks itself. Any key press or mouse movement restarts the clock. With auto-lock off, the app locks only when you press "Lock" or quit. Transactions already sent are not affected by locking.

#### 5.3 Forgotten passcode

On the passcode gate, press "Forgot passcode" to reach "Recover". Only the **primary identity's** credentials can reset the passcode:

- **The primary is a recovery-phrase identity**: the card reads "Enter the primary identity's recovery words to reset the passcode." Fill in the 12 words.
  - You can paste the whole phrase into any box; it spreads out from that box onward.
  - Every box shows dots, and a word not on the word list is marked red.
  - Below the boxes, the fold "Recover with a key file" takes a key file exported from either role of the primary identity ("Export key file" in Settings > Identity key), with its "Password", instead of the words.
- **The primary is an imported local-key identity**: the card reads "Use the primary identity's key file to reset the passcode." Press "Choose a file" to pick the key file exported from the primary identity, and enter the "File password". A wrong password gives "Incorrect password."; a file that is not a key file, or whose encryption parameters are out of bounds, is refused before any decryption work starts ("The keystore file format is not recognised.", "The keystore file has invalid encryption parameters.").

Then set a new passcode, entering it twice ("Set a passcode", then "Enter it again"). On success the wrong-try count is cleared, the old passcode no longer works, and the toast reads "Recovered, passcode reset".

- A secondary identity's words or key file are refused: "This belongs to a secondary identity." and "Only the primary identity recovers the passcode; use the primary's, or restore from a backup."
- Without a primary identity (no identity on this machine yet) there is nothing to recover with; a machine locked out like this offers to reset the empty key store instead.
- "Cancel" returns to the passcode card. Once the gate is "Locked" there is no "Cancel": only recovery remains.

#### 5.4 Restoring from a backup at the gate

At the bottom of the recovery card is "Restore from backup":

1. Choose a whole-machine backup file, enter the "Backup password" and press "Recover".
2. "Replace this machine with the backup?" shows the backup's name and contents. Press "Replace".
3. "Set a new passcode", entering it twice.

The machine's identities, keys and data are replaced by the backup's, re-encrypted under the new passcode, and the wrong-try count is cleared. The toast reads "Restored from backup" followed by what the backup held. Common refusals:

| Message | When |
|---|---|
| "Wrong backup password." | The backup password is wrong (not counted as a wrong passcode try) |
| "Not a ZIKARON backup." | The file is not a backup |
| "The backup comes from a newer version." | Update the app first |
| "The backup contents are malformed." | The backup file is damaged |

#### 5.5 Resetting an empty key store

If a passcode was set but no identity was ever created, and then 5 wrong tries lock the gate, there is nothing in the store to recover. The card says so and offers only "Reset key store":

- It deletes the empty key store and returns to wizard step 1, with the toast "Key store reset; you can set it up again".
- Setting a new passcode then creates a new master key. Any local data sealed under the old key, which can no longer be opened, is moved unchanged into a `set-aside` folder in the machine folder (numbered if that name is taken). Nothing is deleted.
- While the store still holds any identity, this button does not appear.

---

## 6 · Recorder: a full walkthrough

**Recorder seat** (choose "Recorder" at the top of the sidebar):

1. **First run.** Go through the wizard in section 3: set a passcode, create an identity, choose "Ethereum mainnet", create the ledger, send a little ETH to the Recorder address and export a whole-machine backup.
2. **Record a work.** Drop the file onto the drop zone on Home, fill in "Record name" and press "Add to ledger".
3. **Put it on chain.** Click the "Pending" card on Home (or press "Put all on chain" under the "Pending" segment of Records), then press "Confirm and send" on the "Put on chain" card.
4. **Grant it.** Open "Grant" and press "New grant":
   1. Fill in the grantee's address.
   2. Press "Choose a record" and pick a record that is on chain.
   3. Drop in the terms file and choose a validity period.
   4. Press "Add to ledger", then again on the confirmation card.
   5. Put it on chain as in step 3.
5. **Hand it over.** On the grant's detail page, press "Copy grant code" or "Export grant file" and send it to the grantee. The grant file carries your ledger, so they can run the full check.
6. **Day to day.** Watch the Alerts page. To take a grant back, press "Revoke grant" on its detail page.

---

## 7 · Recorder pages

#### Home (Recorder)

When the chain holds records of this ledger that this machine lacks, a red card "Ledger needs fetching" comes first: "N records on chain are not on this machine. Until fetched, nothing can be written, anchored or exported." Its "Fetch ledger" opens Settings > Local data.

**Top left: the drop zone** "Drop files to record them". Drop one item or click to choose one, and "New record" opens.

**Top right: the "Pending" card** shows how many entries are queued. It has three states:

- "Click the card to put them on chain": click it to open the "Put on chain" card straight away.
- "Waiting to go on chain": the transaction has been sent and is waiting for confirmation.
- "All on chain".

**Three tiles in the middle.** The whole tile is clickable:

| Tile | Shows | Click to open |
|---|---|---|
| "Ledger status" | The last ledger check's result, "OK" or "Problems", with the total entry count below | Ledger |
| "Records" | How many records are not deleted, with the newest record's name below | Records |
| "Expiring within 30 days" | How many unrevoked grants end within 30 days, by chain time, with the soonest below ("#N · date") | Grant |

- A tile without a reading shows "--" with the reason below: "Not read yet" before anything has been read (for "Ledger status", also before the first ledger check).
- "Expiring within 30 days" shows "No chain time" when no chain time is known yet.
- Clicking "Ledger status" goes to the Ledger page and opens the "Ledger check" fold in its ledger status card.

**"Recent"**: the last five ledger entries, with number, type, summary and on-chain state. Click a row to open it. With no entries it shows "No entries yet".

### Record on chain

"Records" in the sidebar. On the right of the toolbar is the segmented control "All" | "Pending N".

The "All" segment, top to bottom:

- The drop zone "Drop files to record them"; dropping or choosing opens "New record".
- The search field "Find by record name or content fingerprint", with "Start date" and "End date" beside it filtering by first anchor block time. To see whether a file is in this ledger, type its SHA-256 into the search field.
- The record table, with the columns "Entry", "On-chain state" and "First anchor block time", newest first. Deleted records are struck through. With no records yet it reads "No records yet. Drop a file to start."

#### New record

"New record" is one card: fill it in and one press writes it. Top to bottom:

- **The content**: the "File" row gives the name and size of the chosen file, folder or Git repository ("None selected" if nothing is chosen yet), with "Content fingerprint" below it (copyable). To choose or change the content, press "Choose a file" at the card's bottom left, or drop a file, folder or Git repository onto the card. The fingerprint is computed in the background: until it is ready "Add to ledger" (or "Send to chain now") says "Computing the fingerprint" with a turning ring and cannot be pressed; with several files, the key says "Recording" after the press until they are written, and the card closes after. The content fingerprint is computed this way:

  | Kind | How |
  |---|---|
  | File | SHA-256 of the file's bytes |
  | Folder | Over every file name and its content; paths inside the folder are joined by `/` on every system, so the same folder gives the same fingerprint on macOS, Linux and Windows |
  | Git repository | Over the current commit |

  If "Record name" is empty, it is filled in with the full name of the file, folder or repository, extension and all.
- "Cost": "Charged when put on chain", or "Gas estimated in the next step" with "Auto put on chain" on. "Ledger location" is under "Details".
- "Record name".
- The "Recorded for" fold, marked "Optional" (once filled, it shows what you filled in). It has four fields: "App", "Their identity (0x…)", "Their reference" and "Their role".
  - If you fill in any of them, the first three are required, and "Their identity (0x…)" must be `0x` followed by 40 hexadecimal digits; "Their role" may stay empty.
  - These words are written into the record entry as they are; the app itself does not read them.
- The "Advanced options" fold, marked with the linked repository's name, or "No repository linked". Inside, two rows:
  - "Linked Git repository": "Add repository" chooses a repository and saves it in settings.
  - The reading: "Linked, not checked yet"; after "Check commits" beside it, "N commits since the last record".
- At the foot: "Choose a file" on the left; "Cancel" and the red "Add to ledger" ("Send to chain now" when "Auto put on chain" is on) on the right.

"Add to ledger" writes the entry and queues it. The card closes, with the toast "Added to pending, N in total" and a second line "Waiting in the ledger to be put on chain by hand" ("Offline: check against the chain before anchoring" when offline). If the "Recorded for" fields are not right, the write is refused by name and nothing is written. "Cancel" or Esc closes the card and writes nothing. With "Auto put on chain" on, the app estimates gas right after writing and opens the "Put on chain" card. The card covers every entry in the queue that can be sent.

**Batches.** With the card open, drop two or more files onto it at once to make a batch (the drop zones on Home and Records take one item at a time):

- The "File" row reads "N files chosen · signed one by one", with "Clear" beside it to empty it and the file names listed below.
- On pressing, each file becomes one record, with the SHA-256 of its bytes as the fingerprint. The one "Record name" is written into every entry. The toast reads "Signed N, N waiting to go on chain".
- The batch stops at the first failure. Entries already signed stay signed, and the toast reads "Signed N, stopped at file K: file name · reason". The remaining files stay in the list; fix the problem and press again.
- A batch cannot contain folders: the batch stops at the first folder.

For each file signed (a single file, or each file of a batch), this machine remembers the file name and where the file was. Record kit export uses this to find originals. Folders and Git repositories are not remembered.

#### Record detail

Click a row in the record table. The title is the record's name, with "#N · Record" and the on-chain state below it.

**"Basic information"**

- "On-chain state": once confirmed, this row gives the first anchor block time with a check mark; otherwise it gives the state, with a separate "First anchor block time" row when the chain has been read for it.
- "Entry number".
- "File": the file name signed on this machine.

"Details" holds "Content fingerprint", "Entry ID", "Previous entry", "Size (bytes)" and "On-chain transaction".

**"Actions"** (a deleted record has none), one row per action; the whole row is the button:

- "Put on chain now": only while it is in the pending queue and not yet sent.
- "Grant to someone": only once the state is "Confirmed"; until then the row is dimmed and, on hover, says "Put it on chain before granting it", or "Waiting for this check" when only the previous session's check confirmed it.
- "Export record kit".

At the bottom is "Delete record" in red (a deleted record has none).

#### Deleting a record

"Delete record" opens the card "Delete this record?". Press "Delete".

- A "Deletion" entry pointing at the record is added to the ledger. The original entry is not changed by a single byte.
- From then on the record is struck through, its page says "Deleted by entry #N. It can no longer be granted.", its page no longer offers the grant and export buttons, and New grant no longer offers it.

Whether the deletion itself goes on chain depends on whether the record was ever made public:

| The deleted record | Result |
|---|---|
| Still queued and not sent, or never on chain | The record leaves the queue and the deletion stays local. The two rows show "Deleted" and "Deleted locally", and the toast reads "Record deleted. Not put on chain" |
| Already sent, in a block or on chain | The deletion entry is queued and goes on chain like any other entry; the toast reads "Record deleted. N pending" |

In the first case both rows stay on this machine only. With "Hide entries deleted on this machine" turned on in Settings > Local data, the Records and Ledger lists leave these two rows out; a record deleted after it went on chain is still listed. This changes only what the lists show: entry numbers, counts, the ledger check and detail pages stay as they are, and turning it off brings the rows back.

With a network set up, no ledger check yet, and the record not in the pending queue, deleting is refused: "The ledger has not been checked." and "Click Sync at the bottom of the sidebar."

For the entry's format and how it is read, see Appendix C.

### Pending queue

**The pending segment** ("Pending N" in the Records toolbar):

- On top is the search field "Find by entry number or summary".
- The table lists number, type, summary, the date each entry was queued and a state light. When empty it reads "Nothing pending".
- Entries the command line hands to the desktop with "Queue only" (Settings > Command line and AI control, "Putting on chain from the command line") join this queue, with the toast "N records from the command line are queued".
- The queue follows the ledger and the chain: an entry the ledger check finds on chain already while it waits unsent in the queue (one the command line put on chain itself, say) leaves the queue, recorded as on chain, and is not sent again; an entry of the ledger that is neither on chain nor queued (one adopted in place, say) joins the queue after the next ledger check every node answered.
- Below the table, while anything is queued or a batch has been sent, is the "This batch" card: "N entries · gas not estimated" before an estimate and "N entries · gas estimate …" (a gas amount) after, with "Put all on chain" and "Sync". After a batch has been sent, a "Last batch" line appears ("N put on chain · N removed"). If a submitted batch's chain has no node in the current settings, the page says "A batch was submitted and is waiting for its receipt. There is no node for chain N right now, so it cannot continue."
- Click a row for its detail: the title is the entry's summary, with "#N · type" and the on-chain state below. "Basic information" holds "On-chain state", "Entry number", "Queued at" and "Sent together" ("First N", with "Put on chain now" beside it; once sent it reads "Submitted, waiting for its receipt" and has no button), plus "First anchor block time" when the chain has been read for it. "Details" holds "Content fingerprint" (records only), "Entry ID", "Previous entry" and "Size (bytes)".

**Putting entries on chain**

1. Press "Put all on chain" or "Put on chain now". The "Put on chain" card opens, showing "Entry count" ("First N"), "Gas estimate" and "Gas cap"; until the estimate is in, both say "Estimating".
2. Each time the card opens the gas is estimated again (an earlier estimate is not reused). The estimate runs in the background; meanwhile "Confirm and send" says "Estimating" with a turning ring and cannot be pressed, while "Cancel" can (not while sending). Once the estimate is in, press "Confirm and send". The app signs, sends the transaction and waits for the receipt; meanwhile the key says "Sending" with a turning ring.

All entries in a batch travel in one transaction.

**The gas cap** is worked out from the chain's actual fees: (base fee × 2 + tip) × the gas limit this transaction carries. The limit is one and a half times the gas the nodes estimate (rounded up), at most 200,000; an estimate above 200,000 is refused and nothing is sent. The base fee is read at the block every node has reached (the lowest height when nodes differ), and nodes are compared on the base fee alone. The tip is the median, over the 20 blocks up to that block, of each block's median paid tip (blocks whose median is zero are left out unless all are zero), at most 1 gwei; when the nodes give no answer, refuse, disagree or answer in another shape, it is 1 gwei. If the base fee cannot be read, the whole pair falls back (a 3 gwei cap and a 1 gwei tip; the limit still follows the estimate, so at most 0.0006 ETH): the cap on the confirmation card is then the fallback's, the card says "No base fee from the nodes; cap at the fallback", and the chain is asked again the next time the card opens. Nodes serving another chain are not read for fees, and the card names them. The gas estimate takes its block by the same rule (the lowest block every node has reached); when the nodes estimate different amounts, the largest any node gives is taken, and above 200,000 it is still refused. Only this chain's nodes are asked. On the layer-two networks Arbitrum One and OP Mainnet the tip is likewise what the chain actually paid, usually far below 1 gwei, so what is set aside and what is paid are a small fraction of mainnet's.

**Checks before sending**

- If the balance cannot cover the cap, nothing is sent. The message is "Insufficient balance." and "Needs N ETH, have N ETH.", with "Copy address" so you can top it up.
- The balance is asked of every node at the lowest block they have all reached; if the balances differ or the nodes' heights are too far apart, nothing is sent and the message is "Nodes returned different results.". A node that gives no balance is never read as zero.
- If the estimate fails, the card becomes "Cannot estimate gas" and gives the reason; "Retry" tries again.
- Nothing can be sent while the ledger is locked. Nothing is sent either while this data folder is read-only (an imported identity whose ledger is not fetched yet, or a chain holding anchors the ledger lacks), while the ledger's own entries break its chain, or once the ledger has been handed off to another place. Before sending, the chain is read again; if it holds records this machine lacks, sending is refused: "Ledger is not up to date" and "N records on chain are not on this machine".
- Before signing, the chain is asked whether it already holds an anchor of any hash in the batch from this key. If one is there, nothing is sent: "Some records in this batch are already on chain; nothing was sent this time." and "Wait for the ledger's self-audit to read the ones already on chain and take them off the queue, then send the rest."; when no node can say, nothing is sent either, never on a guess.

**After sending**

- In a block within 30 seconds: the toast reads "Put N on chain".
- Not yet in a block after that: the toast reads "Submitted N, waiting for confirmation", and the app asks for the receipt again every 15 seconds.
- A batch is recorded in the queue as submitted (with its transaction hash and nonce; its detail reads "Submitted, waiting for its receipt") once it is signed and before it is broadcast. So if you quit and reopen the app, or a send stops half way, the app goes on asking for that transaction's receipt by its hash; it never signs it again or sends it twice.
- If the receipt shows the transaction failed: "The transaction was sent but failed on chain." The entries stay in the queue and can be sent again.
- When a batch is still not included at the end of a wait and the price now (the base fee at the chain head) has risen above the cap it went out with, **"Resend with higher fees"** appears in the "Last batch" row of the ledger status card at the top of the Ledger page. Its card "Resend this batch with higher fees" lists "Entry count", "Cap it went out with", "Price now" and "Cap of the resend"; confirming signs the batch again at the same nonce with the fees worked out now, each fee at least 10% above what it went out with (nodes take a replacement only then). The old and the new transaction are both watched and the first included counts. A press while a receipt check is running waits for it ("Sending after this receipt check…"). If the fees have risen above the cap shown on the card in the meantime, the resend is refused; open the card again to see the new figure. A batch is resent at most 3 times; after that the row reads "Resent 3 times; no further raises" and the app keeps waiting for it.
- When no node holds the batch any more (most likely dropped from the pools) and its nonce is still unused, the "Last batch" row reads "No node holds this batch; its nonce is unused. Resend at the price now", and **"Send again at the price now"** opens a card of the same name ("Entry count", "Price now", "Cap of the resend") that sends it once more at the same nonce, at the price now and a fresh gas estimate. The 3-resend limit applies here too.
- When no node holds the batch and its nonce has been used by another transaction, the batch lapses: "This batch lapsed: its nonce was used by another transaction; none was included". Its entries go back to the queue and can be sent again.

**When a node refuses**

- The transaction itself was refused: "Transaction nonce already used.", "Transaction already submitted, waiting for confirmation.", "Gas price too low.", "Gas limit too low.", "The registry contract rejected the transaction."
- The node's limits or setup: "The node is rate-limiting requests.", "This node does not accept transactions.", "This node requires credentials.", "The node is on a different network."
- Connection problems: "Cannot reach any node.", "The node did not answer in time.", "The node's answer was too long.", "The node's answer could not be read.", "Could not open a secure connection to the node."
- Anything else: "The node rejected the transaction.", with the node's own words under details.

**The command line sends this queue too.** The command line through the desktop (`anchor --home`) and with its credential (`anchor --ledger`) sends this data folder's pending queue as well, through the same sender as "Put all on chain" and past the same checks (see "One sender" in section 12). While "With a credential" is on, the desktop and the command line take this data folder's writer lock in turn: a batch waiting for its receipt holds no lock, so the other side may write, and may send the rest of the queue, meanwhile; it finds that batch in flight and leaves it alone, never sending it again.

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

- The "More" menu: "Import existing records", "Key handover", "Add note" and "Sign a claim for someone".
- The segmented control "All", "Unconfirmed" and "Grant". "Unconfirmed" lists every entry the chain has not yet confirmed: those in the queue (queued, sent and awaiting the receipt, reverted, refused), those included but not yet counted by a ledger check, and those recorded without being queued. It is a wider set than "Pending" on Records, which counts the queue.

When a Recorder has no ledger here yet, a blue "Create the ledger" is under the table; it opens the wizard at that step. It is not there once there is a ledger.

**The ledger status card** at the top of the page: "Ledger status · OK/Problems" with "N total" below. Before the first check it reads "Never checked", or "Never checked: no network set yet" when nodes, chain ID or registry contract are missing.

- If some entries can be sent, "Put all on chain" appears on the right; entries not yet queued are queued first.
- If the root entry is not yet confirmed on chain, a "Ledger root" row reads "Root not confirmed on chain" with a red cross.
- A stuck batch adds a "Last batch" row with its notice and its resend button (see "After sending" under "Pending queue").
- The last row is the "Ledger check" fold, with the check's result on its right; open it to unfold the ledger check inside the card.

Below it is the search field "Find by number, type or summary", with "Start date" and "End date" filtering by first anchor block time.

**The entry table**, newest first:

- The columns are number, type, summary, first anchor block time and on-chain state.
- A deletion's summary reads "Deleted record · record name". One that does not follow the convention reads "Invalid deletion: …" (see Appendix C).

**Entry detail.** Click a row. The title is the entry's summary (a record's name), with "#N · type" and the on-chain state below it ("Deleted" for a deleted record).

- "Basic information": "On-chain state" and "Entry number", read as on "Record detail"; a record also has "File".
- A grant: see "Grant detail" under "Grant list".
- An "Actions" group, one row per action, the whole row being the button; a row that cannot be taken yet is dimmed and says why on hover:
  - Still queued: "Put on chain now".
  - Neither on chain nor queued: "Add to pending". An entry that has been on chain before cannot be queued again: "This entry is already on chain." and "No need to queue it again. The on-chain state comes from the last sync; sync again if unsure."
  - A record: "Use for grant" (only when confirmed) and "Export record kit"; a deleted record has neither.
  - Confirmed entries other than records: "Add note".
- "Details": "Content fingerprint" (records only), "Entry ID", "Type", "Sequence", "Author", "Previous entry", "Size (bytes)" and "On-chain transaction".
- A record not deleted has "Delete record" in red at the bottom.

### Ledger check

The ledger check compares every local entry with the anchors on chain. It runs:

- On the interval set in Settings > Notifications (300 seconds by default). With the interval at 0 it does not run on a timer, only when you press Sync or in the case below.
- Straight away when this ledger has no check result yet, or when the ledger has changed (a new entry written, a batch put on chain).
- Only when nodes, chain ID and registry contract are all set. Otherwise it shows "Never checked: no network set yet".

The ledger check sits in the "Ledger check" fold of the ledger status card, with the result "Passed", "Has gaps" or "Failed" on the fold's right. Unfolded, it gives a mark and a count for each item, two to a row, in this order:

- "On chain", "Not on chain yet", "Deleted locally", "Mismatched on chain"
- "Findings", "Excluded", "Imports unproven", "Unrecognized types"
- "Malformed entries", "Unavailable", "Unproven", "Void", "Discarded"
- "Deleted locally" shows only once the ledger table has been read, in grey, never as a warning.

Below them are this check's "Check result", "Ledger head", "Root entry", "Checked against", "Entry count" and "Last check". Before the first check the fold reads "Never checked", and unfolded says only "Not checked yet. Click Sync to start".

- **When the ledger is "OK"**: there is a check result, the chain is not broken, and "Mismatched on chain", "Findings", "Imports unproven", "Malformed entries", "Unproven", "Void" and "Unrecognized types" (beyond the deletion entries) are all zero. Entries not yet on chain, "Excluded", "Unavailable" and "Discarded" do not make it "Problems".
- A check counts deletion entries as unrecognized types. When that count equals exactly the number of deletion entries, the item shows as a green "N convention entries".
- "Not on chain yet" is judged as the pending queue and Alerts judge it: an entry deleted before it was made public and its deletion entry (the pair deleted locally; see "Deleting a record" under "Record on chain") stay on this machine and are never put on chain, so they are not counted there; they get a line of their own right under it, "Deleted locally", in grey and never a warning. Until the ledger's entries have been read there is no such line, and "Not on chain yet" gives the check result's own count.
- If the result is "Failed" (a broken chain), the whole ledger becomes read-only and the broken-chain bar appears at the top.

#### Notes

"Add note" in the "More" menu, or "Add note" on an entry's detail page, opens "Add note to entry":

1. Write the "Note text" (required).
2. Press "Select" beside "Target entry", search the floating card (by number, type or summary, or by a range of first anchor dates) and pick the entry to annotate; or under "Advanced options" use "Or paste an entry id".
3. Press "Add note" to write it.

A note is saved as a new entry; the original entry is not changed.

### Import anchor proofs

This brings anchors that a key put on chain earlier, but that are not in the ledger, into the ledger. Open the "Import existing records" card; the segmented control at its top chooses whose anchors:

- **"This machine's key"** lists anchors this machine's key sent earlier that are not in the ledger ("Sent earlier by this key"). The list comes from the last ledger check, so it needs the network set up and one check done; otherwise it shows "Chain not read".
- **"Another key"**: fill in "Key address" (pick one from "Address book" if you like), and it scans the chain for the anchors that key sent ("Anchors on chain").

**The list** gives the network and the count under its title, with "Select all" beside it. Each anchor is a row with its time, its state and a switch; block number and digest are under "Details". The states are:

- "Passed": switched on by default. Only these rows can be imported.
- "No such transaction", "Wrong sender", "Digest does not match"
- "Already in the ledger"

**With "Another key"**, a "Their signature" group appears under the address:

1. Press "Copy" beside "Text" and send the text to the holder of that key to sign.
2. When they sign it and send it back, copy their signature and press "Paste" beside "Signature". It is checked on the spot, and the "Their signature" row shows "Waiting", "Passed" or "Mismatch". The full text and signature are under "Details".

You can import without their signature; those anchors are then recorded as unproven. If the ledger gets new entries, they need to sign again.

**By hand.** The "By hand" fold takes anchors typed one per line; press "Check these rows".

Finally, press "Import N". This writes one adoption entry and queues it.

**Signing a claim for someone.** When you hold the key someone else wants to claim, choose "Sign a claim for someone" from "More":

1. Paste in the text they sent. The card shows "Who is claiming" and "Anchors"; the head of their ledger is under "Details".
2. Enter this machine's passcode to sign.
3. Copy the signature and send it back to them.

Text the app cannot read gives "This is not a claim text."

### Hand over ledger

"Key handover" opens the card "Key handover", in two steps:

1. Fill in three fields and press "Next":
   - "New key address": checked as soon as it is complete. The new key must have no anchor proofs on chain. The reading shows "The new key has N anchor proofs"; you can only continue at zero.
   - "Kind": pick "Key change only" or "Hand over to someone" from "Choose".
   - "Statement" (optional). Left empty, the kind's words are written.
2. Check "New key address", "Kind" and "Statement" (the address row can be copied) and press the solid red "Hand over" to write it. "Back" returns to step one; "Cancel" closes the card.

The toast reads "Handed over to a new key. N pending". From then on the new key continues the ledger. This machine becomes read-only, with "Ledger handed over to a new key; read-only" at the top of the page; entries already queued, the handover entry among them, can still be put on chain; the handover entry must go on chain, or the new key cannot verify the handover.

### Grant list

"Grant" in the sidebar. On the right of the toolbar is the blue "New grant".

**The grant list**

- Search: "Find by grantee, record or state", with "Start date" and "End date" beside it filtering by the block time the grant entry was anchored at.
- The table columns are "Entry", "Validity", "Grant no." and "State", plus a light for the on-chain state.
- The state is judged by chain time: "Active", "Expired", "Not yet active", "Revoked" or "Unknown".
- With no grants it reads "No grants yet".

### Draft a grant

Ways in:

- "New grant" on the grant list starts an empty form.
- "Grant to someone" on a record's detail page, or "Use for grant" on an entry's detail page, fills in that record.

The form's fields:

- **"Grantee"**: paste a `0x…` address. "Recent addresses" on the right opens a floating card with a search field, listing the address book and earlier grantees.
- **"Which record"**: press "Choose a record" to open the "Choose a record" sheet: a search field and "Start date" and "End date" (each opens a calendar) at the top, to search by record name or content fingerprint or filter by first anchor date; below, one row per record: entry number, record name, first anchor block time ("Not on chain" when not yet anchored), the chosen one with a check. Click a row to choose it and close the sheet; "Cancel" or Esc closes it unchanged. Only records on chain can be chosen; the rest are greyed out with the reason:
  - "Grant after it is on chain"
  - "Going on chain, please wait"
  - "Waiting for this check"
  - "Deleted"
- **"Terms file"** (required): drop the terms file in, or click to choose it.
  - Once chosen, the file's name and size are shown; "Change" picks another.
  - The file's SHA-256 is filled into "Terms file fingerprint" under "Advanced options", marked "Filled from the terms file".
  - When the grant is signed, a copy of the terms file is kept in the data folder. It later goes to the grantee with the grant file and record kits, so they can recompute the fingerprint themselves.
- **"Validity"**: "7 days", "30 days" (the default), "90 days" or "Custom".
  - The presets count from chain time: opening the page reads the latest block's time once from the current network and starts there. If the latest cannot be read but a chain time was heard before, that one is used, and the date blocks are noted "Going by chain time YYYY-MM-DD; the latest could not be read". With no chain time at all, it says "No chain time for presets: sync first or choose Custom"; sync first, or use Custom.
  - "Custom": the start's date block opens the calendar to pick the day; below it, "Start time" takes the hour and minute (empty is 0:00; the start day defaults to the chain time's day), and "Length" takes days, hours and minutes (at most 999 days, 23 hours, 59 minutes; past that it turns red and says so). The end is the start plus the length, shown on the date block at the right; with no length there is no validity window. Switching from a preset to custom fills the fields with that preset's window.
- **The overlap light**: "Overlapping live grants: N". It counts grants on the same record that are marked exclusive on this machine, overlap in time and are not revoked. Until a record is chosen (or while a custom period cannot be read) it reads "Choose a record to check for overlaps".
- **The "Exclusivity, sublicense and scope" fold**:
  - The "Exclusive grant" switch: "Recorded on this machine only. Cannot be changed after signing." The exclusive mark is kept only on this machine and is not written into the grant; put the exclusivity terms in the terms file.
  - "Upstream grant (sublicense only)".
  - "Scope".

Addresses and hashes in the grantee, terms file fingerprint and upstream fields must be all lowercase. With a capital letter, the form says "The address has capital letters. Change it to lowercase before signing." and "Confirm" is disabled.

**Signing**

1. Press "Confirm" at the foot of the page ("Cancel" goes back). It works once the grantee, the record and the terms file fingerprint are all filled in (until then, hovering over it says "Still needed: …"), and, with a preset validity, once there is chain time.
2. The confirmation card "Sign grant" shows "Grantee", "Entry", "Validity", "Attached terms file" (the file's name, or "Terms fingerprint typed by hand"), "Exclusivity, sublicense and scope" and "Cost" ("Charged when put on chain"; "Gas estimated in the next step" when "Auto put on chain" is on). The grantee's full address, the content fingerprint and the terms file fingerprint are under "Details".
3. Press "Add to ledger" on the card ("Send to chain now" when "Auto put on chain" is on) to sign. The toast reads "Grant issued. N pending". Then put it on chain as described under "Pending queue".

If the same record already has an exclusive grant with an overlapping period, signing is refused and the card "Exclusive grant conflict" lists the grants that clash; "Cancel" closes it.

### Pre-signing checklist

"Pre-signing checklist" opens the New grant form (see "Draft a grant" above). The form takes the page's full width, with no side column and no checklist.

#### Grant detail

Click a row in the grant list. The title is the record's name, with "#N · Grant" under it.

**"Basic information"**

- "Grantee", "Validity".
- "On-chain state": once anchored, the first anchor block time; while the grant is still queued, its on-chain state, with "Put on chain now" on the right.
- "Entry".
- "Exclusive": "Yes · recorded on this machine only" or "No".
- "Attached terms file": the file name, or "No terms file".

**Buttons** (below "Basic information")

- **"Copy grant code"** puts the `zikaron-grant:…` code on the clipboard ("Grant code copied"). A relicensed grant's code carries every hop up to the root, the same text as in this grant's badge and grant file.
- **"Export grant file"** opens the "Export grant file" card, which shows "Grant" (#N · record) and "Saved to" (by default `kits` in the data folder; "Choose folder" picks another). Press "Export grant file" on the card to write a `grant-<first ten hex characters of the grant's entry ID>.zkgrant` file, numbered if the name is taken.
  - The file holds the entries of every hop in the grant chain, the issuer's ledger, the terms files and the grant code.
  - It reads the chain once before exporting and refuses if the chain cannot be read, or if it holds records this machine lacks: "Ledger is not up to date" and "N records on chain are not on this machine".
  - The toast gives the location, how many hops, and how many terms files.

"Details" holds the grantee, the terms file fingerprint and the entry's machine values.

Below that is "Show revocations", which lists "Revocation #N" with its on-chain time for each revocation, or says "No revocations". While the grant is not revoked, "Revoke grant" is under it.

### Revoke grant

"Revoke grant" opens the card "Revoke this grant":

- The card shows "Entry", "Original validity" and "Grantee", and below them a field "Ruling file fingerprint", marked "Optional".

Press "Revoke" to write the revocation entry and queue it; the toast reads "Grant revoked. N pending". Once it is on chain, the grantee's revocation watch will find it.

### Record kit

Open it with "Export record kit" on a record's or entry's detail page; that entry is already selected.

**Entry range.** The "Entry range" line shows "All N" (choosing nothing means the whole ledger) or "N selected".

- "Select" opens "Choose entries": a search field and "Start date" / "End date" on top, a switch on each entry so you can pick any set. "All" picks the entries the search and the dates keep; "Clear selection" clears them all; "Apply" takes the choice, "Cancel" drops it.
- Revocations of the chosen grants, and the ledger's creation and handover entries, come along automatically; the receiver needs them to tell whose ledger this is.
- If some chosen entries are not on chain yet, a notice says "N selected entries are not on chain yet: the package holds no on-chain proof for them", with "Put on chain now" under it.

**Originals of the selected records.** This lists the files this machine signed for the chosen records, each with its entry number. You can "Remove" any of them; removed ones drop out of the list and a row "N originals removed" offers "Put them back". When none is indexed here it says "None"; add the file or folder they were registered from as an attachment instead.

- A file no longer where it was signed is marked "No longer at its registered location".
- A file that has been changed is marked "Not an original of a selected record; left out of the package".

**Attachments · Optional.** Press "Add" to choose files or folders, or drop them on the page. Each item shows the name it will have in the kit ("Exported as files/…"), or "Checking…" while it is read, and can be removed with "Remove"; with two or more, a row "Clear attachments" removes them all with "Clear".

- File names inside a kit may use only lowercase letters, digits, `.`, `_` and `-`. Other names are rewritten automatically: lowercased, each run of other characters replaced by one `-`, leading and trailing `-` and `.` trimmed, and `-` plus the first eight characters of the original name's digest inserted before the extension (for example `name-1a2b3c4d.pdf`).
- When names were rewritten, the kit carries a table of original names, `files/zikaron-names.json`.
- A row whose name cannot be rewritten is marked red "This file name cannot be converted. Rename the file.", and the kit cannot be exported until it is fixed.
- Attachments must be originals of the chosen records; any other file is left out.

**Other settings.** "Note" adds a note to the kit. "Export as a folder" is off by default: off, one `.zkit` file is exported; on, a folder (use a folder to publish the kit at an address).

**Encrypted record package.** "Encrypt the record package" is off by default. On, "Export as a folder" is grey: only a one-file record package is encrypted. An encrypted package opens with one key only; anyone else who gets it cannot even verify it.

- The "Export record kit" card has two more fields, "Recipient address" and "Recipient public key". Fill one; the other turns grey. "Export" can be pressed once the field is in its form: an address is `0x` and 40 hex digits; a public key is a point of the curve (`0x` and 66 or 130 hex digits).
- An address: the export key shows progress while the app reads the chain. It reads this machine's current network and every read-only network added, looks for anchor transactions that address itself signed, and recovers its public key from a transaction's signature. An anchor sent through a relay or a contract was not signed by that address and does not count.
  - Found: the package is encrypted and written, the key shows a check mark, the dialog closes by itself a moment later, and a notice says "Encrypted record package exported".
  - Every network read through, and no anchor of that address: the dialog closes and a notice says "Encrypted record package not exported: this address has not anchored on chain"; nothing is written.
  - Not found, and some network not read (a node not answering, nodes disagreeing): the dialog closes and a notice says "Encrypted record package not exported: the chain could not be read"; nothing is written.
- A public key: the chain is not read; the package is encrypted and written at once. The recipient's public key is the "Public key" row under "Details" in their Settings > Identity key; they copy it and send it to you. This is the way for someone who has never been on chain.
- An encrypted package is named `kit.zkit` by default (not from its contents), numbered on when the name is taken; you can change it.
- Its head holds the format's mark, a version and a one-time public key, and nothing of the recipient's address or key; from outside one can see only that it is a ZIKARON encrypted record package, roughly how large, and its file name.
- "Record packages exported on this machine" lists an encrypted package as any other, with the facts of the package before it was encrypted, and never the recipient's address or key; "Delete local copy" deletes it as any other.

**Limits of encryption:**

- Whoever is not the recipient cannot even verify it: the verify page, the command line and an AI app all refuse an encrypted package by name and leave no plaintext. If the sender is not the recipient, the sender cannot open it either.
- Once the recipient opens it, they can still pass the plaintext on; encryption covers the delivery only.
- The outer encryption does not prove who sent it: anyone with the recipient's public key can make a package encrypted to them. Whether the records are genuine is judged, as always, by verifying the package inside (signatures and anchors on chain).
- No forward secrecy: if the recipient's identity key leaks later, every package sent to them earlier can be opened. Addressing a package straight to an address carries this cost.
- The signing key is also the decryption key: one key both signs transactions and decrypts (the encryption between Ethereum nodes uses keys the same way, with no practical attack known). A separate encryption key would not be the one recovered from the chain, and the address way would break, so the key is shared.
- After the recipient hands over to a new key, the old identity, while it stays on their machine, still opens packages (a handover does not delete the old key); with the old identity deleted, they no longer open.
- With a public key given, whose key it is depends on how the sender got it; the app only checks it is a point of the curve.
- An address that has only anchored bare from the command line (not through the registry) has no anchor logs and cannot be found: use its public key.
- Evidence for a GALEED judge follows GALEED's delivery rules instead (the plain record package's files, encrypted with age to the key the judge publishes), not this encryption.

**Exporting.** Press "Verify and export" at the foot of the page ("Cancel" goes back). The "Export record kit" card opens, showing "Entry count" (N entries) and "Saved to" (by default `kits` in the data folder; "Choose folder" picks another). Press "Export" on the card:

1. The app reads the chain once. If the chain holds records this machine lacks, export is refused: "Ledger is not up to date" and "N records on chain are not on this machine".
2. It writes a file `kit-<first eight characters of the earliest chosen record's fingerprint>.zkit` at the save location (a folder of that name with "Export as a folder" on; plain `kit` if no record is chosen), numbering it if the name is taken. The file is written item by item and has no size limit.
3. The kit checks itself and is written only if it passes. The card closes, the toast reads "Record kit exported to …", and the page shows "Record kit exported · N entries · N files", naming any file left out and why.

If it cannot be written, the reason is given:

| Message | Reason |
|---|---|
| "Cannot write to this location." | The save location is not writable |
| "Something with the same name is already at the save location." | A name clash at the save location |
| "A file name cannot be used inside the package." | A file name could not be rewritten |
| "Two files inside the package have the same name." | Two files with the same name in the kit |
| "The package failed its own check and was not saved." | The self-check failed |

**Where the kit says it is anchored.** On export a line is added at the end of the kit's note, the same in every interface language: `anchored-on: eip155:<chain ID> · registry <registry contract> · from <start block>`, taken from this data folder's network at that moment.

- With no network set, no line is added.
- A note that already ends with such a line has it taken off first, so exporting again never doubles it.
- In the index of "Record packages exported on this machine" the note stays as you wrote it; the line is kept in a cell of its own.
- The line only points the way: verification never judges by it, it only says which network to add (see "Record verifier").

**The side column**

- "Readings and reference (not in the package)": shown when the selection holds a record, with "Reading for #N · record name" (the first chosen record). "Calculate" on the "Max depth" row computes three readings, "First recorded", "Max depth" and "Continuity", with a line "Reference: this session ledger check …". They are shown on the page only and do not go into the kit.
- "Record packages exported on this machine": one row for each kit this machine has exported from this ledger (its file name and the day it was made), or "No record packages exported yet". Click a row to open a card titled with the file name:
  - It lists "Location" and "Record kit ID".
  - "Fetch address" (marked "Optional"): the https address where the kit can be fetched online, saved with "Save address" inside the field. Left empty, a folder kit's is the publish address plus the kit's path; a kit in one file has none.
  - "Delete local copy" turns into "Confirm deleting the local copy (the ledger stays)"; pressing that deletes the local kit (its file or folder). The ledger is not touched. "Close" closes the card.

### Depth

Depth readings describe how long a record has been in the ledger and how continuously it has been on chain. There are three: "First recorded", "Max depth" and "Continuity" (on chain / span). They appear in these places:

- Recorder: "Readings and reference (not in the package)" in the side column of the Record kit page; press "Calculate".
- User: in the Record verifier, when "Record hash" is filled in.
- Due diligence: on the Other ledgers page with "Record hash" filled in, "Max depth" is under "Details"; when a read-only network was not read this time and no anchor was read, it reads "Chain not read".

The command line's `depth` verb uses the same calculation.

---

## 8 · User

#### A full walkthrough

**As a User** (choose "User" at the top of the sidebar):

1. The first time the app opens, the wizard runs as a Recorder: set a passcode, create an identity, choose a network and create the ledger (free, local only). The rest can wait.
2. Switch to "User" at the top of the sidebar. A User ledger is needed only to sublicense; when you get there, press "Create the ledger" on the sublicense page.
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

  - Until this session's re-verification has run: "Needs attention" shows the last results, marked "last checked MM-DD HH:MM"; "Active grants" shows how many grants you hold, marked "Not verified". Until a verdict has been saved once, "Needs attention" shows "--" with "Not read yet".
  - "Check a received record" reflects only grant checks made in this session: until one has been run with a file, it shows "--".
- **"Recent"**: this session's grant check and record verification results, revocation-watch alerts, and "N pending" when sublicenses wait to be put on chain. Click a row to open its page. With nothing to show it reads "No recent activity".

### Grant vault

"My grants" in the sidebar. The toolbar has the segmented control "All" | "By issuer", "Verify again" and the blue "Add grant". Above the table are a search box, "Find by record, issuer or verdict", and two boxes, "Start date" and "End date", that filter by the time each grant went on chain.

#### Add grant

The "Add grant" dialog:

- Paste a grant code into "Grant code", or choose a grant file with "Choose a file" at the bottom left (or drop it onto the card); its name shows on the "Grant file" row ("None selected" until one is chosen). If the code carries a sublicense chain, every hop in it is added.
- A chosen grant file is checked on the spot, and "… verified · N hops in the chain · M terms files" shows under "Grant file".
- A pasted grant code gets a note: a grant code carries no issuer ledger, so only the signature and validity can be verified. For the full check, ask the issuer to export a grant file.
- "Grant name" and "Issuer name" are optional and kept only on this machine. They are kept for the grant this addition is for (the last hop of a sublicense chain) and its issuer only; the issuers of earlier hops are left as they were. The issuer name belongs to the issuer's address, so it shows on every grant from that issuer.

Press "Verify and add":

- Signatures and format are checked first. If any grant fails, none are added; the dialog stays open and gives the reason.
- Grants already held, byte for byte, are skipped. If all of them are already held, it says "This grant is already in My grants." and the dialog stays open.
- A copy of the grant file is kept in the data folder, and the issuer ledger inside it is used for later checks.

#### Each grant

Each grant has a row: "Entry", "Issuer", "Validity", "First anchor block time" (the time the grant went on chain in its issuer's ledger; "On-chain time not read" until it has been read) and "Verdict".

- The verdict is "All passed", "Has gaps", "Failed" or "Revoked by issuer".
- Until this session's re-verification reaches it, a row shows the last verdict marked "last checked …", turning grey after a day.
- A grant never re-verified shows "Not verified".
- If a file in the store cannot be read, the top of the page says "File rejected: …" for each one, with "Not added. Ask the sender to send it again."

Click a row for its detail page. The title is the record's name, with the issuer under it:

- **Status notes** (under the title)
  - Revoked: "The issuer has revoked this grant." and "Sublicenses based on it are void too."
  - Has gaps: when the issuer ledger was found nowhere, a note says it is missing; any other gap (including a chain that could not be read or no nodes set) reads "This grant is not yet confirmed on chain. Check again in a few minutes."
  - Issuer ledger handed over: "The issuer's ledger was handed over to a new key".
  - Each network the re-verification could not read is named, with "Unreachable" or "Fingerprint mismatch".
- **"Basic information"**
  - "Issuer", "Validity", "First anchor block time".
  - "Remaining": for example "Expires in N days", by chain time.
  - "Upstream": "None (issuer is the author)", or the state of the upstream ledger ("Upstream ledger: …" or "Upstream ledger not read").
- **"Credential"**: a row "Create credential" with the key of the same name (see "Credential"). When this session's re-verification has passed all six checks, "Sublicense to others" appears under it.
- **"Upstream ledger folder"**: the folder saved, or "Not set". "Change folder" on the right opens the "Upstream ledger folder" card: choose the "Folder" with "Choose folder" and press "Save upstream" to remember it.
- **"Details"**: the issuer's address, "Granted to", the entry ID, terms file fingerprint, content fingerprint, the saved upstream ledger folder, verdict, the raw result of each of the six checks, and "Check result" (the issuer ledger's check result).

#### Verify again

"Verify again" re-verifies every grant you hold:

1. It looks for the issuer's ledger, first on this machine and then in the vault. The vault is the ledgers carried by grant files you added, plus folders saved with "Save upstream".
2. It scans the chain, updates each card's verdict and records the result. It reads the main network and every one of the "Read-only networks" in Settings; a network that could not be read is named on the grant's detail page, and an anchor that could only be on it is not counted as missing. Chain time comes from the main network's nodes only: without them "Within validity" cannot be judged, so the grant stays "Has gaps" and does not offer "Sublicense to others".

Re-verification also runs on each sync and on the "Grant verification interval" in Settings > Notifications (600 seconds by default; empty or 0 means by hand only), once the main network's nodes, chain ID and registry contract are set.

### Upstream ledgers

The "By issuer" segment in the My grants toolbar groups the cards by issuer, with groups whose issuer ledger is broken first. Each group's header gives the issuer's name and the state of the issuer's ledger: "Issuer ledger: …" (or "Issuer ledger not checked") and "Last on chain N days ago" (or "On-chain time not read"). Grants not yet re-verified form a group of their own, "Not verified", with "Verify again to group by issuer".

### Credential

A credential is proof of a grant for a third party, such as your customers: a text file and a QR code. Press "Create credential" in the detail's "Credential" section; the "Create credential" card opens ("Saved to" defaults to `kits` in the data folder; "Choose folder" picks another); press "Create credential" on the card:

- It needs the network set up, and it reads the chain once; if the chain holds records this machine lacks, it is refused: "Ledger is not up to date" and "N records on chain are not on this machine".
- The app follows the grant chain back to its root using the grants you hold, and creates a `badge-<first ten characters of the grant ID>` folder at the save location containing `badge.txt` and `badge.svg` (the QR code).
- The "Create credential" row then shows "Credential created" with "Grant chain" (N hops), "Text file" and "QR code image", and the QR code beside them.

The other party verifies it by pasting the text into Verify > Verify grant.

### Draft sublicense

**Starting.** A grant that this session's re-verification has passed on all six checks has "Sublicense to others" on its detail page:

1. A confirmation card "Sublicense" shows "Entry", "Issuer" and "Upstream validity".
2. "Draft" opens the sublicense page. Nothing is written at this point.

**The sublicense page.** The upper half shows the upstream grant; the lower half is the same form as "Draft a grant":

- The record is fixed to the upstream grant's record.
- The validity is not automatically capped at the upstream validity. Watch it yourself.
- Press "Confirm" at the foot of the page. The confirmation card is titled "Sign sublicense" ("Upstream" is added under "Details"); press "Add to ledger" on it ("Send to chain now" when "Auto put on chain" is on) to sign.

The sublicense is written to your own (User) ledger and goes into your own pending queue. After signing, "Go put them on chain" and "N pending" appear under the form.

**What it needs.** Sublicensing needs a User signing key and ledger:

- No key: the page says "No key has been created." Press "Create key".
- No ledger: the page says "No ledger yet." and "Open your ledger before sublicensing." Press "Create the ledger".

### Revocation watch

After each re-verification of all your grants, the app checks two things:

- Has an issuer revoked a grant you hold?
- Has an issuer's ledger been handed over to a new key?

If either has happened:

- The window asks the system for attention once (on macOS the Dock icon bounces), and a toast appears: "Alert: Held grant revoked" or "Alert: The issuer's ledger was handed over to a new key". When a pass finds several alerts, the toast names the first and adds "N more alerts: see Alerts".
- A red "Revocation watch" row is added at the end of the Alerts page, linking to My grants.

Each event alerts only once, also across restarts; a ledger handed over again to yet another key alerts again. While the first-run wizard is open there is no toast and no attention request.

---

## 9 · Verify

"Verify" in the sidebar. Both roles have three tabs: "Verify grant", "Verify record" and "Look up a ledger". Where Home and a file dropped on the window lead still depends on the role: the Recorder's drop opens a new record, the User's goes to "Verify record".

Verifying sends no transactions. Three things are written on this machine: changes to the address book, snapshot files written by "Save snapshot", and the verification result files written by "Verify record" (see "Record verifier").

### Grant check

Put in all three things, then press "Verify" once:

1. **"Grant code"**: paste it into "Paste a grant code you received", or press "Choose a file" inside that field to pick a grant file.
2. **"File"** (marked "Optional"): press "Choose a file" to pick the work you received; while a file is dragged over the window, this row becomes a drop zone, "Drop or choose the file".
3. **"Terms"** (marked "Optional"): the same, for the terms file ("Drop or choose the terms file").

Until chosen, a row reads "None selected"; once chosen it shows the file's name and size, with "Change" and "Remove". Without a grant code, "Verify" cannot be pressed and says "Paste a grant code to verify it".

**"Advanced options"** (on its right, "Issuer ledger hops: N")

- "Hop 1", "Hop 2", …: one row for each hop of the grant chain; point it at the issuer's ledger folder by hand with "Choose folder". "Add next hop" chooses the next hop's folder.
- "Nodes for this check" (marked "Optional", one per line), "Registry for this check" (marked "Optional"), "From block": filled in with the main network from Settings and open to change; they set the main network only.
- "Check as of (seconds)" (marked "Optional"): empty means chain time.

**Which networks are read.** The check reads the main network plus every one of the "Read-only networks" in Settings; with only read-only networks set up and no main network, it still checks and does not report "No nodes configured". A network that could not be read is named, with "Unreachable" or "Fingerprint mismatch"; then a "Confirmed on chain" light with no anchor found is grey with "Chain unreadable right now", and the next step reads "The chain cannot be read right now. Check again later.", not "Not yet confirmed on chain", since the anchor may be on the network not read.

**Where the issuer's ledger comes from.** The app looks in this order:

1. This machine: the issuer's ledger on this same machine.
2. The vault: ledgers carried by grant files you added, and upstream folders you saved.
3. A record kit: the kit or grant file you point to, and the ledger carried by the grant file being checked.
4. A publish address: the issuer's https address. Each file is fetched and the kit must pass its check before it is used.

Below the result, the "Ledger source" group has one row per hop, "Ledger source: …", naming where it was found: "This machine", "Vault", "Record kit" or "Published address"; if nowhere, "Ledger source: None".

- On the row's right is "Change", or "Add" when nothing was found. It opens an "Issuer ledger" box ("Drop in a record kit, or enter an https address") for that hop, where you give an https address, record kit, ledger folder or grant file yourself. A folder chosen with "Choose folder" inside it checks again at once; a path typed in checks again on Enter. When the hop's ledger came from this machine and all six checks passed, there is nothing to fill and the key is not shown.
- A ledger from a publish address shows "From https://… · N files · verified" on the line below, with "Fetch again".
- If a source cannot be read, the reason is given for each one: "… could not be read: …".

**The result card** opens with the conclusion, the record's name below it, then three figures, "Grantor", "Validity" and "First anchor block time" (for several hops, all of the last hop):

- "All match" (green): the grant passed all six checks, and the file and terms, where given, match. Anything not given does not affect this.
- "N do not match" (red): the grant failed, or the file or terms do not match; each counts as one.
- "Not everything could be checked" (amber): nothing mismatches, but the grant has gaps or a file could not be read.

"Has gaps" never counts as passing.

**"Six checks"** is a group of its own. At its top come the rows that say more than the conclusion:

| Row | When it shows | Shows |
|---|---|---|
| "Grant" | When the grant did not pass every check | "Has gaps" or "Failed", with the tag "Undecided" or "Invalid" |
| "File" | When a file was given | The file name, or why it could not be read, tagged "Match", "Mismatch" or "Unreadable" (against the work hash the grant points to; for several hops, the last hop) |
| "Terms" | When terms were given | The same (against the terms fingerprint in the grant) |

Next, each kind of gap gets one sentence saying what to do next, for example "The issuer ledger is missing. Drop in a record kit or a grant file, or enter an https address, then verify again." or "This grant is not yet confirmed on chain. Check again in a few minutes."; with no nodes set it is a row "No nodes configured" with "Go to Settings" on its right. Each read-only network not read this time is named with "Unreachable" or "Fingerprint mismatch", and each file that could not be read is named with the reason.

Then the six checks, one row and one mark each:

- "Grant signature", "Issuer ledger intact", "In the issuer's ledger"
- "Confirmed on chain", "Within validity", "Not revoked"

A check that could not be completed is grey, with what is missing beside it: "Issuer ledger missing", "Issuer ledger unreadable", "No nodes configured · Go to Settings", "Chain unreadable right now", "Not yet confirmed on chain", "Chain time unavailable".

**Sublicenses (several hops).** Each hop's six checks come under a row "Hop N · issued by … · verdict", and after the hops each link has a row "Hop N links to hop N+1".

**"Details"** under the result holds the grantor, grantee and work addresses of the last hop, the verdict, the raw result of each check, the reason for each file that could not be read, and "Equivalent verb": the command-line verb that performs the same check (`check-grant` for one hop, `chain-check` for several).

### Check a received record

When you receive the work itself, put it into the "File" box on the Verify grant page and press "Verify" together with the grant. The app computes the SHA-256 of the file's bytes and compares it with the work hash the grant points to (for several hops, the last hop). The result card's "File" line reads "Match" or "Mismatch"; "Details" shows the byte count, the computed digest and the expected digest side by side. The work must be a single file: a folder is refused and the line reads "Unreadable". After changing the file, press "Verify" again. The User home tile "Check a received record" shows the last result.

### Record verifier

The "Verify record" tab under "Verify" in the sidebar (both roles).

Drop a record kit (a `.zkit` file or a folder), a grant file or a single record file onto "Drop a record kit or record file", or click to choose one; a kit in one file is read out item by item and checked, with no size limit. It is checked as soon as it is in. A publish address cannot be dropped: type it in "Record content" under "Advanced options" and press "Verify now".

**Encrypted record package.** Dropped in, it is tried with each identity on this machine in turn (a key for each role of each identity). If one is the recipient, the package is opened and verified as usual, and its originals can then be taken out; if none opens it, it is refused with "No identity on this machine is the recipient of this record package"; a package changed, cut short or reordered is refused with "The encrypted record package is damaged or incomplete; it was not opened". Opening only computes with the private key: it signs nothing and spends no gas; a failure leaves no plaintext. Dropped anywhere else (others' ledgers, due diligence, grant checks), it is refused with "This is an encrypted record package. Open it on the verify page".

**"Advanced options"** (on its right, the record hash given, or "None"):

- "Record content": a local file or folder, or an `https://` publish address; what was dropped shows here too. An `http://` or other address gets "Only https addresses are supported." under the field.
- "Record hash" (marked "Optional").
- "Verify now": checks "Record content" again.

**The result card** opens with the conclusion: "N mismatches." when anything does not match; otherwise, for a kit, "Record kit intact, signatures valid" or "Record kit check failed: …" (with the file at fault); otherwise "All match". From a publish address, the line below reads "From https://… · N files · verified", with "Fetch again" on the right. For a kit, grant file or encrypted record package on this machine, the right holds "Take out originals" (see below); a single file has no such key.

Below the conclusion, only what fell short is listed:

| Row | What it can show |
|---|---|
| "Record kit" | When there are mismatches and the kit check failed: "Record kit check failed: …" |
| "On-chain state" | "N not on chain"; "Not compared with the chain" when the chain could not be read (including no network) or a network was not read; "Not added" when the kit's network is not added |
| "Issuer ledger" | The ledger check result when it is not complete; "Issuer ledger not read" if it could not be read |

Each of the three results ("Record kit intact, signatures valid", "Verified as a single file", "N match the chain", and so on) is under "Details". A kit that carries only part of the ledger counts as complete and reads "This is part of the ledger".

**Mismatches**

- With any mismatch the card also says "See the error details and confirm with the sender before accepting.", and each one is listed under "Mismatch" in "Details". Each of these counts once: a failed kit check, a kit file that breaks the format, an entry file that could not be read or whose signature fails, a ledger check result other than complete (for a kit, a ledger that is only part of the whole is fine), a record not on chain, and a record hash filled in that the ledger does not hold.
- A chain not read counts as a mismatch: a failed chain read counts once, and each record on a network not read counts once. So when the kit's network is not added, no network is set up or no node answers, the conclusion reads "N mismatches." however intact the kit is.

With a record hash filled in, three readings for that record follow the conclusion: "First recorded", "Max depth" and "Continuity". When a chain was not read this time, a cell with no reading or a short one reads "Chain not read".

**Record by record.** For a kit, a grant file or a publish address, a "Record by record" group follows, one row per record: the record's name on the left ("Unnamed record" when it has none) and its on-chain state on the right: "On chain", "Not on chain" or "Chain not read". When the original's recomputed fingerprint does not match the record, the name has "Original in kit ✗" under it. When the kit holds no original, it has "Original in kit Missing". A match adds nothing.

**When the kit's network is not added.** When a kit says which chain it is anchored on (see "Record kit") and that chain with its registry contract is neither the main network nor among the "Read-only networks":

- The kit is still checked and the chain is not read. "On-chain state" reads "Not added", and each record's on-chain state is "Chain not read".
- The card shows the chain's name and "Stated by the author · Add this network in Settings", with "Add". "Add" goes to Settings > Network and opens the sheet for a new read-only network, with the chain ID, registry contract and start block filled in; you fill in the nodes.

**Which networks were read.** A network that could not be read this time (a read-only network, or the main network when read-only networks are set up) is named on the card, with "Unreachable" or "Fingerprint mismatch". A record with no anchor on the chains that were read then reads "Chain not read", not "Not on chain", since its anchor may be on the network not read. "Details" holds:
- the source, the entry count and the rejected entries
- the three results and "Networks read" (the networks read this time)
- the record hashes, and the record kit ID with its verdict
- "Chain ID", "Registry contract", and "First anchor location": each record's first-anchor time, block number and transaction, the transaction in full
- "Result file", with this pass's result file path, or why it could not be written

**Take out originals.** "Take out originals" sits to the right of the result card's conclusion, and only for a kit, grant file or encrypted record package on this machine: a single file has none, and a publish address has "Fetch again" there instead. It can be pressed once the record package passes verification and holds originals matching their records. Otherwise it cannot, and it says why ("The record package did not pass verification", "No original matches its record"). Pressed, it opens a dialog:

- "Take out to" holds a folder: the one used last in this run of the app, else the Documents folder. "Choose folder" at the bottom left changes it through the system's dialog; "Cancel" and "Take out" are at the bottom right. Only "Take out" writes, in the background, and a notice then says "Took out N originals".
- The dialog lists the originals "Taking out" by name, with how many ("N originals"); those that do not match their records are listed by name with "Does not match its record; not taken", those the package lacks with "Not in the package; not taken".
- Matching: a single file, the SHA-256 of its bytes is the content recorded; a signed folder, its folder digest recomputed by the same method signing used is the content recorded, and the whole folder is taken out as it is; a folder signed on Windows by earlier versions (paths joined by `\`) is recognised too. Taking out reads the package again and judges again; it does not trust the verification's result.
- Names: each original gets back its name from the package's name table. A name that is not one valid file name (a path separator, `.` or `..`, a control character, a name the system reserves) is replaced by its name in the package, and the dialog says "Its name cannot name a file; the package name is used". A package path not in the kit form is refused: "A path in the record package is not of the package path form. Nothing was taken out." Two originals that would take one name (in any case) refuse the batch: "Two originals would take the same name. Nothing was taken out."
- A name the chosen folder already holds refuses the whole batch, not a byte written: "The chosen folder already holds items with these names. Nothing was taken out.", naming which. What was chosen not being a folder is refused with "What was chosen is not a folder. Nothing was taken out." A link at that name in the chosen folder is refused with "This name in the chosen folder is a link. Nothing was taken out.", and nothing is ever written outside the chosen folder by following one. An original whose fingerprint does not match its record when checked gets "An original taken out does not match its record; the package may have changed meanwhile. Nothing was taken out."; a failed write gets "Writing to disk failed and was taken back. Nothing was taken out."
- Everything is first written into a staging folder inside the chosen folder and checked file by file against its fingerprint, then moved into place one by one; if any step fails, what was placed is taken back and the staging folder removed, and the chosen folder is as it was before.
- An encrypted record package, once opened, can be taken out the same way.

**The verification result file.** Each kit, grant file or publish address verified writes a result in the machine folder under `kits/verified/`, named `0x<sha256 of the kit's manifest file>.json`; verifying again replaces it whole.

- It holds whether the kit holds; each record's earliest counted anchor (chain ID, registry contract, block, time, transaction); the anchors this pass found, each with its registry contract; the networks read this time; the networks not read (chain ID and registry contract, with the reading code `down` (unreachable) or `fingerprint` (fingerprint mismatch)); the digest of the core that judged. For a kit that does not hold, only its verdict is written.
- It is written when the kit does not hold too; when the kit holds and no chain was read this time (its network not added, no network set up, no node answering), it is not written.
- The file is plain, for other apps to read. ZIKARON does not read it back, and it changes no verdict.

### Other ledgers

The "Look up a ledger" tab under "Verify" in the sidebar.

Fill in "Author address" ("Address book" inside the field, on its right, offers saved ones) and press "Read". The app scans the chain for that address's anchor proofs and looks for its ledger in the same four places as described under "Grant check". The main network must be set up first (with only read-only networks it does not read); any read-only networks are read as well.

- "Record content" (marked "Optional") under "Advanced options" can point to a record kit, grant file, ledger folder or https publish address, or be chosen with "Choose folder" inside the field.
- Beside "Read" is "Add to address book" when the address is not in the address book, or "Remove from address book" when it is; they change only this machine's address book.

**"Overview"** opens with the ledger check result, with "<author address> · N entries" below it; with no ledger content, the result reads "Anchor proofs only, no ledger". Three figures follow:

| Figure | Shows |
|---|---|
| "Record count" | Not counting deleted records |
| "Grant history" | How many grants |
| "Last on chain" | The date and entry number of the last thing put on chain ("<date> · #N") |

- Where the ledger came from and how many anchor proofs are on chain are under "Ledger source" and "On-chain records" in "Details". When a place could not be read or none had the ledger, the overview adds a "Ledger source: …" row, and each place that could not be read is given with its reason ("… could not be read: …").
- If none of the four places has the ledger, it says "No ledger content yet: provide a record package or publish address", and the record and grant counts read "Not obtained" rather than 0.
- Further down, the "Grant history" group has one row per grant: the record's name, with its first anchor block time and its validity on the right ("No grants yet" when there are none).

**"Entry list"** lists the other ledger entry by entry; click a row for a read-only detail. Deletions are read the same way as in your own ledger (Appendix C). When a network was not read this time, "Overview" names it with "Unreachable" or "Fingerprint mismatch"; an entry with no anchor on the chains that were read gets a grey light, and its detail's "On-chain state" reads "Chain not read", not "Not on chain".

### Due diligence

A User does due diligence on the "Look up a ledger" page, which has more under "Advanced options":

- "Record hash" (marked "Optional").
- "Validity needed" (marked "Optional"): "From (seconds)" and "To (seconds)".
- "Save snapshot" (see below).

Below the overview comes a "Grant conflict check" group, which tells you whether the period you need overlaps active grants the author has already issued. It checks only when both "Record hash" and "Validity needed" are filled in; otherwise it shows "No record entered; not checked" or "No period entered; not checked".

- Overlap, in red: "N active grants overlap this period".
- No overlap, in green: "No active grants overlap this period".

"Details" at the foot of the page also gives the "Key lineage"; each handover of the author's ledger to a new key is one line in the "Key handover" row; with a record hash there is "Max depth" too ("Chain not read" when a network was not read and no anchor was found).

"Save snapshot" opens a dialog of that name: "Snapshot location" is the folder used last in this run of the app, else `kits` in this data folder, else the Documents folder; "Choose folder" at the bottom left changes it, and "Cancel" and "Save snapshot" are at the bottom right. "Save snapshot" writes the last due-diligence result (press "Read" first) there as `snapshot-<first ten characters of the address>.json`. A snapshot is not proof.

---

## 10 · Alerts

### Alert list

"Alerts" in the sidebar. Only what needs action is listed, one matter per row: a light (amber or red), the item and its current state. The "Pending" and "Queue backlog" rows have "Put all on chain" at their right, which sends everything that can be sent. Every other row can be clicked as a whole and goes to the page where it can be dealt with, opening the relevant part (for example the ledger check card, or the grouping by issuer). With nothing to deal with, the card holds a check mark and "Nothing needs your attention".

| Recorder | User |
|---|---|
| "Pending" | "My grants expiring" |
| "Queue backlog" | "Grant revoked" |
| "Grants expiring" | "Ledger handover" |
| "Ledger check result" | "Upstream issue" |

- When "Pending" and "Queue backlog" say the same thing, they are merged into one row.
- **"Grants expiring"** and **"My grants expiring"** count grants whose validity ends within seven days, by chain time only; "My grants expiring" also counts expired ones (then red). Grants without chain time are left out, not guessed.
- The whole-machine backup is not on this page. Its state is under "Last backup" in Settings > Local data, and in the "Whole-machine backup" row of the setup check on the About page.
- **"Grant revoked"** is red and **"Ledger handover"** amber.
- **Revocation watch** alerts come at the end of the table, one row each. Each is named "Revocation watch", is red, and reads "Held grant revoked · <record>" or "The issuer's ledger was handed over to a new key"; clicking it opens My grants. They are listed only in the session that found them (the "Grant revoked" and "Ledger handover" rows count only these); after a restart they are gone, though a revoked grant still shows "Revoked by issuer" in My grants.
- The number next to "Alerts" in the sidebar counts the rows on this page.
- The full message from the last failed chain read is under "Details".

**How alerts reach you.** Alerts travel as a window attention request plus a toast. When one of these newly appears, the window asks the system for attention once and an alert toast appears:
- for the Recorder: "Grants expiring", and "Ledger check result" when the chain is broken;
- for the User: "My grants expiring" (expired ones included), "Upstream issue", and revocation watch.

The toast reads "Alert: " and the row's name, followed, where it applies, by the grant or issuer it is about; revocation watch reads "Alert: Held grant revoked" or "Alert: The issuer's ledger was handed over to a new key". When several arrive together it names the first and adds "N more alerts: see Alerts". Other amber and red rows (for example "Pending" and "Queue backlog") show only on this page and in the sidebar count, with no alert, and the whole-machine backup never alerts. While the first-run wizard is open, nothing is toasted.

- Each matter alerts only once; the alerts already given are remembered in the data folder's settings. If the situation changes (a new deadline, the ledger broken somewhere else, new entries in an upstream ledger), it counts as a new matter and alerts again.
- Network errors never alert; they only show "Sync failed" in the status line.
- There is no setting to turn alerts off.

---

## 11 · Settings

"Settings" in the sidebar opens a list of eight sections; click one to open it. Both roles have the same sections.

| Section | Contains |
|---|---|
| "Language and time" | Interface language and time zone |
| "Appearance" | Light, dark or follow the system |
| "Identity key" | Identities, passcode and key backup |
| "Network" | Nodes, registry contract and publish address |
| "Notifications" | How often the ledger is checked, or grants re-verified |
| "Local data" | Encryption, backup and data folder |
| "Command line and AI control" | Terminal and AI agent automation |
| "About" | Version and setup check |

**Where settings are kept**

- Appearance and the last language chosen are kept on this machine. They apply to every identity and are used on the passcode gate too. Auto-lock, the record of the last whole-machine backup, the "Read-only networks", "Proxy", "Putting on chain from the command line" and the command line's two switches are kept on this machine as well, shared by every identity; all of them but the read-only networks are encrypted and read only once unlocked (see Appendix B). The network last chosen in the wizard is kept here too, but it only decides which choice a new identity starts on; each identity uses its own network.
- Everything else is kept in the current data folder. Without a data folder, or when it is read-only, these cannot be changed: the page stays as it was and a toast gives the reason.

#### 11.1 Language and time

- **"Language"**: "中文" or "English". The choice is kept on this machine (the passcode gate and the first-run wizard use it; the first-run wizard switches it too) and, with a data folder open, in that folder's settings; a data folder that can only be read refuses the change.
- **"Time zone"**: "UTC" (the default) or "Follow the system".
  - Every time in the window is shown in it. The full form is `2026-09-24 23:46:12`; narrow places show only the date or `MM-DD HH:MM`.
  - If the system time zone cannot be read: "The system time zone could not be read; moments show in UTC".

#### 11.2 Appearance

"Light" (the default), "Dark" or "Follow the system". It takes effect at once.

### Identity and keys

Settings > Identity key.

A card at the top shows the current identity's name and its role and kind. "Copy address" copies the current role's signing address. The card's three readings:

| Reading | Meaning |
|---|---|
| "Key address" | The current role's signing address; when its key cannot be found, the reason: "No signing key yet" or "Key not in this machine's key store"; for the empty role of a local-key identity, "Unoccupied", and a note at the top of the page reads "This key is already used by the other identity. This identity cannot sign and has no data folder." |
| "Gas balance" | The balance of the current role's signing address, or "Not read"; when the last read failed, "This read failed: …" with the reason |
| "Backup status" | "Recovery phrase confirmed, key file exported", "Recovery phrase confirmed", "Key file exported" or "Not backed up"; when the exported key file is not on disk right now, "Marked as backed up, but the file is not there now" |

**The "Passcode" group**

- **"Change passcode"**: enter the current passcode, then the new one twice. The toast reads "Passcode updated". A wrong current passcode counts as a wrong try.
- **"Auto-lock"**, and while auto-lock is on, **"Idle time"** below it (see 5.2).
- With no passcode yet, the group has only the "Passcode" row, reading "No passcode yet", with "Go set one" at its right.

**The "Export" group**

- **"Export key file"**: a standard keystore file for use in other wallets. A key file exported from the primary identity (either role) also recovers the passcode at the gate. Steps:
  1. Enter this machine's passcode.
  2. Enter a file password in "Password" (hint "At least 8 characters") and again in "Confirm password". The strength reading is only a guide and never blocks you.
  3. Choose the folder under "Saved to" and press "Export key file".
  - The file can be read only by you. The app reads it back to check it; only then does it say "Key backed up to …" and update the backup status.
  - The row is unavailable while the current role has no signing key.
- **"Show recovery phrase"**: after you enter the passcode, the 12 words are shown. "Hide and close" clears them. A local-key identity has no recovery phrase: this row is unavailable, noting "No recovery phrase for this identity. Back it up with a key file."

**The "Identity" group**

- **"Primary"**: "This identity" when the current identity is primary, else the primary's name, with "Set as primary" below. The primary identity is the one whose recovery phrase or exported key file recovers the passcode.
  - On the "Set as primary" card, enter this machine's passcode and press "Set as primary".
  - The old primary becomes a secondary identity. Every key and all local data are re-encrypted under a new master key; the passcode stays the same. The change happens completely or not at all.
  - A local-key identity imported from a raw private key must export its key file before it can become primary.
  - It is refused while a data folder is not where it should be (an external drive not connected) or while background writes are running. Try again later.
- **"Switch identity"**: its right side says how many there are ("N in all"); with only one identity it reads "Only one" and cannot be pressed. The card it opens has one row per identity: its name and kind (the primary one followed by "Primary"), "Switch" at the right, the one in use marked "In use"; every identity's two full addresses are under "Details" at the card's foot. Only "Switch" actually switches. The switch runs in the background (the status says "Switching identity") and the window does not stall; the new identity's pages replace the old ones in place once read. Switching to another identity on the way, the last one asked stands; a switch that fails says why and stays on the identity it was.
- **"New identity"**: an optional "Label", then under it "Network" for the network this identity uses, then generate, write down and check 12 words, as in wizard step 2.
  - "Network" lists each preset network of Appendix A, then "Custom"; the one this machine last chose in the wizard is selected first, Ethereum mainnet if it never chose.
  - The chosen network fills both roles' data folders of this identity; both roles use it.
  - With "Custom", this identity's data folders have no network, and reading the chain or anchoring says it is not configured, until it is filled in and saved in Settings > Network (by hand or from a preset). Saved in one role, the other role takes the same network when it opens; other identities are not affected.
- **"Import key"**: three tabs, "Recovery phrase", "Private key" and "Key file".
  - Recovery phrase: 12 boxes; the whole phrase can be pasted.
  - Private key: 64 hexadecimal digits, with or without 0x.
  - Key file: drop in the keystore file and enter the "Key file password".
  - For a private key or key file, choose "Import as which identity" ("Recorder" or "User"). The key fills only that role; the other stays empty.
  - On a machine with no identity yet, the imported one becomes primary. A private key then exports its key file as it is imported: the form adds "Password" (hint "At least 8 characters"), "Confirm password" and "Saved to", and the identity is created only after the exported file is read back and checked. Left empty, it is refused with "This identity has not exported its key file.".
  - An optional "Label", and "Network" as for "New identity". Press "Import identity". Opened from the wizard, the form has no "Network": wizard step 3 decides it.
  - Imported where identities already exist, it is secondary and cannot recover this machine's passcode. An identity already on this machine is not imported twice, but if its keys are missing, importing it puts them back.
  - An imported identity cannot write until its ledger has been checked against this key's records on chain (see "Fetching the ledger" under "Storage").
- **"Rename"**: change an identity's label. It only helps you tell identities apart and affects no verdict.

New and imported identities are secondary. The exception is a machine with no identity yet, where the first one created becomes primary.

**Deleting an identity.** "Delete identity" at the bottom:

- **The primary identity cannot be deleted directly.** First make another identity primary ("Set primary"). If it is the only identity, create or import another one first.
- **Deleting a secondary identity**:
  - The card states the consequence ("The ledger can no longer be continued" for a Recorder, "Grants under this identity will no longer be yours" for a User) and lists which roles' ledgers already have entries.
  - Enter this machine's passcode and press "Delete identity".
  - Only the keys in the key store and the identity's registration are removed. The data folders are kept.
- **An identity never backed up cannot be deleted**, unless a handover from its key is already recorded in one of its ledgers: "This identity is not backed up and cannot be deleted." and "Back up the key or confirm the recovery phrase first. A recorder can also record a handover."
- A Recorder should record a handover first (see "Hand over ledger"); otherwise a new key cannot continue the ledger.

**"Details"**

- The current role's key address, with its "Public key" on the next row and a copy icon: what someone fills in to encrypt a record package to you (see "Record kit"). Both roles' full addresses ("Unoccupied" for an empty role).
- "Backup file": when the key file was exported, if it has been; another row says whether that file is on disk right now: "The file is there now", "Marked as backed up, but the file is not there now" or "No key file has been exported".
- "Created": the date this identity was created, or "Not recorded".
- "Saved to": "This machine's key store (passcode encrypted)".
- Derivation paths, for recovery-phrase identities only.
- The identity list file.
- "Signing domains", and "Domains this identity signs" in its current role: a Recorder signs entries (`entry`) and co-signatures (`cosign`) and sends anchor transactions (`anchor`); a User signs entries in its own ledger and sends anchor transactions.

#### 11.4 Network

**Overview**: the first line gives the network's name and chain ("<name> · Chain N"), with "Read chain" on the right; the line below is the last chain reading.

- The name:
  - "Ethereum mainnet" or another preset's name: this data folder's four cells equal that row of Appendix A. When the identity later chooses another network, a data folder that already has one stays as it is.
  - "Custom": set by hand.
  - An amber "No network set yet": no nodes, chain ID or registry contract at all; "Read chain" cannot be pressed then, and "Edit nodes" is the main key.
- The reading:
  - "N nodes agree"
  - "Only 1 node responded; no cross-check"
  - "Not read"
  - "This read failed: …" with the reason, when the last read failed
  - When some nodes give no answer, the reading ends with "N node(s) did not answer", and what each node said is under "What each said".

**"Nodes and chain"** is a group: "Nodes" ("Edit nodes" on its right; "Not set" when there are none), "Registry contract", and the "Auto put on chain" switch (off by default, kept separately for each data folder; see "Pending queue"). The nodes, chain ID, registry contract and start block in full are under "Details" at the foot of the page.

**Buttons**

- **"Edit nodes"** opens "Edit nodes and chain". A data folder with no network is filled in here and only here.
  - At the top, one of two: "Ethereum mainnet" fills the nodes, chain ID, start block and registry contract from that row of Appendix A; "Custom" empties the four cells and shows "Preset" on the right, where a preset network of Appendix A fills them in. A preset only fills the cells: "Save" makes it take effect. The "Preset" in the read-only network dialog is the same table.
  - "Nodes" are written as `chain-id=node-url`, separated by spaces; "Chain ID", "Start block" and "Registry contract" are one cell each. "Save" can be pressed once all four are filled in; until then it is grey and says "Still needed: …" on hover. "Cancel" saves nothing.
  - "Save" saves the chain settings first, when they changed, then the nodes. Saving the chain settings first asks the nodes for that registry's code: if it is not the build ZIKARON pins, nothing is saved and the message is "The registry … is not the pinned build; the chain settings were not saved". With no nodes yet the settings are saved unchecked, and the same check runs once nodes are set. The last check's reading stays beside "Registry contract": "Agreed", "Single source", "Unreachable", "Fingerprint mismatch" or "Not checked: no node".
  - Once you have saved your own values, "Network" reads "Custom"; if the chain ID, registry contract, start block and nodes (in any order) saved equal a row of Appendix A as it is today, it reads that row's name.
  - Saving other nodes or other chain settings voids the gas estimate and fees in hand; the "Put on chain" card estimates again the next time it opens.
  - When this identity chose "Custom", the chain ID, registry and nodes, once complete in one role's data folder, are recorded on this identity: the other role's data folder takes them when it opens, so you do not configure them twice. Other identities do not take them.
- **"Read chain"**: reads the signing key's balance once.

**Node connections.** A node address starts with `https://` or `http://`. For https nodes, the certificate and host name are checked on every connection; if the certificate fails, the chain cannot be read. When a node refuses or a connection fails, the message names the cause (rate limit, credentials needed, wrong network, certificate, timeout and so on), with the node's own words under details. When a node's address resolves to several IP addresses, each is tried; every question has one overall deadline that covers connecting, sending and reading; a rate-limited node is asked again after waits from one table. An access key inside a node address (in its path or query) never appears in any message, detail or log.

**"Proxy"**: how connections to nodes go out, one setting for every identity on this machine. Nodes on this machine (`localhost`, `127.0.0.1` and the like) are never reached through a proxy.

- **"System"** (the default): follows the system's proxy settings: first the automatic configuration script (PAC) at the address the system names, then the proxies and exceptions typed into the system. The system's "automatically detect proxy settings" (automatic discovery) is not read: the app never looks for `wpad` by name, and a system with only that switched on counts as one with nothing set (straight); to use a proxy such a network gives, enter its configuration script's address or the proxy in the system. The app fetches the script itself (never through a proxy; at most 1 MiB within 5 seconds): an `https://` address from anywhere (its certificate checked); an `http://` address only on this machine, a literal loopback address (`127.0.0.0/8`, `::1`) or the name `localhost`, judged as written with no lookup. An `http://` script on another machine is not fetched, nor one a redirect points to on the way; the way goes without it (`PAC_REMOTE_HTTP`), and the remedy is an `https://` address or the proxy entered under "Custom proxy", whose address is on the line `PROXY host:port` of the script opened in a browser, or the network's administrator can give it (so nobody on the network can hand in a script; a remote `http` configuration script used before upgrading is no longer used after). A `file:` script is not read. The app runs the script in an isolated JavaScript engine (QuickJS): only the standard PAC functions, no network, files, modules or timers, at most 2 seconds, 8 MiB of memory and 512 KiB of stack a run; name lookups the script asks for are answered outside it, at most 1 second each. The script sees a node's scheme, host and port, never its path (where an access key often is). The ways the script names are tried in turn until one connects; `SOCKS` tries SOCKS5, then SOCKS4. A script that cannot be used leaves the way without one; when that way does not connect either, the failure's next step says the script's trouble and the way out (nothing is said when it connects, and there is no status line for it): a remote http script, "Proxy script is on remote http; not used. Set a custom proxy."; not fetched, "Can't fetch the proxy script. Start your proxy app, or set a custom proxy."; any other, "Proxy script failed (code). Set a custom proxy.". The command line says the same on its error line, with `--proxy` as the way out. The codes: `PAC_UNFETCHED` (not fetched), `PAC_SCHEME` (not an http/https address), `PAC_REMOTE_HTTP` (an http address on another machine), `PAC_TOO_LARGE`, `PAC_FAILED` (an error, or it does not parse), `PAC_TIMEOUT` (ran too long), `PAC_MEMORY` (memory or stack ran out), `PAC_NO_FUNCTION` (neither `FindProxyForURL` nor `FindProxyForURLEx`), `PAC_ANSWER` (its answer does not read). Where each system is read: macOS, the network configuration; Windows, the current settings as the system answers them (the machine's when it sets one for every user); Linux, the desktop's stored settings first (KDE's `kioslaverc`; GNOME, Cinnamon and MATE's GSettings), then `https_proxy`, `http_proxy`, `all_proxy`, `no_proxy`.
- **"Off"**: always connects directly.
- **"Custom proxy"**: fill in its cells: the type (HTTP; HTTPS, with TLS to the proxy too, its certificate checked; SOCKS5; SOCKS4), host and port, and for a proxy that asks who you are, the user name and password (as they are: no `%40` or `%3A` for `@` or `:`; SOCKS4 has a "User ID" cell only); press "Save". The port takes a whole number from 1 to 65535; a cell that is wrong is framed red with its reason below, and "Save" cannot be pressed. A whole address copied from proxy software (`socks5://user:password@host:port`, or `host:port`) pasted into the host cell is split into the cells; an address typed into the host cell is split once the cell is left, never mid-way; a user name or password the address does not carry stays in its cell. The password is kept in the encrypted machine settings; once saved its cell stays empty and the page says "Password set". Saving with that cell empty and the same type, host, port and user name keeps it; change any of them and the password does not go along (it is never given to a proxy it was not given to), so type it again. Private (`10.x`, `172.16`–`172.31`, `192.168.x`), link-local (`169.254.x`, `fe80::`), this machine's and `.local` nodes never go through a custom proxy.
- **A proxy that asks for credentials**: when a proxy asks for a user name and password and none were given, the message says so ("The proxy asks for a user name and password", `PROXY_CREDENTIALS_MISSING`); when it refuses the ones given, it says that (`PROXY_CREDENTIALS_REFUSED`); either way the next step points at the custom proxy's user name and password cells. Following the system with a system proxy that wants a password is the same: enter that proxy with its user name and password as a custom proxy. The command line answers `E_UNREACHABLE` with the same `token`, and says how `--proxy` carries credentials.

With a VPN: a VPN that tunnels all traffic needs nothing here; one that only sets a system proxy (an automatic configuration script included) works with "System"; if nodes still cannot be reached, enter the proxy address the VPN gives under "Custom proxy".

**"Published address".** The https address where you intend to put record kits online:

- Only `https://` is accepted; anything else gives "Only https addresses are supported."
- The app does not upload anything. Put the record kits on that static host yourself.
- Publishing takes a record kit as a folder (export it with "Export as a folder" on). Comparing a kit in one file says "A record package in one file is not published at an address; to publish, export it as a folder."
- The "Published address" row has "Edit" on its right, which opens a dialog for the address; press "Save published address". The "Check publication" row below is always there, and cannot be pressed until an address is saved. It opens the "Check publication" dialog: "Record kit to compare" is filled in with the newest folder record kit in this data folder, "Choose folder" at the bottom left picks another, and "Check" at the bottom right fetches the kit from the publish address file by file and compares it with your local copy:
  - "Published": "All N files match."
  - "Publication incomplete": "N files missing, M do not match.", listing the missing and mismatched files.
  - "Cannot reach the published address", with the reason.

**Read-only networks.** The "Read-only networks" section under the main network is shared by every identity on this machine and kept in the machine folder; with none added there is no file.

- They are only for checking other people's material: "Verify record", "Look up a ledger", grant verification and due diligence read the main network and every network here. Writing the ledger, putting on chain, the balance and the ledger check use the main network only.
- "Add", the list's last row, opens the "New network" dialog; the "Preset" in its "Name" field offers each preset network of Appendix A and fills in the cells.
- In the dialog, fill in "Name", "Nodes" (one per line), "Chain ID", "Start block" and "Registry contract", and press "Save"; until they are filled in, "Save" cannot be pressed and says which are missing; "Cancel" saves nothing. Saving only writes on this machine; the chain is not read.
- "Name": for Ethereum mainnet, OP Mainnet, Base, Arbitrum One and their testnets it is filled in and cannot be changed; any other chain can be named by hand, and without a name it shows as "Chain <chain ID>". The name is shown on this machine only.
- A row with the same chain and registry contract as one already listed, or as the main network, is refused: "This network is already listed."
- Each row of the list gives the name, "Chain N" and the reading; clicking a row opens its dialog: at the top a "Chain reading" row with the reading and "Read the chain" beside it, then the cells, "Save" and "Cancel" at the bottom right and "Remove" at the bottom left. "Remove" pressed once asks "Remove this network?"; press "Remove" again to remove it.
- "Read the chain" gives "Agreed" (every node that answered gave the code of the pinned registry build), "Single source" (one node answered), "Unreachable" (no node answered) or "Fingerprint mismatch" (the registry's code is not the build ZIKARON pins).
- Before a read-only network is read, the code at its registry is checked every time. A row with "Fingerprint mismatch" is not used that time, and not one anchor on it counts. The main network is checked when "Save" is pressed in "Edit nodes and chain" (see above).
- Rows on the same chain are read as one window: every registry contract, from the earliest start block. A row that could not be read this time is named in the result, and the others are checked as usual.

#### 11.5 Notifications

Both roles have both rows, each a field of seconds with the unit "seconds" and a save key beside it:

- "Ledger check": 300 seconds by default, saved with "Save interval".
- "Grant verification interval": 600 seconds by default, saved with "Save review interval".

A field takes a whole number of seconds only. Otherwise its save key cannot be pressed and says "Period must be a whole number of seconds"; it also cannot be pressed while the value equals the saved one. Empty or 0 turns the automatic run off, and the field shows "Manual only".

How alerts reach you is covered in section 10.

### Storage

Settings > Local data.

**Overview at the top.** The first line says how the data is kept and whether it can be written: "Passcode-encrypted · Writable", or "Passcode-encrypted · Paused: reconcile after restore", with "Reconcile now" on the right, which checks the local ledger offline and, if it passes, allows writing again. Three figures follow:

- "Ledger entries"
- "Last reconciled": the check result with "N entries"; "Not run yet" before the first reconciliation
- "Last backup": the time, or "Never". When ledger entries or received grants were added since, "N entries" follows the time and the figure turns amber.

"Usage" ("… of … used", with a red line under it when over the limit) is under "Data folder". What this machine holds ("N identities · N ledger entries · N records · settings") is under "Contents" in "Details" at the foot of the page.

#### Whole-machine backup

One backup file restores everything: every identity and key on this machine, all local data and all settings. It also carries the "Read-only networks". It does not contain the master key, the passcode or the wrong-try count, nor the record of checked facts (asked again after restoring), the data folders that deleted identities left behind, or exported record kits with their index and verification results.

The "Whole-machine backup" group has two rows, "Export backup" and "Restore from backup"; the Recorder also has "Export ledger mirror" (see "Ledger mirror"). Where the backup stands shows in "Last backup" at the top, and in the "Whole-machine backup" row of the setup check on the About page. Never backed up, entries added since, or the last export not written or not read back: that row has "Export backup" on its right. Up to date: it reads "Done". One successful export clears it.

- "Export backup": the dialog's "Contents" row says what this backup holds; "Saved to" starts at the folder chosen last, else where the last backup went, else the Documents folder, and "Choose folder" at the bottom left changes it.
  1. Choose a password in "Backup password" (at least 8 characters) and repeat it in "Enter the backup password again".
  2. Enter "This machine's passcode" and press "Export".
  - The file is named `zikaron-backup-YYYY-MM-DD.zikaron`. A second export on the same day gets a number; nothing is overwritten. The dialog's "Details" gives the file name, header, encryption and what is "Not included".
  - The app reads the file back and opens it to check it; only then does it say "Backup exported: …".
  - If you forget the backup password, the backup cannot be restored.
- "Restore from backup":
  - Choose the backup file with "Choose a file" at the bottom left, enter the "Backup password" and "This machine's passcode", and press "Recover".
  - The machine's identities, keys and data are replaced by the backup's. The passcode stays the same. Anything on this machine that is not in the backup is lost. The backup's read-only networks replace this machine's; a backup that carries none leaves them as they are.
  - The restore happens completely or not at all.
  - Old files that still cannot be opened afterwards are moved unchanged into the `set-aside` folder in the machine folder. Nothing is deleted.

With only a recovery phrase and no whole-machine backup, only data readable on chain can come back.

#### Fetching the ledger (imported identities)

When you import an identity from its recovery phrase, private key or key file, its ledger does not come with it, and the key may have written elsewhere. So an imported identity can read but cannot write at first, and a red box appears at the top of this page, "This identity was imported: its ledger has not been checked against the chain".

Whether it can write turns on one thing: this role's ledger has been checked against this key's records on chain, and every record on chain is in the ledger. Where the ledger came from does not matter:

- Once a network is set, the app checks by itself (if one was already set when importing, right after the import). If the chain holds no record of this key (a new key, say) or every record is in the ledger, writing opens.
- After "Adopt in place" takes over an existing ledger folder, it is checked again, and writing opens if it matches.
- If the chain holds records the ledger lacks, the box changes to "Newer entries exist elsewhere: N records on chain are not in the fetched ledger" and stays read-only; fetch the ledger from a whole-machine backup.

To fetch it from a whole-machine backup:

1. Choose a whole-machine backup, enter the "Backup password" and press "Fetch ledger".
2. The app takes this identity's ledger from the backup and compares it with this key's anchors on chain:
   - **Everything matches**: "Fetched N entries; all N records on chain are in the ledger: writing is open". Writing is allowed again.
   - **The chain has anchors the ledger lacks**: the box changes to "Newer entries exist elsewhere: N records on chain are not in the fetched ledger" and stays read-only. Find the newer backup and fetch again.
   - **Entries on this machine conflict with the fetched ones**: a conflict card appears. After "Fetch and replace", the machine's previous data is kept as "Old data" and can be opened read-only with "Look up" in the "Old data" group on this page.

### Ledger mirror

Only on a Recorder's Local data page.

"Export ledger mirror" opens a dialog of that name, subtitled "An export of the ledger; it cannot restore this machine.", with three rows: "Last exported", "Added since" and "Mirror location". The location starts where the last export went, else `ledger-mirror` in the Documents folder; "Choose folder" at the bottom left changes it, and "Cancel" and "Export mirror" are at the bottom right. "Export mirror":

- Exports the ledger to a folder you choose, laid out as `ZIKARON-backup/<address>/author` (the address in lowercase, without `0x`).
- If the folder already holds an older mirror of this identity, only the new entries are added.
- A mirror is an export for other tools to read. It cannot be used to restore this machine; use a whole-machine backup for that.

#### Data folder

Each role of each identity has its own data folder.

- "Change data folder": choose another folder and press "Open data folder". This lasts only until you quit; the next launch returns to this identity's own data folder.
  - Only a folder that is already a data folder (an older one missing a room counts) or an empty folder can be chosen (one holding only files the system leaves, such as `.DS_Store`, `Thumbs.db` or `desktop.ini`, counts as empty); an empty folder gets a data folder laid out in it.
  - Any other folder (a ledger folder made by the command line, a folder of documents) is refused: "This folder is neither a data folder nor empty.". Nothing is written there and the current data folder stays open. To take over a ledger made by the command line, use "Adopt in place" under "Advanced options" below.
  - A data folder that belongs to another identity or role, or was sealed on another machine, is also refused before anything is written there.
- "Measure usage": counts the space used and the number of entries.
- "Hide entries deleted on this machine" (Recorder, a switch): when on, the Records and Ledger lists leave out entries deleted before they went on chain, and their deletion entries (see "Deleting a record" under "Record on chain"); entries deleted after going on chain are still listed. It changes only what the lists show, is off by default, and is kept in this data folder.
- The empty role of a local-key identity has no data folder, so these buttons are unavailable.

**"Advanced options"**

- "Usage limit", a field in "bytes", then "Save limit".
- "Move to an empty folder":
  - Choose a folder and press "Move". The data folder moves there and becomes this identity's data folder. The copy runs in the background and the key says "Moving"; meanwhile this data can only be read (writing and another move are refused), and only once copied and checked does the app switch to the new place, the old place unchanged.
  - If the chosen folder is not empty, the data goes into a new `ZIKARON` subfolder inside it.
  - The new place cannot be the current data folder itself or lie inside it (judged by the real path, symbolic links resolved): such a choice is refused with "The new location is inside the current data folder; nothing was copied."
  - The old location is left untouched; delete it yourself afterwards.
- "Adopt existing ledger folder", then "Adopt in place": takes over an existing ledger folder, re-checking every entry before connecting it. It also takes a bundle "Export ledger mirror" wrote (this version's or 0.1.2's), each entry checked against the mirror's own manifest; one that fails refuses the whole bundle. What is adopted must be one whole ledger, holding exactly one root entry (its "Create the ledger" entry): entries with none (some of a ledger's files without its first entry, say) or with two are refused whole and nothing is written; the refusal says the entries must hold exactly one root, and its next step reads "Choose the ledger's whole folder or a ledger mirror, not some of its entries.". When this home already has its own ledger, another ledger is refused whole and nothing is written ("This home already has its own ledger, and the one to adopt is another; nothing was adopted."); the same ledger adopted again is taken, what is already there skipped. For an imported identity, the ledger is then checked against this key's records on chain, and writing opens if it matches (see "Fetching the ledger"). The first-run wizard's "Create the ledger" step offers the same adoption (see section 3).
- "Import grants folder" (User): choose a folder and press "Import folder"; every file in the folder (grant files and grant entry files, whatever they are called) is verified and imported one by one; a file that is not a grant is refused by name and the rest are taken.
  - Side files the system leaves do not count. A folder with no file at all says "This folder has no files in it."
- "Check every record on chain again", then "Check everything again": clears this machine's record of checked facts (see "How the chain is read" in 4.1); the next sync asks about every anchor again.

"Details" gives the data folder's path, the local data encryption, the path of the last whole-machine backup and the backup encryption, where the folder setting is kept, the instance lock, and any missing subfolders.

Only one window can write to a data folder. A window opened later can only read, and shows the read-only bar at the top.

#### 11.7 Command line and AI control

Settings > Command line and AI control: the `zikaron` command in the terminal and connecting AI apps, how the command line may work with the data this machine keeps sealed, and what happens when it puts records on chain. The two "Automation level" switches and "Putting on chain from the command line" are kept on this machine (in the encrypted machine settings), shared by every identity and every data folder; "Enable the zikaron command" changes this user's command path on this machine.

**"Terminal"**

- **"Enable the zikaron command"**: when on ("In the terminal"), typing `zikaron` in any terminal runs the command line installed with the app: on macOS a link in `/usr/local/bin` (the folder is created if missing; the system asks for an administrator password in its own dialog), on Linux a link in `~/.local/bin`, on Windows the command line's folder added to this user's `PATH` (terminals opened afterwards see it). Turning it off removes only what the switch made. When the install already put the command line's folder on the command path (the `.deb` package on Linux; on Windows, the system-wide `Path`), it shows "Provided by the installed package"; anything else already in that place is never overwritten and shows "Taken by another install: …" (after a `.pkg` install `/usr/local/bin/zikaron` is the installed command line, so this is what it shows and nothing needs turning on); an app opened straight from its disk image or AppImage cannot be put in the terminal and shows "Not supported for this install", with the reason under "Details".
- **"Connect an AI app"**: "Copy config" copies the configuration that lets an AI app start this machine's command line, and says "Connection config copied"; pasted into the AI app's MCP server settings, it lets the AI app read and write this machine's data through the command line (see "Connecting an AI app (`mcp`)" in section 12). The configuration names the command line program by its full path, so "Enable the zikaron command" need not be on. For an app opened straight from its disk image or AppImage the key is grey and says "Not supported for this install" on hover. What an AI app may do follows "Automation level" and "Putting on chain from the command line" below; there is no switch of its own.

**"Automation level"**: two switches decide how the command line may write the data this machine keeps sealed and send its pending queue. The higher outranks the lower, so there is one way at any time.

- "Through the desktop app" (on by default): while the desktop is unlocked with this data folder open, the command line hands writes and putting on chain to it with `--home` (see "Through the desktop" in section 12); no passcode. Switching it says "The command line may work through the desktop app" or "The command line no longer works through the desktop app".
- "With a credential" (off by default): switching it on opens a card of the same name: enter the passcode and a validity (days, hours and minutes, together a minute to a year; or "Does not expire" switched on) and press "Turn on". The command line then writes the sealed data and sends a data folder's pending queue itself, without `--home`, with a credential, whether the desktop is open or not (see "With a credential" in section 12). Turning it on says "With a credential is on, …" (followed by "Expires: …" or "Does not expire"); turning it off says "With a credential is off". The credential is kept in the machine folder, readable and writable by you only; turning the switch off removes it, its expiry removes it with a line saying so, and a new master key (another primary identity, a whole-machine restore) makes it worthless and turns the switch off, saying "The master key was replaced; “With a credential” is off."; changing the passcode leaves it working. While it is on, the line under the switch says when it expires.
- While "With a credential" is on, "Through the desktop app" is greyed out and cannot be switched (the line under it reads "Cannot be switched while “With a credential” is on."), and the door is closed.
- While "With a credential" is on, the desktop takes this data folder's writer lock only for each write to the ledger or the anchor queue and lets it go after; if the command line wrote meanwhile, the ledger, the queue and the settings are read again before the desktop writes. The desktop, unlocked meanwhile, looks at the ledger on disk once a second even when nobody touches it (it wakes once a second for this, asked where the self-audit and the other timers are asked), so entries the command line wrote show at once in the ledger, the pending count and the local data page; with "With a credential" off, or the desktop locked, it does not wake to look. A desktop write that meets the command line writing is not done and says "The command line is writing." and "Try again shortly, or turn off “With a credential”.", and its "Go to settings" comes to this page.

**"Anchoring"**

- **"Putting on chain from the command line"**: what happens when the command line puts records on chain; both ways, through the desktop app and with a credential, follow it. "Send automatically" (the default) sends everything the queue can send at once, as "Put all on chain" does; "Queue only" sends nothing and leaves the entries in the pending queue for you to send from the desktop, and the command line answers `E_QUEUED` (exit 3); on the way through the desktop, the desktop also shows the toast "N records from the command line are queued". Changing it says "Putting on chain from the command line: …".

### About

- The first line reads `ZIKARON Desk 0.1.4`, with the build type below it.
- **"Setup check"**: one row per item. A done item has a check mark, with "Done" beside "Create the ledger" and "Whole-machine backup", and "Ready" beside "Gas", "Node setup" and "Ready". An item not done has the key that does it on its right, for example "Export backup" or "Edit nodes"; the others open their step of the wizard.
  - Recorder: "Signing key", "Passcode", "Gas", "Create the ledger", "Whole-machine backup".
  - User: "Signing key", "Passcode", "Node setup", "Ready", "Whole-machine backup".
- **"Run wizard again"** on the "Getting started" row: see section 3. It is unavailable when the current identity is a local-key identity and the current role is its empty one (so are the setup check's keys); otherwise it opens at the first step not yet done (the passcode step when every step is done).
- **"Run self-check"** on the "Self-check" row: runs a self-check in the background; the last self-check's reading is written under the row.
- **"Third-party licences"** (folded, at the bottom of the page, the number of components on its right): every third-party component with its version and licence, then each licence text; made at build time from the components this platform actually uses and carried in the app, scrollable.

### Diagnostics

The "Details" fold below the "Self-check" row: "Cores and signing domains", "Build type", "Fonts", "Log output", "Running tasks". The fonts row says where each face comes from: the Latin and monospace fonts are built into the app; Chinese comes from the system's PingFang on macOS, from the system's Microsoft YaHei UI on Windows (its bold for strong text; Noto Sans SC built into the app when it is not there), and from Noto Sans SC built into the app on Linux.

---

## 12 · Command line (`zikaron`)

The command line reads and writes the ledger folder you give it; with `--home`, the running desktop app does the work in its own data folder instead (see "Through the desktop" below).

A ledger in ZIKARON Desk's own data folder is encrypted, and the command line has no passcode; while "With a credential" is on, the command line reads and writes the ledgers of the desktop's data folders, and sends their pending queues, with that credential (see "With a credential", "Putting on chain with a credential" and "Reading with a credential" below). Sealed entries taken out of their data folder (a `ledger` folder copied on its own) cannot be read: the first line of standard error is `E_UNREADABLE` followed by where the `--ledger` argument stands and its length (for example `#3 (40 bytes)`), the next line reads "Locked: this is local data ZIKARON Desk keeps sealed; the command line reads it only as the ledger of a home the desktop keeps, with the credential" (in Chinese on a Chinese system), and the exit code is 2. The `--ledger` of `audit` (and through it `check-grant`, `chain-check` and `depth`) and of `show --entry` can also point straight at a mirror folder (the level holding `mirror.json`) or a record kit (a `.zkit` file or a folder). A record kit is checked by the kit law first and refused with `E_KIT` if it fails. The verbs that write entries, and `init`, take ledger folders and the desktop's data folders only.

**With a credential.** While "With a credential" is on in Settings > Command line and AI control, `history`, `grant`, `revoke`, `adopt`, `succeed`, `annotate` and `retract` with `--ledger` pointing at the `ledger` folder of one of the desktop's data folders write the sealed ledger directly with that credential: the signing key is the one the desktop's key store keeps for that data folder's identity and role (for a data folder of no role, the current identity and role's), so `--key` and `--key-file` are not taken (giving one is misuse); the write gate applies as ever (`E_WOULD_BREAK`), and the entry is queued for anchoring the way the desktop queues its own; a file written in with `history --file` is recorded in this machine's index of signed files as the desktop records the files it signs, so a record kit exported later carries the original. `anchor` with `--ledger` pointing there sends that data folder's pending queue with the same credential (see "Putting on chain with a credential" below). `init` is not taken there (such a data folder has its root entry already). For each write the command line takes the data folder's writer lock and lets it go after. A refusal writes nothing and exits 1: no credential (the switch is off) `E_CREDENTIAL_MISSING`; an expired one `E_CREDENTIAL_EXPIRED` (removed at the same time); one that does not read, does not open or is not this machine's key store's `E_CREDENTIAL_SHAPE`; the desktop writing the data folder at that moment `E_HOME_HELD` (try again shortly); the data folder's writer mark naming another machine (a data folder on a synced disk) `E_HOME_OTHER_MACHINE`; the data folder read-only or its ledger broken `E_READ_ONLY`, a ledger handed off `E_HANDED_OFF` (see the three write gates under "One sender" below). See section 12 of `CLI-SCHEMA.md`.

**Putting on chain with a credential (`anchor --ledger`).** `anchor --ledger <the ledger folder of one of the desktop's data folders>` sends that data folder's pending queue with the credential, the desktop open or not: the chain, the registry contract and the nodes come from that data folder's settings, the proxy from this machine's, and the signing key is the one the desktop's key store keeps for its identity and role. `--key`, `--key-file`, `--endpoint`, `--form`, `--registry`, `--hash`, `--calldata` and `--proxy` are not taken (giving one is misuse, exit 2); `--wait-secs` is (90 seconds without it); a `--ledger` that is not a desktop data folder's ledger is misuse too. It follows "Putting on chain from the command line" in Settings > Command line and AI control: "Send automatically" sends everything the queue can send (in one transaction), waits for the receipt and settles it (the entries leave the queue with their block); "Queue only" sends nothing and answers `E_QUEUED` (exit 3). With nothing to send but a batch in flight (a sender stopped half way), it asks about that batch by its transaction hash; when no node's pool holds that batch any more, it is judged by its nonce, by the rule the direct `anchor` reruns by: its nonce used by another transaction, it lapses; still unused, "Send automatically" signs the batch again as recorded at that same nonce and sends it (past the same checks as a new batch, answered in the same shape), so at most one transaction at that nonce can be included, while "Queue only" sends nothing and answers `E_TX_NOT_YET`. With neither, it answers `E_SEND_REFUSED` with `token` `QUEUE_EMPTY`. The answers have the shape of the same `anchor` through the desktop: included, exit 0 with `blockNumber` and `tx`; reverted `E_TX_STATUS`; not in a block when the wait runs out `E_TX_NOT_YET` (exit 4; the batch stays in flight, and the desktop or the next `anchor --ledger` asks about it again); no node answering the receipt question `E_UNREACHABLE` with `tx`; lapsed `E_TX_VOID` (exit 1; its entries go back to the queue); already on chain `E_ON_CHAIN` (nothing sent this time; the entries every node reads on chain then leave the queue, recorded as on chain, and the next run sends the rest; one node saying so moves none); any other refusal before anything went out `E_SEND_REFUSED` (`token` the desktop's code, for example `INSUFFICIENT_FUNDS` for a short balance; `detail` its evidence: nodes, numbers in wei; exit 4 for the network family, 1 otherwise); opening the data folder is refused for the reasons of the paragraph above. A resend at a higher fee is pressed on the desktop only; the command line does not offer it.

**One sender.** The desktop's pending queue, `anchor --home` through the desktop and `anchor --ledger` with the credential all send through one sender, judged alike:

- Before signing, the chain is asked whether it already holds an anchor of these hashes from this key (read as a scan reads it); if one is there, nothing is sent: the desktop says "Some records in this batch are already on chain; nothing was sent this time.", code `ON_CHAIN`; the command line answers `E_ON_CHAIN` with `names`, `tx` and `blockNumber`. When no node can say, nothing is sent either.
- A batch is recorded in the queue as submitted (with its transaction hash and nonce) once it is signed and before it is broadcast; a sender stopped half way and started again asks about it by its hash and never signs it again.
- A batch in flight that no node's pool holds any more is judged by the sending account's nonce on chain, by the rule the direct `anchor` reruns by: its nonce used by another transaction, it lapses and its entries go back to the queue; still unused, the desktop offers "Send again at the price now" on the ledger's status line for you to press, and `anchor --ledger` at "Send automatically" signs it again as recorded at that nonce and sends it; at most one transaction at a nonce is ever included.
- Entries already on chain leave the queue only by the ledger check's own judgement (what every node reads alike): the desktop's self-audit and a send with the credential both go by it; one node saying so moves none.
- The writer lock: readying the batch and signing and sending it hold this data folder's writer lock; the receipt wait holds none, so the other side (the desktop or the command line) may write, and may send the rest of the queue, meanwhile: it finds that batch in flight, leaves it alone and sends only the rest; settling takes the lock again, reads the queue anew and changes only the entries still awaiting that batch. When the command line cannot take the lock back within sixty seconds it answers `E_HOME_HELD`, and the batch stays in flight for the desktop or the next `anchor --ledger` to settle.
- Three write gates, asked alike before a send and before a write with the credential, and the first two before a handover with the credential (`hand-over`): this data folder under its read-only mark (an imported identity whose ledger is not fetched yet, `LEDGER_NOT_FETCHED`; the chain holding anchors the ledger lacks, `NEWER_ELSEWHERE`), the ledger's own entries breaking its chain (`BROKEN`), the ledger handed off (`HANDED_OFF`). A send answers `E_SEND_REFUSED` with that code in `token`; a write or a handover with the credential answers `E_READ_ONLY` with that code in `token` (a write to a ledger handed off still answers `E_HANDED_OFF`; a ledger handed off is handed over again as before, the same handoff file, see "Handing a ledger over" below). On a refusal not one byte is written and nothing is sent.

**Reading with a credential.** The same credential opens for reading: `audit`, `check-grant`, `chain-check`, `depth`, `show --entry` and `kit-export` with `--ledger` pointing at the `ledger` folder of one of the desktop's data folders read that sealed ledger, opened the way a write opens it and refused for the same reasons (`E_CREDENTIAL_MISSING`, `E_CREDENTIAL_EXPIRED`, `E_CREDENTIAL_SHAPE`, `E_HOME_OTHER_MACHINE`); a read takes no writer lock, so the desktop writing at the same time is no refusal. `check-grant` read this way has the issuer's whole ledger, so whether the grant was revoked is answered. `kit-export` from such a data folder takes the chosen records' originals from the same place the desktop does (the files this machine signed, while still at their place with the bytes recorded, named as the desktop names them), carries the terms documents kept when the chosen grants were signed, as the desktop does, and ends the note with the desktop's "where it is anchored" line; the answer lists the originals taken in `originals` and those not taken (gone, no longer the bytes signed, or with a name that cannot be made legal in a package) in `leftOut`; with `--originals none` no original is taken and both are empty. A plain ledger folder has no record of signed files: `--originals` there is refused as misuse, and its originals are each given with `--file`. With `--ledger` in place of `--basis`, `scan` takes the basis (chain, registry contract, start block, senders) from that data folder's settings and ledger, scanning up to the head its nodes say; with no `--endpoint` (and no `--fixture`) the nodes are those the data folder's settings list for that chain, and the proxy, without `--proxy`, this machine's, read as "Putting on chain with a credential" reads them; no node for that chain answers `E_UNREACHABLE`, and node addresses are never echoed.

**The folder after adoption.** A plain ledger the command line made itself, once the desktop has taken it in ("Import key", then "Adopt in place"), is written from then on through the desktop's sealed copy: writing the original folder again (`hand-over` and `take-over` included) is refused with `E_ADOPTED` (exit 1), nothing written, and a line on standard error says ZIKARON Desk has adopted the ledger and its folder is no longer written; to go on writing that ledger from the command line, turn on "With a credential" in Settings > Command line and AI control and give the `ledger` of the desktop's data folder instead. A folder moved or renamed is still recognised, and the refusal holds with the switch on or off; the original folder itself is never touched. The desktop keeps, in the machine folder, a random salt and a list of adopted ledgers, one line each, the SHA-256 of the salt followed by the ledger's root entry id: the list alone names no ledger, and only someone holding that ledger can find its line.

**Audited before writing.** Before any entry-writing verb (`init` and `retract` included) writes, "this ledger plus this entry" goes to the core for an offline audit; if it would add a chain finding (the old key writing after a key change, a skipped sequence number, a second entry at one place, a link to the wrong entry), it is refused with `E_WOULD_BREAK`, `names` saying which, and not one byte is written. `--seq` and `--prev` given by hand pass the same gate.

**Node addresses** can be `http://` or `https://`, in any case; https verifies the certificate chain and host name, the same way the app connects. While `anchor` waits for a receipt, a round the node refuses is not an answer: it asks again after a pause until `--wait-secs` runs out, and says `E_UNREACHABLE` only when the node never answered in that time.

**`anchor` estimates before it sends.** After reading the fees it asks the node for a gas estimate of this very transaction (the app's own rule), and the transaction carries one and a half times the estimate, rounded up, at most 200,000. If the node refuses the estimate for a reason about this transaction (it would revert; the node's words are in `detail`), or the estimate is above 200,000, it is refused with `E_GAS_REFUSED` (exit 1) and nothing is sent; if the node refuses for its own reasons (a rate limit, an unsupported method, credentials needed, a wrong chain and the like) or does not answer, it says `E_UNREACHABLE` (exit 4) and nothing is sent either.

**`anchor` does not anchor again what the chain holds.** With `--form registry` and `--hash`, before signing it asks the node whether the registry already holds an anchor of those hashes from this key (reading the registry's `Anchored` logs as a scan reads them, from the registry's deployment block to the head; an unknown registry from block 0). Any one already on chain, put there by the desktop or by another machine, is answered `E_ON_CHAIN` (exit 1), `names` listing those hashes and `tx`, `blockNumber` the earliest such anchor, and nothing is sent; a node that cannot say is `E_UNREACHABLE` (exit 4), and nothing is sent either. `--form bare` and `--calldata` have no log to ask by and are sent as before. Sending a queue, through the desktop and with the credential, asks the same first (see "One sender" above).

**A rerun of `anchor` asks first.** Sending directly (with `--key` or `--key-file`, without `--home` or `--ledger`), `anchor` keeps the nonce and hash of every transaction it signs for one anchoring (the same chain, key, recipient and data) in `cli-sent/` in the machine folder; it keeps no key and no node. When the same command runs again, it first asks the node where those transactions stand, by their hashes: one already included is answered as included and nothing is sent; one still in the node's pool is waited on; if none is held and their nonce is unused, it signs again at that same nonce, so at most one can ever be included; if their nonce was used by another transaction, it answers `E_TX_VOID` (exit 1) and sends nothing, and the next run starts afresh. Running the same command again after it succeeded therefore answers the earlier transaction and never pays twice.

**Through the desktop (`--home`).** With `--home <the desktop's data folder>`, eleven verbs (`init`, `history`, `grant`, `revoke`, `adopt`, `attest`, `annotate`, `retract`, `anchor`, `kit-export`, `kit-verify`) are handed to the running ZIKARON Desk, unlocked and holding that data folder as its writer: the entry is written into the desktop's ledger with the desktop's identity and shows in its window at once; the command line holds no key and needs no passcode on this path. With `--home` a verb never takes `--ledger`, `--key`, `--root`, `--seq` or `--prev`; `history` takes `--file` and `--note`, and `anchor` takes no other flag. `attest` always answers `E_ON_DESKTOP` (exit 1): attesting asks for the passcode, which is given on the desktop only. `succeed` with `--home` also always answers `E_ON_DESKTOP` (exit 1), and the desktop is not asked: handing the ledger to another key is final, so it is done on the desktop by hand only. `kit-export` takes `--entry`, `--note`, `--out`, `--as` (one file without it), `--to` and `--to-key` (encrypted to one recipient, see "Encrypted record packages" below; an address's key is looked for by the desktop on its networks) and `--originals`: the chosen records' originals go in, taken as the desktop's export page takes them, unless `--originals none`, and the answer carries `originals` and `leftOut`; other attachments and proof bundles are chosen on the desktop. `kit-verify` takes `--kit` and `--originals-to`: the desktop verifies on its own networks, and opens an encrypted package with its identities. When the desktop is locked, has not opened that data folder or has quit, the answer is `E_DESKTOP` (exit 4); while "With a credential" is on the desktop keeps this door shut, and the answer is `E_DESKTOP` with `detail` `ALONE`: write without `--home`; when the desktop refuses a verb other than `anchor`, it is `E_DESKTOP_REFUSED` with the desktop's own code in `token` (exit 4 for the family that is no answer: a network refusal, and a network not read for an encrypted export, `ADDRESS_CHAIN_UNREAD`; 1 otherwise). `anchor` follows "Putting on chain from the command line" in Settings > Command line and AI control: "Send automatically" sends everything the queue can send, as the queue page's "Put all on chain" does, and answers in the shape of `anchor --ledger` (included, exit 0 with `blockNumber` and `tx`; `E_TX_STATUS`, `E_TX_NOT_YET`, `E_TX_VOID`, `E_UNREACHABLE` with `tx`); already on chain is `E_ON_CHAIN` (with `names`, `tx`, `blockNumber`); any other refusal before anything went out is `E_SEND_REFUSED` (`token` the desktop's code, `detail` its evidence; exit 4 for the network family, 1 otherwise); "Queue only" queues the records and answers `E_QUEUED` (exit 3). Which flags each verb takes with `--home` is in section 11 of `CLI-SCHEMA.md`.

**Connecting an AI app (`mcp`).** `zikaron mcp` lets an AI app with no terminal ask the command line through the Model Context Protocol (MCP): the AI app starts it as a child process and speaks one message per line on its standard input and output. It is not another door and holds no right of its own: each tool call is a verb of the command line, done by this same program started anew with the arguments a person would type, and its answer is that command's answer, its bytes and its exit code exactly. The write tools write this machine's home only: at each call, a credential on this machine means writing with it (`--ledger <home>/ledger`), none means handing the write to the desktop (`--home <home>`), and both ways closed is answered as the terminal answers; the master key is opened only in that one process. Putting on chain follows "Putting on chain from the command line": with "Send automatically" it spends this identity's gas.
- Tools: the writes `history`, `grant`, `revoke`, `adopt`, `annotate`, `retract`, `kit-export` (with `to` and `to-key`, encrypted to one recipient; `originals` set to `none` leaves the originals out), `anchor` and `kit-verify`, each taking the flags its verb takes with `--home`, and `home` (the data folder's full path); `kit-verify` takes `kit`, `fragment`, `unavailable`, `originals-to` and `home`, takes the originals out when given `originals-to`, and may be given no `home` (a plain package is verified and taken out with none; an encrypted one is given `home` and opened with this machine's identities, with the credential or by the desktop as the switches say; through the desktop `fragment` and `unavailable` are not taken and are refused by name). The reads `show`, `audit` (no `out`), `scan` (`home`; the home's chain settings and nodes), `check-grant`, `chain-check`, `depth`, `badge` (decoding only) and `contract`. `keygen`, every way that takes `--key`, `succeed`, `hand-over`, `take-over` and the fee bump are not given; no tool takes a private key.
- A path is taken only in full; a relative path, a parameter not the tool's or a value of the wrong kind runs nothing and is answered as an error naming it. Exits 0, 1, 3 and 4 are given as they are; only misuse (2) is marked an error.
- Calls are done one at a time, in the order they come; a cancelled read stops and is not answered, a cancelled write is done and answered (a send cut halfway cannot be undone). The desktop's "closed" is said as it is and not retried. Ledger text in an answer (notes, statements, other ledgers' content, nodes' replies) is data, given as it came.
- The configuration is copied with "Connect an AI app" in Settings > Command line and AI control; the protocol is section 13 of `CLI-SCHEMA.md`.

**A record kit is one file.** `kit-export` writes one file by default (`--as file`): the kit is laid out and checks itself first, then is written item by item into the `--out` file, with no size limit; `--as folder` writes a folder. The file has the grant file's format (its first line `zikaron-kit-file/1`); the desktop names it `.zkit`. Wherever a record kit is taken, a file and a folder are both taken.

**Verifying a record kit.** `kit-verify --kit <record kit>` (a file or a folder) answers record by record by the same judge as the desktop's "Verify record": whether each record's original in the kit is the one it records (`original`: `match`, `mismatch` for another file, `missing` when the kit lacks it), and whether it is anchored (`anchored`: `anchored` with the first anchoring's block time `firstAt`, `notAnchored`, `unread`). The chain is read from `--fragment` (a `scan` reading); without it every record is `unread`, never "not anchored". Every record matching and anchored is exit 0; an original that is not the one recorded is `E_RECORDS`, exit 1; otherwise (a record not anchored, the chain not read, an original missing) `E_RECORDS`, exit 3. A kit that fails the kit law is refused with `E_KIT`.

**Taking originals out.** `kit-verify --kit <record kit> --originals-to <folder>` then takes the originals that match their records into that folder, which must be there, by the desktop's "Take out originals" judge and way of writing (see "Record verifier"): the answer adds `taken` (the names they were taken out under) and `path`; not taken, it is `E_EXTRACT` with `token` saying why (`EXISTS` names already there, `LINK` a link on the way, `CLASH` two originals for one name, `NOTHING` none matches, `NOT_A_FOLDER` not a folder that is there, `PATH` a package path not in the kit form, `CHANGED` the package changed meanwhile, `LANDING` a write failed and was taken back), and not a byte is written in the folder. A plain package needs no home and no key.

**Encrypted record packages.** The desktop's encryption and the desktop's judge; the three places answer in the same words and codes:

- Export: `kit-export … --to <address>` (`0x` and 40 hex digits, either case, so a checksummed address as pasted is taken) or `--to-key <public key>` (`0x` and 66 or 130 hex digits). Through the desktop (`--home`) the desktop looks for the key on its networks; with the credential (`--ledger <home>/ledger`) the command line looks on the data folder's network and nodes and this machine's read-only networks, with its proxy setting (`--proxy` when given; set to "System", it follows the system's proxy settings as the desktop does, never the terminal's proxy variables); a read-only network with the data folder's own chain and registry is that network itself, not another; a node answering no code at the registry (not yet synced to the block it was deployed in) does not count, and the network is read on the others; directly on a plain ledger folder, `--to-key` is enough, and `--to` also takes `--endpoint <chain id>=<url>` (repeatable; each chain one of the known deployments in appendix A). The key is looked for by the desktop's judge: every network read through and no anchor of that address is `E_ADDRESSED` with `token` `ADDRESS_NOT_ANCHORED` (exit 1); some network not read is `token` `ADDRESS_CHAIN_UNREAD` (exit 4: no answer, never an answer of "none"). Nothing is written either way.
- Misuse (exit 2, nothing on stdout, nothing written): `--to` and `--to-key` together; either with `--as folder`; an address or key not in its form; `--to` on a plain folder without `--endpoint`, or with a chain not among the known deployments; `--endpoint` or `--proxy` without `--to`; `--endpoint` with a data folder's ledger.
- Opening: `kit-verify --kit <encrypted package>`; through the desktop (`--home`) the desktop's identities try it; with the credential (`--ledger <home>/ledger`) every identity in this machine's key store tries it (`--key` and `--key-file` are then not taken); directly, `--key-file` (or `--key`). Opened, it is verified as a plain package, and its originals taken out when `--originals-to` is given. No key being the recipient is `E_ADDRESSED` with `token` `ADDRESSED_NOT_RECIPIENT`; a package changed or cut short is `ADDRESSED_BROKEN`; no key at all is misuse. Through the desktop the desktop reads its own networks, while the credential and the direct ways read the chain from `--fragment`: the same package can read `unread` on those two ways and `anchored` through the desktop.
- Every other verb that reads record kits (`audit`, `show --entry`, …) refuses an encrypted package with `E_ADDRESSED`, `token` `ADDRESSED_ELSEWHERE`.

**Handing a ledger over.** A ledger can go back and forth between two places: say a plain ledger folder of the command line on a remote machine, and a data folder of the desktop here. The same two commands at both ends. `hand-over --ledger <this place> --out <file>` writes the whole ledger into one handoff file (`.zkhand`, the record kit's one-file format), and this place stops writing it: a write is refused with `E_HANDED_OFF`, and the desktop does not write it either. If that file is lost on the way, running `hand-over` again writes the same handoff. A desktop data folder is handed over (with the credential) only past its write gates, judged as for a send or a write: under its read-only mark (`LEDGER_NOT_FETCHED`, `NEWER_ELSEWHERE`) or with a ledger whose own entries break its chain (`BROKEN`), it answers `E_READ_ONLY` (that code in `token`, exit 1), no handoff file is written and the ledger is not recorded as handed off. `take-over --ledger <this place> --handoff <file>` takes a handoff, as a follow-on only: every entry this place has must be in it and its ledger must be one line; then the entries this place lacks are written in sequence, and this place writes again. A place that handed the ledger off takes only the handoff coming back. Anything else is refused with `E_HANDOFF`, its `token` saying why (`NOT_BACK` not the one coming back, `AHEAD` this place has an entry the handoff lacks, `NOT_ONE_LINE` the handoff's ledger is not one line, or a file that does not read), and not one byte is written. This machine's side reads and writes with the credential, the desktop locked or not; the desktop open meanwhile shows the new entries at once. Which side anchored what does not travel with the handoff: the desktop's next ledger check reads it from the chain and queues only what is not on chain, so what the other side anchored is not sent again; a plain ledger folder keeps no queue, so for it the command line sends only what `anchor` is given, and a desktop data folder's queue is sent by `anchor --ledger` (see "Putting on chain with a credential" above). Which ledgers a machine handed off or took back is kept in a blind list in its machine folder (under the adopted list's salt): the list alone names no ledger.

**Grant files.** `check-grant --grant` takes a grant entry file or a grant file exported by the app (`.zkgrant`): the command line opens it by the app's own reading (the bundle's shape, the kit law over the kit inside, the grant code, and every hop of the code carried in the bundle) and checks the grant the file is for; one that does not open is refused with `E_GRANT_FILE`.

**When an entry is refused.** When a member of an entry is missing or malformed, standard output is `E_ENTRY` with `token` `E_BODY_FIELD` (exit 1); standard error adds one line for people naming the member and the flag that gives it, for example `mode: missing or malformed; given by --mark and --toolchain (or --file)` (the whole line in Chinese on a Chinese system). That line is for people only and not part of the output contract.

**Recording a file with `history`.** `history --file <file>` follows the app's own convention for a recorded file: `content` is the SHA-256 of the file's bytes, and `mode` is `{"mark":"bytes-sha256/1","toolchain":<the SHA-256 of the text "bytes-sha256/1">}`. For the same file, the command line and the app write these two members alike. `--file` stands for `--content`, `--mark` and `--toolchain` and cannot be given with them; without `--file`, those three are given as before, and the law refuses what is left out.

**Other.** `audit --out <file>` also saves the audit input this run assembled (its six members), usable as a `chain-check --hop <grant>=<audit input>`. The in-kit path of `kit-export --file <in-kit path>=<file>` is relative to the kit's `files/` folder, and that of `--proof` to `proofs/`. On misuse, the first line of standard error is `<reason> <subject>`: a flag is named as `--name`, while a value or path is named only by where it stands among the arguments and its length (`#3 (12 bytes)`), so a private key or a node address carrying an access key is never echoed. The second line explains it for people, in Chinese when the system locale (the first of `LC_ALL`, `LC_MESSAGES`, `LANG` and `LANGUAGE` that is set) is Chinese and in English otherwise. `--key-file <file>` gives the private key from a file instead of `--key` (the file must be readable by its owner only). `anchor` and `scan` take `--proxy`: `system` (the default; as programs in a terminal do, the terminal's `https_proxy`, `http_proxy`, `all_proxy` and `no_proxy`, in either case, are followed when any proxy among them is set, on every system, and only when none is set are the system's proxy settings read), `none`, or a proxy address written as for "Custom proxy" in the settings (`http`, `https`, `socks5`, `socks4`, `user:password@` allowed, with an `@` or `:` in them written `%40`, `%3A`; a password on the command line is visible to other processes of this machine while the command runs).

| Verb | What it does |
|---|---|
| `keygen` | Generate a key and print its address and private key |
| `init` | Write the ledger's creation entry into an empty ledger |
| `history` | Append an anchored record (`--file` records a file by the app's convention) |
| `grant` | Sign a grant and write it to the ledger |
| `revoke` | Sign a revocation and write it to the ledger (ruling file digest optional) |
| `adopt` | Write an adoption entry |
| `attest` | Produce a co-signature with an outside key |
| `succeed` | Write a key change or handover entry |
| `annotate` | Write a note |
| `retract` | Write a deletion entry (the target must be a record in this ledger that is not already deleted) |
| `anchor` | Anchor a hash on chain (through the registry contract, or as a bare self-transfer); with `--ledger` naming a desktop data folder's ledger, send its pending queue with the credential |
| `scan` | Scan for anchors using the chain settings (`--endpoint` takes http and https; with `--ledger` naming a desktop data folder's ledger, the basis comes from its settings, and its nodes too when no `--endpoint` is given) |
| `audit` | Audit a ledger against scan results and print a report and verdict (`--ledger` takes a ledger folder, a desktop data folder's ledger, a ledger mirror or a record kit) |
| `check-grant` | Run the six checks on a grant (`--grant` takes a grant entry file or a grant file) |
| `chain-check` | Check a sublicense chain hop by hop (`--hop <grant file>[=<audit input file>]`, repeatable) |
| `depth` | Depth readings for a record |
| `fpm-sign` | Sign a fingerprint manifest |
| `ack-sign` | Sign an acknowledgement |
| `badge` | Encode or decode a credential (exactly one of `--encode` and `--decode`) |
| `kit-export` | Export a record kit (written only if it passes its self-check; one file by default, `--as folder` for a folder; `--to`, `--to-key` encrypt it to one recipient; from a desktop data folder the chosen records' originals go in, `--originals none` leaves them out) |
| `kit-verify` | Verify a record kit record by record: is each original the one recorded, is it anchored (the desktop's "Verify record" judge); an encrypted one opened first; `--originals-to` takes the originals out |
| `hand-over` | Hand the whole ledger to another place (writes a handoff file; this place stops writing) |
| `take-over` | Take a handoff in (a follow-on only); this place writes again |
| `show` | Show an entry's author, ID, type, previous entry, sequence and content |
| `contract` | Print the command line's own contract (verbs, each verb's flags, exit codes, refusal reasons), made from the same tables in the code |
| `mcp` | Let AI apps call the command line through the Model Context Protocol (MCP) (see "Connecting an AI app" above) |

**Exit codes**

| Code | Meaning |
|---|---|
| 0 | Affirmative |
| 1 | Negative (the entry was refused, or the verdict is FAIL, BROKEN_CHAIN or NO_LABEL) |
| 2 | Misuse (nothing on standard output) |
| 3 | Partial (PARTIAL, GAPS, UNAVAILABLE; an `anchor` through the desktop or with the credential that was only queued: `E_QUEUED`) |
| 4 | No answer (node unreachable, readings disagree, the desktop did not answer, and so on) |

Standard output is one canonical JSON value with no trailing newline (`mcp` aside: one message per line). `--now` injects the moment to judge at; without it, only chain time is used. For output shapes and every flag, see `CLI-SCHEMA.md` in the repository.

---

## Appendix A · Built-in networks

Each network comes with two public nodes from different providers, so their answers can be cross-checked.

| | Ethereum mainnet (default) | Sepolia testnet |
|---|---|---|
| Chain ID | 1 | 11155111 |
| Registry contract | `0x36Ea8A857a5FE813429d4D9947000C644A88809A` | `0xC29410B882c4C3b77e33659d2f06ac563e7B08a3` |
| Start block | 26087229 | 11715660 |
| Nodes | `https://mainnet.gateway.tenderly.co`<br>`https://rpc.mevblocker.io` | `https://sepolia.gateway.tenderly.co`<br>`https://rpc.sepolia.ethpandaops.io` |

| | Arbitrum One | OP Mainnet |
|---|---|---|
| Chain ID | 42161 | 10 |
| Registry contract | `0x36Ea8A857a5FE813429d4D9947000C644A88809A` | same |
| Start block | 511445184 | 157735914 |
| Nodes | `https://arb1.arbitrum.io/rpc`<br>`https://arbitrum.gateway.tenderly.co` | `https://mainnet.optimism.io`<br>`https://optimism.gateway.tenderly.co` |

On the two layer-two networks the registry contract is the same build as mainnet's, at the same address.

Each preset node was measured to hold the registry's complete logs since its start block, both nodes of a network matching entry for entry; a query range too wide for a node is refused by it in words, and the app asks again and, after three refusals, splits the range at the limit it names.

To use the testnet: choose "Sepolia testnet" under "Network" when making the identity; or choose "Custom" in wizard step 3, then in Settings > Network press "Edit nodes", choose "Custom", pick "Sepolia testnet" under "Preset", and press "Save".

## Appendix B · Files on this machine and encryption

**The machine folder** holds the key store, the identity registry, the machine settings, the record kit index and so on. The machine settings are in two files: `machine.json` holds only what the passcode gate needs, the language and the appearance; the rest (auto-lock, proxy, network, the command line's two switches, the data folders used, the whole-machine backup record) is in the encrypted `machine-<…>.sealed`, read once unlocked.

- Its location is given by the pointer file `~/.zikaron-desk` (one line holding an absolute path), which the app writes each time it starts. Without a pointer it is `~/.zikaron-desk.d/`. On Windows both sit in `%LOCALAPPDATA%\ZIKARON\`: `%LOCALAPPDATA%\ZIKARON\.zikaron-desk` and `%LOCALAPPDATA%\ZIKARON\.zikaron-desk.d\`.

**The data folders**, one for each role of each identity, hold the ledger, settings, pending queue, received grants and so on, in the subfolders `ledger`, `kits`, `grants-held` and `settings`. By default they sit in the machine folder: one folder per identity, its name computed with this machine's key, holding `author` (Recorder) and `grantee` (User).

**Local encryption.** Ledger entries, settings, the pending queue, the identity registry, the record index, hand-filled record kit links, the record of checked facts (`checked/facts.json` in the machine folder), the index of what the last whole-machine backup holds, and received grants (with their last verification results and kept grant files) and terms files are all encrypted with a local data key derived from the master key (XChaCha20-Poly1305). While the app is locked none of it can be read, and someone who copies this machine's files cannot read it without the passcode. File names, and the part of each file's header that says which file it is, are computed with this machine's key, so someone holding the disk cannot tell from entry or grant numbers which entries or grants this machine keeps.

Files are written and read only in the current encrypted format. A file in any other form (a plain file, or one 0.1.2 or an earlier version wrote in an older form) is not read, and nothing converts it (see "Upgrading from an earlier version" in section 2). A file that cannot be opened (written by a newer version, or damaged, or in an older form) is never overwritten: it is named with the reason and left as it is. Files written by this version cannot be read by 0.1.2 or earlier versions.

**Files that are not encrypted this way**

- Files read before unlocking: the key store itself (encrypted separately), `machine.json` (the language and the appearance only), and the data folder pointer.
- The command line's credential `cli-credential-<…>.json` (only while "With a credential" is on): readable and writable by you only; it holds a random key and the master key sealed under it, so whoever has it can write the sealed data as the command line does; the credential is also what sends a data folder's pending queue. That is why turning the switch off removes it.
- The adoption list `adopted-salt` and `adopted-list`: a random salt and, per adopted ledger, one line holding the SHA-256 of the salt followed by its root entry id; no ledger name, path or entry id. The handoff list `handed-pens` uses the same salt, one line per ledger handed over or taken, and holds none of these either.
- Exports meant for others: record kits, the record kit index, kit verification results (`kits/verified/` in the machine folder), grant files, credentials, ledger mirrors, key files and whole-machine backups. A whole-machine backup is encrypted with its backup password.
- The read-only networks (`read-networks.json` in the machine folder), which hold no account fact.
- `cli-sent/` in the machine folder: the command line's record of the anchor transactions it signed (nonce and hash only, no key and no node).
- Files other people send you.

**Whole-machine backups** derive their key from the backup password with scrypt (standard parameters N=262144, r=8, p=1) and are encrypted with XChaCha20-Poly1305.

## Appendix C · How deletion entries are read

"Delete record" writes an entry of type `retraction`, signed and put on chain like any other:

| Field | Value |
|---|---|
| `entryType` | `retraction` |
| Body `subject` | Required: the entry ID of a record in this ledger, `0x` followed by 64 lowercase hexadecimal digits |
| Body `note_md` | Optional: a note |

This is ZIKARON Desk's reading convention and is not part of the `zikaron/1` protocol text. The command line's `zikaron retract` writes the same entry by the same rules. Other tools list such an entry as an unknown type (`UNKNOWN_TYPE`), and the ledger's verdict does not change because of it.

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
