# Actuator protocol contracts

Source: [actuator.proto](actuator.proto), package `permanexus.actuator.v1`.

## Endpoint status

| RPC | Contract | Current server behavior |
| --- | --- | --- |
| `ReadRegister` | Read an allowlisted register of an explicit width. | Implemented in the RAM simulator. |
| `WriteRegister` | One atomic compare-and-write with a required expected value. | Implemented in the RAM simulator. |
| `ReadTelemetry` | Read one allowlisted component/sensor, typed unit and finite value, sample time, and observed Micro-State snapshot. | Contract only; generated service default returns `UNIMPLEMENTED`. |
| `ApplyCommand` | Request an allowlisted component's Micro-State transition with state/revision/instance preconditions. | Contract only; generated service default returns `UNIMPLEMENTED`; no state mutation. |

Existing messages, enum numbers, RPC names, and field numbers are preserved. New messages/RPCs are additive. Older servers return `UNIMPLEMENTED` for new methods. Generated bindings are build products and remain ignored.

## Telemetry and command semantics

Component and sensor IDs contain 1..64 ASCII letters, digits, underscores, dots, or hyphens and must resolve through server configuration. Units are explicit; ratios use [0, 1]. Presence-bearing readings, timestamps, and revisions distinguish zero from absence. A successful telemetry response requires a finite reading and recognized configured unit; do not synthesize a zero when a sensor is unavailable. Timestamp metadata does not establish ordering, authenticity, or freshness.

`observed_state` captures state, revision, and a 16-byte instance ID atomically for the target component. Sensor sampling and state capture are not a multi-register transaction. A command uses that snapshot as its required precondition. The backend must validate shape, permissions, transition policy, and all preconditions before changing state. Each completed command advances revision exactly once, even when the desired state equals the existing state. Revision exhaustion is rejected before mutation; instance IDs must change after restart/reset or loss of revision history. These constraints detect stale state cycles within an instance and prevent use of a previous instance's snapshot when the implementation honors them.

`command_id` is a 16-byte correlation value, not an idempotency promise or replay-protection mechanism. It grants no retry permission. Deadlines and transport errors can hide completed commands. Only a successful response represents definitive completion; retain conservative wear accounting for unknown outcomes. State labels do not specify hardware register mappings, safe operating ranges, or physical power savings.

The schema encodes data shape; protobuf decoding alone does not enforce the validation rules in its comments. No telemetry source, Micro-State executor, revision store, safe Rust wrapper for these endpoints, or deduplication store is implemented in this change. Existing register writes are separate from future Micro-State state/revision tracking; architect review must define their ownership and interaction before enabling commands.

## Architect handoff and acceptance gates

Before implementing the new endpoints, settle component/sensor allowlists, state-transition semantics, caller identity, revision ownership across direct register writes, and instance lifecycle. Define safe hardware completion evidence and crash/partial-transition handling. Reject unsupported atomic semantics rather than emulating them through unsafe multi-register writes. Keep the current loopback/message/deadline boundaries.

Required future checks: absent/invalid IDs and snapshots, zero readings/revisions, nonfinite samples, unknown enums, sensor-unit mismatch, stale revision/state/instance, revision exhaustion, same-state completion, contention with one winner, cancellation/unknown outcomes, and rejection without mutation. Physical hardware access requires explicit authorization and a reviewed safety design.

Current verification generates and compiles C++/Rust bindings, checks Rust wire-presence/enum preservation, and uses the real C++/Rust integration suite to confirm the new endpoints return `UNIMPLEMENTED` without changing existing registers. Run `make check test test-grpc` with compatible dependencies; see [platform commands](../README.md) and [local dependency setup](../handoff.md).
