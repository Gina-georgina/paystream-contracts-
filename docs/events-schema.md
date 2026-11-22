# PayStream Event Schema

This document describes the structured event schema for PayStream smart contracts. Off-chain indexers can use this schema to parse Soroban events correctly and detect schema changes across contract upgrades.

## Changelog

Indexer-facing record of event schema changes. **Process:** any PR that adds, renames or removes an event, topic or data field must add an entry here (and bump the schema version for breaking changes).

Each entry lists the event, the change (added field / renamed topic / removed field) and migration steps for indexers.

### v1.0 (2026-09-27) — baseline

| Event | Change | Migration |
|-------|--------|-----------|
| `init`, `created`, `withdraw`, `status`, `cancelled`, `topup`, `paused`, `rate_upd`, `stop_upd`, `near_exp`, `exp_unres`, `upg_prop`, `upg_exec`, `upg_cncl`, `at_set`, `at_up`, `thresh_up`, `split`, `p_stats` | Baseline: topic prefixed with `"v1"` | None. Match on `topic[0] == "v1"`, then `topic[1]` |
| `receipt`, `archived`, `expired`, `xfr_strm`, `low_bal` | Baseline: emitted **without** a version prefix | Match on `topic[0]` directly. A future version will add the `"v1"` prefix and list it here |

### v1.1 (2026-10-01) — event version stamp & migrate progress

| Event | Change | Migration |
|-------|--------|-----------|
| `contract_event_version` (instance storage) | Added: instance value holding the contract event schema version (`u32`, currently `1`). Written on `initialize` and on `migrate` | Read once via `get_contract_event_version` (or the instance ledger entry) at indexer startup; no resync needed. Absent on contracts deployed before v1.1 — treat as `1` |
| `evt_ver` | Added: emitted on `initialize`/`migrate` with the stamped version | Optional. Use to detect version bumps without polling instance storage |
| `mig_prog` | Added: emitted by `migrate` with backfill progress (`done`, `total`) | Optional. Use to track legacy-stream backfill; no schema change to existing events |
=======

<!-- Template for new entries:
### vX.Y (YYYY-MM-DD)
| Event | Change | Migration |
|-------|--------|-----------|
| `name` | Added field `foo` (type) at data[n] | Treat missing `foo` as default for events before vX.Y |
-->

## Schema Version

Current schema version: **v1**

All event topics include the schema version as the first component to enable version detection and compatibility handling.

### Contract Event Version (instance value)

In addition to the per-topic prefix, the contract stores a **contract event version** as an instance value under the key `contract_event_version` (`u32`). It is set on `initialize` and re-stamped on `migrate`, so indexers can read the current schema version directly from contract state without replaying history or resyncing.

- **Key**: `contract_event_version` (instance storage)
- **Type**: `u32`
- **Current value**: `1`
- **Written by**: `initialize`, `migrate`
- **Read by indexers**: via the `get_contract_event_version` view, or by reading the instance ledger entry
- **Legacy contracts**: if the value is absent (deployed before this stamp), assume `1`

When the event schema changes in a breaking way, this value is bumped together with the topic prefix (e.g. `"v2"`), and the change is recorded in the changelog above.

## Event Format

Soroban events have two parts:
- **Topic**: An array that identifies the event type and associated keys
- **Data**: The event payload values

Topics always follow this pattern:
```
[<schema_version>, <event_name>, ...event_specific_keys]
```

## Events

### contract_initialized

Emitted when the contract is initialized with an admin address.

**Topic**: `["v1", "init"]`

**Data**: 
- `admin` (Address): The contract admin address

**Example**:
```json
{
  "topic": ["v1", "init"],
  "data": ["GABCD..."]
}
```

---

### stream_created

Emitted when a new salary stream is created.

**Topic**: `["v1", "created", <stream_id>]`

**Data**:
- `employer` (Address): The employer address
- `employee` (Address): The employee address
- `rate` (i128): Tokens streamed per second

**Example**:
```json
{
  "topic": ["v1", "created", 1],
  "data": ["GABCD...", "GEFGH...", 1000000]
}
```

---

### withdrawn

Emitted when an employee withdraws tokens from a stream.

**Topic**: `["v1", "withdraw", <stream_id>]`

**Data**:
- `employee` (Address): The employee address
- `amount` (i128): Amount withdrawn

**Example**:
```json
{
  "topic": ["v1", "withdraw", 1],
  "data": ["GEFGH...", 500000]
}
```

---

### stream_status_changed

Emitted when a stream's status changes (Active, Paused, Cancelled, Exhausted).

**Topic**: `["v1", "status", <stream_id>]`

**Data**:
- `status` (StreamStatus enum): The new status
  - `0`: Active
  - `1`: Paused
  - `2`: Cancelled
  - `3`: Exhausted

**Example**:
```json
{
  "topic": ["v1", "status", 1],
  "data": [1]
}
```

---

### stream_cancelled

Emitted when a stream is cancelled, recording the exact cash-flow split.

**Topic**: `["v1", "cancelled", <stream_id>]`

**Data**:
- `claimable_amount` (i128): Tokens paid to the employee at cancellation time
- `refund_amount` (i128): Tokens returned to the employer (unstreamed portion)

**Example**:
```json
{
  "topic": ["v1", "cancelled", 1],
  "data": [300000, 700000]
}
```

---

### topped_up

Emitted when an employer adds more funds to an existing stream.

**Topic**: `["v1", "topup", <stream_id>]`

**Data**:
- `employer` (Address): The employer address
- `amount` (i128): Amount added to the stream

**Example**:
```json
{
  "topic": ["v1", "topup", 1],
  "data": ["GABCD...", 200000]
}
```

---

### auto_topup_set

Emitted by `set_auto_topup` (`enabled = true`) and `cancel_auto_topup` (`enabled = false`).

**Topic**: `["v1", "at_set", <stream_id>]`

**Data**:
- `enabled` (bool): `true` when configured, `false` when cancelled

**Example**:
```json
{
  "topic": ["v1", "at_set", 1],
  "data": true
}
```

---

### auto_topped_up

Emitted after a `withdraw` / `withdraw_all` when an auto top-up successfully pulls funds from the employer.

**Topic**: `["v1", "at_up", <stream_id>]`

**Data**:
- `amount` (i128): Amount added to the stream by this top-up
- `total_topped_up` (i128): Cumulative auto top-up total after this top-up

**Example**:
```json
{
  "topic": ["v1", "at_up", 1],
  "data": [500000, 500000]
}
```

---

### contract_paused

Emitted when the contract-wide pause flag is toggled.

**Topic**: `["v1", "paused"]`

**Data**:
- `paused` (bool): Whether the contract is paused

**Example**:
```json
{
  "topic": ["v1", "paused"],
  "data": [true]
}
```

---

### rate_updated

Emitted when a stream's rate_per_second is changed.

**Topic**: `["v1", "rate_upd", <stream_id>]`

**Data**:
- `old_rate` (i128): Previous rate per second
- `new_rate` (i128): New rate per second

**Example**:
```json
{
  "topic": ["v1", "rate_upd", 1],
  "data": [1000000, 2000000]
}
```

---

### cliff_reached

Emitted once per stream the first time it is touched (`withdraw`,
`withdraw_all`, `cancel_stream`, `settle_stream`) at or after its cliff
(PROD-17). Streams without a cliff never emit it.

**Topic**: `["v1", "cliff", <stream_id>]`

**Data**:
- `cliff_time` (u64): The stream's cliff timestamp

**Example**:
```json
{
  "topic": ["v1", "cliff", 7],
  "data": 1767225600
}
```

---

### upgrade_proposed

Emitted when a contract upgrade is proposed.

**Topic**: `["v1", "upg_prop"]`

**Data**:
- `wasm_hash` (BytesN<32>): The hash of the new WASM blob
- `unlock_time` (u64): Timestamp when the upgrade can be executed

**Example**:
```json
{
  "topic": ["v1", "upg_prop"],
  "data": ["abcd1234...", 1234567890]
}
```

---

### upgrade_executed

Emitted when a contract upgrade is executed.

**Topic**: `["v1", "upg_exec"]`

**Data**:
- `wasm_hash` (BytesN<32>): The hash of the executed WASM blob

**Example**:
```json
{
  "topic": ["v1", "upg_exec"],
  "data": ["abcd1234..."]
}
```

---

### upgrade_cancelled

Emitted when a pending contract upgrade is cancelled.

**Topic**: `["v1", "upg_cncl"]`

**Data**: (empty)

**Example**:
```json
{
  "topic": ["v1", "upg_cncl"],
  "data": []
}
```

---

### event_version_stamped

Emitted when the contract event version is stamped on `initialize` or re-stamped on `migrate`.

**Topic**: `["v1", "evt_ver"]`

**Data**:
- `version` (u32): The contract event version now stored in instance storage

**Example**:
```json
{
  "topic": ["v1", "evt_ver"],
  "data": [1]
}
```

---

### migrate_progress

Emitted by `migrate` while backfilling indexes for legacy streams. Bounded per call; emitted once per invocation with the progress after that call.

**Topic**: `["v1", "mig_prog"]`

**Data**:
- `done` (u32): Number of legacy streams backfilled so far
- `total` (u32): Total number of legacy streams to backfill

**Example**:
```json
{
  "topic": ["v1", "mig_prog"],
  "data": [50, 200]
}
```

## Type Reference

| Type | Description | Example |
|------|-------------|---------|
| Address | Stellar address (32 bytes) | `GABCD...` |
| i128 | Signed 128-bit integer | `1000000` |
| u64 | Unsigned 64-bit integer | `1234567890` |
| bool | Boolean | `true` or `false` |
| BytesN<32> | Fixed 32-byte array | `abcd1234...` |
| StreamStatus enum | Stream status (0-3) | `0`, `1`, `2`, or `3` |

## Indexer Compatibility Guide

### Detecting Schema Version

To detect the schema version from an event topic:

1. Parse the first element of the topic array
2. If it matches `"v1"`, use the v1 schema
3. If it's an unknown version, either:
   - Fail gracefully and log a warning
   - Implement a migration path if you have the schema documentation for that version

### Handling Schema Changes

When a new schema version is released:

1. Update your indexer to support the new version
2. Keep support for old versions to parse historical events
3. Use the version prefix to route events to the correct parser
4. Monitor for unknown versions and alert for schema updates

### Example Pseudocode

```javascript
function parseEvent(topic, data) {
  const version = topic[0];
  
  switch (version) {
    case "v1":
      return parseV1Event(topic, data);
    case "v2":
      return parseV2Event(topic, data);
    default:
      console.warn(`Unknown schema version: ${version}`);
      return null;
  }
}

function parseV1Event(topic, data) {
  const eventName = topic[1];
  
  switch (eventName) {
    case "created":
      return {
        type: "stream_created",
        streamId: topic[2],
        employer: data[0],
        employee: data[1],
        rate: data[2]
      };
    // ... other events
  }
}
```

## Schema Migration History

| Version | Date | Changes |
|---------|------|---------|
| v1 | 2026-09-27 | Initial schema version |
| v1.1 | 2026-10-01 | Added `contract_event_version` instance value (stamped on `initialize`/`migrate`), `evt_ver` and `mig_prog` events. No change to existing event topics or data |

## Testing

Event schemas are validated in the contract test suite. See `contracts/stream/src/test.rs` for tests that verify event structure matches this documentation.
