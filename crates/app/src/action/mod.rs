use crate::feature::Feature;
use crate::key::Address;
use crate::keystore::Params;
use crate::shell::{Page, Shell};
use crate::task::{Done, Kind, Reaped, Spawned};
use crate::trace;

#[derive(Clone, PartialEq, Eq)]
pub enum ImportForm {
    Words(crate::secret::Secret),
    PrivateKey { key: crate::secret::Secret, keyfile: Option<KeyFileOut> },
    Keystore { path: String, password: crate::secret::Secret },
}

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

macro_rules! closed_actions {
    ($(#[$em:meta])* pub enum $closed:ident { $( $(#[$m:meta])* $name:ident $( ( $($tup:tt)* ) )? $( { $($body:tt)* } )? ),* $(,)? }) => {
        $(#[$em])*
        pub enum $closed { $( $(#[$m])* $name $( ( $($tup)* ) )? $( { $($body)* } )? ),* }

        impl $closed {
            pub const NAMES: &'static [&'static str] = &[$(stringify!($name)),*];

            pub fn name(&self) -> &'static str {
                match self {
                    $( $closed::$name { .. } => stringify!($name), )*
                }
            }
        }
    };
}

pub fn recount_audit(shell: &mut Shell) -> bool {
    audit::recount(shell)
}

closed_actions! {
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Action {
    Show(Page),
    SelfCheck,
    Measure,
    Quit,
    SwitchRole,
    ReadIdentities,
    NewIdentity,
    ConfirmIdentity { answers: Vec<(usize, crate::secret::Secret)>, label: String, network: String },
    DropFresh,
    ImportIdentity { form: ImportForm, seat: crate::roles::Role, label: String, network: String },
    SwitchIdentity { id: String },
    DeleteIdentity { id: String, pin: crate::secret::Secret },
    NameIdentity { id: String, label: String },
    SetPin { pin: crate::secret::Secret, again: crate::secret::Secret },
    Unlock { pin: crate::secret::Secret },
    Reseal { pin: crate::secret::Secret },
    ResetEmptyKeybox,
    SetMachineAside,
    Lock,
    ChangePin { old: crate::secret::Secret, pin: crate::secret::Secret, again: crate::secret::Secret },
    RecoverWords { words: crate::secret::Secret, pin: crate::secret::Secret, again: crate::secret::Secret },
    RecoverKeystore { path: String, password: crate::secret::Secret, pin: crate::secret::Secret, again: crate::secret::Secret },
    RevealWords { pin: crate::secret::Secret },
    HideWords,
    BackupKey { pin: crate::secret::Secret, password: crate::secret::Secret, again: crate::secret::Secret, dir: String },
    SetAutoLock { on: bool, secs: u64 },
    SetPrimary { id: String, pin: crate::secret::Secret },
    ExportBackup { pin: crate::secret::Secret, password: crate::secret::Secret, again: crate::secret::Secret, dir: String },
    PeekBackup { path: String, password: crate::secret::Secret },
    RestoreBackup { path: String, password: crate::secret::Secret, how: RestoreHow },
    Resume,
    CatchUp,
    ChooseNetwork { name: String },
    OpenHome { root: String },
    ChangeHome { root: String },
    MigrateHome { to: String },
    SetCap { bytes: u64 },
    ExportMirror { to: String },
    Reconcile,
    RecheckAll,
    ReadChain,
    ReadChainTime,
    SetEndpoints { specs: String },
    SetLogAsks { n: u64 },
    Genesis { statement: String },
    Adopt { dir: String },
    ReadLedger,
    OpenEntry { id: String },
    Annotate { subject: String, note_md: String },
    Retract { subject: String, note_md: String },
    Audit,
    SetBasis { chain: String, registry: String, from_block: String },
    MoveChain { chain: String, registry: String, from_block: String, nodes: String },
    SetAuditEvery { secs: u64 },
    SetAutoAnchor { on: bool },
    SetHideLocalDeletions { on: bool },
    TakeContent { source: crate::anchorx::Source, path: String },
    RecordWork { note_md: String, files: Vec<String>, for_: Option<crate::anchorx::For> },
    RegisterRepo { path: String },
    SetKitLink { path: String, link: String },
    DropKitCopy { path: String },
    CheckRepo,
    TakeDropped { path: String },
    EstimateGas { count: usize },
    SendBatch { count: usize },
    BumpFee { tx: String, cap: u64 },
    PickKit { from: String, to: String, ids: String },
    ExportKit { from: String, to: String, ids: String, attach: String, note: String, out: String, form: String, recipient: String, recipient_key: String, originals: String, companion: bool },
    VetAttachments { paths: Vec<String> },
    Peek { ask: crate::peekx::Ask },
    DraftGrant { draft: Box<crate::grantx::Draft>, exclusive: bool, terms_file: Option<String> },
    QueueEntry { id: String },
    ReadGrants,
    CheckClash { work: String, from: String, to: String },
    Revoke { grant: String, case: String },
    MigrateGrants,
    ReadStory { grant: String },
    Upgrade,
    UpgradeCost,
    Rotate { step: RotateStep },
    ReadBook { address: String, dir: String },
    RememberAddress { address: String },
    ForgetAddress { address: String },
    Diligence { address: String, dir: String, work: String, from: String, to: String },
    SaveSnapshot { to: String },
    VerifyWork { path: String, work: String, then_to: String },
    ExtractOriginals { kit: String, to: String },
    CleanExports { yes: String },
    CheckDelivery { path: String, expect: String },
    CheckPayload { typed: String, ledgers: String, endpoints: String, registry: String, from_block: String, now: String, file: String, terms: String },
    AuditCheck,
    AuditOthers,
    ImportGrant { typed: String },
    ImportGrantDir { dir: String },
    SetUpstream { grant: String, dir: String },
    NoteHeld { grant: String, note: String, issuer_note: String },
    ReviewVault,
    ListHeld,
    SetReviewEvery { secs: String },
    SetLang { lang: crate::lang::Lang },
    SetZone { zone: crate::when::Zone },
    SetAppearance { appearance: String },
    ExportBadge { grant: String, out: String },
    CopyGrantCode { grant: String },
    ExportGrantFile { id: String, to: String, original: String, seal_to: String, key: Option<String> },
    SetPublish { url: String },
    CheckPublished { local: String },
    FetchLedger { from: String, password: crate::secret::Secret },
    FetchAside { from: String, password: crate::secret::Secret },
    CheckTail,
    ViewOldData { root: String },
    LeaveOldData,
    TakeWriter,
    SaveReadNetwork { was: Option<(u64, String)>, name: String, chain: String, registry: String, from_block: String, nodes: String },
    RemoveReadNetwork { chain: u64, registry: String },
    ReadReadNetwork { chain: u64, registry: String },
    ReadCliPath,
    SetCliPath { on: bool },
    SetCliAnchor { to: crate::machine::CliAnchor },
    SetCliDesktop { on: bool },
    SetCliAlone { lasts: crate::machine::Lasts, pin: crate::secret::Secret },
    CliAloneOff,
    SendAsked { count: usize },
    SetProxy { choice: String },
    SetRestMode { on: bool },
    AddRest { base: String, key: crate::secret::Secret },
    RemoveRest { base: String },
    FaceReread,
    FaceHome { chain: u64 },
    FaceAccount { platform: String, handle: String, note: String },
    FaceKey { address: String, role: String, note: String, consent: String, sign_here: bool, pin: crate::secret::Secret },
    FaceName { name: String },
    FaceRelease { subject: String },
    FaceReseat { entry: String },
    FaceConsent { identity: String, role: String, pin: crate::secret::Secret },
    CliPassAdd { note: String, pin: crate::secret::Secret },
    CliPassDrop { tag: String },
    FaceBooks,
    FaceReroot { to: String },
    FaceOrigin { from: String },
    FaceRerootMake { answers: Vec<(usize, crate::secret::Secret)>, label: String, chain: u64 },
    FaceRerootFee { go: bool },
    FaceJoined { root: String, to: String },
}

}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoreHow {
    FirstRun,
    Settings { pin: crate::secret::Secret },
    Locked { pin: crate::secret::Secret, again: crate::secret::Secret },
}

impl Action {
    pub fn forget_pin(&mut self) {
        match self {
            Action::BackupKey { pin, .. }
            | Action::DeleteIdentity { pin, .. }
            | Action::RevealWords { pin }
            | Action::ExportBackup { pin, .. }
            | Action::SetCliAlone { pin, .. }
            | Action::FaceKey { pin, .. }
            | Action::FaceConsent { pin, .. }
            | Action::CliPassAdd { pin, .. } => pin.clear(),
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

    pub fn hand_pin(&mut self) -> Option<crate::secret::Secret> {
        let cell = match self {
            Action::BackupKey { pin, .. }
            | Action::DeleteIdentity { pin, .. }
            | Action::RevealWords { pin }
            | Action::ExportBackup { pin, .. }
            | Action::SetCliAlone { pin, .. }
            | Action::FaceKey { pin, .. }
            | Action::FaceConsent { pin, .. }
            | Action::CliPassAdd { pin, .. }
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

    pub fn pin_asked(&self) -> Option<&crate::secret::Secret> {
        match self {
            Action::BackupKey { pin, .. }
            | Action::DeleteIdentity { pin, .. }
            | Action::RevealWords { pin }
            | Action::ExportBackup { pin, .. }
            | Action::SetCliAlone { pin, .. }
            | Action::FaceConsent { pin, .. }
            | Action::CliPassAdd { pin, .. } => Some(pin),
            Action::FaceKey { pin, address, consent, sign_here: true, .. } if consent.trim().is_empty() && !address.trim().starts_with(zikaron_glue::face::CONSENT_PREFIX) => Some(pin),
            Action::SetPrimary { .. } | Action::RestoreBackup { .. } => None,
            Action::SetPin { .. }
            | Action::Unlock { .. }
            | Action::Reseal { .. }
            | Action::ChangePin { .. }
            | Action::RecoverWords { .. }
            | Action::RecoverKeystore { .. } => None,
            _ => None,
        }
    }

    pub fn needs_key(&self) -> bool {
        match self {
            Action::Genesis { .. }
            | Action::RecordWork { .. }
            | Action::Annotate { .. }
            | Action::Retract { .. }
            | Action::DraftGrant { .. }
            | Action::Revoke { .. }
            | Action::MigrateGrants
            | Action::SendBatch { .. }
            | Action::BumpFee { .. }
            | Action::Upgrade
            | Action::Rotate { step: RotateStep::Begin { .. } | RotateStep::Hand { .. } | RotateStep::Declare { .. } | RotateStep::Withdraw { .. } | RotateStep::Write | RotateStep::Accede { .. } | RotateStep::Sweep { .. } | RotateStep::Leave { .. } | RotateStep::Cancel | RotateStep::Enroll { .. } }
            | Action::SetPrimary { .. }
            | Action::ExportBackup { .. }
            | Action::RestoreBackup { how: RestoreHow::FirstRun | RestoreHow::Settings { .. }, .. }
            | Action::ConfirmIdentity { .. }
            | Action::ImportIdentity { .. }
            | Action::DeleteIdentity { .. }
            | Action::BackupKey { .. }
            | Action::SetCliDesktop { .. }
            | Action::SetCliAlone { .. }
            | Action::CliAloneOff
            | Action::AddRest { .. }
            | Action::FaceHome { .. }
            | Action::FaceAccount { .. }
            | Action::FaceKey { .. }
            | Action::FaceName { .. }
            | Action::FaceRelease { .. }
            | Action::FaceReseat { .. }
            | Action::FaceConsent { .. }
            | Action::FaceReroot { .. }
            | Action::FaceOrigin { .. }
            | Action::FaceRerootMake { .. }
            | Action::FaceRerootFee { go: true }
            | Action::CliPassAdd { .. }
            | Action::CliPassDrop { .. } => true,
            Action::RevealWords { .. } => false,
            Action::Rotate { step: RotateStep::Look | RotateStep::Vet { .. } | RotateStep::SweepTo { .. } } | Action::UpgradeCost => false,
            Action::NameIdentity { .. } => false,
            Action::Show(_)
            | Action::SelfCheck
            | Action::Quit
            | Action::Measure
            | Action::ReadIdentities
            | Action::NewIdentity
            | Action::DropFresh
            | Action::SwitchRole
            | Action::SwitchIdentity { .. }
            | Action::SetPin { .. }
            | Action::Unlock { .. }
            | Action::Reseal { .. }
            | Action::ResetEmptyKeybox
            | Action::SetMachineAside
            | Action::Lock
            | Action::SetAutoLock { .. }
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
            | Action::ReadChainTime
            | Action::SetEndpoints { .. }
            | Action::SetLogAsks { .. }
            | Action::SetRestMode { .. }
            | Action::RemoveRest { .. }
            | Action::SaveReadNetwork { .. }
            | Action::RemoveReadNetwork { .. }
            | Action::ReadReadNetwork { .. }
            | Action::Adopt { .. }
            | Action::ReadLedger
            | Action::OpenEntry { .. }
            | Action::Audit
            | Action::SetBasis { .. }
            | Action::MoveChain { .. }
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
            | Action::Peek { .. }
            | Action::ReadGrants
            | Action::CheckClash { .. }
            | Action::ReadStory { .. }
            | Action::ReadBook { .. }
            | Action::FaceReread
            | Action::FaceBooks
            | Action::FaceJoined { .. }
            | Action::FaceRerootFee { go: false }
            | Action::CheckPayload { .. }
            | Action::AuditCheck
            | Action::AuditOthers
            | Action::ImportGrant { .. }
            | Action::ImportGrantDir { .. }
            | Action::SetUpstream { .. }
            | Action::NoteHeld { .. }
            | Action::ReviewVault
            | Action::ListHeld
            | Action::VerifyWork { .. }
            | Action::ExtractOriginals { .. }
            | Action::CleanExports { .. }
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

    pub fn halts_when_locked(&self) -> bool {
        matches!(self, Action::Audit | Action::ReviewVault | Action::SendBatch { .. } | Action::BumpFee { .. } | Action::Resume | Action::CatchUp | Action::FaceReread | Action::FaceBooks | Action::FaceJoined { .. } | Action::FaceRerootFee { .. })
    }

    pub fn feature(&self) -> Feature {
        match self {
            Action::Show(_) | Action::SelfCheck | Action::Quit => Feature::H1,
            Action::SwitchRole
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
            | Action::SetMachineAside
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
            | Action::ReadChainTime
            | Action::SetEndpoints { .. }
            | Action::SetLogAsks { .. }
            | Action::SetRestMode { .. }
            | Action::AddRest { .. }
            | Action::RemoveRest { .. }
            | Action::ChooseNetwork { .. }
            | Action::SaveReadNetwork { .. }
            | Action::RemoveReadNetwork { .. }
            | Action::ReadReadNetwork { .. }
            | Action::SetProxy { .. } => Feature::H4,
            Action::Genesis { .. } | Action::Adopt { .. } => Feature::H5,
            Action::ReadLedger | Action::OpenEntry { .. } | Action::Annotate { .. } | Action::Retract { .. } | Action::SetHideLocalDeletions { .. } => Feature::W1,
            Action::Audit | Action::SetBasis { .. } | Action::MoveChain { .. } | Action::SetAuditEvery { .. } => Feature::W2,
            Action::QueueEntry { .. } | Action::SetAutoAnchor { .. } | Action::SetCliAnchor { .. } | Action::SendAsked { .. } => Feature::W4,
            Action::TakeContent { .. }
            | Action::RecordWork { .. }
            | Action::RegisterRepo { .. }
            | Action::CheckRepo
            | Action::TakeDropped { .. } => Feature::W3,
            Action::EstimateGas { .. } | Action::SendBatch { .. } | Action::BumpFee { .. } => Feature::W4,
            Action::PickKit { .. } | Action::ExportKit { .. } | Action::SetKitLink { .. } | Action::DropKitCopy { .. } | Action::VetAttachments { .. } => Feature::W5,
            Action::Peek { ask } => match ask {
                crate::peekx::Ask::Kit { .. } | crate::peekx::Ask::Names { .. } => Feature::W5,
                crate::peekx::Ask::Files { .. } => Feature::W1,
                crate::peekx::Ask::GrantFile { .. } => Feature::D6,
                crate::peekx::Ask::Landing { .. } => Feature::H3,
                crate::peekx::Ask::Size { .. } => Feature::P1,
            },
            Action::DraftGrant { .. } | Action::MigrateGrants => Feature::W7,
            Action::ReadGrants | Action::CheckClash { .. } => Feature::W8,
            Action::Revoke { .. } | Action::ReadStory { .. } => Feature::W9,
            Action::Upgrade | Action::UpgradeCost | Action::Rotate { .. } => Feature::W11,
            Action::FaceReread
            | Action::FaceBooks
            | Action::FaceReroot { .. }
            | Action::FaceOrigin { .. }
            | Action::FaceRerootMake { .. }
            | Action::FaceRerootFee { .. }
            | Action::FaceJoined { .. }
            | Action::FaceHome { .. }
            | Action::FaceAccount { .. }
            | Action::FaceKey { .. }
            | Action::FaceName { .. }
            | Action::FaceRelease { .. }
            | Action::FaceReseat { .. }
            | Action::FaceConsent { .. } => Feature::H2,
            Action::ReadBook { .. }
            | Action::RememberAddress { .. }
            | Action::ForgetAddress { .. } => Feature::W14,
            Action::Diligence { .. } | Action::SaveSnapshot { .. } | Action::AuditOthers => Feature::D1,
            Action::VerifyWork { .. } => Feature::D2,
            Action::ExtractOriginals { .. } => Feature::D2,
            Action::CleanExports { .. } => Feature::W5,
            Action::CheckDelivery { .. } => Feature::D4,
            Action::ImportGrant { .. } | Action::ImportGrantDir { .. } | Action::SetUpstream { .. } | Action::NoteHeld { .. } | Action::ReviewVault => Feature::D6,
            Action::ListHeld => Feature::D6,
            Action::SetReviewEvery { .. } => Feature::D7,
            Action::SetLang { .. } | Action::SetZone { .. } => Feature::H6,
            Action::ReadCliPath | Action::SetCliPath { .. } | Action::SetCliDesktop { .. } | Action::SetCliAlone { .. } | Action::CliAloneOff | Action::CliPassAdd { .. } | Action::CliPassDrop { .. } => Feature::H6,
            Action::SetAppearance { .. } => Feature::H0,
            Action::ExportBadge { .. } | Action::CopyGrantCode { .. } => Feature::D9,
            Action::ExportGrantFile { .. } => Feature::D6,
            Action::SetPublish { .. } | Action::CheckPublished { .. } => Feature::W14,
            Action::FetchLedger { .. } => Feature::H8,
            Action::FetchAside { .. } => Feature::H8,
            Action::CheckTail => Feature::H8,
            Action::CheckPayload { .. } | Action::AuditCheck => Feature::P1,
        }
    }

    pub fn exit(&self) -> Option<crate::exitgate::Exit> {
        use crate::exitgate::Exit;
        match self {
            Action::Show(..) => None,
            Action::SelfCheck => None,
            Action::Measure => None,
            Action::Quit => None,
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
            Action::SetMachineAside => None,
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
            Action::ReadChainTime => None,
            Action::SetEndpoints { .. } => None,
            Action::SetLogAsks { .. } => None,
            Action::SetRestMode { .. } | Action::AddRest { .. } | Action::RemoveRest { .. } => None,
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
            Action::MoveChain { .. } => None,
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
            Action::BumpFee { .. } => None,
            Action::PickKit { .. } => None,
            Action::ExportKit { .. } => Some(Exit::Kit),
            Action::VetAttachments { .. } => None,
            Action::Peek { .. } => None,
            Action::DraftGrant { .. } => None,
            Action::QueueEntry { .. } => None,
            Action::ReadGrants => None,
            Action::CheckClash { .. } => None,
            Action::Revoke { .. } => None,
            Action::MigrateGrants => None,
            Action::ReadStory { .. } => None,
            Action::Upgrade => None,
            Action::Rotate { .. } => None,
            Action::UpgradeCost => None,
            Action::ReadBook { .. } => None,
            Action::FaceReread | Action::FaceBooks | Action::FaceJoined { .. } | Action::FaceRerootFee { .. } => None,
            Action::FaceReroot { .. } | Action::FaceOrigin { .. } | Action::FaceRerootMake { .. } => None,
            Action::FaceHome { .. }
            | Action::FaceAccount { .. }
            | Action::FaceKey { .. }
            | Action::FaceName { .. }
            | Action::FaceRelease { .. }
            | Action::FaceReseat { .. }
            | Action::FaceConsent { .. } => None,
            Action::RememberAddress { .. } => None,
            Action::ForgetAddress { .. } => None,
            Action::Diligence { .. } => None,
            Action::SaveSnapshot { .. } => None,
            Action::VerifyWork { .. } => None,
            Action::ExtractOriginals { .. } => None,
            Action::CleanExports { .. } => None,
            Action::CheckDelivery { .. } => None,
            Action::CheckPayload { .. } => None,
            Action::AuditCheck => None,
            Action::AuditOthers => None,
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
            Action::SetCliDesktop { .. } | Action::SetCliAlone { .. } | Action::CliAloneOff | Action::CliPassAdd { .. } | Action::CliPassDrop { .. } => None,
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

    pub fn takes_turn(&self) -> bool {
        self.writes_ledger()
            || matches!(
                self,
                Action::QueueEntry { .. } | Action::MoveChain { .. } | Action::CatchUp | Action::Adopt { .. } | Action::FetchLedger { .. } | Action::FetchAside { .. }
            )
    }

    pub fn reads_old_data(&self) -> bool {
        self.reads_only() || matches!(self, Action::Lock | Action::Unlock { .. } | Action::Reseal { .. } | Action::LeaveOldData)
    }

    pub fn writes_ledger(&self) -> bool {
        matches!(
            self,
            Action::Genesis { .. }
                | Action::RecordWork { .. }
                | Action::Annotate { .. }
                | Action::Retract { .. }
                | Action::DraftGrant { .. }
                | Action::Revoke { .. }
                | Action::MigrateGrants
                | Action::SendBatch { .. }
                | Action::BumpFee { .. }
                | Action::Upgrade
                | Action::Rotate { step: RotateStep::Write }
                | Action::FaceHome { .. }
                | Action::FaceAccount { .. }
                | Action::FaceKey { .. }
                | Action::FaceName { .. }
                | Action::FaceRelease { .. }
                | Action::FaceReseat { .. }
                | Action::FaceReroot { .. }
                | Action::FaceOrigin { .. }
        )
    }

    pub fn is_legal(&self) -> bool {
        matches!(
            self,
            Action::Genesis { .. }
                | Action::Annotate { .. }
                | Action::Retract { .. }
                | Action::RecordWork { .. }
                | Action::SendBatch { .. }
                | Action::BumpFee { .. }
                | Action::DraftGrant { .. }
                | Action::Revoke { .. }
                | Action::MigrateGrants
                | Action::Upgrade
                | Action::Rotate { step: RotateStep::Write }
        )
    }

    pub fn verb(&self) -> Option<&'static str> {
        match self {
            Action::Genesis { .. } => Some("init"),
            Action::Annotate { .. } => Some("annotate"),
            Action::Retract { .. } => Some("retract"),
            Action::RecordWork { .. } => Some("history"),
            Action::SendBatch { .. } | Action::BumpFee { .. } | Action::SendAsked { .. } => Some("anchor"),
            Action::Audit => Some("audit"),
            Action::DraftGrant { .. } | Action::MigrateGrants => Some("grant"),
            Action::ExportKit { .. } => Some("kit-export"),
            Action::VerifyWork { .. } => Some("kit-verify"),
            Action::Revoke { .. } => Some("revoke"),
            Action::Upgrade => Some("migrate"),
            Action::ReadBook { .. } => Some("scan"),
            Action::ReviewVault => Some("check-grant"),
            Action::ExportBadge { .. } => Some("badge"),
            Action::CleanExports { .. } => Some("clean"),
            Action::ExportGrantFile { .. } => Some("grant-package"),
            Action::FaceReread => Some("face-table"),
            Action::FaceReroot { .. } => Some("face-reroot"),
            Action::FaceOrigin { .. } => Some("face-origin"),
            Action::FaceJoined { .. } => Some("face-joined"),
            Action::FaceHome { .. } => Some("face-home"),
            Action::FaceAccount { .. } => Some("face-account"),
            Action::FaceKey { .. } => Some("face-key"),
            Action::FaceName { .. } => Some("face-name"),
            Action::FaceRelease { .. } => Some("face-release"),
            Action::EstimateGas { .. } => None,
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Applied {
    Shown(Page),
    Started(Kind),
    Refused(Kind),
    Stopped(Reaped),
    Seated(crate::roles::Role),
    Identities(usize),
    FreshWords,
    FreshDropped,
    IdentityMade { id: String, seats: Vec<(crate::roles::Role, Address)>, restored: bool },
    IdentityNamed { id: String, label: String },
    IdentitySwitched { id: String },
    IdentityKept { id: String },
    IdentityDeleted { id: String },
    Unlocked,
    LockedUp,
    KeyboxReset,
    MachineAside { from: String, to: String },
    PinChanged,
    AutoLockSet { on: bool, secs: u64 },
    PrimarySet { id: String },
    BackupRestored(crate::backup::Summary),
    Resumed(bool),
    CaughtUp { audit: bool, review: bool, resume: bool },
    NetworkChosen { name: String, filled: bool },
    AutoAnchor(bool),
    HideLocalDeletions(bool),
    Recovered,
    WordsShown,
    WordsHidden,
    KeyBackedUp { path: String, address: Address },
    Homed { root: String, mode: crate::lock::Mode },
    FactsForgotten { had: bool },
    Migrated { root: String },
    Capped(u64),
    Mirrored { path: String, entries: usize, added: usize, topped_up: bool },
    Endpoints(usize),
    LogAsks(u64),
    RestMode(bool),
    RestAdded { base: String },
    RestKeyed { base: String },
    RestRemoved { left: usize },
    Genesised { id: String, queued: usize, next: Next },
    AdoptedInPlace { entries: usize, linked: usize, label: String },
    Opened { id: String, bytes: usize },
    Annotated(String),
    Retracted { id: String, dropped: bool, queued: usize, local: bool, next: Next },
    Basis { chain: u64 },
    Every(u64),
    Took { source: crate::anchorx::Source, hex: String },
    Recorded { id: String, queued: usize, next: Next },
    RecordedBatch { ids: Vec<String>, stopped: Option<(usize, String, crate::fault::Fault)>, queued: usize, next: Next },
    KitLinked { path: String, link: Option<String> },
    GrantCode { text: String },
    KitDropped { id: String, dir_removed: bool },
    Registered { path: String },
    Since { head: String, grew: Option<usize> },
    Gas { count: usize, gas: u64, calldata: String },
    Picked { items: usize, pulled: usize },
    Granted { id: String, queued: usize, next: Next },
    Queued { id: String, queued: usize, next: Next },
    GrantFileExported { path: String, why: crate::home::Why, hops: usize, terms: usize, ledger: bool, files: usize, kit_id: String, original: bool, taken: Vec<String>, left_out: Vec<String>, sealed: bool },
    PublishSet { url: Option<String> },
    ReadNets(usize),
    Clashed(usize),
    Revoked { id: String, queued: usize, next: Next, notice: Option<Notice> },
    GrantsMigrated { count: usize, queued: usize, next: Next },
    Storied { grant: String, revocations: usize },
    Upgraded { stage: String, queued: usize, next: Next },
    Moved { queued: usize, left_out: bool },
    Rotated { stage: String },
    Booked { address: String, on: bool },
    Snapshot { path: String, bytes: usize },
    Held { ids: Vec<String>, grant: String, renewed: usize },
    HeldRenewed { grant: String },
    HeldPartly { ids: Vec<String>, refused: usize, renewed: usize },
    Upstream { grant: String },
    HeldNoted { grant: String },
    ReviewEvery(u64),
    Spoken(crate::lang::Lang),
    Zoned(crate::when::Zone),
    Appeared(String),
    ProxySet(String),
    CliAnchorSet(crate::machine::CliAnchor),
    CliDesktopSet(bool),
    CliAloneSet(Option<Option<u64>>),
    CliPathRead(zikaron_os::cli_path::State),
    SendAsked { count: usize },
    Trouble(crate::fault::Fault),
    Consented { text: String },
    RerootMade { id: String },
    CliPassAdded { pass: crate::secret::Secret, row: zikaron_seal::credential::Row },
    CliPassDropped { tag: String },
}

fn after_vault_shut(shell: &mut Shell) {
    shell.reread_vault();
    if !shell.unlocked() {
        shell.after_lock();
    }
}

pub fn held_back(shell: &Shell, a: &Action) -> Option<crate::fault::Known> {
    if shell.rekeying && !matches!(a, Action::Show(_) | Action::Quit | Action::SelfCheck) {
        return Some(crate::fault::Known::Rekeying);
    }
    if shell.swapping && !a.reads_only() {
        return Some(crate::fault::Known::Rekeying);
    }
    if shell.old_view.is_some() && !a.reads_old_data() {
        return Some(crate::fault::Known::ReadOnly);
    }
    if a.halts_when_locked() && !shell.unlocked() {
        return Some(crate::fault::Known::Locked);
    }
    if a.needs_key() && !shell.unlocked() {
        return Some(crate::fault::Known::Locked);
    }
    None
}

fn gate_first(shell: &mut Shell, a: Action) -> Applied {
    let ask = match crate::exitgate::ask_of(shell) {
        Ok(x) => x,
        Err(f) => return shell.trouble(f),
    };
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf());
    let seal = match &a {
        Action::ExportGrantFile { seal_to, .. } if !seal_to.trim().is_empty() => match sealing_for(shell, seal_to) {
            Ok(x) => Some(x),
            Err(f) => return shell.trouble(f),
        },
        _ => None,
    };
    match shell.tasks.spawn(Kind::Gate, move || {
        let pass = crate::exitgate::pass(&ask)?;
        let a = match (a, seal) {
            (Action::ExportGrantFile { id, to, original, seal_to, .. }, Some((root, main, readonly))) => {
                let k = crate::addressedx::key_for(&root, &main, &readonly)?;
                Action::ExportGrantFile { id, to, original, seal_to, key: Some(zikaron::hexfmt::encode(&k)) }
            }
            (a, _) => a,
        };
        Ok(Done::GatePassed { root, then: Box::new(a), pass: Box::new(pass) })
    }) {
        Spawned::Started => Applied::Started(Kind::Gate),
        Spawned::InFlight => Applied::Refused(Kind::Gate),
    }
}

fn clean_exports(shell: &mut Shell, yes: bool) -> Applied {
    let kind = if yes { Kind::Clean } else { Kind::CleanList };
    match shell.tasks.spawn(kind, move || {
        let machine = crate::home::machine_dir()?;
        let plan = if yes { crate::exports::clean(&machine)? } else { crate::exports::plan(&machine)? };
        Ok(Done::Cleaned { plan, done: yes })
    }) {
        Spawned::Started => Applied::Started(kind),
        Spawned::InFlight => Applied::Refused(kind),
    }
}

fn sealing_for(shell: &Shell, root: &str) -> Result<([u8; 20], crate::addressedx::Main, Vec<crate::readnets::Net>), crate::fault::Fault> {
    let grantee = crate::key::Address::parse(root.trim()).ok_or_else(|| crate::fault::Fault::known(crate::fault::Known::AddressShape, root.trim().to_string()))?;
    let main = crate::addressedx::Main {
        chain: shell.settings.chain_id,
        registry: shell.settings.registry.map(|a| a.0),
        from_block: shell.settings.from_block,
        nodes: shell.endpoints.iter().filter(|e| Some(e.chain) == shell.settings.chain_id).map(|e| e.url.clone()).collect(),
    };
    Ok((grantee.0, main, read_nets_now()?))
}

pub fn gate_landed(shell: &mut Shell, root: Option<std::path::PathBuf>, then: Action, pass: &crate::exitgate::Pass, stale: bool) -> Option<Applied> {
    let here = shell.home.as_ref().map(|h| h.root().to_path_buf());
    if stale || here != root {
        if shell.tasks.in_flight(Kind::Gate) {
            return None;
        }
        return Some(apply(shell, then));
    }
    trace::mark(then.feature());
    if let Some(k) = held_back(shell, &then) {
        return Some(shell.trouble(crate::fault::Fault::known(k, String::new())));
    }
    Some(match then {
        Action::ExportMirror { to } => match export_mirror(shell, &to, pass) {
            Ok((path, entries, added, topped_up)) => Applied::Mirrored { path, entries, added, topped_up },
            Err(f) => shell.trouble(f),
        },
        Action::ExportGrantFile { id, to, original, key, .. } => match export_grant_file(shell, &id, &to, &original, key.as_deref(), pass) {
            Ok(x) => grant_file_exported(x),
            Err(f) => shell.trouble(f),
        },
        other => apply(shell, other),
    })
}

pub fn apply(shell: &mut Shell, a: Action) -> Applied {
    trace::mark(a.feature());
    shell.moved += 1;
    if let Some(k) = held_back(shell, &a) {
        return shell.trouble(crate::fault::Fault::known(k, String::new()));
    }
    if a.pin_asked().is_some() {
        if a.writes_ledger() {
            if let Err(f) = shell.may_write_entries() {
                return shell.trouble(f);
            }
        }
        let mut rest = a;
        let pin = rest.hand_pin().unwrap_or_default();
        rest.forget_pin();
        return vault(shell, move || {
            crate::keybox::check_pin(pin.expose())?;
            Ok(crate::task::Vault::Gate(Box::new(rest)))
        });
    }
    body(shell, a)
}

fn vault(shell: &mut Shell, work: impl FnOnce() -> Result<crate::task::Vault, crate::fault::Fault> + Send + 'static) -> Applied {
    match shell.tasks.spawn(Kind::Vault, move || work().map(Done::Vault)) {
        Spawned::Started => Applied::Started(Kind::Vault),
        Spawned::InFlight => Applied::Refused(Kind::Vault),
    }
}

pub fn vault_landed(shell: &mut Shell, got: Result<crate::task::Vault, crate::fault::Fault>) -> Applied {
    use crate::task::Vault;
    match got {
        Ok(Vault::Opened) => {
            shell.after_unlock();
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
            shell.reread_vault();
            if shell.lock_pending {
                return shell.trouble(crate::fault::Fault::known(crate::fault::Known::Locked, String::new()));
            }
            if let Some(k) = held_back(shell, &a) {
                return shell.trouble(crate::fault::Fault::known(k, String::new()));
            }
            body(shell, *a)
        }
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
        Ok(Vault::Identity { row, restored, fresh, was }) => identity_landed(shell, row, restored, fresh, was),
        Ok(Vault::RerootMade { id }) => {
            shell.new_words = None;
            shell.new_label = None;
            match crate::register::view() {
                Ok(v) => shell.seat_identities(Some(v)),
                Err(f) => shell.faults.push(f),
            }
            Applied::RerootMade { id }
        }
        Err(f) => {
            shell.rekeying = false;
            shell.new_label = None;
            after_vault_shut(shell);
            shell.trouble(f)
        }
    }
}

fn busy_for_rekey(shell: &Shell) -> Option<crate::fault::Fault> {
    let others: Vec<Kind> = shell.tasks.flying().into_iter().filter(|k| k.writes_local()).collect();
    (!others.is_empty()).then(|| {
        crate::fault::Fault::known(crate::fault::Known::BusyForRekey, others.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(" "))
    })
}

pub fn apply_settled(shell: &mut Shell, a: Action) -> Applied {
    let first = apply(shell, a);
    if let Applied::Started(k) = first {
        if lands_said(k) {
            return said_settled(shell, k);
        }
    }
    if first == Applied::Started(Kind::Gate) {
        return gate_settled(shell);
    }
    if first != Applied::Started(Kind::Vault) {
        return first;
    }
    loop {
        if let Some(done) = shell.vault_said.take() {
            return done;
        }
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

pub fn wait_path(shell: &mut Shell, wait: crate::platform::Wait) -> crate::task::Spawned {
    shell.tasks.spawn(Kind::Path, move || wait().map(Done::Path))
}

pub fn lands_said(k: Kind) -> bool {
    crate::landing::goes_back(k, crate::landing::Back::Said)
}

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
        (Kind::Seat, got) => seat_landed(shell, got),
        (Kind::Migrate, Ok(Done::Copied { old, root })) => match migrate_landed(shell, &old, &root) {
            Ok(root) => Applied::Migrated { root },
            Err(f) => shell.trouble(f),
        },
        (_, Err(f)) => shell.trouble(f),
        (_, Ok(_)) => lost(shell),
    }
}

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

fn gate_settled(shell: &mut Shell) -> Applied {
    shell.gate_said = None;
    loop {
        if let Some(done) = shell.gate_said.take() {
            if done != Applied::Started(Kind::Gate) {
                return done;
            }
        }
        let finished = shell.tasks.finished_in_flight(Kind::Gate);
        let got = shell.drain_at(0.0);
        if let Some(f) = got.iter().filter(|o| o.kind == Kind::Gate && !o.stale).find_map(|o| o.result.as_ref().err()) {
            return Applied::Trouble(f.clone());
        }
        shell.keep_back(got);
        if shell.gate_said.is_none() {
            if (finished && shell.tasks.in_flight(Kind::Gate)) || !shell.tasks.in_flight(Kind::Gate) {
                return shell.trouble(crate::fault::Fault::known(crate::fault::Known::OutcomeLost, Kind::Gate.as_str().to_string()));
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

fn said_settled(shell: &mut Shell, k: Kind) -> Applied {
    loop {
        if let Some(done) = shell.said.remove(&k) {
            if done != Applied::Started(k) {
                return done;
            }
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

pub fn gas_landed(shell: &mut Shell, got: Result<Done, crate::fault::Fault>) -> Applied {
    if got.is_err() {
        shell.fees_left.clear();
    }
    match got {
        Ok(Done::Gas { count, gas, calldata, fees, fees_left, head_time, lane }) => {
            if let Some(t) = head_time {
                shell.note_chain_time(t);
            }
            shell.gas = Some((count, gas));
            shell.gas_lane = Some(lane);
            shell.fees = Some(fees);
            shell.fees_left = fees_left;
            Applied::Gas { count, gas, calldata }
        }
        Ok(_) => shell.trouble(crate::fault::Fault::known(crate::fault::Known::OutcomeLost, Kind::Gas.as_str().to_string())),
        Err(f) => {
            shell.gas = None;
            shell.gas_lane = None;
            shell.fees = None;
            shell.trouble(f)
        }
    }
}

pub fn behind(shell: &mut Shell, kind: Kind, work: impl FnOnce() -> Result<Done, crate::fault::Fault> + Send + 'static) -> Spawned {
    shell.tasks.spawn(kind, work)
}

fn body(shell: &mut Shell, a: Action) -> Applied {
    if a.writes_ledger() {
        if let Some(s) = shell.unfetched.as_ref() {
            return shell.trouble(crate::restorex::fault(s).gate_owned());
        }
    }
    if a.takes_turn() {
        if let Err(f) = shell.take_turn() {
            return shell.trouble(f);
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
        Action::Measure => match remeasure(shell) {
            None => shell.trouble(crate::fault::Fault::known(crate::fault::Known::NoHome, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail001).to_string()))),
            Some(Spawned::Started) => Applied::Started(Kind::Archive),
            Some(Spawned::InFlight) => Applied::Refused(Kind::Archive),
        },
        Action::Quit => {
            for w in shell.words.iter_mut().flatten() {
                w.clear();
            }
            shell.words = None;
            shell.new_words = None;
            shell.door_off = true;
            shell.door_sync();
            Applied::Stopped(shell.tasks.shutdown())
        }

        Action::SwitchRole => match switch_role(shell) {
            Ok(a) => a,
            Err(f) => shell.trouble(f),
        },
        Action::ReadIdentities => match crate::register::view() {
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
        Action::SwitchIdentity { id } => switch_identity(shell, &id),
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
                shell.after_lock();
                Applied::KeyboxReset
            }
            Err(f) => shell.trouble(f),
        },
        Action::SetMachineAside => set_machine_aside(shell),
        Action::Lock => {
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
            vault(shell, move || crate::keybox::change_pin(old.expose(), pin.expose()).map(|_| crate::task::Vault::Changed))
        }
        Action::RecoverWords { words, pin, again } => {
            if let Err(f) = same_twice(&pin, &again) {
                return shell.trouble(f);
            }
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
            match shell.tasks.spawn(Kind::Backup, move || crate::backup::peek(std::path::Path::new(&path), password.expose()).map(|s| Done::BackupSeen { path, summary: s })) {
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
                    crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail002).to_string()),
                ));
            };
            let chain = shell.settings.chain_id;
            match shell.tasks.spawn(Kind::Chain, move || read_chain(&eps, &who, chain))
            {
                Spawned::Started => Applied::Started(Kind::Chain),
                Spawned::InFlight => Applied::Refused(Kind::Chain),
            }
        }
        Action::ReadChainTime => {
            let eps = shell.endpoints.clone();
            let chain = shell.settings.chain_id.unwrap_or(0);
            match shell.tasks.spawn(Kind::ChainTime, move || crate::chainx::head_time(&eps, chain).map(|(at, _, _)| Done::ChainTime(at))) {
                Spawned::Started => Applied::Started(Kind::ChainTime),
                Spawned::InFlight => Applied::Refused(Kind::ChainTime),
            }
        }
        Action::SetEndpoints { specs } => match set_endpoints(shell, &specs) {
            Ok(n) => Applied::Endpoints(n),
            Err(f) => shell.trouble(f),
        },
        Action::SetLogAsks { n } => match set_log_asks(shell, n) {
            Ok(n) => Applied::LogAsks(n),
            Err(f) => shell.trouble(f),
        },
        Action::SetRestMode { on } => match set_rest_mode(shell, on) {
            Ok(on) => Applied::RestMode(on),
            Err(f) => shell.trouble(f),
        },
        Action::AddRest { base, key } => match add_rest(shell, &base, &key) {
            Ok(rest::Added::Row(base)) => Applied::RestAdded { base },
            Ok(rest::Added::Keyed(base)) => Applied::RestKeyed { base },
            Err(f) => shell.trouble(f),
        },
        Action::RemoveRest { base } => match remove_rest(shell, &base) {
            Ok(left) => Applied::RestRemoved { left },
            Err(f) => shell.trouble(f),
        },

        Action::Genesis { statement } => match genesis(shell, &statement) {
            Ok((id, n)) => {
                if let Some(Err(f)) = shell.home.as_ref().map(crate::restorex::forget_on) {
                    shell.faults.push(f);
                }
                Applied::Genesised { id, queued: n.queued, next: n.next }
            }
            Err(f) => shell.trouble(f),
        },
        Action::ReadLedger => {
            let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else {
                return shell.trouble(crate::fault::Fault::known(
                    crate::fault::Known::NoHome,
                    crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail003).to_string()),
                ));
            };
            let report = shell.audit.as_ref().map(|a| a.report.clone());
            let remembered = shell.remembered.clone();
            let wall = (shell.clock)();
            let queue = shell.queue.clone();
            let mine = shell.anchor;
            let r#gen = shell.rows_gen;
            let fragment = shell.audit.as_ref().map(|a| a.fragment.clone());
            match shell.tasks.spawn(Kind::Ledger, move || {
                let home = crate::home::Home::open(&root)?;
                let mut t = crate::ledgerx::table_remembering(&home, report.as_ref(), &queue, remembered.as_ref(), wall)?;
                let pile = home
                    .ledger()?
                    .pile()?;
                let handed = crate::succeedx::handed_over(&pile.items, mine);
                let office = crate::rotatex::office_of(&pile.items);
                let upgrade = crate::upgradex::stage(&pile.items, fragment.as_ref(), mine.map(|a| a.hex()).as_deref(), &queue);
                let upgrade_read = fragment.is_some();
                if let Some(f) = &fragment {
                    crate::ledgerx::stamp(&mut t.rows, &pile.items, f);
                }
                let seats = zikaron_glue::face::seat_chains(&pile.items);
                let faces = rotate::faces_in(&pile.items);
                let places = zikaron_seal::queue::places_of(&pile.items);
                Ok(Done::Ledger { rows: t.rows, strays: t.strays, handed, office, upgrade, upgrade_read, seats, faces, places, r#gen })
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
        Action::MoveChain { chain, registry, from_block, nodes } => match move_chain(shell, &chain, &registry, &from_block, &nodes) {
            Ok((queued, left_out)) => Applied::Moved { queued, left_out },
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
                    let d = crate::kitx::digest_of(std::path::Path::new(p.trim())).map(|d| d.iter().map(|x| zikaron::hexfmt::encode(x)).collect());
                    (p, d)
                })
                .collect();
            Ok(Done::Vetted(rows))
        }) {
            Spawned::Started => Applied::Started(Kind::Vet),
            Spawned::InFlight => Applied::Refused(Kind::Vet),
        },
        Action::Peek { ask } => match shell.tasks.spawn(Kind::Peek, move || {
            let got = crate::peekx::read(&ask);
            Ok(Done::Peeked(Box::new((ask, got))))
        }) {
            Spawned::Started => Applied::Started(Kind::Peek),
            Spawned::InFlight => Applied::Refused(Kind::Peek),
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
        Action::EstimateGas { count } => match gas_ask(shell, count) {
            Ok(ask) => match shell.tasks.spawn(Kind::Gas, move || estimate_on(ask)) {
                Spawned::Started => {
                    shell.gas = None;
                    shell.gas_lane = None;
                    shell.fees = None;
                    shell.said.remove(&Kind::Gas);
                    shell.gas_asked = Some(shell.gas_epoch);
                    Applied::Started(Kind::Gas)
                }
                Spawned::InFlight => Applied::Refused(Kind::Gas),
            },
            Err(f) => {
                shell.gas = None;
                shell.gas_lane = None;
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

        Action::PickKit { from, to, ids } => match pick_kit(shell, &from, &to, &ids) {
            Ok((n, pulled)) => {
                let p = pulled.len();
                shell.picked = Some((n, pulled));
                Applied::Picked { items: n, pulled: p }
            }
            Err(f) => shell.trouble(f),
        },
        Action::ExportKit { from, to, ids, attach, note, out, form, recipient, recipient_key, originals, companion } => {
            match export_kit(shell, &from, &to, &ids, &attach, &note, &out, &form, &recipient, &recipient_key, &originals, companion) {
                Ok(s) => match s {
                    Spawned::Started => Applied::Started(Kind::Kit),
                    Spawned::InFlight => Applied::Refused(Kind::Kit),
                },
                Err(f) => shell.trouble(f),
            }
        }
        a @ Action::DraftGrant { .. } => grant_first(shell, a),
        Action::MigrateGrants => migrate_first(shell),
        Action::ReadGrants => {
            let Some(root) = shell.home.as_ref().map(|h| h.root().to_path_buf()) else {
                return shell.trouble(crate::fault::Fault::known(
                    crate::fault::Known::NoHome,
                    crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail004).to_string()),
                ));
            };
            let r#gen = shell.grants_gen;
            match shell.tasks.spawn(Kind::Grants, move || {
                let home = crate::home::Home::open(&root)?;
                Ok(Done::Grants { r#gen, rows: crate::grantx::table(&home)? })
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
        Action::Revoke { grant, case } => revoke_first(shell, &grant, &case),
        Action::ReadStory { grant } => match read_story(shell, &grant) {
            Ok(n) => Applied::Storied { grant, revocations: n },
            Err(f) => shell.trouble(f),
        },
        Action::Rotate { step } => match rotate(shell, step) {
            Ok(a) => a,
            Err(f) => shell.trouble(f),
        },
        Action::UpgradeCost => match upgrade_cost(shell) {
            Ok(Spawned::Started) => Applied::Started(Kind::Rotate),
            Ok(Spawned::InFlight) => Applied::Refused(Kind::Rotate),
            Err(f) => shell.trouble(f),
        },
        Action::Upgrade => match upgrade(shell) {
            Ok((stage, n)) => Applied::Upgraded { stage: stage.as_str().to_string(), queued: n.queued, next: n.next },
            Err(f) => shell.trouble(f),
        },
        Action::ReadBook { address, dir } => match read_book(shell, &address, &dir) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Book),
                Spawned::InFlight => Applied::Refused(Kind::Book),
            },
            Err(f) => shell.trouble(f),
        },
        Action::FaceReroot { to } => match face::face_reroot(shell, &to) {
            Ok((id, q)) => Applied::Queued { id, queued: q.queued, next: q.next },
            Err(f) => shell.trouble(f),
        },
        Action::FaceOrigin { from } => match face::face_origin(shell, &from) {
            Ok((id, q)) => Applied::Queued { id, queued: q.queued, next: q.next },
            Err(f) => shell.trouble(f),
        },
        Action::FaceRerootMake { answers, label, chain } => match face::reroot_make(shell, &answers, &label, chain) {
            Ok(a) => a,
            Err(f) => shell.trouble(f),
        },
        Action::FaceRerootFee { go } => match face::reroot_fee(shell, go) {
            Ok(Spawned::Started) => Applied::Started(Kind::RerootFee),
            Ok(Spawned::InFlight) => Applied::Refused(Kind::RerootFee),
            Err(f) => shell.trouble(f),
        },
        Action::FaceJoined { root, to } => match face::joined_read(shell, &root, &to) {
            Ok(Spawned::Started) => Applied::Started(Kind::FaceJoined),
            Ok(Spawned::InFlight) => Applied::Refused(Kind::FaceJoined),
            Err(f) => shell.trouble(f),
        },
        Action::FaceBooks => match face_books(shell) {
            Ok(Spawned::Started) => Applied::Started(Kind::FaceBooks),
            Ok(Spawned::InFlight) => Applied::Refused(Kind::FaceBooks),
            Err(f) => shell.trouble(f),
        },
        Action::FaceReread => match face_reread(shell) {
            Ok(Spawned::Started) => Applied::Started(Kind::Persona),
            Ok(Spawned::InFlight) => Applied::Refused(Kind::Persona),
            Err(f) => shell.trouble(f),
        },
        Action::FaceHome { chain } => face::home_first(shell, chain),
        Action::FaceAccount { platform, handle, note } => { let r = face::face_account(shell, &platform, &handle, &note); queued(shell, r) },
        Action::FaceKey { address, role, note, consent, sign_here, .. } => { let r = face::face_key(shell, &address, &role, &note, &consent, sign_here); queued(shell, r) },
        Action::FaceName { name } => { let r = face::face_name(shell, &name); queued(shell, r) },
        Action::FaceRelease { subject } => { let r = face::face_release(shell, &subject); queued(shell, r) },
        Action::FaceReseat { entry } => match face::face_reseat(shell, &entry) {
            Ok(a) => a,
            Err(f) => shell.trouble(f),
        },
        Action::FaceConsent { identity, role, .. } => match face::face_consent(shell, &identity, &role) {
            Ok(text) => Applied::Consented { text },
            Err(f) => shell.trouble(f),
        },
        Action::CliPassAdd { note, .. } => match cli_pass_add(shell, &note) {
            Ok((pass, row)) => Applied::CliPassAdded { pass, row },
            Err(f) => shell.trouble(f),
        },
        Action::CliPassDrop { tag } => match cli_pass_drop(shell, &tag) {
            Ok(()) => Applied::CliPassDropped { tag },
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
        Action::VerifyWork { path, work, .. } => match verify_work(shell, &path, &work) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Verify),
                Spawned::InFlight => Applied::Refused(Kind::Verify),
            },
            Err(f) => shell.trouble(f),
        },
        Action::ExtractOriginals { kit, to } => extract_originals(shell, &kit, &to),
        Action::CleanExports { yes } => match yes.trim() {
            "" => clean_exports(shell, false),
            crate::exports::YES => clean_exports(shell, true),
            w => shell.trouble(crate::fault::Fault::known(crate::fault::Known::CleanWord, w.to_string())),
        },
        Action::CheckDelivery { path, expect } => match check_delivery(shell, &path, &expect) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Delivery),
                Spawned::InFlight => Applied::Refused(Kind::Delivery),
            },
            Err(f) => shell.trouble(f),
        },
        Action::CheckPayload { typed, ledgers, endpoints, registry, from_block, now, file, terms } => {
            match check_payload(shell, &typed, &ledgers, &endpoints, &registry, &from_block, &now, (file, terms)) {
                Ok(s) => match s {
                    Spawned::Started => Applied::Started(Kind::Check),
                    Spawned::InFlight => Applied::Refused(Kind::Check),
                },
                Err(f) => shell.trouble(f),
            }
        }
        Action::AuditCheck => match check::audit_check(shell) {
            Ok(()) => Applied::Started(Kind::Behind),
            Err(f) => shell.trouble(f),
        },
        Action::AuditOthers => match reader::audit_others(shell) {
            Ok(()) => Applied::Started(Kind::Behind),
            Err(f) => shell.trouble(f),
        },
        Action::ImportGrant { typed } => match import_grant(shell, &typed) {
            Ok(vault::Imported::Stored { ids, grant }) => Applied::Held { ids, grant, renewed: 0 },
            Ok(vault::Imported::Renewed { grant }) => Applied::HeldRenewed { grant },
            Err(f) => shell.trouble(f),
        },
        Action::ImportGrantDir { dir } => match import_grant_dir(shell, &dir) {
            Ok((ids, renewed, refused)) if refused.is_empty() => Applied::Held { ids, grant: String::new(), renewed },
            Ok((ids, renewed, mut refused)) if ids.is_empty() && renewed == 0 => {
                let last = refused.pop().expect("非空");
                for f in refused {
                    shell.trouble(f);
                }
                shell.trouble(last)
            }
            Ok((ids, renewed, refused)) => {
                let n = refused.len();
                for f in refused {
                    shell.trouble(f);
                }
                Applied::HeldPartly { ids, refused: n, renewed }
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
            shell.ai_config = cli_beside().filter(|_| !matches!(s, zikaron_os::cli_path::State::Unsupported(_))).map(|cli| ai_config(&cli));
            Applied::CliPathRead(s)
        }
        Action::SetCliPath { on } => match crate::action::cli_beside() {
            Some(cli) => match shell.tasks.spawn(Kind::CliPath, move || cli_path_set(&cli, on)) {
                Spawned::Started => Applied::Started(Kind::CliPath),
                Spawned::InFlight => Applied::Refused(Kind::CliPath),
            },
            None => shell.trouble(crate::fault::Fault::known(crate::fault::Known::CliPathUnsupported, String::new())),
        },
        Action::SetCliDesktop { on } => {
            if shell.machine.cli_path() == crate::machine::CliPath::Alone {
                return shell.trouble(crate::fault::Fault::known(crate::fault::Known::CliAloneOn, String::new()));
            }
            match crate::machine::update(|m| m.cli_desktop = on) {
                Ok(m) => {
                    shell.machine = m;
                    Applied::CliDesktopSet(on)
                }
                Err(f) => shell.trouble(f),
            }
        }
        Action::SetCliAlone { lasts, .. } => match cli_alone_on(shell, lasts) {
            Ok(expires) => Applied::CliAloneSet(Some(expires)),
            Err(f) => shell.trouble(f),
        },
        Action::CliAloneOff => match cli_alone_off(shell) {
            Ok(()) => Applied::CliAloneSet(None),
            Err(f) => shell.trouble(f),
        },
        Action::SetCliAnchor { to } => match crate::machine::update(|m| m.cli_anchor = to) {
            Ok(m) => {
                shell.machine = m;
                Applied::CliAnchorSet(to)
            }
            Err(f) => shell.trouble(f),
        },
        Action::SendAsked { count } => match count {
            0 => shell.trouble(crate::fault::Fault::known(crate::fault::Known::QueueEmpty, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail030).to_string()))),
            n => Applied::SendAsked { count: n },
        },
        Action::ListHeld => match list_held(shell) {
            Ok(s) => match s {
                Spawned::Started => Applied::Started(Kind::Held),
                Spawned::InFlight => Applied::Refused(Kind::Held),
            },
            Err(f) => shell.trouble(f),
        },
        Action::ExportGrantFile { id, to, original, seal_to, .. } => match grant_file_plan(shell, &id, &to) {
            Ok(_) => gate_first(shell, Action::ExportGrantFile { id, to, original, seal_to, key: None }),
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



static LANDED_TAMPER: std::sync::OnceLock<fn(&std::path::Path)> = std::sync::OnceLock::new();

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Next {
    Send,
    Wait,
    Held,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Enqueued {
    pub queued: usize,
    pub next: Next,
}


use zikaron_send::batch::Batch;

pub const ANCHOR_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

mod anchor;
mod archive;
mod audit;
mod badge;
mod check;
mod readnet;
mod rest;
mod delivery;
mod diligence;
mod face;
pub use self::face::{face_cleared, face_entries, reroot_nodes, reroot_row, unseated, unseated_of, FeeReading};
mod genesis;
mod grant;
mod grant_file;
pub use self::grant_file::ask_unwritten;
mod issue;
mod grants;
mod home;
mod identity;
pub use self::check::one_place;
pub use self::diligence::{snapshot_of, Snapshot};
pub use self::identity::seats_with_entries;
mod keys;
mod office;
mod kit;
mod ledger;
mod queue;
mod reader;
mod revoke;
mod sentinel;
mod chain;
mod rotate;
mod upgrade;
mod vault;
mod verify;
use self::anchor::*;
use self::archive::*;
use self::rest::*;
pub use self::rest::add_refused;
use self::audit::*;
use self::badge::*;

fn set_zone(shell: &mut Shell, zone: crate::when::Zone) -> Result<crate::when::Zone, crate::fault::Fault> {
    shell.may_save_settings()?;
    shell.commit_settings(|s| s.zone = Some(zone))?;
    crate::when::set(zone);
    Ok(zone)
}

fn set_lang(shell: &mut Shell, lang: crate::lang::Lang) -> Result<crate::lang::Lang, crate::fault::Fault> {
    if shell.home.is_some() {
        shell.may_save_settings()?;
    }
    shell.machine = crate::machine::update(|m| m.lang = Some(lang))?;
    if shell.home.is_some() {
        shell.commit_settings(|s| s.lang = Some(lang))?;
    }
    crate::lang::set(lang);
    Ok(lang)
}

pub use self::check::*;
pub use self::reader::OthersAsk;
pub use self::audit::{basis_read, basis_stamp, BasisStamp};
pub use self::readnet::*;
pub use self::rotate::{old_words_held_elsewhere, words_note, words_pending, words_settle, WordsNote};
pub use crate::identity::{seat_ledger, SeatLedger};
use self::delivery::*;
use self::diligence::*;
use self::genesis::*;
use self::grant::*;
use self::issue::{grant_first, migrate_first, revoke_first};
pub use self::issue::{issued, migratable, withdrew, Notice};
use self::grant_file::*;
use self::upgrade::*;
pub use self::rotate::{follow_office, last_move, ledger_root, seat_before, LedgerFaces, rotation_owed, rotation_stage, start_stage, sweep_key, sweep_open, swept_now, RotateStep};
use self::rotate::enrolment_read;
use self::rotate::rotate;
pub use self::chain::moved_ids;
use self::chain::move_chain;
use self::grants::*;
pub use self::home::*;
pub use self::identity::*;
use self::keys::*;
use self::office::*;
pub use self::office::{follow_register, settle, Back, Followed, Seating};
use self::kit::*;
use self::ledger::*;
pub use self::queue::*;
use self::reader::*;
use self::revoke::*;
use self::sentinel::*;
use self::vault::*;
use self::verify::*;

pub fn cli_beside() -> Option<std::path::PathBuf> {
    zikaron_os::cli_path::beside_this_program()
}

pub fn ai_config(cli: &std::path::Path) -> String {
    use zikaron::json::Value;
    let o = |k: &str, v: Value| Value::Obj(vec![(k.to_string(), v)]);
    let server = Value::Obj(vec![("command".to_string(), Value::Str(cli.display().to_string())), ("args".to_string(), Value::Arr(vec![Value::Str("mcp".to_string())]))]);
    String::from_utf8_lossy(&zikaron::json::canon_bytes(&o("mcpServers", o("zikaron", server)))).into_owned()
}

pub fn ai_config_with_pass(config: &str, pass: &str) -> Option<String> {
    use zikaron::json::Value;
    let mut v = zikaron::json::parse(config.as_bytes()).ok()?;
    let Value::Obj(top) = &mut v else { return None };
    let Value::Obj(servers) = &mut top.iter_mut().find(|(k, _)| k == "mcpServers")?.1 else { return None };
    let Value::Obj(server) = &mut servers.iter_mut().find(|(k, _)| k == "zikaron")?.1 else { return None };
    server.retain(|(k, _)| k != "env");
    server.push(("env".to_string(), Value::Obj(vec![(zikaron_seal::credential::PASS_ENV.to_string(), Value::Str(pass.to_string()))])));
    Some(String::from_utf8_lossy(&zikaron::json::canon_bytes(&v)).into_owned())
}

fn cli_path_now() -> zikaron_os::cli_path::State {
    match cli_beside() {
        Some(cli) => zikaron_os::cli_path::state(&cli),
        None => zikaron_os::cli_path::State::Unsupported(String::new()),
    }
}

fn cli_path_set(cli: &std::path::Path, on: bool) -> Result<Done, crate::fault::Fault> {
    use crate::fault::{Fault, Known};
    use zikaron_os::cli_path::{disable, enable, Refused};
    let got = if on { enable(cli) } else { disable(cli) };
    got.map(Done::CliPath).map_err(|r| match r {
        Refused::Taken(what) => Fault::known(Known::CliPathTaken, what),
        Refused::Cancelled => Fault::known(Known::CliPathCancelled, String::new()),
        Refused::NotAllowed(said) => Fault::known(Known::CliPathNotAllowed, said),
        Refused::Unsupported(why) => Fault::known(Known::CliPathUnsupported, why),
        Refused::TooLong(n) => Fault::known(Known::CliPathNotAllowed, crate::lang::both_of(|| crate::lang::fill1(crate::lang::Key::TailCliPathTooLong, &n.to_string()))),
    })
}

fn grant_file_exported(x: crate::grantfilex::Exported) -> Applied {
    Applied::GrantFileExported { path: x.path.display().to_string(), why: x.chosen.why, hops: x.hops, terms: x.terms, ledger: x.ledger, files: x.files, kit_id: x.kit_id, original: x.original, taken: x.taken, left_out: x.left_out, sealed: x.sealed }
}

fn cli_alone_on(shell: &mut Shell, lasts: crate::machine::Lasts) -> Result<Option<u64>, crate::fault::Fault> {
    if !lasts.ok() {
        return Err(crate::fault::Fault::known(crate::fault::Known::MachineShape, crate::machine::member::CLI_ALONE.to_string()));
    }
    let issued = crate::lastread::now_secs();
    let term = zikaron_seal::credential::Term { issued, expires: lasts.expires(issued) };
    crate::keybox::issue_credential(term)?;
    match crate::machine::update(|m| m.cli_alone = Some(lasts)) {
        Ok(m) => shell.machine = m,
        Err(f) => {
            let _ = crate::keybox::drop_credential();
            return Err(f);
        }
    }
    shell.credential = Some(term);
    shell.passes.clear();
    Ok(term.expires)
}

fn cli_pass_add(shell: &mut Shell, note: &str) -> Result<(crate::secret::Secret, zikaron_seal::credential::Row), crate::fault::Fault> {
    match crate::keybox::add_pass(note, crate::lastread::now_secs()) {
        Ok((pass, row)) => {
            shell.passes.push(row.clone());
            Ok((pass, row))
        }
        Err(f) => {
            match f.which() {
                Some(crate::fault::Known::CredentialExpired) => cli_alone_ended(shell, crate::fault::Known::CredentialExpired),
                Some(crate::fault::Known::CredentialShape) => cli_alone_ended(shell, crate::fault::Known::CredentialReclaimed),
                Some(crate::fault::Known::CredentialMissing) if shell.machine.cli_alone.is_some() => cli_alone_ended(shell, crate::fault::Known::CredentialMissing),
                _ => {}
            }
            Err(f)
        }
    }
}

fn cli_pass_drop(shell: &mut Shell, tag: &str) -> Result<(), crate::fault::Fault> {
    let done = crate::keybox::drop_pass(tag);
    shell.reread_credential();
    done
}

fn cli_alone_off(shell: &mut Shell) -> Result<(), crate::fault::Fault> {
    crate::keybox::drop_credential()?;
    shell.credential = None;
    shell.passes.clear();
    shell.machine = crate::machine::update(|m| m.cli_alone = None)?;
    Ok(())
}

pub fn cli_alone_ended(shell: &mut Shell, why: crate::fault::Known) {
    let had = match crate::keybox::drop_credential() {
        Ok(had) => had,
        Err(f) => {
            shell.faults.push(f);
            return;
        }
    };
    alone_ended(shell, why, had);
}

pub fn alone_ended(shell: &mut Shell, why: crate::fault::Known, had: bool) {
    shell.credential = None;
    shell.passes.clear();
    if shell.machine.cli_alone.is_none() && !had {
        return;
    }
    match crate::machine::update(|m| m.cli_alone = None) {
        Ok(m) => shell.machine = m,
        Err(f) => shell.faults.push(f),
    }
    shell.faults.push(crate::fault::Fault::known(why, crate::places::credential_file()));
}

fn queued(shell: &mut Shell, r: Result<(String, Enqueued), crate::fault::Fault>) -> Applied {
    match r {
        Ok((id, q)) => Applied::Queued { id, queued: q.queued, next: q.next },
        Err(f) => shell.trouble(f),
    }
}

#[cfg(test)]
mod ai_config_tests {
    #[test]
    fn the_config_copied_carries_the_key_just_made() {
        use zikaron::json::Value;
        let base = super::ai_config(std::path::Path::new("/opt/zikaron/bin/zikaron"));
        let with = super::ai_config_with_pass(&base, "zkp_test").expect("in shape");
        let v = zikaron::json::parse(with.as_bytes()).expect("json");
        let server = v.member("mcpServers").and_then(|m| m.member("zikaron")).expect("the server");
        assert_eq!(server.member("env").and_then(|e| e.member(zikaron_seal::credential::PASS_ENV)), Some(&Value::Str("zkp_test".into())));
        assert_eq!(server.member("command"), Some(&Value::Str("/opt/zikaron/bin/zikaron".into())));
        assert_eq!(server.member("args"), Some(&Value::Arr(vec![Value::Str("mcp".into())])));
        let mut back = zikaron::json::parse(with.as_bytes()).expect("json");
        if let Value::Obj(top) = &mut back {
            if let Some((_, Value::Obj(servers))) = top.iter_mut().find(|(k, _)| k == "mcpServers") {
                if let Some((_, Value::Obj(s))) = servers.iter_mut().find(|(k, _)| k == "zikaron") {
                    s.retain(|(k, _)| k != "env");
                }
            }
        }
        assert_eq!(String::from_utf8_lossy(&zikaron::json::canon_bytes(&back)), base, "nothing else changed");
        assert!(super::ai_config_with_pass("not json", "zkp_test").is_none());
        assert!(super::ai_config_with_pass(r#"{"mcpServers":{}}"#, "zkp_test").is_none());
    }
}
