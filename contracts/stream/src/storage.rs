use soroban_sdk::{contracttype, Address, Env, Map, Symbol};

/// Minimum delay between proposing and executing an emergency drain (24 hours).
pub const DRAIN_TIMELOCK_SECONDS: u64 = 86_400;

/// Default maximum number of items processed by a single batch entrypoint.
pub const DEFAULT_MAX_BATCH_SIZE: u32 = 50;

/// Lower bound for the admin-configurable batch size.
pub const MIN_MAX_BATCH_SIZE: u32 = 1;

/// Upper bound for the admin-configurable batch size.
pub const MAX_MAX_BATCH_SIZE: u32 = 200;

/// Maximum number of entries allowed in a stream's metadata map.
pub const MAX_METADATA_ENTRIES: u32 = 8;

/// Error returned when a metadata map exceeds `MAX_METADATA_ENTRIES`.
pub const ERR_METADATA_TOO_LARGE: u32 = 0x1003;

/// Error returned when a configured batch size is outside the allowed bounds.
pub const ERR_MAX_BATCH_SIZE_OUT_OF_BOUNDS: u32 = 0x1004;

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

/// Persists a stream's bounded metadata map in persistent storage, extending its TTL.
///
/// Panics with `ERR_METADATA_TOO_LARGE` when the map has more than
/// `MAX_METADATA_ENTRIES` entries.
pub fn set_stream_metadata(env: &Env, stream_id: u64, metadata: &Map<Symbol, Symbol>) {
    if metadata.len() > MAX_METADATA_ENTRIES {
        panic!("{}", ERR_METADATA_TOO_LARGE);
    }
    let key = crate::DataKey::StreamMetadata(stream_id);
    env.storage().persistent().set(&key, metadata);
    env.storage().persistent().extend_ttl(
        &key,
        crate::PERSISTENT_TTL_THRESHOLD,
        crate::PERSISTENT_TTL_EXTEND_TO,
    );
}

/// Returns the metadata map stored for `stream_id`, if any.
pub fn get_stream_metadata(env: &Env, stream_id: u64) -> Option<Map<Symbol, Symbol>> {
    let key = crate::DataKey::StreamMetadata(stream_id);
    let metadata: Option<Map<Symbol, Symbol>> = env.storage().persistent().get(&key);
    if metadata.is_some() {
        env.storage().persistent().extend_ttl(
            &key,
            crate::PERSISTENT_TTL_THRESHOLD,
            crate::PERSISTENT_TTL_EXTEND_TO,
        );
    }
    metadata
}

/// Stores the admin-configurable maximum batch size in instance storage.
///
/// Panics with `ERR_MAX_BATCH_SIZE_OUT_OF_BOUNDS` when `max` is outside
/// `MIN_MAX_BATCH_SIZE..=MAX_MAX_BATCH_SIZE`.
pub fn set_max_batch_size(env: &Env, max: u32) {
    if max < MIN_MAX_BATCH_SIZE || max > MAX_MAX_BATCH_SIZE {
        panic!("{}", ERR_MAX_BATCH_SIZE_OUT_OF_BOUNDS);
    }
    env.storage()
        .instance()
        .set(&crate::DataKey::MaxBatchSize, &max);
}

/// Returns the configured maximum batch size, defaulting to `DEFAULT_MAX_BATCH_SIZE`.
pub fn get_max_batch_size(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&crate::DataKey::MaxBatchSize)
        .unwrap_or(DEFAULT_MAX_BATCH_SIZE)
}

/// Persists the protocol fee paid upfront for `stream_id`, extending its TTL.
pub fn set_stream_fee_paid(env: &Env, stream_id: u64, fee: i128) {
    let key = crate::DataKey::StreamFeePaid(stream_id);
    env.storage().persistent().set(&key, &fee);
    env.storage().persistent().extend_ttl(
        &key,
        crate::PERSISTENT_TTL_THRESHOLD,
        crate::PERSISTENT_TTL_EXTEND_TO,
    );
}

/// Returns the protocol fee recorded as paid for `stream_id`, if any.
pub fn get_stream_fee_paid(env: &Env, stream_id: u64) -> Option<i128> {
    let key = crate::DataKey::StreamFeePaid(stream_id);
    let fee: Option<i128> = env.storage().persistent().get(&key);
    if fee.is_some() {
        env.storage().persistent().extend_ttl(
            &key,
            crate::PERSISTENT_TTL_THRESHOLD,
            crate::PERSISTENT_TTL_EXTEND_TO,
        );
    }
    fee
}

/// Clears the recorded protocol fee for `stream_id` so it cannot be refunded twice.
pub fn clear_stream_fee_paid(env: &Env, stream_id: u64) {
    let key = crate::DataKey::StreamFeePaid(stream_id);
    env.storage().persistent().remove(&key);
}

/// Computes the employer's proportional share of `fee_paid` for the unstreamed
/// portion of a stream.
///
/// `remaining` is the unstreamed deposit and `total` the original deposit. The
/// result is rounded down and clamped to `fee_paid`, so the treasury never
/// refunds more than it received. Returns `0` when `total` is zero.
pub fn compute_fee_refund(fee_paid: i128, remaining: i128, total: i128) -> i128 {
    if fee_paid <= 0 || remaining <= 0 || total <= 0 {
        return 0;
    }
    let remaining = if remaining > total { total } else { remaining };
    let share = fee_paid
        .checked_mul(remaining)
        .map(|v| v / total)
        .unwrap_or(0);
    if share > fee_paid {
        fee_paid
    } else {
        share
    }
}

/// Returns true when at least `DRAIN_TIMELOCK_SECONDS` have elapsed since the proposal.
pub fn drain_timelock_elapsed(env: &Env, proposal: &EmergencyDrainProposal) -> bool {
    let now = env.ledger().timestamp();
    now.saturating_sub(proposal.proposed_at) >= DRAIN_TIMELOCK_SECONDS
}

/// Marks a stream as immutable (non-cancellable and non-pausable by the employer).
pub fn set_stream_immutable(env: &Env, stream_id: u64, immutable: bool) {
    let key = crate::DataKey::StreamImmutable(stream_id);
    env.storage().persistent().set(&key, &immutable);
    env.storage().persistent().extend_ttl(
        &key,
        crate::PERSISTENT_TTL_THRESHOLD,
        crate::PERSISTENT_TTL_EXTEND_TO,
    );
}

/// Returns true when `stream_id` was created as immutable.
pub fn is_stream_immutable(env: &Env, stream_id: u64) -> bool {
    let key = crate::DataKey::StreamImmutable(stream_id);
    let immutable: Option<bool> = env.storage().persistent().get(&key);
    if immutable.is_some() {
        env.storage().persistent().extend_ttl(
            &key,
            crate::PERSISTENT_TTL_THRESHOLD,
            crate::PERSISTENT_TTL_EXTEND_TO,
        );
    }
    immutable.unwrap_or(false)
}

/// Persists a milestone vesting schedule of `(timestamp, fraction_bps)` entries.
///
/// Fractions are expressed in basis points (10_000 = 100%). The schedule is
/// expected to be sorted by timestamp and validated by the caller.
pub fn set_milestone_schedule(env: &Env, stream_id: u64, schedule: &Vec<(u64, u32)>) {
    let key = crate::DataKey::MilestoneSchedule(stream_id);
    env.storage().persistent().set(&key, schedule);
    env.storage().persistent().extend_ttl(
        &key,
        crate::PERSISTENT_TTL_THRESHOLD,
        crate::PERSISTENT_TTL_EXTEND_TO,
    );
}

/// Returns the milestone schedule stored for `stream_id`, if any.
pub fn get_milestone_schedule(env: &Env, stream_id: u64) -> Option<Vec<(u64, u32)>> {
    let key = crate::DataKey::MilestoneSchedule(stream_id);
    let schedule: Option<Vec<(u64, u32)>> = env.storage().persistent().get(&key);
    if schedule.is_some() {
        env.storage().persistent().extend_ttl(
            &key,
            crate::PERSISTENT_TTL_THRESHOLD,
            crate::PERSISTENT_TTL_EXTEND_TO,
        );
    }
    schedule
}

/// Persists a forward rate schedule of `(timestamp, rate_per_second)` entries.
///
/// The schedule is expected to be sorted by timestamp and validated by the caller.
pub fn set_rate_schedule(env: &Env, stream_id: u64, schedule: &Vec<(u64, i128)>) {
    let key = crate::DataKey::RateSchedule(stream_id);
    env.storage().persistent().set(&key, schedule);
    env.storage().persistent().extend_ttl(
        &key,
        crate::PERSISTENT_TTL_THRESHOLD,
        crate::PERSISTENT_TTL_EXTEND_TO,
    );
}

/// Returns the rate schedule stored for `stream_id`, if any.
pub fn get_rate_schedule(env: &Env, stream_id: u64) -> Option<Vec<(u64, i128)>> {
    let key = crate::DataKey::RateSchedule(stream_id);
    let schedule: Option<Vec<(u64, i128)>> = env.storage().persistent().get(&key);
    if schedule.is_some() {
        env.storage().persistent().extend_ttl(
            &key,
            crate::PERSISTENT_TTL_THRESHOLD,
            crate::PERSISTENT_TTL_EXTEND_TO,
        );
    }
    schedule
}

/// Returns the rate in effect at `now` for `stream_id`.
///
/// Walks the stored rate schedule and returns the rate of the last entry whose
/// timestamp is `<= now`. Falls back to `stream.rate_per_second` when no entry
/// has taken effect yet (or when no schedule is stored).
pub fn active_rate_at(env: &Env, stream_id: u64, now: u64) -> i128 {
    let stream = crate::get_stream(env, stream_id);
    let mut active = stream.rate_per_second;
    if let Some(schedule) = get_rate_schedule(env, stream_id) {
        for entry in schedule.iter() {
            let (ts, rate) = entry;
            if ts <= now {
                active = rate;
            } else {
                break;
            }
        }
    }
    active
}

/// Returns the amount vested by milestones for `stream_id` at `now`.
///
/// Sums the basis-point fractions of every milestone whose timestamp is
/// `<= now` and applies them to the stream's total deposit. Milestones before
/// the stream's cliff are ignored, so a cliff is respected even if a milestone
/// timestamp precedes it.
pub fn milestone_claimable_amount(env: &Env, stream_id: u64, now: u64) -> i128 {
    let stream = crate::get_stream(env, stream_id);
    let schedule = match get_milestone_schedule(env, stream_id) {
        Some(schedule) => schedule,
        None => return 0,
    };
    let cliff = stream.cliff_time;
    let mut vested_bps: u32 = 0;
    for entry in schedule.iter() {
        let (ts, fraction_bps) = entry;
        if ts <= now && ts >= cliff {
            vested_bps = vested_bps.saturating_add(fraction_bps);
        }
    }
    if vested_bps > 10_000 {
        vested_bps = 10_000;
    }
    stream
        .deposit_amount
        .saturating_mul(vested_bps as i128)
        / 10_000
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

#[cfg(test)]
mod fee_refund_tests {
    use super::*;

    #[test]
    fn full_unstreamed_refunds_entire_fee() {
        assert_eq!(compute_fee_refund(100, 1_000, 1_000), 100);
    }

    #[test]
    fn partial_unstreamed_refunds_proportionally() {
        // Half the deposit remains unstreamed -> half the fee is refunded.
        assert_eq!(compute_fee_refund(100, 500, 1_000), 50);
        // A quarter remains -> a quarter of the fee.
        assert_eq!(compute_fee_refund(100, 250, 1_000), 25);
    }

    #[test]
    fn nothing_unstreamed_refunds_nothing() {
        assert_eq!(compute_fee_refund(100, 0, 1_000), 0);
    }

    #[test]
    fn refund_never_exceeds_fee_paid() {
        // Remaining greater than total is clamped to total.
        assert_eq!(compute_fee_refund(100, 2_000, 1_000), 100);
    }

    #[test]
    fn zero_total_refunds_nothing() {
        assert_eq!(compute_fee_refund(100, 500, 0), 0);
    }

    #[test]
    fn fee_paid_roundtrip_and_clear() {
        let env = Env::default();
        assert_eq!(get_stream_fee_paid(&env, 7), None);
        set_stream_fee_paid(&env, 7, 42);
        assert_eq!(get_stream_fee_paid(&env, 7), Some(42));
        clear_stream_fee_paid(&env, 7);
        assert_eq!(get_stream_fee_paid(&env, 7), None);
    }
}
