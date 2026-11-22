use soroban_sdk::{contracttype, Address, Env};

/// Minimum delay between proposing and executing an emergency drain (24 hours).
pub const DRAIN_TIMELOCK_SECONDS: u64 = 86_400;

/// Error returned when an emergency drain is attempted before the time-lock elapses.
pub const ERR_DRAIN_TIMELOCK_ACTIVE: u32 = 0x1001;

/// Error returned when a WASM upgrade would downgrade or re-apply the current version.
pub const ERR_VERSION_DOWNGRADE: u32 = 0x1002;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyDrainProposal {
    pub recipient: Address,
    pub proposed_at: u64,
}

const DRAIN_PROPOSAL_KEY: &str = "drain_proposal";

pub fn set_drain_proposal(env: &Env, proposal: &EmergencyDrainProposal) {
    env.storage().instance().set(&DRAIN_PROPOSAL_KEY, proposal);
}

pub fn get_drain_proposal(env: &Env) -> Option<EmergencyDrainProposal> {
    env.storage().instance().get(&DRAIN_PROPOSAL_KEY)
}

pub fn clear_drain_proposal(env: &Env) {
    env.storage().instance().remove(&DRAIN_PROPOSAL_KEY);
}

/// Returns true when at least `DRAIN_TIMELOCK_SECONDS` have elapsed since the proposal.
pub fn drain_timelock_elapsed(env: &Env, proposal: &EmergencyDrainProposal) -> bool {
    let now = env.ledger().timestamp();
    now.saturating_sub(proposal.proposed_at) >= DRAIN_TIMELOCK_SECONDS
}

/// Panics with `ERR_VERSION_DOWNGRADE` unless `new_version` is strictly greater
/// than the stored `DataKey::Version`. Re-applying the same version is rejected.
pub fn assert_version_upgrade(env: &Env, new_version: u32) {
    let stored: u32 = env
        .storage()
        .instance()
        .get(&crate::DataKey::Version)
        .unwrap_or(0);
    if new_version <= stored {
        panic!("{}", ERR_VERSION_DOWNGRADE);
    }
}

/// Stores the contract event schema version in instance storage.
pub fn set_contract_event_version(env: &Env, version: u32) {
    env.storage()
        .instance()
        .set(&crate::DataKey::ContractEventVersion, &version);
}

/// Returns the contract event schema version, or 0 when never set.
pub fn get_contract_event_version(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&crate::DataKey::ContractEventVersion)
        .unwrap_or(0)
}

/// Appends `stream_id` to the composite employer+token index for `(employer, token)`.
/// Idempotent: a stream already present in the index is not added again.
pub fn index_employer_token_stream(
    env: &Env,
    employer: &Address,
    token: &Address,
    stream_id: u64,
) {
    let key = crate::DataKey::EmployerTokenStreams(employer.clone(), token.clone());
    let mut ids: soroban_sdk::Vec<u64> = env.storage().persistent().get(&key).unwrap_or_else(|| {
        soroban_sdk::Vec::new(env)
    });
    for existing in ids.iter() {
        if existing == stream_id {
            return;
        }
    }
    ids.push_back(stream_id);
    env.storage().persistent().set(&key, &ids);
}

/// Returns the stream ids indexed for `(employer, token)`, or an empty vector.
pub fn get_employer_token_streams(
    env: &Env,
    employer: &Address,
    token: &Address,
) -> soroban_sdk::Vec<u64> {
    let key = crate::DataKey::EmployerTokenStreams(employer.clone(), token.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

/// Returns the schema version the migrate backfill has completed up to (0 = never run).
pub fn get_migrate_version(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&crate::DataKey::MigrateVersion)
        .unwrap_or(0)
}

/// Records the schema version the migrate backfill has completed up to.
pub fn set_migrate_version(env: &Env, version: u32) {
    env.storage()
        .instance()
        .set(&crate::DataKey::MigrateVersion, &version);
}

/// Returns the resume cursor for a bounded migrate call (0 = start from beginning).
pub fn get_migrate_cursor(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&crate::DataKey::MigrateCursor)
        .unwrap_or(0)
}

/// Persists the resume cursor for a bounded migrate call.
pub fn set_migrate_cursor(env: &Env, cursor: u64) {
    env.storage()
        .instance()
        .set(&crate::DataKey::MigrateCursor, &cursor);
}

/// Clears the migrate cursor once a backfill pass has completed.
pub fn clear_migrate_cursor(env: &Env) {
    env.storage().instance().remove(&crate::DataKey::MigrateCursor);
}

#[cfg(test)]
mod version_tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    #[should_panic]
    fn downgrade_attempt_panics() {
        let env = Env::default();
        env.storage().instance().set(&crate::DataKey::Version, &2u32);
        assert_version_upgrade(&env, 1);
    }

    #[test]
    #[should_panic]
    fn same_version_upgrade_panics() {
        let env = Env::default();
        env.storage().instance().set(&crate::DataKey::Version, &2u32);
        assert_version_upgrade(&env, 2);
    }

    #[test]
    fn higher_version_upgrade_allowed() {
        let env = Env::default();
        env.storage().instance().set(&crate::DataKey::Version, &2u32);
        assert_version_upgrade(&env, 3);
    }
}
