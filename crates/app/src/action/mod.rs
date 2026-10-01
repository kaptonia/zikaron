//! Every click in the window and every command of the test hooks goes through here.
//!
//! Tests drive the test hooks and people use the window; if each wrote its own actions, what is tested would
//! not be what is used. Actions are one closed enum and applying them is one function, so both paths run the
//! same code.
//!
//! `verb()` names the command-line verb a legal action is equivalent to (`CLI-SCHEMA.md` §6). The tests check
//! that every action with `is_legal()` true has a `verb()`, and that every named verb is in the table.

use crate::feature::Feature;
use crate::key::Address;
use crate::keystore::Params;
use crate::shell::{Page, Shell};
use crate::task::{Done, Kind, Reaped, Spawned};
use crate::trace;

/// The three forms of identity import. `Debug` prints no content: words, private keys and passwords never
/// reach a log line; all three are secret types.
#[derive(Clone, PartialEq, Eq)]
pub enum ImportForm {
    Words(crate::secret::Secret),
    PrivateKey(crate::secret::Secret),
    Keystore { path: String, password: crate::secret::Secret },
}

impl std::fmt::Debug for ImportForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportForm::Words(_) => f.write_str("Words(…)"),
            ImportForm::PrivateKey(_) => f.write_str("PrivateKey(…)"),
            ImportForm::Keystore { path, .. } => write!(f, "Keystore {{ path: {path:?}, password: … }}"),
        }
    }
}

/// The closed set names itself: this macro declares `Action` as written and lists each member's name in
/// declaration order in [`Action::NAMES`], so the member count is a compile-time fact and the name list is
/// never copied.
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
    /// Measure the home once (background): walk the disk and ask the ledger. The frame never touches the
    /// disk, so this is a background task.
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
    /// Confirm the copy: three random cells filled back; a match creates the identity. `label` is an optional
    /// note (for recognition only; no decision reads it).
    ConfirmIdentity { answers: Vec<(usize, crate::secret::Secret)>, label: String },
    /// Abandon: wipe the words in memory.
    DropFresh,
    /// Import an identity (recovery words, private key, or keystore file plus password). `seat` is the seat
    /// an existing key takes (only that one; the other stays empty); recovery words take both seats, and
    /// there `seat` only reads the current identity table. `label` is an optional note.
    ImportIdentity { form: ImportForm, seat: crate::roles::Role, label: String },
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
    /// Make this identity the primary one (the only one that recovers the passcode). The passcode opens the
    /// vault first; a new master key, every key and local file resealed (background, `rekey::set_primary`).
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
    /// Right after unlocking: what the locked time missed, once (the self-audit, the vault review and its
    /// sentinel, the receipt wait; the notices they raise ring once).
    CatchUp,
    /// Choose a network for this machine (a row name from the known deployments table, or `deploy::CUSTOM`).
    /// Written to machine settings; the current home, if it has no chain yet, is filled from that row.
    ChooseNetwork { name: String },
    /// "Use this machine's default network" for an older home: fill it from the row this machine chose
    /// (without one, the table's default row, recorded).
    UseMachineNetwork,
    // ── Archive and single writer ──
    /// Open a home (creating it if missing) and take the writer lock.
    OpenHome { root: String },
    /// Move the home to another path.
    MigrateHome { to: String },
    /// Change the size cap.
    SetCap { bytes: u64 },
    // ── Mirror and restore ──
    /// Write a mirror bundle.
    ExportMirror { to: String },
    /// Reconcile once (background): assemble the audit input for the core. Only COMPLETE releases the pen.
    Reconcile,
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
    /// Record the three basis fields (law §9.4).
    SetBasis { chain: String, registry: String, from_block: String },
    /// Change the self-audit period.
    SetAuditEvery { secs: u64 },
    /// The auto-anchor setting (per home, off by default).
    SetAutoAnchor { on: bool },
    // Anchoring desk.
    /// Take a content hash (one of three entry points).
    TakeContent { source: crate::anchorx::Source, path: String },
    /// Write history entries. A legal action (verb `history`). With `files` empty, one entry for the content
    /// at hand; otherwise a batch: one entry per file, signed one by one, stopping at the first failure
    /// (signed entries stay). `for_` is the optional "recorded for", written into each body as is (parent law
    /// §6.10; this desk does not read it).
    RecordWork { note_md: String, files: Vec<String>, for_: Option<crate::anchorx::For> },
    /// Check a file: compute its digest, read the ledger now, and answer which entry it is and which
    /// transaction anchored it, or that this ledger has no such digest.
    VerifyFile { path: String },
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
    // Disclosure kits.
    /// Pick once: range and record hash fields (disk walked in the background).
    PickKit { from: String, to: String, ids: String },
    /// Write a kit (background): pick, attach, hand to the kit output crate to lay out and self-verify; lands
    /// only on KIT_OK.
    ExportKit { from: String, to: String, ids: String, attach: String, note: String, out: String },
    /// Digest attachments first: the paths dropped on the export page, each digested in the background in
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
    /// genesis never is and entries whose queueing failed are not either; without this, those entries showed
    /// "not on chain" with nothing to press.
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
    /// that key's anchors (the same reading as others' ledgers). Each row is checked against the three
    /// questions and its state returned.
    ListKeyAnchors { address: String },
    /// Read a claim someone sent (the attesting side): who claims, which anchors, their ledger head; with a
    /// network configured, ask each anchor's block in the background.
    ReadClaim { text: String },
    /// Attest for someone: pass the local passcode, sign the claim text's preimage with this seat's key (the
    /// law §6.6 domain), return the signature.
    AttestFor { text: String, pin: crate::secret::Secret },
    // Succession.
    /// Scan the new key once (background): anchors it sent are red.
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
    /// After fetching found the fetched ledger and this one at odds (the same place, other contents), and the
    /// person said yes: this seat's home is set aside whole (kept, readable, never written to or let out), and
    /// a fresh one in its place receives the fetched ledger. `from` and `password` as for `FetchLedger`.
    FetchAside { from: String, password: crate::secret::Secret },
    /// Open old data (a home set aside after a conflict) to read it; where this machine was before is kept
    /// for coming back. The machine pointer does not move.
    ViewOldData { root: String },
    /// Leave old data for the home open before it.
    LeaveOldData,
}

}

/// Where a restore from a backup comes from. Closed: the three entries.
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
            _ => {}
        }
    }

    /// Which actions ask for the local passcode (exporting a key file, deleting an identity, showing the
    /// words).
    ///
    /// "Using a key passes the passcode gate" is carried by this table and the one check in [`apply`]. The
    /// passcode has one gate ([`crate::keybox::unlock`]); a second would be another way around and another
    /// failure count. Export and delete use the same gate, and five failures lock the same vault. A new
    /// passcode-protected action adds a row here and nothing else changes.
    ///
    /// The gate's own actions (set, unlock, change, the two recoveries) are not in the table: they are the
    /// gate, each with its own rules. The tests check that every action with a `pin` field is either here or
    /// in the list below.
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
            | Action::UseMachineNetwork
            | Action::ChangePin { .. }
            | Action::RecoverWords { .. }
            | Action::RecoverKeystore { .. }
            | Action::HideWords
            | Action::OpenHome { .. }
            | Action::ViewOldData { .. }
            | Action::LeaveOldData
            | Action::MigrateHome { .. }
            | Action::SetCap { .. }
            | Action::ExportMirror { .. }
            | Action::Reconcile
            | Action::ReadChain
            | Action::SetEndpoints { .. }
            | Action::Adopt { .. }
            | Action::ReadLedger
            | Action::OpenEntry { .. }
            | Action::Audit
            | Action::SetBasis { .. }
            | Action::SetAuditEvery { .. }
            | Action::SetAutoAnchor { .. }
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
            | Action::ExportBadge { .. }
            | Action::ExportGrantFile { .. }
            | Action::SetPublish { .. }
            | Action::CheckPublished { .. }
            | Action::FetchLedger { .. }
            | Action::FetchAside { .. }
            | Action::RememberAddress { .. }
            | Action::ForgetAddress { .. }
            | Action::VerifyFile { .. }
            | Action::SetKitLink { .. }
            | Action::DropKitCopy { .. } => false,
        }
    }

    /// The background work that reads local data: the self-audit clock, the vault review and its sentinel, the
    /// queue's sending and its receipt wait, and the catch-up after unlocking. Closed; while locked each is
    /// refused here with `LOCKED` (the window does not ask while locked either; this is the rule itself).
    pub fn halts_when_locked(&self) -> bool {
        matches!(self, Action::Audit | Action::ReviewVault | Action::SendBatch { .. } | Action::Resume | Action::CatchUp)
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
            Action::OpenHome { .. } | Action::MigrateHome { .. } | Action::SetCap { .. } => Feature::H3,
            Action::ViewOldData { .. } | Action::LeaveOldData => Feature::H8,
            Action::Measure => Feature::H3,
            Action::ExportMirror { .. }
            | Action::Reconcile
            | Action::ReadChain
            | Action::SetEndpoints { .. }
            | Action::ChooseNetwork { .. }
            | Action::UseMachineNetwork => Feature::H4,
            Action::Genesis { .. } | Action::Adopt { .. } => Feature::H5,
            Action::ReadLedger | Action::OpenEntry { .. } | Action::Annotate { .. } | Action::Retract { .. } => Feature::W1,
            Action::Audit | Action::SetBasis { .. } | Action::SetAuditEvery { .. } => Feature::W2,
            Action::QueueEntry { .. } | Action::SetAutoAnchor { .. } => Feature::W4,
            Action::TakeContent { .. }
            | Action::RecordWork { .. }
            | Action::RegisterRepo { .. }
            | Action::CheckRepo
            | Action::TakeDropped { .. }
            | Action::VerifyFile { .. } => Feature::W3,
            Action::EstimateGas { .. } | Action::SendBatch { .. } => Feature::W4,
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
            Action::SetAppearance { .. } => Feature::H0,
            Action::ExportBadge { .. } => Feature::D9,
            Action::ExportGrantFile { .. } => Feature::D6,
            Action::SetPublish { .. } | Action::CheckPublished { .. } => Feature::W14,
            Action::FetchLedger { .. } => Feature::H8,
            Action::FetchAside { .. } => Feature::H8,
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
            Action::UseMachineNetwork => None,
            Action::OpenHome { .. } => None,
            Action::ViewOldData { .. } => None,
            Action::LeaveOldData => None,
            Action::MigrateHome { .. } => None,
            Action::SetCap { .. } => None,
            Action::ExportMirror { .. } => Some(Exit::Mirror),
            Action::Reconcile => None,
            Action::ReadChain => None,
            Action::SetEndpoints { .. } => None,
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
            Action::TakeContent { .. } => None,
            Action::RecordWork { .. } => None,
            Action::VerifyFile { .. } => None,
            Action::RegisterRepo { .. } => None,
            Action::SetKitLink { .. } => None,
            Action::DropKitCopy { .. } => None,
            Action::CheckRepo => None,
            Action::TakeDropped { .. } => None,
            Action::EstimateGas { .. } => None,
            Action::SendBatch { .. } => Some(Exit::Send),
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
            Action::ExportBadge { .. } => Some(Exit::Badge),
            Action::ExportGrantFile { .. } => Some(Exit::GrantFile),
            Action::SetPublish { .. } => None,
            Action::CheckPublished { .. } => None,
            Action::FetchLedger { .. } => None,
            Action::FetchAside { .. } => None,
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
                | Action::AdoptAnchors { .. }
                | Action::Succeed { .. }
                | Action::Cosign { .. }
        )
    }

    /// Whether an action is legal: the family that touches the anchor key, the ledger or the chain.
    ///
    /// Moving a home and settings are not: they move the same bytes or change local preferences and create no
    /// new fact under the law (any copy is equivalent).
    pub fn is_legal(&self) -> bool {
        matches!(
            self,
            Action::MakeAnchorKey
                | Action::Genesis { .. }
                | Action::Annotate { .. }
                | Action::Retract { .. }
                | Action::RecordWork { .. }
                | Action::SendBatch { .. }
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
            // This column follows the verb table (`CLI-SCHEMA.md` §6), not the law's entry types: the two
            // tables are different sources that happen to share one spelling. The command line is the source
            // of the verb table and the app does not depend on it, so this is a second copy (like envelope
            // keys, see `entryx`); the tests read the table from `CLI-SCHEMA.md` and check every row.
            Action::RecordWork { .. } => Some("history"),
            Action::SendBatch { .. } => Some("anchor"),
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
    /// A network row was chosen; `filled` means the current home was filled from it too.
    NetworkChosen { name: String, filled: bool },
    /// This home now uses the network row the machine chose.
    NetworkAdopted { name: String },
    /// The auto-anchor setting was saved.
    AutoAnchor(bool),
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
    /// The answer of a file check.
    FileVerdict(Box<crate::recordsx::Verdict>),
    /// A kit's `link` changed (`None`: back to the default with no publication base configured).
    KitLinked { path: String, link: Option<String> },
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
    /// Grants went into the vault (a payload may hold several), with the first id.
    Held { ids: Vec<String> },
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

/// Whether `apply` holds this action back now, and by which refusal: the one judgment at the entry of the
/// action layer. The window asks it too before a timed background action (so a refusal it knows is coming is
/// not sent every tick), and never carries a copy of these rules itself.
pub fn held_back(shell: &Shell, a: &Action) -> Option<crate::fault::Known> {
    // While the master key is being changed (making another identity primary, restoring from a backup) every
    // other action waits: the screen says it is resealing.
    if shell.rekeying && !matches!(a, Action::Show(_) | Action::Quit | Action::SelfCheck) {
        return Some(crate::fault::Known::Rekeying);
    }
    // While a home is being swapped for a fresh one (fetch and replace) it is frozen: whatever is written to it
    // now would be copied or not by chance and could end up only in the old data. Reading passes.
    if shell.swapping && !a.reads_only() {
        return Some(crate::fault::Known::Rekeying);
    }
    // Old data open: it is read, never written, fetched into, moved or let out (only reading and coming back).
    if shell.old_view.is_some() && !a.reads_old_data() {
        return Some(crate::fault::Known::ReadOnly);
    }
    // Locked means the background work that reads local data stops (refused, named; resumed by `CatchUp`).
    if a.halts_when_locked() && !shell.unlocked() {
        return Some(crate::fault::Known::Locked);
    }
    // Locked means refused: key-using actions stop here without touching the disk.
    if a.needs_key() && !shell.unlocked() {
        return Some(crate::fault::Known::Locked);
    }
    None
}

/// The exit gate for the exits that run in the frame (their last step before writing); a refusal that leaves
/// this home marked is taken into the shell at once.
/// The exports that write in the frame (a grant file, a record bundle) pass the exit gate in the background
/// first: reading the chain waits on nodes, and the window never waits for the network. When the gate
/// passes, the export runs where the result lands ([`gate_landed`]); a refusal lands as a trouble, and the
/// home takes its read-only mark there (`Shell::gate_refused`).
fn gate_first(shell: &mut Shell, a: Action) -> Applied {
    let ask = match crate::exitgate::ask_of(shell) {
        Ok(x) => x,
        Err(f) => return shell.trouble(f),
    };
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf());
    match shell.tasks.spawn(Kind::Gate, move || {
        crate::exitgate::pass(&ask)?;
        Ok(Done::GatePassed { root, then: Box::new(a) })
    }) {
        Spawned::Started => Applied::Started(Kind::Gate),
        Spawned::InFlight => Applied::Refused(Kind::Gate),
    }
}

/// An export's exit gate passed for the home at `root`, started on a source that `stale` says has since moved
/// or not. The pass holds only for that home and that source: then the export runs now. Otherwise the pass is
/// void here: when another press of an export is being gated already, that gate answers for itself and this
/// one says nothing (`None`); if not, the export is applied again through the one entry, refused there as any
/// press would be (locked, no home) or gated again for the home and source now open.
pub fn gate_landed(shell: &mut Shell, root: Option<std::path::PathBuf>, then: Action, stale: bool) -> Option<Applied> {
    let here = shell.home.as_ref().map(|h| h.root().to_path_buf());
    if stale || here != root {
        if shell.tasks.in_flight(Kind::Gate) {
            return None;
        }
        return Some(apply(shell, then));
    }
    shell.gate_cleared = true;
    let said = apply(shell, then);
    shell.gate_cleared = false;
    Some(said)
}

/// Apply. The one place the window and the test hooks share.
pub fn apply(shell: &mut Shell, a: Action) -> Applied {
    trace::mark(a.feature());
    if let Some(k) = held_back(shell, &a) {
        return shell.trouble(crate::fault::Fault::known(k, String::new()));
    }
    // Using a key passes the passcode gate: actions that carry a passcode pass `keybox::unlock` here. One
    // gate, one failure count: each failure is recorded on disk, and five lock the vault, leaving only
    // recovery.
    //
    // This comes after "locked means refused": a locked vault still refuses every key-using action by the
    // closed table, and this gate re-identifies the person on an open vault (someone at an unlocked machine
    // cannot export the key or delete an identity).
    //
    // Key derivation runs in the background: the unlock runs in a `Kind::Vault` task, and only when it
    // succeeds does the action body run where the result lands (`vault_landed` then `body`); the passcode
    // field is wiped before it is handed on.
    if let Some(pin) = a.pin_asked() {
        let pin = pin.clone();
        let mut rest = a;
        rest.forget_pin();
        return vault(shell, move || {
            crate::keybox::unlock(pin.expose())?;
            Ok(crate::task::Vault::Gate(Box::new(rest)))
        });
    }
    body(shell, a)
}

/// Start a background task for a passcode action's key derivation. Single flight: in flight answers "in
/// flight" by name and no second derivation is queued.
fn vault(shell: &mut Shell, work: impl FnOnce() -> Result<crate::task::Vault, crate::fault::Fault> + Send + 'static) -> Applied {
    match shell.tasks.spawn(Kind::Vault, move || work().map(Done::Vault)) {
        Spawned::Started => Applied::Started(Kind::Vault),
        Spawned::InFlight => Applied::Refused(Kind::Vault),
    }
}

/// A passcode task landed; continue with the frame half. Called where the shell receives results
/// (`Shell::drain_at`); success and refusal both return an `Applied`, the same sentence as if done in the
/// frame.
pub fn vault_landed(shell: &mut Shell, got: Result<crate::task::Vault, crate::fault::Fault>) -> Applied {
    use crate::task::Vault;
    match got {
        Ok(Vault::Opened) => {
            shell.after_unlock();
            // When the vault opened but the reseal did not land, say so: the next start will ask the same
            // passcode again, and the person should know why (disk full, read-only directory).
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
            // After any failure, reread the vault state. The final failure locks the vault; a shell still
            // showing "locked, four failures" would keep the eight cells and "0 tries left" with no sixth try
            // left, so the screen follows the disk and the final failure goes through the full lock path.
            after_vault_shut(shell);
            shell.trouble(f)
        }
    }
}

/// A master key change (set as primary, restore) reseals every local file, and fetch and replace copies a
/// home's rooms before swapping it: a background task that writes local data from its own thread
/// (`Kind::writes_local`) could write under the key being replaced, or into the home after its rooms were
/// copied, so each is refused until those land.
fn busy_for_rekey(shell: &Shell) -> Option<crate::fault::Fault> {
    let others: Vec<Kind> = shell.tasks.flying().into_iter().filter(|k| k.writes_local()).collect();
    (!others.is_empty()).then(|| {
        crate::fault::Fault::known(crate::fault::Known::BusyForRekey, others.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(" "))
    })
}

/// Apply for the test hooks: the same as [`apply`], but when a passcode action starts a background task, wait
/// for it to land and return that `Applied` (a test step is one whole path, not spread over frames). Other results
/// received while waiting are recorded as usual and left for the next receive.
pub fn apply_settled(shell: &mut Shell, a: Action) -> Applied {
    let first = apply(shell, a);
    if first != Applied::Started(Kind::Vault) {
        return first;
    }
    loop {
        if let Some(done) = shell.vault_said.take() {
            return done;
        }
        // Its worker finished before this drain and the pass is still in flight after it: the outcome never
        // came back, and this says so by name instead of waiting forever.
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

/// The action body (after "locked means refused" and the passcode gate).
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
        Action::Quit => Applied::Stopped(shell.tasks.shutdown()),

        Action::MakeAnchorKey => match make_anchor_key(shell) {
            Ok(a) => Applied::AnchorKey(a),
            Err(f) => shell.trouble(f),
        },
        Action::SwitchRole => match switch_role(shell) {
            Ok(r) => Applied::Seated(r),
            Err(f) => shell.trouble(f),
        },
        Action::ReadIdentities => match crate::identity::view(shell.settings.role) {
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
        Action::ConfirmIdentity { answers, label } => match confirm_identity(shell, &answers, &label) {
            Ok(started) => started,
            Err(f) => shell.trouble(f),
        },
        Action::DropFresh => {
            shell.new_words = None;
            Applied::FreshDropped
        }
        Action::ImportIdentity { form, seat, label } => match import_identity(shell, form, seat, &label) {
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
                // A new passcode makes a new master key: files sealed under a key store that is gone are moved
                // aside first (they could never open, and left in place they stop their home from opening).
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
        // After opening, in the same background task: staged files settle, plain files of an older version are
        // sealed, an older vault's primary identity is settled (`local::after_open`).
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
                // The file is gone, so the vault is now `Absent`: key, words and address are wiped from the
                // shell as for a lock, the vault state is reread, the gate closes (`gate_up` answers false
                // for `Absent`) and the wizard lands on step 1.
                shell.after_lock();
                Applied::KeyboxReset
            }
            Err(f) => shell.trouble(f),
        },
        Action::Lock => {
            // A task that writes local data is still running: the lock takes hold on screen now (the gate is
            // up and every action is refused as locked) and completes (the master key wiped, the home closed)
            // when those tasks have landed, so a write already under way, a broadcast anchor's queue mark
            // above all, is never lost to a missing key.
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
            // The old passcode in a change goes through the same failure count: the final failure locks this
            // session in place, and the screen follows the disk where the result lands.
            vault(shell, move || crate::keybox::change_pin(old.expose(), pin.expose()).map(|_| crate::task::Vault::Changed))
        }
        Action::RecoverWords { words, pin, again } => {
            if let Err(f) = same_twice(&pin, &again) {
                return shell.trouble(f);
            }
            // Words pass the core's word list and checksum first (a malformed phrase is refused by name,
            // never tried against the vault as bytes); entropy never leaves the identity layer.
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
                let secret = crate::keystore::decrypt(&bytes, password.expose())?;
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
            shell.words = None;
            Applied::WordsHidden
        }
        Action::ChooseNetwork { name } => match choose_network(shell, &name) {
            Ok(filled) => Applied::NetworkChosen { name, filled },
            Err(f) => shell.trouble(f),
        },
        Action::UseMachineNetwork => match use_machine_network(shell) {
            Ok(name) => Applied::NetworkAdopted { name },
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
        Action::ViewOldData { root } => match view_old(shell, &root) {
            Ok(mode) => Applied::Homed { root, mode },
            Err(f) => shell.trouble(f),
        },
        Action::LeaveOldData => match leave_old(shell) {
            Ok((root, mode)) => Applied::Homed { root, mode },
            Err(f) => shell.trouble(f),
        },
        Action::MigrateHome { to } => match migrate(shell, &to) {
            Ok(root) => Applied::Migrated { root },
            Err(f) => shell.trouble(f),
        },
        Action::SetCap { bytes } => match set_cap(shell, bytes) {
            Ok(n) => Applied::Capped(n),
            Err(f) => shell.trouble(f),
        },

        // What can be refused without reading the chain is refused before the exit gate starts (the same plan
        // is asked again where the gate landed, on the state then).
        Action::ExportMirror { to } if !shell.gate_cleared => match mirror_plan(shell, &to) {
            Ok(_) => gate_first(shell, Action::ExportMirror { to }),
            Err(f) => shell.trouble(f),
        },
        Action::ExportMirror { to } => match export_mirror(shell, &to) {
            Ok((path, entries, added, topped_up)) => Applied::Mirrored { path, entries, added, topped_up },
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
            // The source of the anchored color is the last audit's report; with no audit yet there is no
            // report and no green lamp.
            let report = shell.audit.as_ref().map(|a| a.report.clone());
            // The last pass's anchored set (disk cache): until a report arrives, a "checked last time" lamp,
            // never presented as anchored.
            let remembered = shell.remembered.clone();
            let wall = (shell.clock)();
            // Lamps are computed from the queue file: which step, which block, the same source on every page.
            let queue = shell.queue.clone();
            let mine = shell.anchor;
            let gen = shell.rows_gen;
            // The first-anchor block times come from the chain fragment of the last self-audit.
            let fragment = shell.audit.as_ref().map(|a| a.fragment.clone());
            match shell.tasks.spawn(Kind::Ledger, move || {
                let home = crate::home::Home::open(&root)?;
                let mut t = crate::ledgerx::table_remembering(&home, report.as_ref(), &queue, remembered.as_ref(), wall)?;
                // Who writes this ledger from now on: once a succession is in the ledger, this desk has
                // handed it over (law §7.3). Read in the same pass as the table, so the notice and the table
                // speak of the same moment.
                let pile = home
                    .ledger()?
                    .pile()?;
                let handed = crate::succeedx::handed_over(&pile.items, mine);
                if let Some(f) = &fragment {
                    crate::ledgerx::stamp(&mut t.rows, &pile.items, f);
                }
                Ok(Done::Ledger { rows: t.rows, strays: t.strays, handed, gen })
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
        Action::SetBasis { chain, registry, from_block } => {
            match set_basis(shell, &chain, &registry, &from_block) {
                Ok(c) => Applied::Basis { chain: c },
                Err(f) => shell.trouble(f),
            }
        }
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
        // ── Anchoring desk ──
        Action::TakeContent { source, path } => {
            match crate::anchorx::of(source, std::path::Path::new(path.trim())) {
                Ok(c) => {
                    let hex = c.hex();
                    shell.content = Some(c);
                    // A new content was taken; the previous three-step flow is void, since a half-green flow
                    // left on screen would be a silent failure.
                    shell.flow = crate::anchorx::Flow::default();
                    Applied::Took { source, hex }
                }
                Err(f) => shell.trouble(f),
            }
        }
        Action::RecordWork { note_md, files, for_ } if files.is_empty() => {
            match record_work(shell, &note_md, for_.as_ref()) {
                Ok((id, n)) => Applied::Recorded { id, queued: n.queued, next: n.next },
                Err(f) => shell.trouble(f),
            }
        }
        Action::RecordWork { note_md, files, for_ } => {
            let (ids, stopped, n) = record_files(shell, &note_md, &files, for_.as_ref());
            if let Some((_, _, f)) = &stopped {
                shell.faults.push(f.clone());
            }
            Applied::RecordedBatch { ids, stopped, queued: n.queued, next: n.next }
        }
        Action::VerifyFile { path } => match verify_file(shell, &path) {
            Ok(v) => Applied::FileVerdict(Box::new(v)),
            Err(f) => shell.trouble(f),
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
        Action::TakeDropped { path } => match take_dropped(shell, &path) {
            Ok((source, hex)) => {
                Applied::Took { source, hex }
            }
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
        Action::EstimateGas { count } => match estimate_gas(shell, count) {
            Ok((g, calldata, fees)) => {
                shell.gas = Some((count, g));
                shell.fees = Some(fees);
                Applied::Gas { count, gas: g, calldata }
            }
            Err(f) => {
                // Without an estimate this batch cannot be sent: clear the last reading so an old number
                // cannot release a new batch.
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
            let gen = shell.grants_gen;
            match shell.tasks.spawn(Kind::Grants, move || {
                let home = crate::home::Home::open(&root)?;
                Ok(Done::Grants { gen, rows: crate::grantx::table(&home, &flags)? })
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
        // ── First-window checklist ──
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
            Ok(ids) => Applied::Held { ids },
            Err(f) => shell.trouble(f),
        },
        Action::ImportGrantDir { dir } => match import_grant_dir(shell, &dir) {
            Ok((ids, refused)) if refused.is_empty() => Applied::Held { ids },
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
        Action::ListHeld => match list_held(shell) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Held),
                Spawned::InFlight => Applied::Refused(Kind::Held),
            },
            Err(f) => shell.trouble(f),
        },
        Action::ExportGrantFile { id, to } if !shell.gate_cleared => match grant_file_plan(shell, &id, &to) {
            Ok(_) => gate_first(shell, Action::ExportGrantFile { id, to }),
            Err(f) => shell.trouble(f),
        },
        Action::ExportGrantFile { id, to } => match export_grant_file(shell, &id, &to) {
            Ok(x) => Applied::GrantFileExported {
                path: x.path.display().to_string(),
                why: x.chosen.why,
                hops: x.hops,
                terms: x.terms,
                ledger: x.ledger,
                files: x.files,
            },
            Err(f) => shell.trouble(f),
        },
        Action::SetPublish { url } => match set_publish(shell, &url) {
            Ok(url) => Applied::PublishSet { url },
            Err(f) => shell.trouble(f),
        },
        Action::FetchLedger { from, password } => match fetch_ledger(shell, &from, password) {
            Ok(Spawned::Started) => Applied::Started(Kind::Fetch),
            Ok(Spawned::InFlight) => Applied::Refused(Kind::Fetch),
            Err(f) => shell.trouble(f),
        },
        Action::FetchAside { from, password } => {
            // A background task writing local data could write into this home after its rooms are copied and
            // before the swap: refused until those land (as for a master key change).
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

/// A test hook between landing a file and reading it back.
///
/// "Landing said yes but the bytes are wrong" (disk full after half a write, the location swapped, another
/// key written) cannot be produced from outside: a landing that succeeds reads back exactly what it wrote.
/// This hook lets tests touch the file between the two steps, so that case can be produced and answers
/// `BACKUP_NOT_LANDED`.
///
/// As with `keybox::set_light_kdf`: it can be set once, and only the test hooks set it (the tests scan for
/// it); the window never does, so in the shipped app this step is empty.
static LANDED_TAMPER: std::sync::OnceLock<fn(&std::path::Path)> = std::sync::OnceLock::new();


// Ledger view.

/// What comes after queueing. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Next {
    /// Auto-anchor on: estimate gas and show the confirmation card.
    Send,
    /// Auto-anchor off: only added to the ledger, waiting for a manual send.
    Wait,
    /// Not queued (queueing failed, it was anchored before, or it is a local deleted pair): nothing comes
    /// next.
    Held,
}

/// The result of queueing: queue length and what comes next.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Enqueued {
    pub queued: usize,
    pub next: Next,
}



// Anchor queue.

/// The anchors of this batch and the three things needed to send them (chain id, registry, an endpoint on
/// that chain).
struct Batch {
    ids: Vec<String>,
    hashes: Vec<[u8; 32]>,
    chain: u64,
    registry: Address,
    /// The chain's whole endpoint table, in settings order.
    urls: Vec<String>,
    /// The backoff deadlines (a shell field the tests may change).
    backoff: Vec<std::time::Duration>,
}

/// How long to wait for an anchor to be included. Six seconds is shorter than one block on any real chain, so
/// this is generous; not included in time is "not yet", not a verdict, and the anchor stays queued.
pub const ANCHOR_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

mod adopt;
mod anchor;
mod archive;
mod audit;
mod badge;
mod check;
mod delivery;
mod depth;
mod diligence;
mod genesis;
mod grant;
mod grant_file;
mod grants;
mod home;
mod identity;
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
