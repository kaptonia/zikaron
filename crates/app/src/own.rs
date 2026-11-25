use std::path::Path;

pub fn home_follows(from: &Path, to: &Path) -> Result<(), crate::fault::Fault> {
    let _ = (from, to);
    Ok(())
}

pub fn on_chain(shell: &crate::shell::Shell, rows: &[crate::ledgerx::Row]) -> Vec<(String, u64)> {
    use crate::ledgerx::Lamp;
    let report = shell.audit.as_ref().map(|a| a.report.clone()).unwrap_or(zikaron::json::Value::Null);
    let mut on_chain: Vec<(String, u64)> = Vec::new();
    for q in shell.queue.items.iter().filter(|q| !q.step.in_flight()) {
        if rows.iter().any(|r| r.id == q.id && r.lamp == Lamp::Anchored && crate::auditx::held_on(&report, q.chain, &q.id, &r.author)) {
            on_chain.push((q.id.clone(), q.chain));
        }
    }
    on_chain
}

pub fn owed(shell: &crate::shell::Shell, rows: &[crate::ledgerx::Row]) -> Vec<(String, bool)> {
    use crate::ledgerx::Lamp;
    let table_read = matches!(shell.persona, crate::facex::State::Read(_));
    let homes = |r: &crate::ledgerx::Row| r.kind == zikaron_glue::book::Kind::Face && r.facts.face.as_ref().is_some_and(|(s, _, _)| *s == "home");
    let last_home = rows.iter().filter(|r| homes(r)).map(|r| r.seq).max();
    let superseded = |r: &crate::ledgerx::Row| homes(r) && Some(r.seq) != last_home;
    let ledger_unanchored = shell.audit.as_ref().map(|a| crate::auditx::entry_ids(&a.report, zikaron::v2::tokens::Key::Unanchored)).unwrap_or_default();
    let noticed: Vec<String> = rows.iter().filter(|r| r.kind == zikaron_glue::book::Kind::Grant && r.facts.member).map(|r| r.id.to_ascii_lowercase()).collect();
    let by_notice = |r: &crate::ledgerx::Row| r.kind == zikaron_glue::book::Kind::Revocation && r.facts.subject.as_deref().is_some_and(|g| noticed.contains(&g.to_ascii_lowercase()));
    rows.iter()
        .filter(|r| r.lamp == Lamp::Landed && ledger_unanchored.contains(&r.id.to_ascii_lowercase()) && !by_notice(r))
        .filter(|r| table_read || r.kind != zikaron_glue::book::Kind::Face)
        .filter(|r| !superseded(r))
        .map(|r| (r.id.clone(), r.kind.goes_alone()))
        .collect()
}

pub fn receipts_behind(shell: &crate::shell::Shell) -> Vec<String> {
    let Some(a) = shell.audit.as_ref() else { return Vec::new() };
    let unanchored = crate::auditx::entry_ids(&a.report, zikaron::v2::tokens::Key::Unanchored);
    let mut ids: Vec<String> = shell.queue.blocks.iter().map(|b| b.id.to_ascii_lowercase()).filter(|id| unanchored.contains(id)).collect();
    ids.sort();
    ids.dedup();
    ids
}

pub const DEPLOYMENTS: [crate::deploy::Deployment; 4] = [
    deploy_row(0, crate::lang::Key::DeployMainnet),
    deploy_row(1, crate::lang::Key::DeploySepolia),
    deploy_row(2, crate::lang::Key::ChainArbitrumOne),
    deploy_row(3, crate::lang::Key::ChainOpMainnet),
];

const fn deploy_row(i: usize, label: crate::lang::Key) -> crate::deploy::Deployment {
    let r = zikaron_anchor::deploy::ROWS[i];
    crate::deploy::Deployment { name: r.name, label, chain_id: r.chain_id, registry: r.registry, from_block: r.from_block, nodes: r.nodes, own: zikaron_anchor::revoke::row(r.chain_id) }
}

pub type DeployOwn = Option<zikaron_anchor::revoke::Row>;

pub const DEPLOY_DEFAULT: &str = "mainnet";

pub const DEPLOY_CODE_HASH: &str = zikaron_anchor::deploy::CODE_HASH;

#[cfg(test)]
mod deploy_own {
    use crate::deploy::{named, KNOWN};

    #[test]
    fn the_revocation_contract_stands_on_the_rows_that_deploy_it() {
        for d in KNOWN {
            assert_eq!(d.own, zikaron_anchor::revoke::row(d.chain_id), "{}", d.name);
        }
        let sepolia = named("sepolia").and_then(|d| d.own).expect("Sepolia's row names the revocation contract");
        assert_eq!((sepolia.at, sepolia.from_block), ("0x506E536d4b463f48BB053Fe7C3DF265BC21B02Fe", 12_101_379));
        for (name, block) in [("mainnet", 26_387_028), ("arbitrum-one", 523_089_554), ("op-mainnet", 159_372_720)] {
            let r = named(name).and_then(|d| d.own).unwrap_or_else(|| panic!("{name}'s row names the revocation contract"));
            assert_eq!((r.at, r.from_block), ("0x0Af771f809358FcC2afd13Bdff98fb7cd6c9Fe42", block), "{name}");
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OwnStep {
    Network,
    Genesis,
    Gas,
}

impl OwnStep {
    pub fn title(self) -> crate::lang::Key {
        match self {
            OwnStep::Network => crate::lang::Key::SetChain,
            OwnStep::Gas => crate::lang::Key::WizGasTitle,
            OwnStep::Genesis => crate::lang::Key::WizGenesisTitle,
        }
    }

    pub fn deferrable(self, _role: crate::roles::Role) -> bool {
        match self {
            OwnStep::Network => false,
            OwnStep::Gas | OwnStep::Genesis => true,
        }
    }

    pub fn opens(self, role: crate::roles::Role) -> bool {
        self == OwnStep::Genesis && role == crate::roles::Role::Author
    }
}

pub const WALK: [crate::steps::Step; 6] = [
    crate::steps::Step::Pin,
    crate::steps::Step::Key,
    crate::steps::Step::Own(OwnStep::Network),
    crate::steps::Step::Own(OwnStep::Genesis),
    crate::steps::Step::Own(OwnStep::Gas),
    crate::steps::Step::Backup,
];

#[allow(non_upper_case_globals)]
impl crate::steps::Step {
    pub const Network: crate::steps::Step = crate::steps::Step::Own(OwnStep::Network);
    pub const Genesis: crate::steps::Step = crate::steps::Step::Own(OwnStep::Genesis);
    pub const Gas: crate::steps::Step = crate::steps::Step::Own(OwnStep::Gas);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OwnProgress {
    pub network: bool,
    pub gas: bool,
    pub genesis: bool,
}

impl OwnProgress {
    pub fn of(shell: &crate::shell::Shell) -> OwnProgress {
        OwnProgress {
            network: shell.machine.network.is_some(),
            gas: matches!(shell.chain_reading(), Ok(Some(crate::task::Done::Chain { gas_wei: Some(w), .. })) if *w > 0),
            genesis: shell.rooted,
        }
    }

    pub fn done(&self, s: OwnStep) -> bool {
        match s {
            OwnStep::Network => self.network,
            OwnStep::Gas => self.gas,
            OwnStep::Genesis => self.genesis,
        }
    }
}
