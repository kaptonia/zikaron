//! Every window click and every test-hook command goes through here.
//!
//! Actions are one closed enum applied by one function, so tests (through the test hooks) and people (through
//! the window) run the same code.
//!
//! `verb()` names the command-line verb a legal action is equivalent to (`CLI-SCHEMA.md` §6). Tests check that
//! every action with `is_legal()` true has a `verb()`, and that every named verb is in the table.

use crate::feature::Feature;
use crate::key::Address;
use crate::keystore::Params;
use crate::shell::{Page, Shell};
use crate::task::{Done, Kind, Reaped, Spawned};
use crate::trace;

/// The three forms of identity import. `Debug` prints no secrets: words, private keys and passwords never reach
/// a log line.
#[derive(Clone, PartialEq, Eq)]
pub enum ImportForm {
    Words(crate::secret::Secret),
    /// A bare private key, plus the key file it is written as in the same pass (`keyfile`). Required when the
    /// import creates the primary identity: without a key file the primary cannot recover a forgotten passcode.
    PrivateKey { key: crate::secret::Secret, keyfile: Option<KeyFileOut> },
    Keystore { path: String, password: crate::secret::Secret },
}

/// A key file to write: its password twice and the output folder (the "export key file" fields).
#[derive(Clone, PartialEq, Eq)]
pub struct KeyFileOut {
    pub password: crate::secret::Secret,
    pub again: crate::secret::Secret,
    pub dir: String,
}

impl std::fmt::Debug for ImportForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportForm::Words(_) => f.write_str("Words(…)"),
            ImportForm::PrivateKey { keyfile, .. } => write!(f, "PrivateKey {{ key: …, keyfile: {} }}", if keyfile.is_some() { "…" } else { "none" }),
            ImportForm::Keystore { path, .. } => write!(f, "Keystore {{ path: {path:?}, password: … }}"),
        }
    }
}

/// Declares `Action` as written and lists each variant's name in declaration order in [`Action::NAMES`], so the
/// variant count is known at compile time and the name list is never copied by hand.
macro_rules! closed_actions {
    ($(#[$em:meta])* pub enum $closed:ident { $( $(#[$m:meta])* $name:ident $( ( $($tup:tt)* ) )? $( { $($body:tt)* } )? ),* $(,)? }) => {
        $(#[$em])*
        pub enum $closed { $( $(#[$m])* $name $( ( $($tup)* ) )? $( { $($body)* } )? ),* }

        impl $closed {
            /// Each member's name, in declaration order.
            pub const NAMES: &'static [&'static str] = &[$(stringify!($name)),*];

            /// This member's name (the same spelling as in [`Self::NAMES`]).
            pub fn name(&self) -> &'static str {
                match self {
                    $( $closed::$name { .. } => stringify!($name), )*
                }
            }
        }
    };
}

closed_actions! {
/// What the shell can do. Closed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Action {
    // ── Shell ──
    /// Go to a page.
    Show(Page),
    /// Run a self-check (background).
    SelfCheck,
    /// Measure the home once (background): walk the disk and read the ledger. The UI thread never touches the
    /// disk.
    Measure,
    /// Shut down: reap every background task.
    Quit,
    // ── Identity and keys ──
    /// Create the anchor key and put it in the local key vault. A legal action.
    MakeAnchorKey,
    /// Switch seat. An existing-key identity switches view only (key and home unchanged); a recovery-word
    /// identity switches derived key and that seat's home.
    SwitchRole,
    // Identities.
    /// Read the identities on this machine once (the registry; without one, the existing slot).
    ReadIdentities,
    /// Generate twelve new words (in memory only, until the person confirms the copy).
    NewIdentity,
    /// Confirm the copy: three randomly chosen words typed back; a match creates the identity. `label` is an
    /// optional note (for recognition only; no decision reads it). `network` is the network the identity
    /// chooses (a row name from the known deployments table, or `deploy::CUSTOM`): both its seats' homes take it.
    ConfirmIdentity { answers: Vec<(usize, crate::secret::Secret)>, label: String, network: String },
    /// Abandon: wipe the words in memory.
    DropFresh,
    /// Import an identity (recovery words, private key, or keystore file plus password). `seat` is the seat
    /// an existing key takes (only that one; the other stays empty); recovery words take both seats, and
    /// there `seat` only reads the current identity table. `label` is an optional note; `network` as with
    /// [`Action::ConfirmIdentity`] (an identity imported again keeps the network it had).
    ImportIdentity { form: ImportForm, seat: crate::roles::Role, label: String, network: String },
    /// Switch to another identity (landing on its author seat).
    SwitchIdentity { id: String },
    /// Delete an identity: only the key slots and the registry row; neither seat's home is deleted. Asks for
    /// the local passcode first.
    DeleteIdentity { id: String, pin: crate::secret::Secret },
    /// Name an identity. No decision reads the name.
    NameIdentity { id: String, label: String },
    /// Set the passcode for the first time (a wizard step; refused when the vault already has one).
    SetPin { pin: crate::secret::Secret, again: crate::secret::Secret },
    /// Unlock: a correct passcode loads the master key into memory; a wrong one increments the failure count
    /// on disk.
    Unlock { pin: crate::secret::Secret },
    /// Reseal at the floor: when the vault file records key-derivation parameters below this binary's floor,
    /// unlocking is refused by name and the person comes here: open with the recorded parameters, then write
    /// the floor back and reseal the passcode seal at the floor.
    Reseal { pin: crate::secret::Secret },
    /// Delete an empty vault entirely: only for a vault with a passcode set and no identity built yet. Any
    /// slot or recovery seal present is refused by name (`keybox::reset_empty`) and the file is unchanged.
    ResetEmptyKeybox,
    /// Lock: the master key is wiped at once and key-using actions are refused from then on.
    Lock,
    /// Change the passcode: the old one first, then the same master key is resealed under the new one.
    ChangePin { old: crate::secret::Secret, pin: crate::secret::Secret, again: crate::secret::Secret },
    /// Recover with words: reopen the master key, set a new passcode, reset the failure count.
    RecoverWords { words: crate::secret::Secret, pin: crate::secret::Secret, again: crate::secret::Secret },
    /// Recover with a key file (existing-key identities only): the keystore V3 file and its password.
    RecoverKeystore { path: String, password: crate::secret::Secret, pin: crate::secret::Secret, again: crate::secret::Secret },
    /// Show the current identity's twelve words. Asks for the passcode.
    RevealWords { pin: crate::secret::Secret },
    /// Hide the twelve words (the copy in memory is cleared).
    HideWords,
    /// Back up this seat's key as a keystore V3 file (password set by the person, `UTC--` naming). Asks for
    /// the local passcode first.
    BackupKey { pin: crate::secret::Secret, password: crate::secret::Secret, again: crate::secret::Secret, dir: String },
    /// Turn idle locking on or off and choose how long idle before it locks (machine-wide, `machine.json`).
    /// Only the five closed values; changing takes effect at once, nothing to save.
    SetAutoLock { on: bool, secs: u64 },
    /// Make this identity the primary one (the only one that can recover the passcode). The passcode opens the
    /// vault first; then a new master key is made and every key and local file is resealed (background,
    /// `rekey::set_primary`).
    SetPrimary { id: String, pin: crate::secret::Secret },
    // ── Whole-machine backup ──
    /// Write a whole-machine backup into a folder, sealed with a backup password (at least eight characters,
    /// typed twice). The passcode opens the vault first.
    ExportBackup { pin: crate::secret::Secret, password: crate::secret::Secret, again: crate::secret::Secret, dir: String },
    /// Open a backup with its password and say what it holds (nothing here changes; works locked).
    PeekBackup { path: String, password: crate::secret::Secret },
    /// Replace this machine's identities and local data with a backup's (three entries, `RestoreHow`).
    RestoreBackup { path: String, password: crate::secret::Secret, how: RestoreHow },
    // ── Background work that reads local data (queue, self-audit, sentinel) ──
    /// Resume waiting for a submitted anchor's receipt (never resending).
    Resume,
    /// Right after unlocking, catch up once on what was missed while locked (the self-audit, the vault review
    /// and its sentinel, the receipt wait); the notices they raise fire once.
    CatchUp,
    /// The wizard's network step: the network of the identity the wizard created (a row name from the known
    /// deployments table, or `deploy::CUSTOM`). Recorded as the current identity's network and as this
    /// machine's last choice (preselected next time); the current home takes the row, or with "custom" is left
    /// without a network, unless its network was configured by hand.
    ChooseNetwork { name: String },
    // ── Archive and single writer ──
    /// Open a home (creating it if missing) and take the writer lock.
    OpenHome { root: String },
    /// "Change data folder": open the folder the person chose, only when it is a home or empty (anything else
    /// refused by name, nothing written).
    ChangeHome { root: String },
    /// Move the home to another path.
    MigrateHome { to: String },
    /// Change the size cap.
    SetCap { bytes: u64 },
    // ── Mirror and restore ──
    /// Write a mirror bundle.
    ExportMirror { to: String },
    /// Reconcile once (background): assemble the audit input for the core. Only COMPLETE releases the pen.
    Reconcile,
    /// Check everything again: drop this machine's record of chain facts already checked (`checkedx`), so the
    /// next scan asks about every log.
    RecheckAll,
    /// Read the chain once (background): balance and two read-only calls.
    ReadChain,
    /// Set chain endpoints (`<chain>=<url>`, several allowed).
    SetEndpoints { specs: String },
    // ── First run and adoption ──
    /// Write genesis. A legal action (verb `init`).
    Genesis { statement: String },
    /// Adopt an existing ledger directory in place.
    Adopt { dir: String },
    // Ledger view.
    /// Read the ledger table once (background): walk the ledger directory and read bytes into rows.
    ReadLedger,
    /// Open an entry's details: canonical bytes read from disk now.
    OpenEntry { id: String },
    /// Write an annotation. A legal action (verb `annotate`).
    Annotate { subject: String, note_md: String },
    /// Delete a work record: write a `retraction` entry by the local reading convention (`retractx`). A legal
    /// action. The verb table has no equivalent row, so `verb()` returns `None`.
    Retract { subject: String, note_md: String },
    // Self-audit.
    /// Run a self-audit (background): scan the chain, assemble the input, the core writes the report.
    Audit,
    /// Record the three basis fields (chain, registry, start block).
    SetBasis { chain: String, registry: String, from_block: String },
    /// Change the self-audit period.
    SetAuditEvery { secs: u64 },
    /// The auto-anchor setting (per home, off by default).
    SetAutoAnchor { on: bool },
    /// Whether the records and ledger pages leave out what a deletion leaves on this machine only (per home,
    /// off by default; display only).
    SetHideLocalDeletions { on: bool },
    // Anchoring desk.
    /// Take a content hash (one of three entry points).
    TakeContent { source: crate::anchorx::Source, path: String },
    /// Write history entries. A legal action (verb `history`). With `files` empty, one entry for the content
    /// at hand; otherwise a batch: one entry per file, signed one by one, stopping at the first failure
    /// (signed entries stay). `for_` is the optional "recorded for" field, written into each body as given
    /// (the app never reads it back).
    RecordWork { note_md: String, files: Vec<String>, for_: Option<crate::anchorx::For> },
    /// Register a git repository.
    RegisterRepo { path: String },
    // Kit index.
    /// Set a kit's `link` by hand (empty returns to the default: publication base plus kit path). Kits are
    /// identified by their index row's path.
    SetKitLink { path: String, link: String },
    /// Delete a kit's local copy (its index row; the directory only when it holds exactly this kit; the
    /// ledger is untouched). Kits are identified by path.
    DropKitCopy { path: String },
    /// Compute the registered repository's passive indicator. Once per page open, no resident polling.
    CheckRepo,
    /// A dropped path: file, directory or git repository, decided by reading the disk now.
    TakeDropped { path: String },
    // Anchor queue.
    /// Estimate gas for this batch. Shown before sending: without this reading, sending is not allowed.
    EstimateGas { count: usize },
    /// Send this batch of anchors. A legal action (verb `anchor`).
    SendBatch { count: usize },
    /// Resend a stuck batch with higher fees at the same nonce: `tx` is the batch's last transaction and `cap`
    /// the fee cap its card showed (`Shell::stuck`, read when the last receipt wait ended); the resend goes out
    /// at exactly the fees shown, or not at all. A legal action (verb `anchor`: the same anchoring at a higher
    /// price). Only a person's press sends it; nothing resends automatically.
    BumpFee { tx: String, cap: u64 },
    // Disclosure kits.
    /// Pick once: range and record hash fields (disk walked in the background).
    PickKit { from: String, to: String, ids: String },
    /// Write a kit (background): pick, attach, hand to the kit output crate to lay out and self-verify; lands
    /// only on KIT_OK.
    ExportKit { from: String, to: String, ids: String, attach: String, note: String, out: String },
    /// Digest attachments first: each path dropped on the export page is digested in the background in
    /// content form.
    VetAttachments { paths: Vec<String> },
    // Depth.
    /// Read depth once (background): the audit input from the core, the three measures from the kit core.
    ReadDepth { work: String },
    // Grant drafting.
    /// Draft a grant. A legal action (verb `grant`). `exclusive` and `terms_file` are recorded once at
    /// signing (`termsx`, local bookkeeping); no action changes them afterwards.
    DraftGrant { draft: Box<crate::grantx::Draft>, exclusive: bool, terms_file: Option<String> },
    /// Queue an entry that is in the ledger but not in the queue. Writing an entry normally queues it, but
    /// entries whose queueing failed (or older ledgers' genesis) are not; this gives them a way onto the chain.
    QueueEntry { id: String },
    // Grant ledger.
    /// Read the grant table once (background).
    ReadGrants,
    /// Run the overlap check (guard against selling the same work twice).
    CheckClash { work: String, from: String, to: String },
    // First-run checklist.
    /// Tick a step. Skipping a step is refused by name.
    WizardTick { step: String, said: String },
    /// Start over.
    WizardReset,
    // Revocation.
    /// Assemble a revocation. A legal action (verb `revoke`).
    Revoke { grant: String, case: String },
    /// Read a grant's three-part story.
    ReadStory { grant: String },
    // Adoption.
    /// Check each entered anchor (background chain queries).
    VerifyAnchors { rows: String },
    /// Verify a cosignature (local): only then may its two fields go into the body.
    Cosign { rows: String, attestor: String, attestation: String },
    /// Assemble an adoption. A legal action (verb `adopt`).
    AdoptAnchors { rows: String, attestor: String, attestation: String },
    /// List the anchors a key sent (background). With `address` empty, this key: read the fragment the
    /// opening audit already fetched and list those outside the ledger. With an address, scan the chain for
    /// that key's anchors (the same reading as others' ledgers). Each row is verified on chain and its state
    /// returned.
    ListKeyAnchors { address: String },
    /// Read a claim someone sent (the attesting side): who claims, which anchors, their ledger head; with a
    /// network configured, ask each anchor's block in the background.
    ReadClaim { text: String },
    /// Attest for someone: pass the local passcode, sign the claim text's preimage with this seat's key (the
    /// adoption signing domain), and return the signature.
    AttestFor { text: String, pin: crate::secret::Secret },
    // Succession.
    /// Scan the new key once (background): anchors it already sent are flagged.
    LookAtKey { to: String },
    /// Assemble a succession. A legal action (verb `succeed`).
    Succeed { to: String, kind: String, effective: String, statement_md: String },
    // Others' ledgers.
    /// Read someone else's ledger (background).
    ReadBook { address: String, dir: String },
    /// Address book: remember or forget an address (local only).
    RememberAddress { address: String },
    ForgetAddress { address: String },
    // Due diligence.
    /// Run due diligence (background): scan anchors, have the core give the label and three measures, read
    /// the grant and succession history, check for a double sale.
    Diligence { address: String, dir: String, work: String, from: String, to: String },
    /// Save the last due-diligence panels as a snapshot (no evidential weight).
    SaveSnapshot { to: String },
    // Record verification.
    /// Verify a record (background): kit verification (kit core), anchor review (anchoring crate and core),
    /// depth reading (kit core).
    VerifyWork { path: String, work: String },
    // Delivery check.
    /// Delivery check (background): read the delivered bytes and compare their sha256 with the recorded hash.
    CheckDelivery { path: String, expect: String },
    // Grant check.
    /// Check a grant (background): a payload or document file, ledger directories per hop, endpoints and
    /// basis filled in on the page (no settings written), now injected or read from the chain; six checks and
    /// chain check by the kit core. `file` and `terms` are optional: the granted work and terms files,
    /// compared by the delivery check in the same pass.
    CheckPayload { typed: String, ledgers: String, endpoints: String, registry: String, from_block: String, now: String, file: String, terms: String },
    // Grant vault.
    /// Import a grant document (file or payload): the kit core decodes the payload, the core accepts it, it
    /// goes into the vault.
    ImportGrant { typed: String },
    /// Import a folder of grants (grantee settings, data): each file goes through the import path.
    ImportGrantDir { dir: String },
    /// Record where a held grant's upstream bytes are (local bookkeeping).
    SetUpstream { grant: String, dir: String },
    /// Name a held grant and its issuer for this machine only (local bookkeeping, sealed with the settings).
    /// An empty name leaves that name as it was.
    NoteHeld { grant: String, note: String, issuer_note: String },
    /// Review the whole vault (background): six checks per grant by the kit core, upstream label by the core,
    /// countdowns by chain time only.
    ReviewVault,
    // Vault listing.
    /// List the vault (background): walk grants-held.
    ListHeld,
    /// Change the periodic review (sentinel scan) interval.
    SetReviewEvery { secs: String },
    /// Change the language and save it in settings.
    SetLang { lang: crate::lang::Lang },
    /// Choose the time zone moments are shown in, and keep it in settings.
    SetZone { zone: crate::when::Zone },
    /// How the window looks on this machine (light, dark, or following the system); written to machine
    /// settings, since it belongs to the machine and not to a home.
    SetAppearance { appearance: String },
    // Badges.
    /// Write a badge (background): cascade from a held grant to its root, encode and self-verify with the kit
    /// core, QR code, land.
    ExportBadge { grant: String, out: String },
    /// The grant code of a grant, whole: its chain cascaded to the root and encoded (the code the copy key puts
    /// on the clipboard; the badge and the grant file encode the same chain the same way). It does not pass the
    /// exit gate: the code says only what the grant's own entries say.
    CopyGrantCode { grant: String },
    // Grant files and publication.
    /// Write a grant file (single-file container): chain, terms documents, grant code, publication pointer,
    /// issuer's ledger; lands where the person chose.
    ExportGrantFile { id: String, to: String },
    /// Set the publication address (`https://` only); empty clears it.
    SetPublish { url: String },
    /// Check publication (background): fetch each file of the local kit back and compare.
    CheckPublished { local: String },
    // Restored identities.
    /// Fetch this identity's full ledger from a whole-machine backup (`from`, opened with `password`), land it
    /// in this home, and scan the chain to check the tail.
    FetchLedger { from: String, password: crate::secret::Secret },
    /// When fetching found the fetched ledger and this one in conflict (same position, different contents) and
    /// the person accepted: this seat's home is set aside whole (kept and readable, never written to or
    /// exported), and a fresh home in its place receives the fetched ledger. `from` and `password` as for
    /// `FetchLedger`.
    FetchAside { from: String, password: crate::secret::Secret },
    /// Check the tail of this identity's seats against the chain, whatever their ledgers came from: each seat
    /// home holding the not-fetched mark is checked (`exitgate::tail`), and a passing one opens for
    /// writing. The app starts it itself when it is due (`Shell::tail_due`).
    CheckTail,
    /// Open old data (a home set aside after a conflict) to read it; the current home is remembered for
    /// returning. The machine pointer does not move.
    ViewOldData { root: String },
    /// Leave old data for the home open before it.
    LeaveOldData,
    /// Write the open home from this machine: its writer mark named another machine, so this one opened it
    /// read-only (`lock::Mode::OtherMachine`); the mark is rewritten to this machine and this instance writes.
    TakeWriter,
    // Read-only networks (machine-wide, every identity's).
    /// Add a read-only network (`was` empty) or change the one `was` names (chain id, registry), from the fields
    /// a person typed. Only written to the machine directory's table; no chain is contacted.
    SaveReadNetwork { was: Option<(u64, String)>, name: String, chain: String, registry: String, from_block: String, nodes: String },
    /// Remove a read-only network (chain id, registry).
    RemoveReadNetwork { chain: u64, registry: String },
    /// Read one read-only network once (background): its nodes, and the code at its registry against the
    /// pinned build.
    ReadReadNetwork { chain: u64, registry: String },
    /// "Enable command line": read what is currently at the command line's location on the terminal's command
    /// path (the settings row shows it each time it is shown).
    ReadCliPath,
    /// "Enable command line": put the command line that ships beside the app on the terminal's command path
    /// (`on`), or take off what this switch put there (background: the system may ask for an administrator in its
    /// own dialog). Something else at that place is refused by name and never touched.
    SetCliPath { on: bool },
    /// Choose how the command line's `anchor` is handled when it goes through the desktop app (machine-wide,
    /// `machine.json`): sent as the send button would, or left in the queue for the user. Takes effect at once.
    SetCliAnchor { to: crate::machine::CliAnchor },
    /// The command line asked the desktop app to send the queue while sending is left to the user
    /// (`CliAnchor::Queue`): nothing is sent; the request is reported and the `count` entries wait in the queue
    /// for the user to send from the queue page. With nothing queued (either setting) it is refused like the
    /// send button's batch (`QUEUE_EMPTY`).
    SendAsked { count: usize },
    /// Choose how this machine's connections to nodes go out (machine-wide): `system` (follow the system's
    /// proxy settings), `none`, or one proxy address (`http://host:port`, `socks5://host:port`). Taken by the
    /// next new connection; a malformed address is refused by name and nothing is written.
    SetProxy { choice: String },
}

}

/// Where a restore from a backup is started. A closed set of three.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoreHow {
    /// First run, the identity step: the passcode was just set.
    FirstRun,
    /// Settings: the passcode opens the vault first (the same failure count).
    Settings { pin: crate::secret::Secret },
    /// The locked card: a new passcode, typed twice.
    Locked { pin: crate::secret::Secret, again: crate::secret::Secret },
}

impl Action {
    /// Wipe the passcode field (once it has opened the vault, the action body no longer needs it).
    pub fn forget_pin(&mut self) {
        match self {
            Action::BackupKey { pin, .. }
            | Action::DeleteIdentity { pin, .. }
            | Action::RevealWords { pin }
            | Action::AttestFor { pin, .. }
            | Action::ExportBackup { pin, .. } => pin.clear(),
            // The gate's own fields ([`Action::gate_pin`]), all of them.
            Action::SetPin { pin, again } | Action::RecoverWords { pin, again, .. } | Action::RecoverKeystore { pin, again, .. } => {
                pin.clear();
                again.clear();
            }
            Action::Unlock { pin } | Action::Reseal { pin } | Action::SetPrimary { pin, .. } | Action::RestoreBackup { how: RestoreHow::Settings { pin }, .. } => pin.clear(),
            Action::RestoreBackup { how: RestoreHow::Locked { pin, again }, .. } => {
                pin.clear();
                again.clear();
            }
            Action::ChangePin { old, pin, again } => {
                old.clear();
                pin.clear();
                again.clear();
            }
            _ => {}
        }
    }

    /// The passcode a gate action hands to its own background task, for actions where [`Action::pin_asked`] is
    /// `None` because the action is the gate itself: setting the passcode (the new one), unlocking (the one
    /// typed), resealing, changing (the old one, which opens before the new is set), the two recoveries (the new
    /// one), and actions that open the vault inside their own task (making another identity primary, restoring
    /// a backup from settings or the lock card). Together with [`Action::pin_asked`] this is closed: every
    /// action carrying a passcode answers one of the two. Each moves its fields into its task and keeps no
    /// copy; [`Action::forget_pin`] wipes them in place.
    pub fn gate_pin(&self) -> Option<&crate::secret::Secret> {
        match self {
            Action::SetPin { pin, .. }
            | Action::Unlock { pin }
            | Action::Reseal { pin }
            | Action::RecoverWords { pin, .. }
            | Action::RecoverKeystore { pin, .. }
            | Action::SetPrimary { pin, .. }
            | Action::RestoreBackup { how: RestoreHow::Settings { pin } | RestoreHow::Locked { pin, .. }, .. } => Some(pin),
            Action::ChangePin { old, .. } => Some(old),
            _ => None,
        }
    }

    /// Moves the passcode this action carries out to the task that derives with it, leaving the action's field
    /// empty so no second copy stays in memory. Gated actions ([`Action::pin_asked`]) hand it over in
    /// [`apply`]; the gate's own actions ([`Action::gate_pin`]) move it out when destructured. `None` when the
    /// action carries no passcode or has already handed it over. A passcode never handed over is zeroed when the
    /// action is dropped (`Secret`'s drop), whether on a lock, a quit or a refusal.
    pub fn hand_pin(&mut self) -> Option<crate::secret::Secret> {
        let cell = match self {
            Action::BackupKey { pin, .. }
            | Action::DeleteIdentity { pin, .. }
            | Action::RevealWords { pin }
            | Action::AttestFor { pin, .. }
            | Action::ExportBackup { pin, .. }
            | Action::SetPin { pin, .. }
            | Action::Unlock { pin }
            | Action::Reseal { pin }
            | Action::RecoverWords { pin, .. }
            | Action::RecoverKeystore { pin, .. }
            | Action::SetPrimary { pin, .. }
            | Action::RestoreBackup { how: RestoreHow::Settings { pin } | RestoreHow::Locked { pin, .. }, .. } => pin,
            Action::ChangePin { old, .. } => old,
            _ => return None,
        };
        if cell.is_empty() {
            return None;
        }
        Some(std::mem::take(cell))
    }

    /// Which actions ask for the local passcode (exporting a key file, deleting an identity, showing the
    /// words).
    ///
    /// "Using a key passes the passcode gate" is enforced by this table and the single check in [`apply`]. The
    /// passcode has one gate ([`crate::keybox::unlock`]); a second would be another way around it with another
    /// failure count. Export and delete share the gate, and five failures lock the same vault. A new
    /// passcode-protected action adds a row here and nothing else.
    ///
    /// The gate's own actions (set, unlock, change, the two recoveries) are not in the table: they are the
    /// gate, each with its own rules. Tests check that every action with a `pin` field is either here or in
    /// the list below.
    pub fn pin_asked(&self) -> Option<&crate::secret::Secret> {
        match self {
            Action::BackupKey { pin, .. }
            | Action::DeleteIdentity { pin, .. }
            | Action::RevealWords { pin }
            | Action::AttestFor { pin, .. }
            | Action::ExportBackup { pin, .. } => Some(pin),
            // Making another identity primary and restoring from settings seal the new vault with the passcode
            // itself, so each opens the vault with it inside its own task (the same gate and failure count).
            Action::SetPrimary { .. } | Action::RestoreBackup { .. } => None,
            // The gate's own actions do not pass the gate.
            Action::SetPin { .. }
            | Action::Unlock { .. }
            | Action::Reseal { .. }
            | Action::ChangePin { .. }
            | Action::RecoverWords { .. }
            | Action::RecoverKeystore { .. } => None,
            _ => None,
        }
    }

    /// Whether an action uses a key. When locked, all such actions are refused ("locked").
    ///
    /// Closed: each new action must be answered here. Key-using actions are refused at the entry of [`apply`]
    /// before touching the disk; [`crate::keybox::get`] is the second barrier, so no path can forget to ask
    /// about the lock.
    ///
    /// Local data is sealed under a key derived from the vault's master key, so while locked nothing local can
    /// be read either: the background work that reads it is refused by its own table
    /// ([`Action::halts_when_locked`]), and every other local read is refused by the local data layer
    /// (`local::read`).
    pub fn needs_key(&self) -> bool {
        match self {
            // Writing entries, anchoring, signing, cosigning, handing over: each needs this seat's key.
            Action::Genesis { .. }
            | Action::RecordWork { .. }
            | Action::Annotate { .. }
            | Action::Retract { .. }
            | Action::DraftGrant { .. }
            | Action::Revoke { .. }
            | Action::SendBatch { .. }
            | Action::BumpFee { .. }
            | Action::AdoptAnchors { .. }
            | Action::Cosign { .. }
            | Action::Succeed { .. }
            | Action::AttestFor { .. }
            // The key actions themselves: create, build an identity, import, delete an identity, back up to a
            // file, show the words.
            | Action::MakeAnchorKey
            | Action::SetPrimary { .. }
            | Action::ExportBackup { .. }
            | Action::RestoreBackup { how: RestoreHow::FirstRun | RestoreHow::Settings { .. }, .. }
            | Action::ConfirmIdentity { .. }
            | Action::ImportIdentity { .. }
            | Action::DeleteIdentity { .. }
            | Action::BackupKey { .. } => true,
            // Showing the words is not in this table: its passcode is the unlock question itself (the
            // `pin_asked` gate), so a locked machine can still show the words once.
            Action::RevealWords { .. } => false,
            // Naming only changes a registry field no decision reads; no key is touched.
            Action::NameIdentity { .. } => false,
            Action::Show(_)
            | Action::SelfCheck
            | Action::Quit
            | Action::Measure
            | Action::ReadIdentities
            | Action::NewIdentity
            | Action::DropFresh
            // Switching seat or identity uses no key itself, but the register and the homes it opens are sealed
            // local data: while locked the local data layer (`local::read`) refuses them, so a locked machine
            // does not switch seat or identity (by design; unlocking comes first).
            | Action::SwitchRole
            | Action::SwitchIdentity { .. }
            | Action::SetPin { .. }
            | Action::Unlock { .. }
            | Action::Reseal { .. }
            | Action::ResetEmptyKeybox
            | Action::Lock
            | Action::SetAutoLock { .. }
            // Restoring on the locked card is the way out of the lock: it brings its own new passcode.
            | Action::RestoreBackup { how: RestoreHow::Locked { .. }, .. }
            | Action::PeekBackup { .. }
            | Action::Resume
            | Action::CatchUp
            | Action::ChooseNetwork { .. }
            | Action::ChangePin { .. }
            | Action::RecoverWords { .. }
            | Action::RecoverKeystore { .. }
            | Action::HideWords
            | Action::OpenHome { .. }
            | Action::ChangeHome { .. }
            | Action::ViewOldData { .. }
            | Action::LeaveOldData
            | Action::TakeWriter
            | Action::MigrateHome { .. }
            | Action::SetCap { .. }
            | Action::ExportMirror { .. }
            | Action::Reconcile
            | Action::RecheckAll
            | Action::ReadChain
            | Action::SetEndpoints { .. }
            | Action::SaveReadNetwork { .. }
            | Action::RemoveReadNetwork { .. }
            | Action::ReadReadNetwork { .. }
            | Action::Adopt { .. }
            | Action::ReadLedger
            | Action::OpenEntry { .. }
            | Action::Audit
            | Action::SetBasis { .. }
            | Action::SetAuditEvery { .. }
            | Action::SetAutoAnchor { .. }
            | Action::SetHideLocalDeletions { .. }
            | Action::QueueEntry { .. }
            | Action::TakeContent { .. }
            | Action::RegisterRepo { .. }
            | Action::CheckRepo
            | Action::TakeDropped { .. }
            | Action::EstimateGas { .. }
            | Action::PickKit { .. }
            | Action::ExportKit { .. }
            | Action::VetAttachments { .. }
            | Action::ReadDepth { .. }
            | Action::ReadGrants
            | Action::CheckClash { .. }
            | Action::WizardTick { .. }
            | Action::WizardReset
            | Action::ReadStory { .. }
            | Action::VerifyAnchors { .. }
            | Action::ListKeyAnchors { .. }
            | Action::ReadClaim { .. }
            | Action::LookAtKey { .. }
            | Action::ReadBook { .. }
            | Action::CheckPayload { .. }
            | Action::ImportGrant { .. }
            | Action::ImportGrantDir { .. }
            | Action::SetUpstream { .. }
            | Action::NoteHeld { .. }
            | Action::ReviewVault
            | Action::ListHeld
            | Action::VerifyWork { .. }
            | Action::CheckDelivery { .. }
            | Action::Diligence { .. }
            | Action::SaveSnapshot { .. }
            | Action::SetReviewEvery { .. }
            | Action::SetLang { .. }
            | Action::SetZone { .. }
            | Action::SetAppearance { .. }
            | Action::SetProxy { .. }
            | Action::SetCliAnchor { .. }
            | Action::SendAsked { .. }
            | Action::ReadCliPath
            | Action::SetCliPath { .. }
            | Action::ExportBadge { .. }
            | Action::CopyGrantCode { .. }
            | Action::ExportGrantFile { .. }
            | Action::SetPublish { .. }
            | Action::CheckPublished { .. }
            | Action::FetchLedger { .. }
            | Action::FetchAside { .. }
            | Action::CheckTail
            | Action::RememberAddress { .. }
            | Action::ForgetAddress { .. }
            | Action::SetKitLink { .. }
            | Action::DropKitCopy { .. } => false,
        }
    }

    /// The background work that reads local data: the self-audit clock, the vault review and its sentinel, the
    /// queue's sending and its receipt wait, and the catch-up after unlocking. Closed; while locked each is
    /// refused here with `LOCKED` (the window does not ask while locked either; this is the rule itself).
    pub fn halts_when_locked(&self) -> bool {
        matches!(self, Action::Audit | Action::ReviewVault | Action::SendBatch { .. } | Action::BumpFee { .. } | Action::Resume | Action::CatchUp)
    }

    /// The component this action belongs to; its trace mark.
    pub fn feature(&self) -> Feature {
        match self {
            Action::Show(_) | Action::SelfCheck | Action::Quit => Feature::H1,
            Action::MakeAnchorKey
            | Action::SwitchRole
            | Action::ReadIdentities
            | Action::NewIdentity
            | Action::ConfirmIdentity { .. }
            | Action::DropFresh
            | Action::ImportIdentity { .. }
            | Action::NameIdentity { .. }
            | Action::SwitchIdentity { .. }
            | Action::DeleteIdentity { .. }
            | Action::SetPin { .. }
            | Action::Unlock { .. }
            | Action::Reseal { .. }
            | Action::ResetEmptyKeybox
            | Action::Lock
            | Action::SetAutoLock { .. }
            | Action::ChangePin { .. }
            | Action::RecoverWords { .. }
            | Action::RecoverKeystore { .. }
            | Action::RevealWords { .. }
            | Action::HideWords
            | Action::SetPrimary { .. }
            | Action::CatchUp
            | Action::BackupKey { .. } => Feature::H2,
            Action::ExportBackup { .. } | Action::PeekBackup { .. } | Action::RestoreBackup { .. } => Feature::H4,
            Action::Resume => Feature::W4,
            Action::OpenHome { .. } | Action::ChangeHome { .. } | Action::TakeWriter | Action::MigrateHome { .. } | Action::SetCap { .. } => Feature::H3,
            Action::ViewOldData { .. } | Action::LeaveOldData => Feature::H8,
            Action::Measure => Feature::H3,
            Action::ExportMirror { .. }
            | Action::Reconcile
            | Action::RecheckAll
            | Action::ReadChain
            | Action::SetEndpoints { .. }
            | Action::ChooseNetwork { .. }
            | Action::SaveReadNetwork { .. }
            | Action::RemoveReadNetwork { .. }
            | Action::ReadReadNetwork { .. }
            | Action::SetProxy { .. } => Feature::H4,
            Action::Genesis { .. } | Action::Adopt { .. } => Feature::H5,
            Action::ReadLedger | Action::OpenEntry { .. } | Action::Annotate { .. } | Action::Retract { .. } | Action::SetHideLocalDeletions { .. } => Feature::W1,
            Action::Audit | Action::SetBasis { .. } | Action::SetAuditEvery { .. } => Feature::W2,
            Action::QueueEntry { .. } | Action::SetAutoAnchor { .. } | Action::SetCliAnchor { .. } | Action::SendAsked { .. } => Feature::W4,
            Action::TakeContent { .. }
            | Action::RecordWork { .. }
            | Action::RegisterRepo { .. }
            | Action::CheckRepo
            | Action::TakeDropped { .. } => Feature::W3,
            Action::EstimateGas { .. } | Action::SendBatch { .. } | Action::BumpFee { .. } => Feature::W4,
            Action::PickKit { .. } | Action::ExportKit { .. } | Action::SetKitLink { .. } | Action::DropKitCopy { .. } | Action::VetAttachments { .. } => Feature::W5,
            Action::ReadDepth { .. } => Feature::W6,
            Action::DraftGrant { .. } => Feature::W7,
            Action::ReadGrants | Action::CheckClash { .. } => Feature::W8,
            Action::WizardTick { .. } | Action::WizardReset => Feature::W13,
            Action::Revoke { .. } | Action::ReadStory { .. } => Feature::W9,
            Action::VerifyAnchors { .. }
            | Action::Cosign { .. }
            | Action::AdoptAnchors { .. }
            | Action::ListKeyAnchors { .. }
            | Action::ReadClaim { .. }
            | Action::AttestFor { .. } => Feature::W10,
            Action::LookAtKey { .. } | Action::Succeed { .. } => Feature::W11,
            Action::ReadBook { .. }
            | Action::RememberAddress { .. }
            | Action::ForgetAddress { .. } => Feature::W14,
            Action::Diligence { .. } | Action::SaveSnapshot { .. } => Feature::D1,
            Action::VerifyWork { .. } => Feature::D2,
            Action::CheckDelivery { .. } => Feature::D4,
            Action::ImportGrant { .. } | Action::ImportGrantDir { .. } | Action::SetUpstream { .. } | Action::NoteHeld { .. } | Action::ReviewVault => Feature::D6,
            Action::ListHeld => Feature::D6,
            Action::SetReviewEvery { .. } => Feature::D7,
            Action::SetLang { .. } | Action::SetZone { .. } => Feature::H6,
            Action::ReadCliPath | Action::SetCliPath { .. } => Feature::H6,
            Action::SetAppearance { .. } => Feature::H0,
            Action::ExportBadge { .. } | Action::CopyGrantCode { .. } => Feature::D9,
            Action::ExportGrantFile { .. } => Feature::D6,
            Action::SetPublish { .. } | Action::CheckPublished { .. } => Feature::W14,
            Action::FetchLedger { .. } => Feature::H8,
            Action::FetchAside { .. } => Feature::H8,
            Action::CheckTail => Feature::H8,
            Action::CheckPayload { .. } => Feature::P1,
        }
    }

    /// Whether this action lets facts of this ledger leave the machine, and which way (the exit gate's closed
    /// table, `exitgate`). Every action is answered here, one arm each, with no catch-all: a new action does not
    /// compile until it is classified. Landing entries is not an exit (it stays offline); sending anchors and
    /// exporting a record bundle, a grant file, a mirror or a badge are. Reading what is published, the
    /// whole-machine backup (sealed to oneself), a key file and a diligence snapshot (someone else's ledger) are
    /// not.
    pub fn exit(&self) -> Option<crate::exitgate::Exit> {
        use crate::exitgate::Exit;
        match self {
            Action::Show(..) => None,
            Action::SelfCheck => None,
            Action::Measure => None,
            Action::Quit => None,
            Action::MakeAnchorKey => None,
            Action::SwitchRole => None,
            Action::ReadIdentities => None,
            Action::NewIdentity => None,
            Action::ConfirmIdentity { .. } => None,
            Action::DropFresh => None,
            Action::ImportIdentity { .. } => None,
            Action::SwitchIdentity { .. } => None,
            Action::DeleteIdentity { .. } => None,
            Action::NameIdentity { .. } => None,
            Action::SetPin { .. } => None,
            Action::Unlock { .. } => None,
            Action::Reseal { .. } => None,
            Action::ResetEmptyKeybox => None,
            Action::Lock => None,
            Action::ChangePin { .. } => None,
            Action::RecoverWords { .. } => None,
            Action::RecoverKeystore { .. } => None,
            Action::RevealWords { .. } => None,
            Action::HideWords => None,
            Action::BackupKey { .. } => None,
            Action::SetAutoLock { .. } => None,
            Action::SetPrimary { .. } => None,
            Action::ExportBackup { .. } => None,
            Action::PeekBackup { .. } => None,
            Action::RestoreBackup { .. } => None,
            Action::Resume => None,
            Action::CatchUp => None,
            Action::ChooseNetwork { .. } => None,
            Action::OpenHome { .. } => None,
            Action::ChangeHome { .. } => None,
            Action::ViewOldData { .. } => None,
            Action::LeaveOldData => None,
            Action::TakeWriter => None,
            Action::MigrateHome { .. } => None,
            Action::SetCap { .. } => None,
            Action::ExportMirror { .. } => Some(Exit::Mirror),
            Action::Reconcile => None,
            Action::RecheckAll => None,
            Action::ReadChain => None,
            Action::SetEndpoints { .. } => None,
            Action::SaveReadNetwork { .. } => None,
            Action::RemoveReadNetwork { .. } => None,
            Action::ReadReadNetwork { .. } => None,
            Action::Genesis { .. } => None,
            Action::Adopt { .. } => None,
            Action::ReadLedger => None,
            Action::OpenEntry { .. } => None,
            Action::Annotate { .. } => None,
            Action::Retract { .. } => None,
            Action::Audit => None,
            Action::SetBasis { .. } => None,
            Action::SetAuditEvery { .. } => None,
            Action::SetAutoAnchor { .. } => None,
            Action::SetHideLocalDeletions { .. } => None,
            Action::TakeContent { .. } => None,
            Action::RecordWork { .. } => None,
            Action::RegisterRepo { .. } => None,
            Action::SetKitLink { .. } => None,
            Action::DropKitCopy { .. } => None,
            Action::CheckRepo => None,
            Action::TakeDropped { .. } => None,
            Action::EstimateGas { .. } => None,
            Action::SendBatch { .. } => Some(Exit::Send),
            // A resend carries only the call the send it replaces already took out (its hashes, its nonce): no
            // fact of this ledger leaves with it that had not left. It still reads the chain afresh at the gate
            // before its bytes go (`resend_batch`), as the send did.
            Action::BumpFee { .. } => None,
            Action::PickKit { .. } => None,
            Action::ExportKit { .. } => Some(Exit::Kit),
            Action::VetAttachments { .. } => None,
            Action::ReadDepth { .. } => None,
            Action::DraftGrant { .. } => None,
            Action::QueueEntry { .. } => None,
            Action::ReadGrants => None,
            Action::CheckClash { .. } => None,
            Action::WizardTick { .. } => None,
            Action::WizardReset => None,
            Action::Revoke { .. } => None,
            Action::ReadStory { .. } => None,
            Action::VerifyAnchors { .. } => None,
            Action::Cosign { .. } => None,
            Action::AdoptAnchors { .. } => None,
            Action::ListKeyAnchors { .. } => None,
            Action::ReadClaim { .. } => None,
            Action::AttestFor { .. } => None,
            Action::LookAtKey { .. } => None,
            Action::Succeed { .. } => None,
            Action::ReadBook { .. } => None,
            Action::RememberAddress { .. } => None,
            Action::ForgetAddress { .. } => None,
            Action::Diligence { .. } => None,
            Action::SaveSnapshot { .. } => None,
            Action::VerifyWork { .. } => None,
            Action::CheckDelivery { .. } => None,
            Action::CheckPayload { .. } => None,
            Action::ImportGrant { .. } => None,
            Action::ImportGrantDir { .. } => None,
            Action::SetUpstream { .. } => None,
            Action::NoteHeld { .. } => None,
            Action::ReviewVault => None,
            Action::ListHeld => None,
            Action::SetReviewEvery { .. } => None,
            Action::SetLang { .. } => None,
            Action::SetZone { .. } => None,
            Action::SetAppearance { .. } => None,
            Action::SetProxy { .. } => None,
            Action::SetCliAnchor { .. } => None,
            Action::ReadCliPath => None,
            Action::SetCliPath { .. } => None,
            // Nothing leaves: the request is only reported.
            Action::SendAsked { .. } => None,
            Action::ExportBadge { .. } => Some(Exit::Badge),
            Action::CopyGrantCode { .. } => None,
            Action::ExportGrantFile { .. } => Some(Exit::GrantFile),
            Action::SetPublish { .. } => None,
            Action::CheckPublished { .. } => None,
            Action::FetchLedger { .. } => None,
            Action::FetchAside { .. } => None,
            Action::CheckTail => None,
        }
    }

    /// Actions that only read what is here (nothing written to the home, the machine or the chain). Closed.
    pub fn reads_only(&self) -> bool {
        matches!(
            self,
            Action::Show(_)
                | Action::Quit
                | Action::SelfCheck
                | Action::ReadLedger
                | Action::OpenEntry { .. }
                | Action::ReadGrants
                | Action::ReadStory { .. }
                | Action::ReadIdentities
                | Action::ListHeld
                | Action::Measure
        )
    }

    /// What may run while old data is open (`ViewOldData`): reading it, coming back, and the vault gate.
    pub fn reads_old_data(&self) -> bool {
        self.reads_only() || matches!(self, Action::Lock | Action::Unlock { .. } | Action::Reseal { .. } | Action::LeaveOldData)
    }

    /// The closed table of ledger-writing and anchoring actions: for a restored identity, before the full
    /// ledger is fetched and its tail checked, these are refused by name at the entry of `body`.
    /// `FetchLedger` itself is not here: it is the way to writing.
    pub fn writes_ledger(&self) -> bool {
        matches!(
            self,
            Action::Genesis { .. }
                | Action::RecordWork { .. }
                | Action::Annotate { .. }
                | Action::Retract { .. }
                | Action::DraftGrant { .. }
                | Action::Revoke { .. }
                | Action::SendBatch { .. }
                | Action::BumpFee { .. }
                | Action::AdoptAnchors { .. }
                | Action::Succeed { .. }
                | Action::Cosign { .. }
        )
    }

    /// Whether an action is legal: the family that touches the anchor key, the ledger or the chain.
    ///
    /// Moving a home and settings are not: they move the same bytes or change local preferences and create no
    /// new recorded fact (any copy is equivalent).
    pub fn is_legal(&self) -> bool {
        matches!(
            self,
            Action::MakeAnchorKey
                | Action::Genesis { .. }
                | Action::Annotate { .. }
                | Action::Retract { .. }
                | Action::RecordWork { .. }
                | Action::SendBatch { .. }
                | Action::BumpFee { .. }
                | Action::DraftGrant { .. }
                | Action::Revoke { .. }
                | Action::AdoptAnchors { .. }
                | Action::Succeed { .. }
        )
    }

    /// The command-line verb this action is equivalent to. No legal action returns `None` (the tests check).
    pub fn verb(&self) -> Option<&'static str> {
        match self {
            Action::MakeAnchorKey => Some("keygen"),
            Action::Genesis { .. } => Some("init"),
            Action::Annotate { .. } => Some("annotate"),
            Action::Retract { .. } => Some("retract"),
            // This follows the verb table (`CLI-SCHEMA.md` §6), not the format's entry types; the two happen to
            // share this spelling. The command line owns the verb table and the app does not depend on it, so
            // this is a second copy (like the envelope keys, see `entryx`); tests read the table from
            // `CLI-SCHEMA.md` and check every row.
            Action::RecordWork { .. } => Some("history"),
            // `SendAsked` is the command line's `anchor` routed through the desktop app when sending is left to
            // the user.
            Action::SendBatch { .. } | Action::BumpFee { .. } | Action::SendAsked { .. } => Some("anchor"),
            // Attesting for someone is the command line's `attest`; it asks for the passcode, so it is never run
            // on the command line's behalf (`pin_asked`).
            Action::AttestFor { .. } => Some("attest"),
            // The two chain reads write nothing, but each has a command-line name.
            Action::Audit => Some("audit"),
            Action::DraftGrant { .. } => Some("grant"),
            Action::ExportKit { .. } => Some("kit-export"),
            Action::ReadDepth { .. } => Some("depth"),
            Action::Revoke { .. } => Some("revoke"),
            Action::AdoptAnchors { .. } => Some("adopt"),
            Action::Succeed { .. } => Some("succeed"),
            Action::ReadBook { .. } => Some("scan"),
            // The six checks' command-line name.
            Action::ReviewVault => Some("check-grant"),
            Action::ExportBadge { .. } => Some("badge"),
            Action::EstimateGas { .. } => None,
            _ => None,
        }
    }
}

/// What happened after applying. The screen says this; there is no branch that says nothing.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Applied {
    Shown(Page),
    Started(Kind),
    Refused(Kind),
    Stopped(Reaped),
    /// The anchor key is in place; its address.
    AnchorKey(Address),
    /// Switched seat.
    Seated(crate::roles::Role),
    /// Read the identities; how many.
    Identities(usize),
    /// New words generated, waiting for copy confirmation.
    FreshWords,
    /// The new words were wiped.
    FreshDropped,
    /// An identity was built (or imported); the current seat is its author seat. `restored`: the identity was
    /// already in the registry and this restored its missing vault slot. `seats` lists the seats it holds and
    /// their addresses (an existing key holds one).
    IdentityMade { id: String, seats: Vec<(crate::roles::Role, Address)>, restored: bool },
    /// An identity was renamed (nothing reads the name).
    IdentityNamed { id: String, label: String },
    /// Switched to this identity.
    IdentitySwitched { id: String },
    /// This identity was deleted (key slots and registry row; homes kept).
    IdentityDeleted { id: String },
    /// The vault is open (set, unlock, change and the recoveries each succeed with this).
    Unlocked,
    /// The vault is locked.
    LockedUp,
    /// An empty vault was deleted; this machine is back to "no passcode set".
    KeyboxReset,
    /// The passcode was changed.
    PinChanged,
    /// Idle locking was switched or its time changed (machine-wide): whether it is on, and after how long.
    AutoLockSet { on: bool, secs: u64 },
    /// This identity is now the primary one (a new master key; everything resealed).
    PrimarySet { id: String },
    /// Restored from a backup: what it held.
    BackupRestored(crate::backup::Summary),
    /// A receipt wait was resumed (false: nothing to resume, or not now).
    Resumed(bool),
    /// The catch-up after unlocking: which of the self-audit, the vault review and the receipt wait started.
    CaughtUp { audit: bool, review: bool, resume: bool },
    /// A network was chosen; `filled` means the current home now has that row's network (false with
    /// "custom", or when the person configured this home's network by hand).
    NetworkChosen { name: String, filled: bool },
    /// The auto-anchor setting was saved.
    AutoAnchor(bool),
    /// The setting that hides local deletions from the two lists was saved.
    HideLocalDeletions(bool),
    /// Recovered, with a new passcode.
    Recovered,
    /// The twelve words are shown.
    WordsShown,
    /// Hidden.
    WordsHidden,
    /// The key was backed up to a keystore file.
    KeyBackedUp { path: String, address: Address },
    /// The home is open, with its mode (writer or reader).
    Homed { root: String, mode: crate::lock::Mode },
    /// The record of checked facts was dropped (`had`: there was one).
    FactsForgotten { had: bool },
    /// The move finished.
    Migrated { root: String },
    /// The cap changed.
    Capped(u64),
    /// A mirror bundle was written.
    Mirrored { path: String, entries: usize, added: usize, topped_up: bool },
    /// Endpoints set.
    Endpoints(usize),
    /// Genesis written, with its id. It is queued at once, with the queue length and what comes next.
    Genesised { id: String, queued: usize, next: Next },
    /// Adopted in place.
    AdoptedInPlace { entries: usize, linked: usize, label: String },
    /// An entry was opened: its id and canonical byte length.
    Opened { id: String, bytes: usize },
    /// An annotation was written.
    Annotated(String),
    /// A work record was deleted: the retraction's id, whether the deleted record was in the anchor queue
    /// (and was removed), the queue length, and whether the pair stays local (the deleted record was never
    /// published, so the retraction is not queued).
    Retracted { id: String, dropped: bool, queued: usize, local: bool, next: Next },
    /// The basis was recorded.
    Basis { chain: u64 },
    /// The self-audit period changed.
    Every(u64),
    /// A content hash was taken.
    Took { source: crate::anchorx::Source, hex: String },
    /// A history entry was written and queued.
    Recorded { id: String, queued: usize, next: Next },
    /// Batch signing: the ids signed (in drop order), where it stopped (index, path, reason; `None` when all
    /// succeeded), the queue length, and what comes next. Signed entries stay.
    RecordedBatch { ids: Vec<String>, stopped: Option<(usize, String, crate::fault::Fault)>, queued: usize, next: Next },
    /// A kit's `link` changed (`None`: back to the default with no publication base configured).
    KitLinked { path: String, link: Option<String> },
    /// The whole grant code of a grant, for the clipboard.
    GrantCode { text: String },
    /// A kit's index row was removed; `dir_removed` when the directory held exactly this kit and was deleted.
    KitDropped { id: String, dir_removed: bool },
    /// A repository was registered.
    Registered { path: String },
    /// The repository's passive indicator was computed.
    Since { head: String, grew: Option<usize> },
    /// Gas was estimated, with the call data handed to the node for the estimate (built right there).
    Gas { count: usize, gas: u64, calldata: String },
    /// Entries picked, with those the closure rule pulled in.
    Picked { items: usize, pulled: usize },
    /// A grant was drafted and queued.
    Granted { id: String, queued: usize, next: Next },
    /// An entry outside the queue was queued, with the queue length.
    Queued { id: String, queued: usize, next: Next },
    /// A grant file landed: where, why there, how many hops, how many terms documents, whether it carries the
    /// issuer's ledger, how many items.
    GrantFileExported { path: String, why: crate::home::Why, hops: usize, terms: usize, ledger: bool, files: usize },
    /// The publication address was recorded (`None` clears it).
    PublishSet { url: Option<String> },
    /// The read-only network table was written: how many networks it holds now.
    ReadNets(usize),
    /// The overlap check ran: how many overlaps.
    Clashed(usize),
    /// A step was ticked, with the next step.
    Ticked { step: &'static str, next: Option<&'static str> },
    /// The checklist starts over.
    Restarted,
    /// A revocation was written.
    Revoked { id: String, queued: usize, next: Next },
    /// A grant's three-part story was read.
    Storied { grant: String, revocations: usize },
    /// The cosignature verified.
    Cosigned { attestor: String },
    /// A claim was read (no network configured, blocks not asked).
    ClaimRead,
    /// A claim was attested for someone: the attestor and the signature (hex).
    Attested { attestor: String, attestation: String },
    /// An adoption was written (with or without a cosignature, as it was).
    Adopted { id: String, cosigned: bool, queued: usize, next: Next },
    /// A succession was written.
    Succeeded { id: String, to: String, queued: usize, next: Next },
    /// The address book changed.
    Booked { address: String, on: bool },
    /// A snapshot was saved (no evidential weight), with where and how many bytes.
    Snapshot { path: String, bytes: usize },
    /// Grants went into the vault (a payload may hold several), and the grant the pass was for (a chain's last
    /// hop; empty for a folder of grants).
    Held { ids: Vec<String>, grant: String },
    /// A folder import took some files and refused others (each refusal is listed in the trouble panel).
    HeldPartly { ids: Vec<String>, refused: usize },
    /// A held grant's upstream location was recorded.
    Upstream { grant: String },
    /// A held grant's or its issuer's local name was recorded.
    HeldNoted { grant: String },
    /// The review period changed.
    ReviewEvery(u64),
    /// The language changed and was saved.
    Spoken(crate::lang::Lang),
    /// The time zone moments are now shown in.
    Zoned(crate::when::Zone),
    /// The appearance was written to machine settings.
    Appeared(String),
    /// The proxy choice was written to machine settings (as written: `system`, `none` or the address).
    ProxySet(String),
    /// How the command line's `anchor` is handled through the desktop app, as written.
    CliAnchorSet(crate::machine::CliAnchor),
    /// What is currently at the command line's location on the terminal's command path.
    CliPathRead(zikaron_os::cli_path::State),
    /// The command line asked to send; nothing was sent, and `count` entries wait for the user.
    SendAsked { count: usize },
    /// It did not happen, with a named reason. There is no silent branch.
    Trouble(crate::fault::Fault),
}

/// After a passcode attempt, bring the shell in line with the disk. The vault locks itself on the final
/// failure (the master key is wiped at once) while the shell still holds what it had when open (anchor
/// address, shown words). Every refused passcode action goes through here.
fn after_vault_shut(shell: &mut Shell) {
    shell.reread_vault();
    if !shell.unlocked() {
        shell.after_lock();
    }
}

/// Whether `apply` holds this action back now, and with which refusal: the single check at the entry of the
/// action layer. The window also asks it before a timed background action (so a refusal it knows is coming is
/// not sent every tick), and never keeps its own copy of these rules.
pub fn held_back(shell: &Shell, a: &Action) -> Option<crate::fault::Known> {
    // While the master key is being changed (making another identity primary, restoring from a backup) every
    // other action waits: the screen says it is resealing.
    if shell.rekeying && !matches!(a, Action::Show(_) | Action::Quit | Action::SelfCheck) {
        return Some(crate::fault::Known::Rekeying);
    }
    // While a home is being swapped for a fresh one (fetch and replace) it is frozen: anything written now
    // might or might not be copied and could end up only in the old data. Reads pass.
    if shell.swapping && !a.reads_only() {
        return Some(crate::fault::Known::Rekeying);
    }
    // With old data open, only reading and leaving are allowed: it is never written, fetched into, moved or
    // exported.
    if shell.old_view.is_some() && !a.reads_old_data() {
        return Some(crate::fault::Known::ReadOnly);
    }
    // While locked, background work that reads local data is refused by name (`CatchUp` resumes it).
    if a.halts_when_locked() && !shell.unlocked() {
        return Some(crate::fault::Known::Locked);
    }
    // While locked, key-using actions are refused here without touching the disk.
    if a.needs_key() && !shell.unlocked() {
        return Some(crate::fault::Known::Locked);
    }
    None
}

/// Runs the exit gate in the background before an export that writes on the UI thread (a grant file, a record
/// bundle): reading the chain waits on nodes, and the window never waits on the network. When the gate passes,
/// the export runs where the result lands ([`gate_landed`]); a refusal lands as a trouble, and the home takes
/// its read-only mark there (`Shell::gate_refused`).
fn gate_first(shell: &mut Shell, a: Action) -> Applied {
    let ask = match crate::exitgate::ask_of(shell) {
        Ok(x) => x,
        Err(f) => return shell.trouble(f),
    };
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf());
    match shell.tasks.spawn(Kind::Gate, move || {
        let pass = crate::exitgate::pass(&ask)?;
        Ok(Done::GatePassed { root, then: Box::new(a), pass: Box::new(pass) })
    }) {
        Spawned::Started => Applied::Started(Kind::Gate),
        Spawned::InFlight => Applied::Refused(Kind::Gate),
    }
}

/// An export's exit gate passed for the home at `root`; `stale` says whether the source has moved since it
/// started. The pass holds only for that home and source, in which case the export runs now. Otherwise the
/// pass is void: if another export is already being gated, that gate answers for itself and this returns
/// `None`; if not, the export is applied again through [`apply`], where it is refused like any press (locked,
/// no home) or gated again for the home and source now open.
pub fn gate_landed(shell: &mut Shell, root: Option<std::path::PathBuf>, then: Action, pass: &crate::exitgate::Pass, stale: bool) -> Option<Applied> {
    let here = shell.home.as_ref().map(|h| h.root().to_path_buf());
    if stale || here != root {
        if shell.tasks.in_flight(Kind::Gate) {
            return None;
        }
        return Some(apply(shell, then));
    }
    // The export runs with the gate's pass; going through `apply` would gate it again.
    trace::mark(then.feature());
    if let Some(k) = held_back(shell, &then) {
        return Some(shell.trouble(crate::fault::Fault::known(k, String::new())));
    }
    Some(match then {
        Action::ExportMirror { to } => match export_mirror(shell, &to, pass) {
            Ok((path, entries, added, topped_up)) => Applied::Mirrored { path, entries, added, topped_up },
            Err(f) => shell.trouble(f),
        },
        Action::ExportGrantFile { id, to } => match export_grant_file(shell, &id, &to, pass) {
            Ok(x) => grant_file_exported(x),
            Err(f) => shell.trouble(f),
        },
        // Only the two exports above are gated through a background pass.
        other => apply(shell, other),
    })
}

/// Applies an action: the single entry point shared by the window and the test hooks.
pub fn apply(shell: &mut Shell, a: Action) -> Applied {
    trace::mark(a.feature());
    if let Some(k) = held_back(shell, &a) {
        return shell.trouble(crate::fault::Fault::known(k, String::new()));
    }
    // Actions that carry a passcode pass `keybox::unlock` here: one gate, one failure count. Each failure is
    // recorded on disk, and five lock the vault, leaving only recovery.
    //
    // This comes after the locked check: a locked vault still refuses every key-using action, and this gate
    // re-identifies the person on an open vault (someone at an unlocked machine cannot export the key or
    // delete an identity).
    //
    // Key derivation runs in the background: the unlock runs in a `Kind::Vault` task, and only on success does
    // the action body run where the result lands (`vault_landed`, then `body`); the passcode field is wiped
    // before the action is handed on. The passcode is moved out of the action into the task (`hand_pin`), never
    // copied: the action handed on carries none.
    if a.pin_asked().is_some() {
        let mut rest = a;
        let pin = rest.hand_pin().unwrap_or_default();
        rest.forget_pin();
        return vault(shell, move || {
            crate::keybox::unlock(pin.expose())?;
            Ok(crate::task::Vault::Gate(Box::new(rest)))
        });
    }
    body(shell, a)
}

/// Starts a background task for a passcode action's key derivation. Single flight: while one runs, another is
/// refused by name rather than queued.
fn vault(shell: &mut Shell, work: impl FnOnce() -> Result<crate::task::Vault, crate::fault::Fault> + Send + 'static) -> Applied {
    match shell.tasks.spawn(Kind::Vault, move || work().map(Done::Vault)) {
        Spawned::Started => Applied::Started(Kind::Vault),
        Spawned::InFlight => Applied::Refused(Kind::Vault),
    }
}

/// A passcode task landed; continue on the UI thread. Called where the shell receives results
/// (`Shell::drain_at`); success and refusal both return the same `Applied` as if run inline.
pub fn vault_landed(shell: &mut Shell, got: Result<crate::task::Vault, crate::fault::Fault>) -> Applied {
    use crate::task::Vault;
    match got {
        Ok(Vault::Opened) => {
            shell.after_unlock();
            // The vault opened but the reseal was not saved: report it, since the next start will ask for the
            // passcode again and the person should know why (disk full, read-only directory).
            if let Some(f) = crate::keybox::take_reseal_trouble() {
                return shell.trouble(f);
            }
            Applied::Unlocked
        }
        Ok(Vault::Changed) => {
            shell.after_unlock();
            Applied::PinChanged
        }
        Ok(Vault::Recovered) => {
            shell.after_unlock();
            Applied::Recovered
        }
        Ok(Vault::Gate(a)) => {
            shell.after_unlock();
            body(shell, *a)
        }
        // A new master key took effect: every copy in hand was read under the old one, so the shell drops them
        // and opens the home again under the new one.
        Ok(Vault::PrimarySet { id }) => {
            shell.rekeying = false;
            shell.unload();
            shell.after_unlock();
            Applied::PrimarySet { id }
        }
        Ok(Vault::Restored { summary }) => {
            shell.rekeying = false;
            shell.unload();
            shell.after_unlock();
            Applied::BackupRestored(summary)
        }
        Ok(Vault::Identity { row, restored, fresh }) => identity_landed(shell, row, restored, fresh),
        Err(f) => {
            shell.rekeying = false;
            // A label taken for a task that did not land is not kept for the next one.
            shell.new_label = None;
            // After any failure, reread the vault state. The final failure locks the vault; a shell that kept
            // showing the passcode fields with no tries left would be wrong, so the screen follows the disk and
            // the final failure goes through the full lock path.
            after_vault_shut(shell);
            shell.trouble(f)
        }
    }
}

/// A master key change (set primary, restore) reseals every local file, and fetch-and-replace copies a home's
/// folders before swapping it. A background task that writes local data from its own thread
/// (`Kind::writes_local`) could write under the key being replaced, or into the home after its folders were
/// copied, so these operations are refused until such tasks land.
fn busy_for_rekey(shell: &Shell) -> Option<crate::fault::Fault> {
    let others: Vec<Kind> = shell.tasks.flying().into_iter().filter(|k| k.writes_local()).collect();
    (!others.is_empty()).then(|| {
        crate::fault::Fault::known(crate::fault::Known::BusyForRekey, others.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(" "))
    })
}

/// Apply for the test hooks: like [`apply`], but when the action starts a background task, wait for it to land
/// and return that `Applied` (a test step is one whole path, not spread over frames). Other results received
/// while waiting are handled as usual.
pub fn apply_settled(shell: &mut Shell, a: Action) -> Applied {
    let first = apply(shell, a);
    if let Applied::Started(k) = first {
        if lands_said(k) {
            return said_settled(shell, k);
        }
    }
    if first != Applied::Started(Kind::Vault) {
        return first;
    }
    loop {
        if let Some(done) = shell.vault_said.take() {
            return done;
        }
        // The worker finished but the task is still in flight after draining: the outcome never came back, so
        // say so by name instead of waiting forever.
        let finished = shell.tasks.finished_in_flight(Kind::Vault);
        shell.drain_hold();
        if shell.vault_said.is_none() {
            if finished && shell.tasks.in_flight(Kind::Vault) {
                return shell.trouble(crate::fault::Fault::known(crate::fault::Known::OutcomeLost, Kind::Vault.as_str().to_string()));
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

/// Waits for the person's answer to an already opened file dialog: not an action (no verb, nothing read or
/// written, nothing to hold back) but a background task, so it starts where tasks start. One at a time: if a
/// dialog is already out this returns `InFlight` and the new wait is dropped (the window only opens a dialog
/// when none is out).
pub fn wait_path(shell: &mut Shell, wait: crate::platform::Wait) -> crate::task::Spawned {
    shell.tasks.spawn(Kind::Path, move || wait().map(Done::Path))
}

/// The kinds whose action result arrives when its task lands (`Shell::said`): the slow half runs in the
/// background, and the UI-thread half and the result come with the landing ([`landed`]).
pub fn lands_said(k: Kind) -> bool {
    crate::landing::goes_back(k, crate::landing::Back::Said)
}

/// The UI-thread half of those kinds, run where the result is received; it returns what the action would have
/// returned had it run inline.
pub fn landed(shell: &mut Shell, k: Kind, got: Result<Done, crate::fault::Fault>) -> Applied {
    let lost = |shell: &mut Shell| shell.trouble(crate::fault::Fault::known(crate::fault::Known::OutcomeLost, k.as_str().to_string()));
    match (k, got) {
        (Kind::Gas, got) => gas_landed(shell, got),
        (Kind::Take, Ok(Done::Took { source, content })) => took_landed(shell, source, content),
        (Kind::Record, Ok(Done::Hashed { note_md, for_, files })) => {
            let (ids, stopped, n) = record_files(shell, &note_md, files, for_.as_ref());
            if let Some((_, _, f)) = &stopped {
                shell.faults.push(f.clone());
            }
            Applied::RecordedBatch { ids, stopped, queued: n.queued, next: n.next }
        }
        (Kind::Migrate, Ok(Done::Copied { old, root })) => match migrate_landed(shell, &old, &root) {
            Ok(root) => Applied::Migrated { root },
            Err(f) => shell.trouble(f),
        },
        (_, Err(f)) => shell.trouble(f),
        (_, Ok(_)) => lost(shell),
    }
}

/// Starts taking a content: its fingerprint is computed in a task (`Kind::Take`).
fn take_started(shell: &mut Shell, source: crate::anchorx::Source, p: std::path::PathBuf) -> Applied {
    match shell.tasks.spawn(Kind::Take, move || crate::anchorx::of(source, &p).map(|content| Done::Took { source, content })) {
        Spawned::Started => {
            shell.content = None;
            shell.said.remove(&Kind::Take);
            Applied::Started(Kind::Take)
        }
        Spawned::InFlight => Applied::Refused(Kind::Take),
    }
}

/// For [`apply_settled`]: waits until an action of those kinds lands (`Shell::said`), as for a passcode task.
fn said_settled(shell: &mut Shell, k: Kind) -> Applied {
    loop {
        if let Some(done) = shell.said.remove(&k) {
            return done;
        }
        let finished = shell.tasks.finished_in_flight(k);
        shell.drain_hold();
        if !shell.said.contains_key(&k) {
            if (finished && shell.tasks.in_flight(k)) || !shell.tasks.in_flight(k) {
                return shell.trouble(crate::fault::Fault::known(crate::fault::Known::OutcomeLost, k.as_str().to_string()));
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

/// A gas estimate landed. On success the batch's estimate and fees go on the shell (shown on the send sheet,
/// used by the balance gate and carried by the transaction); on failure there is no estimate, so the batch
/// cannot be sent, and the refusal is recorded.
pub fn gas_landed(shell: &mut Shell, got: Result<Done, crate::fault::Fault>) -> Applied {
    // A failed estimate has no fee reading, so no nodes are listed as left out of one.
    if got.is_err() {
        shell.fees_left.clear();
    }
    match got {
        Ok(Done::Gas { count, gas, calldata, fees, fees_left, head_time }) => {
            if let Some(t) = head_time {
                shell.note_chain_time(t);
            }
            shell.gas = Some((count, gas));
            shell.fees = Some(fees);
            shell.fees_left = fees_left;
            Applied::Gas { count, gas, calldata }
        }
        Ok(_) => shell.trouble(crate::fault::Fault::known(crate::fault::Known::OutcomeLost, Kind::Gas.as_str().to_string())),
        Err(f) => {
            shell.gas = None;
            shell.fees = None;
            shell.trouble(f)
        }
    }
}

/// The action body (after the locked check and the passcode gate).
fn body(shell: &mut Shell, a: Action) -> Applied {
    // A restored identity is read-only until its full ledger is fetched: while marked, the ledger-writing and
    // anchoring actions are refused by name without touching the disk.
    if a.writes_ledger() {
        if let Some(s) = shell.unfetched {
            return shell.trouble(s.fault());
        }
    }
    match a {
        Action::Show(p) => {
            shell.page = p;
            Applied::Shown(p)
        }
        Action::SelfCheck => {
            match shell.tasks.spawn(Kind::SelfCheck, || crate::probe::run().map(Done::Check)) {
                Spawned::Started => Applied::Started(Kind::SelfCheck),
                Spawned::InFlight => Applied::Refused(Kind::SelfCheck),
            }
        }
        Action::Measure => {
            let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else {
                return shell.trouble(crate::fault::Fault::known(
                    crate::fault::Known::NoHome,
                    crate::lang::t(crate::lang::Key::Tail001).to_string(),
                ));
            };
            match shell.tasks.spawn(Kind::Archive, move || measure(&root)) {
                Spawned::Started => Applied::Started(Kind::Archive),
                Spawned::InFlight => Applied::Refused(Kind::Archive),
            }
        }
        Action::Quit => {
            // Clear the shown and unconfirmed words first (zeroing their buffers): a quitting process may not
            // run every destructor.
            for w in shell.words.iter_mut().flatten() {
                w.clear();
            }
            shell.words = None;
            shell.new_words = None;
            // Close the command-line channel first: pending requests are told the desktop closed.
            shell.door_off = true;
            shell.door_sync();
            Applied::Stopped(shell.tasks.shutdown())
        }

        Action::MakeAnchorKey => match make_anchor_key(shell) {
            Ok(a) => Applied::AnchorKey(a),
            Err(f) => shell.trouble(f),
        },
        Action::SwitchRole => match switch_role(shell) {
            Ok(r) => Applied::Seated(r),
            Err(f) => shell.trouble(f),
        },
        Action::ReadIdentities => match crate::register::view(shell.settings.role) {
            Ok(r) => {
                let n = r.rows.len();
                shell.seat_identities(Some(r));
                Applied::Identities(n)
            }
            Err(f) => shell.trouble(f),
        },
        Action::NewIdentity => match crate::identity::fresh() {
            Ok(f) => {
                shell.new_words = Some(f);
                Applied::FreshWords
            }
            Err(f) => shell.trouble(f),
        },
        Action::ConfirmIdentity { answers, label, network } => match confirm_identity(shell, &answers, &label, &network) {
            Ok(started) => started,
            Err(f) => shell.trouble(f),
        },
        Action::DropFresh => {
            shell.new_words = None;
            Applied::FreshDropped
        }
        Action::ImportIdentity { form, seat, label, network } => match import_identity(shell, form, seat, &label, &network) {
            Ok(started) => started,
            Err(f) => shell.trouble(f),
        },
        Action::SwitchIdentity { id } => match switch_identity(shell, &id) {
            Ok(row) => Applied::IdentitySwitched { id: row.id },
            Err(f) => shell.trouble(f),
        },
        Action::NameIdentity { id, label } => match name_identity(shell, &id, &label) {
            Ok(row) => Applied::IdentityNamed { id: row.id, label: row.label },
            Err(f) => shell.trouble(f),
        },
        Action::DeleteIdentity { id, pin: _ } => match delete_identity(shell, &id) {
            Ok(id) => Applied::IdentityDeleted { id },
            Err(f) => shell.trouble(f),
        },
        Action::SetPin { pin, again } => {
            if let Err(f) = same_twice(&pin, &again) {
                return shell.trouble(f);
            }
            vault(shell, move || {
                // A new passcode makes a new master key: files sealed under a vanished key store are moved aside
                // first (they could never be opened, and left in place they would stop their home from opening).
                if crate::keybox::pin_trouble(pin.expose()).is_none()
                    && matches!(crate::keybox::state()?, crate::keybox::State::Absent)
                {
                    let (n, at) = crate::local::set_aside_sealed()?;
                    if n > 0 {
                        crate::local::note_trouble(crate::fault::Fault::known(
                            crate::fault::Known::LocalSetAside,
                            format!("{n} · {}", at.map(|p| p.display().to_string()).unwrap_or_default()),
                        ));
                    }
                }
                crate::keybox::set_pin(pin.expose()).map(|_| crate::task::Vault::Opened)
            })
        }
        // After opening, in the same task: staged files settle, plain files from older versions are sealed, and
        // an older vault's primary identity is settled (`local::after_open`).
        Action::Unlock { pin } => vault(shell, move || {
            crate::keybox::unlock(pin.expose())?;
            crate::local::after_open();
            Ok(crate::task::Vault::Opened)
        }),
        Action::Reseal { pin } => vault(shell, move || {
            crate::keybox::reseal(pin.expose())?;
            crate::local::after_open();
            Ok(crate::task::Vault::Opened)
        }),
        Action::ResetEmptyKeybox => match crate::keybox::reset_empty() {
            Ok(()) => {
                // The file is gone, so the vault is `Absent`: key, words and address are wiped from the shell as
                // for a lock, the vault state is reread, the gate closes (`gate_up` is false for `Absent`) and
                // the wizard returns to step 1.
                shell.after_lock();
                Applied::KeyboxReset
            }
            Err(f) => shell.trouble(f),
        },
        Action::Lock => {
            // If a task that writes local data is still running, the lock takes effect on screen now (every
            // action is refused as locked) and completes (master key wiped, home closed) once those tasks land,
            // so a write under way, above all a broadcast anchor's queue mark, is never lost to a missing key.
            if shell.tasks.flying().into_iter().any(|k| k.writes_local()) {
                shell.begin_lock();
            } else {
                crate::keybox::lock();
                shell.after_lock();
            }
            Applied::LockedUp
        }
        Action::ChangePin { old, pin, again } => {
            if let Err(f) = same_twice(&pin, &again) {
                return shell.trouble(f);
            }
            // The old passcode goes through the same failure count: the final failure locks this session, and
            // the screen follows the disk when the result lands.
            vault(shell, move || crate::keybox::change_pin(old.expose(), pin.expose()).map(|_| crate::task::Vault::Changed))
        }
        Action::RecoverWords { words, pin, again } => {
            if let Err(f) = same_twice(&pin, &again) {
                return shell.trouble(f);
            }
            // Words are checked against the core's word list and checksum first (a malformed phrase is refused
            // by name, never tried against the vault); the entropy never leaves the identity layer.
            let fresh = match crate::identity::from_words(words.expose()) {
                Ok(f) => f,
                Err(f) => return shell.trouble(f),
            };
            vault(shell, move || {
                crate::identity::recover_with(&fresh, pin.expose())?;
                crate::local::after_open();
                Ok(crate::task::Vault::Recovered)
            })
        }
        Action::RecoverKeystore { path, password, pin, again } => {
            if let Err(f) = same_twice(&pin, &again) {
                return shell.trouble(f);
            }
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => return shell.trouble(crate::fault::classify(&e, &path)),
            };
            vault(shell, move || {
                let secret = crate::keystore::decrypt_typed(&bytes, &password)?;
                crate::key::recover_with(&secret, pin.expose())?;
                crate::local::after_open();
                Ok(crate::task::Vault::Recovered)
            })
        }
        Action::RevealWords { pin: _ } => match reveal_words(shell) {
            Ok(()) => Applied::WordsShown,
            Err(f) => {
                after_vault_shut(shell);
                shell.trouble(f)
            }
        },
        Action::HideWords => {
            // Zero each word's buffer in place, then drop it (dropping zeroes it too).
            for w in shell.words.iter_mut().flatten() {
                w.clear();
            }
            shell.words = None;
            Applied::WordsHidden
        }
        Action::ChooseNetwork { name } => match choose_network(shell, &name) {
            Ok(filled) => Applied::NetworkChosen { name, filled },
            Err(f) => shell.trouble(f),
        },
        Action::SetAutoLock { on, secs } => match set_auto_lock(shell, on, secs) {
            Ok((on, secs)) => Applied::AutoLockSet { on, secs },
            Err(f) => shell.trouble(f),
        },
        Action::SetPrimary { id, pin } => {
            if let Some(f) = busy_for_rekey(shell) {
                return shell.trouble(f);
            }
            let started = vault(shell, move || {
                crate::keybox::unlock(pin.expose())?;
                crate::rekey::set_primary(&id, pin.expose())?;
                Ok(crate::task::Vault::PrimarySet { id })
            });
            shell.rekeying = started == Applied::Started(Kind::Vault);
            started
        }
        Action::ExportBackup { pin: _, password, again, dir } => match export_backup(shell, password, &again, &dir) {
            Ok(Spawned::Started) => Applied::Started(Kind::Backup),
            Ok(Spawned::InFlight) => Applied::Refused(Kind::Backup),
            Err(f) => shell.trouble(f),
        },
        Action::PeekBackup { path, password } => {
            match shell.tasks.spawn(Kind::Backup, move || crate::backup::peek(std::path::Path::new(&path), password.expose()).map(|s| Done::BackupSeen { summary: s })) {
                Spawned::Started => Applied::Started(Kind::Backup),
                Spawned::InFlight => Applied::Refused(Kind::Backup),
            }
        }
        Action::RestoreBackup { path, password, how } => {
            if let RestoreHow::Locked { pin, again } = &how {
                if let Err(f) = same_twice(pin, again) {
                    return shell.trouble(f);
                }
                if let Some(t) = crate::keybox::pin_trouble(pin.expose()) {
                    return shell.trouble(crate::fault::Fault::known(crate::fault::Known::PinShape, t.as_str().to_string()));
                }
            }
            if let Some(f) = busy_for_rekey(shell) {
                return shell.trouble(f);
            }
            let started = vault(shell, move || {
                let at = std::path::Path::new(&path);
                let done = match &how {
                    RestoreHow::FirstRun => crate::backup::restore(at, password.expose(), crate::backup::From::FirstRun)?,
                    RestoreHow::Settings { pin } => {
                        crate::keybox::unlock(pin.expose())?;
                        crate::backup::restore(at, password.expose(), crate::backup::From::Settings(pin.expose()))?
                    }
                    RestoreHow::Locked { pin, .. } => crate::backup::restore(at, password.expose(), crate::backup::From::Locked(pin.expose()))?,
                };
                Ok(crate::task::Vault::Restored { summary: done.summary })
            });
            shell.rekeying = started == Applied::Started(Kind::Vault);
            started
        }
        Action::Resume => Applied::Resumed(resume(shell)),
        Action::CatchUp => catch_up(shell),
        Action::BackupKey { pin: _, password, again, dir } => match backup_key(shell, password, &again, &dir) {
            Ok(Spawned::Started) => Applied::Started(Kind::Keystore),
            Ok(Spawned::InFlight) => Applied::Refused(Kind::Keystore),
            Err(f) => shell.trouble(f),
        },
        Action::OpenHome { root } => match open_home(shell, &root) {
            Ok(mode) => Applied::Homed { root, mode },
            Err(f) => shell.trouble(f),
        },
        Action::ChangeHome { root } => match change_home(shell, &root) {
            Ok(mode) => Applied::Homed { root, mode },
            Err(f) => shell.trouble(f),
        },
        Action::ViewOldData { root } => match view_old(shell, &root) {
            Ok(mode) => Applied::Homed { root, mode },
            Err(f) => shell.trouble(f),
        },
        Action::LeaveOldData => match leave_old(shell) {
            Ok((root, mode)) => Applied::Homed { root, mode },
            Err(f) => shell.trouble(f),
        },
        Action::TakeWriter => match take_writer(shell) {
            Ok((root, mode)) => Applied::Homed { root, mode },
            Err(f) => shell.trouble(f),
        },
        Action::MigrateHome { to } => migrate_begin(shell, &to),
        Action::SetCap { bytes } => match set_cap(shell, bytes) {
            Ok(n) => Applied::Capped(n),
            Err(f) => shell.trouble(f),
        },

        // Whatever can be refused without reading the chain is refused before the exit gate starts (the same
        // plan is checked again when the gate lands, against the state then).
        Action::ExportMirror { to } => match mirror_plan(shell, &to) {
            Ok(_) => gate_first(shell, Action::ExportMirror { to }),
            Err(f) => shell.trouble(f),
        },
        Action::Reconcile => {
            let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else {
                return shell.trouble(crate::fault::Fault::known(
                    crate::fault::Known::NoHome,
                    String::new(),
                ));
            };
            match shell.tasks.spawn(Kind::Reconcile, move || reconcile(&root)) {
                Spawned::Started => Applied::Started(Kind::Reconcile),
                Spawned::InFlight => Applied::Refused(Kind::Reconcile),
            }
        }
        Action::RecheckAll => match crate::checkedx::forget() {
            Ok(had) => Applied::FactsForgotten { had },
            Err(f) => shell.trouble(f),
        },
        Action::ReadChain => {
            let eps = shell.endpoints.clone();
            let Some(who) = shell.anchor else {
                return shell.trouble(crate::fault::Fault::known(
                    crate::fault::Known::KeychainMissing,
                    crate::lang::t(crate::lang::Key::Tail002).to_string(),
                ));
            };
            let chain = shell.settings.chain_id;
            match shell.tasks.spawn(Kind::Chain, move || read_chain(&eps, &who, chain))
            {
                Spawned::Started => Applied::Started(Kind::Chain),
                Spawned::InFlight => Applied::Refused(Kind::Chain),
            }
        }
        Action::SetEndpoints { specs } => match set_endpoints(shell, &specs) {
            Ok(n) => Applied::Endpoints(n),
            Err(f) => shell.trouble(f),
        },

        Action::Genesis { statement } => match genesis(shell, &statement) {
            Ok((id, n)) => {
                shell.rooted = true;
                Applied::Genesised { id, queued: n.queued, next: n.next }
            }
            Err(f) => shell.trouble(f),
        },
        // ── Ledger view ──
        Action::ReadLedger => {
            let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else {
                return shell.trouble(crate::fault::Fault::known(
                    crate::fault::Known::NoHome,
                    crate::lang::t(crate::lang::Key::Tail003).to_string(),
                ));
            };
            // The anchored state comes from the last audit's report; with no audit yet there is no report and
            // no green light.
            let report = shell.audit.as_ref().map(|a| a.report.clone());
            // The last pass's anchored set (disk cache): until a report arrives it shows "checked last time",
            // never "anchored".
            let remembered = shell.remembered.clone();
            let wall = (shell.clock)();
            // Status lights come from the queue file (step, block), the same source on every page.
            let queue = shell.queue.clone();
            let mine = shell.anchor;
            let r#gen = shell.rows_gen;
            // The first-anchor block times come from the chain fragment of the last self-audit.
            let fragment = shell.audit.as_ref().map(|a| a.fragment.clone());
            match shell.tasks.spawn(Kind::Ledger, move || {
                let home = crate::home::Home::open(&root)?;
                let mut t = crate::ledgerx::table_remembering(&home, report.as_ref(), &queue, remembered.as_ref(), wall)?;
                // Who writes this ledger from now on: once a succession is in the ledger, this desk has
                // handed it over. Read in the same pass as the table, so the notice and the table describe the
                // same moment.
                let pile = home
                    .ledger()?
                    .pile()?;
                let handed = crate::succeedx::handed_over(&pile.items, mine);
                if let Some(f) = &fragment {
                    crate::ledgerx::stamp(&mut t.rows, &pile.items, f);
                }
                Ok(Done::Ledger { rows: t.rows, strays: t.strays, handed, r#gen })
            }) {
                Spawned::Started => Applied::Started(Kind::Ledger),
                Spawned::InFlight => Applied::Refused(Kind::Ledger),
            }
        }
        Action::OpenEntry { id } => match open_entry(shell, &id) {
            Ok((id, n)) => Applied::Opened { id, bytes: n },
            Err(f) => shell.trouble(f),
        },
        Action::Annotate { subject, note_md } => match annotate(shell, &subject, &note_md) {
            Ok(id) => Applied::Annotated(id),
            Err(f) => shell.trouble(f),
        },
        Action::Retract { subject, note_md } => match retract(shell, &subject, &note_md) {
            Ok((id, dropped, q, local)) => Applied::Retracted { id, dropped, queued: q.queued, local, next: q.next },
            Err(f) => shell.trouble(f),
        },
        // ── Self-audit ──
        Action::Audit => match start_audit(shell) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Audit),
                Spawned::InFlight => Applied::Refused(Kind::Audit),
            },
            Err(f) => shell.trouble(f),
        },
        Action::SetBasis { chain, registry, from_block } => match set_basis(shell, &chain, &registry, &from_block) {
            Ok(audit::Basis::Saved(c)) => Applied::Basis { chain: c },
            Ok(audit::Basis::Checking(Spawned::Started)) => Applied::Started(Kind::Basis),
            Ok(audit::Basis::Checking(Spawned::InFlight)) => Applied::Refused(Kind::Basis),
            Err(f) => shell.trouble(f),
        },
        Action::SetAuditEvery { secs } => {
            match shell.commit_settings(|s| s.audit_every = secs) {
                Ok(()) => Applied::Every(secs),
                Err(f) => shell.trouble(f),
            }
        }
        Action::SetAutoAnchor { on } => match shell.commit_settings(|s| s.auto_anchor = on) {
            Ok(()) => Applied::AutoAnchor(on),
            Err(f) => shell.trouble(f),
        },
        Action::SetHideLocalDeletions { on } => match shell.commit_settings(|s| s.hide_local_deletions = on) {
            Ok(()) => Applied::HideLocalDeletions(on),
            Err(f) => shell.trouble(f),
        },
        // ── Anchoring desk ──
        // The fingerprint reads the whole content, so it runs as a task (`Kind::Take`) that lands in
        // `took_landed`. Meanwhile the previous content is cleared, so the record sheet cannot continue with the
        // one being replaced.
        Action::TakeContent { source, path } => {
            let p = std::path::PathBuf::from(path.trim());
            take_started(shell, source, p)
        }
        Action::RecordWork { note_md, files, for_ } if files.is_empty() => {
            match record_work(shell, &note_md, for_.as_ref()) {
                Ok((id, n)) => Applied::Recorded { id, queued: n.queued, next: n.next },
                Err(f) => shell.trouble(f),
            }
        }
        // Fingerprints of the files to record are computed in a task (`Kind::Record`); the entries are signed
        // and appended when it lands (`record_files`).
        Action::RecordWork { note_md, files, for_ } => match shell.tasks.spawn(Kind::Record, move || {
            let files = hash_files(&files);
            Ok(Done::Hashed { note_md, for_, files })
        }) {
            Spawned::Started => Applied::Started(Kind::Record),
            Spawned::InFlight => Applied::Refused(Kind::Record),
        },
        Action::VetAttachments { paths } => match shell.tasks.spawn(Kind::Vet, move || {
            let rows = paths
                .into_iter()
                .map(|p| {
                    let d = crate::kitx::digest_of(std::path::Path::new(p.trim())).map(|d| zikaron::hexfmt::encode(&d));
                    (p, d)
                })
                .collect();
            Ok(Done::Vetted(rows))
        }) {
            Spawned::Started => Applied::Started(Kind::Vet),
            Spawned::InFlight => Applied::Refused(Kind::Vet),
        },
        Action::SetKitLink { path, link } => match set_kit_link(shell, &path, &link) {
            Ok(link) => Applied::KitLinked { path, link },
            Err(f) => shell.trouble(f),
        },
        Action::DropKitCopy { path } => match drop_kit_copy(shell, &path) {
            Ok((row, dir_removed)) => Applied::KitDropped { id: row.id, dir_removed },
            Err(f) => shell.trouble(f),
        },
        Action::TakeDropped { path } => match dropped_source(&path) {
            Ok((source, p)) => take_started(shell, source, p),
            Err(f) => shell.trouble(f),
        },
        Action::RegisterRepo { path } => match register_repo(shell, &path) {
            Ok(p) => Applied::Registered { path: p },
            Err(f) => shell.trouble(f),
        },
        Action::CheckRepo => match check_repo(shell) {
            Ok(s) => Applied::Since { head: s.0, grew: s.1 },
            Err(f) => shell.trouble(f),
        },
        // ── Anchor queue ──
        // The estimate queries the network, so it runs as a task (`Kind::Gas`, single flight) that lands in
        // `gas_landed`. Meanwhile the last estimate is cleared so an old number cannot release a new batch, and
        // the send sheet shows the field loading with the send button disabled.
        Action::EstimateGas { count } => match gas_ask(shell, count) {
            Ok(ask) => match shell.tasks.spawn(Kind::Gas, move || estimate_on(ask)) {
                Spawned::Started => {
                    shell.gas = None;
                    shell.fees = None;
                    shell.said.remove(&Kind::Gas);
                    shell.gas_asked = Some(shell.gas_epoch);
                    Applied::Started(Kind::Gas)
                }
                Spawned::InFlight => Applied::Refused(Kind::Gas),
            },
            Err(f) => {
                shell.gas = None;
                shell.fees = None;
                shell.trouble(f)
            }
        },
        Action::SendBatch { count } => match send_batch(shell, count) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Anchor),
                Spawned::InFlight => Applied::Refused(Kind::Anchor),
            },
            Err(f) => shell.trouble(f),
        },
        Action::BumpFee { tx, cap } => match bump_batch(shell, &tx, cap) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Anchor),
                Spawned::InFlight => Applied::Refused(Kind::Anchor),
            },
            Err(f) => shell.trouble(f),
        },

        // ── Disclosure kits ──
        Action::PickKit { from, to, ids } => match pick_kit(shell, &from, &to, &ids) {
            Ok((n, pulled)) => {
                let p = pulled.len();
                shell.picked = Some((n, pulled));
                Applied::Picked { items: n, pulled: p }
            }
            Err(f) => shell.trouble(f),
        },
        Action::ExportKit { from, to, ids, attach, note, out } => {
            match export_kit(shell, &from, &to, &ids, &attach, &note, &out) {
                Ok(s) => match s {
                    Spawned::Started => Applied::Started(Kind::Kit),
                    Spawned::InFlight => Applied::Refused(Kind::Kit),
                },
                Err(f) => shell.trouble(f),
            }
        }
        // ── Depth ──
        Action::ReadDepth { work } => match read_depth(shell, &work) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Depth),
                Spawned::InFlight => Applied::Refused(Kind::Depth),
            },
            Err(f) => shell.trouble(f),
        },
        // ── Grant drafting ──
        Action::DraftGrant { draft, exclusive, terms_file } => {
            match draft_grant(shell, &draft, exclusive, terms_file.as_deref()) {
                Ok((id, q)) => Applied::Granted { id, queued: q.queued, next: q.next },
                Err(f) => shell.trouble(f),
            }
        }
        // ── Grant register ──
        Action::ReadGrants => {
            let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else {
                return shell.trouble(crate::fault::Fault::known(
                    crate::fault::Known::NoHome,
                    crate::lang::t(crate::lang::Key::Tail004).to_string(),
                ));
            };
            let flags = shell.settings.exclusive.clone();
            let r#gen = shell.grants_gen;
            match shell.tasks.spawn(Kind::Grants, move || {
                let home = crate::home::Home::open(&root)?;
                Ok(Done::Grants { r#gen, rows: crate::grantx::table(&home, &flags)? })
            }) {
                Spawned::Started => Applied::Started(Kind::Grants),
                Spawned::InFlight => Applied::Refused(Kind::Grants),
            }
        }
        Action::QueueEntry { id } => match queue_entry(shell, &id) {
            Ok((id, q)) => Applied::Queued { id, queued: q.queued, next: q.next },
            Err(f) => shell.trouble(f),
        },
        Action::CheckClash { work, from, to } => match check_clash(shell, &work, &from, &to) {
            Ok(n) => Applied::Clashed(n),
            Err(f) => shell.trouble(f),
        },
        // ── First-run checklist ──
        Action::WizardTick { step, said } => match wizard_tick(shell, &step, &said) {
            Ok((s, n)) => Applied::Ticked { step: s, next: n },
            Err(f) => shell.trouble(f),
        },
        Action::WizardReset => match wizard_reset(shell) {
            Ok(()) => Applied::Restarted,
            Err(f) => shell.trouble(f),
        },

        // ── Revocation ──
        Action::Revoke { grant, case } => match revoke(shell, &grant, &case) {
            Ok((id, n)) => Applied::Revoked { id, queued: n.queued, next: n.next },
            Err(f) => shell.trouble(f),
        },
        Action::ReadStory { grant } => match read_story(shell, &grant) {
            Ok(n) => Applied::Storied { grant, revocations: n },
            Err(f) => shell.trouble(f),
        },
        // ── Adoption ──
        Action::VerifyAnchors { rows } => match verify_anchors(shell, &rows) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Adopt),
                Spawned::InFlight => Applied::Refused(Kind::Adopt),
            },
            Err(f) => shell.trouble(f),
        },
        Action::Cosign { rows, attestor, attestation } => {
            match cosign(shell, &rows, &attestor, &attestation) {
                Ok(a) => Applied::Cosigned { attestor: a },
                Err(f) => shell.trouble(f),
            }
        }
        Action::AdoptAnchors { rows, attestor, attestation } => {
            match adopt_anchors(shell, &rows, &attestor, &attestation) {
                Ok((id, c, n)) => Applied::Adopted { id, cosigned: c, queued: n.queued, next: n.next },
                Err(f) => shell.trouble(f),
            }
        }
        Action::ListKeyAnchors { address } => match list_key_anchors(shell, &address) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Adopt),
                Spawned::InFlight => Applied::Refused(Kind::Adopt),
            },
            Err(f) => shell.trouble(f),
        },
        Action::ReadClaim { text } => match read_claim(shell, &text) {
            Ok(Some(s)) => match s {
                Spawned::Started => Applied::Started(Kind::Adopt),
                Spawned::InFlight => Applied::Refused(Kind::Adopt),
            },
            Ok(None) => Applied::ClaimRead,
            Err(f) => shell.trouble(f),
        },
        Action::AttestFor { text, .. } => match attest_for(shell, &text) {
            Ok((attestor, attestation)) => Applied::Attested { attestor, attestation },
            Err(f) => shell.trouble(f),
        },
        // ── Succession ──
        Action::LookAtKey { to } => match look_at_key(shell, &to) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Sighting),
                Spawned::InFlight => Applied::Refused(Kind::Sighting),
            },
            Err(f) => shell.trouble(f),
        },
        Action::Succeed { to, kind, effective, statement_md } => {
            match succeed(shell, &to, &kind, &effective, &statement_md) {
                Ok((id, n)) => Applied::Succeeded { id, to, queued: n.queued, next: n.next },
                Err(f) => shell.trouble(f),
            }
        }
        // ── Others' ledgers ──
        Action::ReadBook { address, dir } => match read_book(shell, &address, &dir) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Book),
                Spawned::InFlight => Applied::Refused(Kind::Book),
            },
            Err(f) => shell.trouble(f),
        },
        Action::RememberAddress { address } => match book_address(shell, &address, true) {
            Ok(a) => Applied::Booked { address: a, on: true },
            Err(f) => shell.trouble(f),
        },
        Action::ForgetAddress { address } => match book_address(shell, &address, false) {
            Ok(a) => Applied::Booked { address: a, on: false },
            Err(f) => shell.trouble(f),
        },
        // ── Diligence, record verification, delivery check ──
        Action::Diligence { address, dir, work, from, to } => {
            match diligence(shell, &address, &dir, &work, &from, &to) {
                Ok(s) => match s {
                    Spawned::Started => Applied::Started(Kind::Diligence),
                    Spawned::InFlight => Applied::Refused(Kind::Diligence),
                },
                Err(f) => shell.trouble(f),
            }
        }
        Action::SaveSnapshot { to } => match save_snapshot(shell, &to) {
            Ok(bytes) => Applied::Snapshot { path: to.trim().to_string(), bytes },
            Err(f) => shell.trouble(f),
        },
        Action::VerifyWork { path, work } => match verify_work(shell, &path, &work) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Verify),
                Spawned::InFlight => Applied::Refused(Kind::Verify),
            },
            Err(f) => shell.trouble(f),
        },
        Action::CheckDelivery { path, expect } => match check_delivery(shell, &path, &expect) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Delivery),
                Spawned::InFlight => Applied::Refused(Kind::Delivery),
            },
            Err(f) => shell.trouble(f),
        },
        // ── Grant check ──
        Action::CheckPayload { typed, ledgers, endpoints, registry, from_block, now, file, terms } => {
            match check_payload(shell, &typed, &ledgers, &endpoints, &registry, &from_block, &now, (file, terms)) {
                Ok(s) => match s {
                    Spawned::Started => Applied::Started(Kind::Check),
                    Spawned::InFlight => Applied::Refused(Kind::Check),
                },
                Err(f) => shell.trouble(f),
            }
        }
        // ── Grant vault ──
        Action::ImportGrant { typed } => match import_grant(shell, &typed) {
            Ok((ids, grant)) => Applied::Held { ids, grant },
            Err(f) => shell.trouble(f),
        },
        Action::ImportGrantDir { dir } => match import_grant_dir(shell, &dir) {
            Ok((ids, refused)) if refused.is_empty() => Applied::Held { ids, grant: String::new() },
            // Nothing taken: report the last refusal; the others go to the trouble panel too.
            Ok((ids, mut refused)) if ids.is_empty() => {
                let last = refused.pop().expect("非空");
                for f in refused {
                    shell.trouble(f);
                }
                shell.trouble(last)
            }
            // Some taken, some refused: not reported as success; each refusal goes to the trouble panel.
            Ok((ids, refused)) => {
                let n = refused.len();
                for f in refused {
                    shell.trouble(f);
                }
                Applied::HeldPartly { ids, refused: n }
            }
            Err(f) => shell.trouble(f),
        },
        Action::SetUpstream { grant, dir } => match set_upstream(shell, &grant, &dir) {
            Ok(g) => Applied::Upstream { grant: g },
            Err(f) => shell.trouble(f),
        },
        Action::NoteHeld { grant, note, issuer_note } => match note_held(shell, &grant, &note, &issuer_note) {
            Ok(g) => Applied::HeldNoted { grant: g },
            Err(f) => shell.trouble(f),
        },
        Action::ReviewVault => match review(shell) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Review),
                Spawned::InFlight => Applied::Refused(Kind::Review),
            },
            Err(f) => shell.trouble(f),
        },
        Action::SetReviewEvery { secs } => match set_review_every(shell, &secs) {
            Ok(n) => Applied::ReviewEvery(n),
            Err(f) => shell.trouble(f),
        },
        Action::SetLang { lang } => match set_lang(shell, lang) {
            Ok(l) => Applied::Spoken(l),
            Err(f) => shell.trouble(f),
        },
        Action::SetZone { zone } => match set_zone(shell, zone) {
            Ok(z) => Applied::Zoned(z),
            Err(f) => shell.trouble(f),
        },
        Action::SetAppearance { appearance } => match set_appearance(shell, &appearance) {
            Ok(a) => Applied::Appeared(a),
            Err(f) => shell.trouble(f),
        },
        Action::SetProxy { choice } => match set_proxy(shell, &choice) {
            Ok(c) => Applied::ProxySet(c),
            Err(f) => shell.trouble(f),
        },
        Action::ReadCliPath => {
            let s = cli_path_now();
            shell.cli_path = Some(s.clone());
            Applied::CliPathRead(s)
        }
        Action::SetCliPath { on } => match crate::action::cli_beside() {
            Some(cli) => match shell.tasks.spawn(Kind::CliPath, move || cli_path_set(&cli, on)) {
                Spawned::Started => Applied::Started(Kind::CliPath),
                Spawned::InFlight => Applied::Refused(Kind::CliPath),
            },
            None => shell.trouble(crate::fault::Fault::known(crate::fault::Known::CliPathUnsupported, String::new())),
        },
        Action::SetCliAnchor { to } => match crate::machine::update(|m| m.cli_anchor = to) {
            Ok(m) => {
                shell.machine = m;
                Applied::CliAnchorSet(to)
            }
            Err(f) => shell.trouble(f),
        },
        // With nothing queued, refuse as the send button does; otherwise only report the request (sending
        // happens from the queue page).
        Action::SendAsked { count } => match count {
            0 => shell.trouble(crate::fault::Fault::known(crate::fault::Known::QueueEmpty, crate::lang::t(crate::lang::Key::Tail030).to_string())),
            n => Applied::SendAsked { count: n },
        },
        Action::ListHeld => match list_held(shell) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Held),
                Spawned::InFlight => Applied::Refused(Kind::Held),
            },
            Err(f) => shell.trouble(f),
        },
        Action::ExportGrantFile { id, to } => match grant_file_plan(shell, &id, &to) {
            Ok(_) => gate_first(shell, Action::ExportGrantFile { id, to }),
            Err(f) => shell.trouble(f),
        },
        Action::SaveReadNetwork { was, name, chain, registry, from_block, nodes } => {
            match save_read_network(shell, was, &name, &chain, &registry, &from_block, &nodes) {
                Ok(n) => Applied::ReadNets(n),
                Err(f) => shell.trouble(f),
            }
        }
        Action::RemoveReadNetwork { chain, registry } => match remove_read_network(shell, chain, &registry) {
            Ok(n) => Applied::ReadNets(n),
            Err(f) => shell.trouble(f),
        },
        Action::ReadReadNetwork { chain, registry } => match read_read_network(shell, chain, &registry) {
            Ok(Spawned::Started) => Applied::Started(Kind::ReadNet),
            Ok(Spawned::InFlight) => Applied::Refused(Kind::ReadNet),
            Err(f) => shell.trouble(f),
        },
        Action::SetPublish { url } => match set_publish(shell, &url) {
            Ok(url) => Applied::PublishSet { url },
            Err(f) => shell.trouble(f),
        },
        Action::CheckTail => match check_tail(shell) {
            Ok(Spawned::Started) => Applied::Started(Kind::Fetch),
            Ok(Spawned::InFlight) => Applied::Refused(Kind::Fetch),
            Err(f) => shell.trouble(f),
        },
        Action::FetchLedger { from, password } => match fetch_ledger(shell, &from, password) {
            Ok(Spawned::Started) => Applied::Started(Kind::Fetch),
            Ok(Spawned::InFlight) => Applied::Refused(Kind::Fetch),
            Err(f) => shell.trouble(f),
        },
        Action::FetchAside { from, password } => {
            // A background task writing local data could write into this home after its folders are copied and
            // before the swap, so this is refused until such tasks land (as for a master key change).
            if let Some(f) = busy_for_rekey(shell) {
                return shell.trouble(f);
            }
            match fetch_aside(shell, &from, password) {
                Ok(Spawned::Started) => {
                    shell.swapping = true;
                    Applied::Started(Kind::Fetch)
                }
                Ok(Spawned::InFlight) => Applied::Refused(Kind::Fetch),
                Err(f) => shell.trouble(f),
            }
        }
        Action::CheckPublished { local } => match check_published(shell, &local) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Publish),
                Spawned::InFlight => Applied::Refused(Kind::Publish),
            },
            Err(f) => shell.trouble(f),
        },
        Action::CopyGrantCode { grant } => match grant_code(shell, &grant) {
            Ok(text) => Applied::GrantCode { text },
            Err(f) => shell.trouble(f),
        },
        Action::ExportBadge { grant, out } => match export_badge(shell, &grant, &out) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Badge),
                Spawned::InFlight => Applied::Refused(Kind::Badge),
            },
            Err(f) => shell.trouble(f),
        },

        Action::Adopt { dir } => match adopt(shell, &dir) {
            Ok(a) => {
                // An adopted ledger carries its own genesis, so the clock can start.
                shell.rooted = true;
                Applied::AdoptedInPlace {
                    entries: a.sighting_entries,
                    linked: a.linked,
                    label: a.label,
                }
            }
            Err(f) => shell.trouble(f),
        },
    }
}




// Identities.

/// A test hook that runs between writing a key file and reading it back.
///
/// A write that reports success but leaves wrong bytes (disk full mid-write, the target swapped, another key
/// written) cannot be produced from outside, because a successful write reads back what it wrote. This hook
/// lets tests alter the file between the two steps so that case yields `BACKUP_NOT_LANDED`.
///
/// Like `keybox::set_light_kdf`, it can be set only once and only by the test hooks (tests check this); the
/// window never sets it, so in the shipped app it does nothing.
static LANDED_TAMPER: std::sync::OnceLock<fn(&std::path::Path)> = std::sync::OnceLock::new();


// Ledger view.

/// What comes after queueing. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Next {
    /// Auto-anchor on: estimate gas and show the confirmation card.
    Send,
    /// Auto-anchor off: only added to the ledger, waiting for a manual send.
    Wait,
    /// Not queued (queueing failed, already anchored, or a local-only deleted pair): nothing comes next.
    Held,
}

/// The result of queueing: queue length and what comes next.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Enqueued {
    pub queued: usize,
    pub next: Next,
}



// Anchor queue.

/// The anchors of this batch and what is needed to send them (chain id, registry, endpoints on that chain).
struct Batch {
    ids: Vec<String>,
    hashes: Vec<[u8; 32]>,
    chain: u64,
    registry: Address,
    /// All of the chain's endpoints, in settings order.
    urls: Vec<crate::chainx::NodeAddr>,
    /// The retry backoff delays (a shell field tests may change).
    backoff: Vec<std::time::Duration>,
}

/// How long to wait for an anchor to be included. Not being included in time means "not yet", not a verdict,
/// and the anchor stays queued.
pub const ANCHOR_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

mod adopt;
mod anchor;
mod archive;
mod audit;
mod badge;
mod check;
mod readnet;
mod delivery;
mod depth;
mod diligence;
mod genesis;
mod grant;
mod grant_file;
mod grants;
mod home;
mod identity;
pub use self::check::one_place;
pub use self::identity::seats_with_entries;
mod keys;
mod kit;
mod ledger;
mod queue;
mod reader;
mod revoke;
mod sentinel;
mod succeed;
mod vault;
mod verify;
mod wizard;
use self::adopt::*;
use self::anchor::*;
use self::archive::*;
use self::audit::*;
use self::badge::*;
pub use self::check::*;
pub use self::audit::{basis_read, basis_stamp, BasisStamp};
pub use self::readnet::*;
use self::delivery::*;
use self::depth::*;
use self::diligence::*;
use self::genesis::*;
use self::grant::*;
use self::grant_file::*;
use self::grants::*;
pub use self::home::*;
pub use self::identity::*;
use self::keys::*;
use self::kit::*;
use self::ledger::*;
pub use self::queue::*;
use self::reader::*;
use self::revoke::*;
use self::sentinel::*;
use self::succeed::*;
use self::vault::*;
use self::verify::*;
use self::wizard::*;

/// The command line binary shipped beside this program (located only by `zikaron_os::cli_path`).
pub fn cli_beside() -> Option<std::path::PathBuf> {
    zikaron_os::cli_path::beside_this_program()
}

/// What is currently at the command line's location (a missing bundled command line reads as unsupported).
fn cli_path_now() -> zikaron_os::cli_path::State {
    match cli_beside() {
        Some(cli) => zikaron_os::cli_path::state(&cli),
        None => zikaron_os::cli_path::State::Unsupported(String::new()),
    }
}

/// Turns the command line on or off on the task's thread, via the platform module; each refusal is named.
fn cli_path_set(cli: &std::path::Path, on: bool) -> Result<Done, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    use zikaron_os::cli_path::{disable, enable, Refused};
    let got = if on { enable(cli) } else { disable(cli) };
    got.map(Done::CliPath).map_err(|r| match r {
        Refused::Taken(what) => Fault::known(Known::CliPathTaken, what),
        Refused::Cancelled => Fault::known(Known::CliPathCancelled, String::new()),
        Refused::NotAllowed(said) => Fault::known(Known::CliPathNotAllowed, said),
        Refused::Unsupported(why) => Fault::known(Known::CliPathUnsupported, why),
        Refused::TooLong(n) => Fault::known(Known::CliPathNotAllowed, crate::lang::fill1(crate::lang::Key::TailCliPathTooLong, &n.to_string())),
    })
}

/// A grant file was written, as the UI shows it.
fn grant_file_exported(x: crate::grantfilex::Exported) -> Applied {
    Applied::GrantFileExported { path: x.path.display().to_string(), why: x.chosen.why, hops: x.hops, terms: x.terms, ledger: x.ledger, files: x.files }
}
