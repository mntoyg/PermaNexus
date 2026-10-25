# Actuator protocol contracts

Source: [actuator.proto](actuator.proto), package `permanexus.actuator.v1`.

## Endpoint status

| RPC | Contract | Current server behavior |
| --- | --- | --- |
| `ReadRegister` | Read an allowlisted register of an explicit width. | Implemented in the RAM simulator. |
| `WriteRegister` | One atomic compare-and-write with a required expected value. | Implemented in the RAM simulator. |
| `ReadTelemetry` | Read one allowlisted component/sensor, typed unit and finite value, sample time, and observed Micro-State snapshot. | Implemented for the synthetic fixtures below; no physical measurements. |
| `ApplyCommand` | Request an allowlisted component's Micro-State transition with state/revision/instance preconditions. | Contract only; generated service default returns `UNIMPLEMENTED`; no state mutation. |

Existing messages, enum numbers, RPC names, and field numbers are preserved. New messages/RPCs are additive. Older servers return `UNIMPLEMENTED` for new methods. Generated bindings are build products and remain ignored.

## Telemetry and command semantics

Component and sensor IDs contain 1..64 ASCII letters, digits, underscores, dots, or hyphens and must resolve through server configuration. Units are explicit; ratios use [0, 1]. Presence-bearing readings, timestamps, and revisions distinguish zero from absence. A successful telemetry response requires a finite reading and recognized configured unit; do not synthesize a zero when a sensor is unavailable. Timestamp metadata does not establish ordering, authenticity, or freshness.

`observed_state` captures state, revision, and a 16-byte instance ID atomically for the target component. Sensor sampling and state capture are not a multi-register transaction. A command uses that snapshot as its required precondition. The backend must validate shape, permissions, transition policy, and all preconditions before changing state. Each completed command advances revision exactly once, even when the desired state equals the existing state. Revision exhaustion is rejected before mutation; instance IDs must change after restart/reset or loss of revision history. These constraints detect stale state cycles within an instance and prevent use of a previous instance's snapshot when the implementation honors them.

`command_id` is a 16-byte correlation value, not an idempotency promise or replay-protection mechanism. It grants no retry permission. Deadlines and transport errors can hide completed commands. Only a successful response represents definitive completion; retain conservative wear accounting for unknown outcomes. State labels do not specify hardware register mappings, safe operating ranges, or physical power savings.

The schema encodes data shape; protobuf decoding alone does not enforce the validation rules in its comments. The RAM simulator now validates telemetry requests and synthetic samples, with a separate read-only per-component state store. No physical telemetry source, Micro-State executor, persistent revision store, safe Rust wrapper for these endpoints, or deduplication store is implemented. Register writes remain independent of Micro-State state/revision tracking.

## Architect handoff and acceptance gates

Before implementing the new endpoints, settle component/sensor allowlists, state-transition semantics, caller identity, revision ownership across direct register writes, and instance lifecycle. Define safe hardware completion evidence and crash/partial-transition handling. Reject unsupported atomic semantics rather than emulating them through unsafe multi-register writes. Keep the current loopback/message/deadline boundaries.

### P001 review findings and staged implementation boundary

The first implementation stage is limited to read-only telemetry in the RAM simulator. Use an explicit immutable server-side allowlist and deterministic synthetic samples that are documented as simulation fixtures; do not present them as measurements from physical sensors. Unknown component/sensor pairs return `NOT_FOUND`; unavailable or invalid synthetic samples return `UNAVAILABLE` or the contract-defined validation error. Do not accept caller-provided sensor IDs as paths, register maps, or hardware addresses.

Keep Micro-State in a separate per-component in-memory store, protected so state, revision, and instance ID are read as one snapshot. It starts in a documented simulator default state at revision zero. The server must obtain a fresh 16-byte instance ID from the operating system's random source before serving requests; failure to obtain it is a startup failure. The ID is only an incarnation/concurrency token, never authentication. State resets on restart; no persistence is implied.

Direct `ReadRegister`/`WriteRegister` operations remain owned by `RegisterBank` and do not change Micro-State or its revision. The telemetry/command milestone does not map components to registers or wear-leveling slots. The Rust planner remains independent. Any future hardware mapping requires a separate design that defines one state authority and atomicity across the mapping.

Keep `ApplyCommand` returning `UNIMPLEMENTED` until a reviewed per-component transition table and permissions are defined. Do not interpret `ACTIVE`, `IDLE`, or `LOW_POWER` as universal safe hardware actions. A future RAM-only state transition must validate the complete allowlisted transition and expected state/revision/instance under the same lock, increment revision exactly once, reject exhaustion without mutation, and report success only after the state change commits. After dispatch, a lost response remains an unknown client outcome; never retry automatically. Loopback binding provides no caller authentication, so do not describe the simulator as enforcing caller identity.

This review resolves architecture boundaries but does not select a product sensor catalog or physical transition policy. The deterministic sensor identifiers and values for the initial test fixture should be explicitly documented as simulator-only. Hardware behavior remains out of scope.

Required future checks: absent/invalid IDs and snapshots, zero readings/revisions, nonfinite samples, unknown enums, sensor-unit mismatch, stale revision/state/instance, revision exhaustion, same-state completion, contention with one winner, cancellation/unknown outcomes, and rejection without mutation. Physical hardware access requires explicit authorization and a reviewed safety design.

### Implemented synthetic fixtures (2026-10-25)

These identifiers and values are simulator test data, never physical sensor readings. No request chooses a path, register map, or sample payload.

| Device | Component | Sensor | Value | Unit | Result |
| --- | --- | --- | ---: | --- | --- |
| `simulated-0` | `core-0` | `synthetic-temperature` | 42.5 | Celsius | `OK` |
| `simulated-0` | `core-0` | `synthetic-utilization` | 0.0 | Ratio | `OK` |
| `simulated-0` | `core-1` | `synthetic-voltage` | 1.0 | Volts | `OK` |
| `simulated-0` | `core-1` | `synthetic-unavailable` | Absent | Volts | `UNAVAILABLE`, no sample |

Successful fixtures return a present `sampled_at_unix_ns = 0`, a deterministic epoch-zero timestamp rather than the current read time. Each component has a separate mutex-protected immutable `ACTIVE`/revision-zero snapshot, sharing the process's 16-byte instance ID. There is no transition API. Reads, errors, and direct register writes never advance revisions. No register or wear-planner slot is mapped to a component.

Startup obtains exactly 16 bytes through Windows `BCryptGenRandom`, Linux `getrandom` (handling partial reads/interruption), or macOS `getentropy`; other platforms fail closed. An unavailable random source prevents startup before binding. There is no clock/PRNG/previous-ID fallback. Windows execution is verified; Linux/macOS paths remain unexecuted. The ID provides no caller identity or authentication.

Missing targets or malformed/empty/overlong IDs return `INVALID_ARGUMENT`. Well-formed unknown devices, components, sensors, and cross-component sensor pairs return `NOT_FOUND`. An unavailable or invalid server-side sample (absent/nonfinite value, invalid/mismatched unit, missing timestamp, or out-of-range ratio) cannot become a successful reading and maps to `UNAVAILABLE`. Unknown protobuf fields remain compatible with decoding; responses echo only the canonical validated identifiers. Existing 4096-byte message bounds and loopback binding remain enforced. `ApplyCommand` still returns `UNIMPLEMENTED`.

Verification covers C++ sample/status validation, injected random-source failure with no issued identity, 16 concurrent backend readers, and real C++/Rust RPCs including 32 concurrent readers, register independence, zero presence, invalid/unknown pairs, unavailable samples, message bounds, and fresh instance identity after restart. No forced OS-failure server subprocess or Linux/macOS run is claimed. Published field numbers/messages/RPC signatures are unchanged; only an outdated `.proto` comment was corrected. Run `make check test test-grpc`; see [platform commands](../README.md) and [local dependency setup](../handoff.md).
