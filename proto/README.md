# Actuator protocol contracts

Source: [actuator.proto](actuator.proto), package `permanexus.actuator.v1`.

## Endpoint status

| RPC | Contract | Current server behavior |
| --- | --- | --- |
| `ReadRegister` | Read an allowlisted register of an explicit width. | Implemented in the RAM simulator. |
| `WriteRegister` | One atomic compare-and-write with a required expected value. | Implemented in the RAM simulator. |
| `ReadTelemetry` | Read one allowlisted component/sensor, typed unit and finite value, sample time, and observed Micro-State snapshot. | Implemented for the synthetic fixtures below; no physical measurements. |
| `ApplyCommand` | Request an allowlisted component's Micro-State transition with state/revision/instance preconditions. | C++ RAM-only handler enabled for the two approved simulator components; no hardware mutation. |

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

The per-component transition table and simulator access boundary are approved and implemented for the C++ RAM-only handler in P001 Phase 1. Phase 2 adds the one-shot Rust client method and targeted real C++/Rust gRPC coverage for transitions, preconditions, snapshots, contention, component/register isolation, loopback, and message bounds. Phase 3A Windows verification and the final diff/whitespace review passed; see the dated evidence below and `progress.md`. No state is mapped to hardware actions. The handler validates the allowlisted transition and expected state/revision/instance under the same component lock, increments revision exactly once, rejects exhaustion without mutation, and reports success only after the state change commits. After dispatch, a lost response remains an unknown client outcome; never retry automatically. Loopback binding provides no caller authentication, so do not describe the simulator as enforcing caller identity. Post-commit response loss and deadline expiry remain untested; no production fault injection was added. P001 remains in progress because those limits and further owner review remain.

This review resolves architecture boundaries but does not select a product sensor catalog or physical transition policy. The deterministic sensor identifiers and values for the initial test fixture should be explicitly documented as simulator-only. Hardware behavior remains out of scope.

Required future checks: absent/invalid IDs and snapshots, zero readings/revisions, nonfinite samples, unknown enums, sensor-unit mismatch, stale revision/state/instance, revision exhaustion, same-state completion, contention with one winner, cancellation/unknown outcomes, and rejection without mutation. Physical hardware access requires explicit authorization and a reviewed safety design.

### P001 ApplyCommand architecture review (2026-10-27)

The owner approved this policy for the RAM simulator on 2026-10-27. Phase 1 implements the C++ in-memory handler and focused C++ tests. It mutates only the separate per-component Micro-State for the two allowlisted targets. It does not write registers, wear-planner slots, or hardware. Phase 2 gRPC/Rust integration, Phase 3A Windows verification, and final diff/whitespace review have passed. Deterministic unknown-transport-outcome coverage remains unverified; P001 is not complete.

Approved simulator transition policy, applied independently and identically to `simulated-0/core-0` and `simulated-0/core-1`:

| Current state | Desired `ACTIVE` | Desired `IDLE` | Desired `LOW_POWER` |
| --- | --- | --- | --- |
| `ACTIVE` | Allow as a same-state command; increment revision | Allow | Allow |
| `IDLE` | Allow | Allow as a same-state command; increment revision | Allow |
| `LOW_POWER` | Allow | Deny with `PERMISSION_DENIED`; transition through `ACTIVE` | Allow as a same-state command; increment revision |

All cross-state allows mean only that the RAM simulator may replace one enum with another. They do not assert that these transitions are safe, physically meaningful, energy-saving, or supported by any device. Requiring `LOW_POWER -> ACTIVE -> IDLE` is the approved simulator policy; the two component entries do not describe hardware policy.

Authorization boundary: permit only the approved transition-table edges for a known component; deny other edges with `PERMISSION_DENIED`. There is no caller identity in the contract or current transport, and loopback is not authentication. Any local process able to connect can issue simulator commands. This must not be represented as caller authentication or as suitable for real actuator control. No per-caller role or credential is implemented.

Validation and atomic commit order:

1. Validate request/target/snapshot presence; `command_id` and expected `instance_id` must each be exactly 16 bytes; validate all IDs, known non-`UNSPECIFIED` state enums, and expected revision presence. Invalid shape returns `INVALID_ARGUMENT` without mutation.
2. Resolve the well-formed target through the immutable component allowlist. Unknown devices/components return `NOT_FOUND` without mutation.
3. Acquire only that component's Micro-State lock. Check expected instance ID first (`FAILED_PRECONDITION`), then expected state/revision (`ABORTED`), then transition permission (`PERMISSION_DENIED`), and then revision exhaustion (`FAILED_PRECONDITION`). Each rejection leaves state unchanged.
4. Check cancellation immediately before commit while holding the component lock. If cancellation is observed, return `CANCELLED` without mutation. Otherwise capture the previous snapshot, set the requested state, increment revision exactly once (including a same-state command), and capture the current snapshot while still holding the same lock. Return `OK` only after this in-memory commit; echo the validated command ID and target and return both snapshots.

Every request contending on the same expected snapshot serializes on the component lock: at most one can commit; later requests observe a stale precondition and return `ABORTED`. Independent component locks allow separate targets to progress independently. Instance IDs remain process-incarnation tokens, not credentials. Micro-State revisions reset only with a new process instance; failure to obtain a fresh OS-random instance ID remains a startup failure.

Cancellation and ambiguous outcomes: cancellation observed before entering the commit critical section may return `CANCELLED` with no mutation. Once dispatch has reached the commit boundary, cancellation can race the mutation and response delivery; a client timeout, disconnect, `CANCELLED`, or other transport error does not prove the command did not commit. Only a received `OK` with the result is definitive success. Never retry automatically, including with the same `command_id`; that field is correlation only and there is no deduplication store. A later `ReadTelemetry` can show current state/revision but cannot prove which command caused it, especially after same-state commands or concurrent commands. The client must preserve an unknown outcome and seek explicit reconciliation; the simulator must not claim exactly-once execution.

Phase 1 C++ implementation and test evidence:

- C++ handler and backend support the approved state table for both components, validate command shape/allowlists/enums/snapshots, enforce the specified status precedence, check cancellation at the precommit boundary, and commit coherent previous/current snapshots under a per-component lock.
- Focused CTest targets `micro_state_store_test`, `actuator_service_test`, and `telemetry_store_test` passed on Windows. Backend tests cover every transition cell for both components, rejection/no-mutation cases, revision exhaustion, a cancellation callback, independent component snapshots, and 32 requests contending on one snapshot (one winner). Service tests cover status mapping, command snapshots, and register-bank independence.
- These are C++ unit/service tests, not real transport tests. They do not establish client behavior after timeout/disconnect, Rust interoperability, or planner-state observation.

Phase 2 acceptance status and remaining limits:

- The protobuf schema, field numbers, and RPC semantics were unchanged. One targeted real C++/Rust gRPC test passed and verifies concurrent transport calls, message bounds, and the loopback endpoint; the client method sets the existing two-second deadline.
- The command path does not access or map the Rust wear planner. Planner state was not directly observed in the RPC test.
- No deterministic transport-loss-after-commit test was run. Client behavior after a real disconnect and retry counting under transport failure remain unverified; no test-only faulting server or production fault injection was added. Source inspection confirms one RPC call site with no retry loop.
- Loopback-only binding, message limits, no hardware access, and no caller-authentication claims remain required.

Owner decision (2026-10-27): the owner approved the proposed transition table as written for both `simulated-0/core-0` and `simulated-0/core-1`, for the RAM simulator only. The exception `LOW_POWER -> IDLE` remains denied and must transition through `ACTIVE`. Same-state successful commands increment revision exactly once under the existing contract. This does not authorize hardware behavior or network exposure.

Reviewer note (2026-10-27): the existing contract already requires every successful command, including a same-state command, to increment revision exactly once. Treat that behavior as fixed for this implementation; changing it would require a separate contract decision. For deterministic stale-request semantics, check expected instance/state/revision under the component lock before checking a state-dependent transition edge. This ensures a stale snapshot returns the contract's `FAILED_PRECONDITION`/`ABORTED` result instead of revealing whether the current state permits that edge. Preserve the current unauthenticated loopback-only simulator boundary: any local process able to connect may issue commands, and this is not caller authentication.

Implementation scope: enable `ApplyCommand` only for the in-memory allowlisted components and approved table above. Preserve the existing unauthenticated loopback-only boundary; any local process able to connect can issue simulator commands, so this is not caller authentication. Keep the RPC unavailable outside loopback and do not map commands to registers, wear-planner slots, or hardware. Validate request and target shape first, resolve the allowlist, then under the component lock check instance, expected state/revision, transition permission, and revision exhaustion before committing. Return `OK` only after the atomic in-memory update. Preserve unknown outcomes after dispatch and never retry automatically.

Phase gate (2026-10-27): Phase 1 C++ implementation/review, Phase 2 targeted gRPC/Rust integration, and Phase 3A Windows verification are complete and independently reviewed. The final working diff and whitespace checks passed. See `progress.md` and `handoff.md` for evidence. P001 remains in progress because deterministic transport-loss and induced-deadline behavior remain unverified.

Phase 1 C++ handler and focused tests passed independent review. Phase 2 gRPC/Rust evidence (2026-10-27): `ActuatorClient::apply_command` makes one generated unary RPC call and sets the existing two-second timeout. One targeted real C++ server/Rust client integration test passed. It covered result and snapshot fields, same-state increments, every approved transition cell for both components, malformed/unknown requests, instance/state/revision precedence, permission denial, 32 concurrent requests using one expected snapshot (one success, remaining `ABORTED`), component isolation, unchanged register sentinels, loopback endpoint, and rejection of an oversized command with `RESOURCE_EXHAUSTED`. The test confirms an oversized RPC is rejected; deadline expiry itself was not induced. Source inspection confirms a single call site and no retry loop.

No deterministic lost-response-after-commit test was added. Exercising that path safely would require a dedicated test-only transport/server harness that commits then drops the response; the production server has no such hook, and adding production fault injection was out of scope. Therefore client-visible unknown outcomes after a real disconnect and runtime retry behavior under transport loss remain unverified. The client method contains no retry path; command IDs remain correlation-only. Wear planner remains independent and is not directly observable from this RPC test. Phase 3A verification passed, but these limits remain and P001 is not complete pending review/publication.

### Implemented synthetic fixtures (2026-10-25)

These identifiers and values are simulator test data, never physical sensor readings. No request chooses a path, register map, or sample payload.

| Device | Component | Sensor | Value | Unit | Result |
| --- | --- | --- | ---: | --- | --- |
| `simulated-0` | `core-0` | `synthetic-temperature` | 42.5 | Celsius | `OK` |
| `simulated-0` | `core-0` | `synthetic-utilization` | 0.0 | Ratio | `OK` |
| `simulated-0` | `core-1` | `synthetic-voltage` | 1.0 | Volts | `OK` |
| `simulated-0` | `core-1` | `synthetic-unavailable` | Absent | Volts | `UNAVAILABLE`, no sample |

Successful fixtures return a present `sampled_at_unix_ns = 0`, a deterministic epoch-zero timestamp rather than the current read time. Each component has a separate mutex-protected Micro-State snapshot, initialized to `ACTIVE`/revision zero and sharing the process's 16-byte instance ID. `ApplyCommand` can update this state only through the owner-approved simulator transition table documented above. Telemetry reads, errors, and direct register writes never advance revisions. No register or wear-planner slot is mapped to a component.

Startup obtains exactly 16 bytes through Windows `BCryptGenRandom`, Linux `getrandom` (handling partial reads/interruption), or macOS `getentropy`; other platforms fail closed. An unavailable random source prevents startup before binding. There is no clock/PRNG/previous-ID fallback. Windows execution is verified; Linux/macOS paths remain unexecuted. The ID provides no caller identity or authentication.

Missing targets or malformed/empty/overlong IDs return `INVALID_ARGUMENT`. Well-formed unknown devices, components, sensors, and cross-component sensor pairs return `NOT_FOUND`. An unavailable or invalid server-side sample (absent/nonfinite value, invalid/mismatched unit, missing timestamp, or out-of-range ratio) cannot become a successful reading and maps to `UNAVAILABLE`. Unknown protobuf fields remain compatible with decoding; responses echo only the canonical validated identifiers. Existing 4096-byte message bounds and loopback binding remain enforced. Command validation and status behavior are described in the P001 section above.

Verification covers C++ sample/status validation, injected random-source failure with no issued identity, 16 concurrent backend readers, and real C++/Rust RPCs including 32 concurrent readers, register independence, zero presence, invalid/unknown pairs, unavailable samples, message bounds, and fresh instance identity after restart. No forced OS-failure server subprocess or Linux/macOS run is claimed. Published field numbers/messages/RPC signatures are unchanged; only an outdated `.proto` comment was corrected. Run `make check test test-grpc`; see [platform commands](../README.md) and [local dependency setup](../handoff.md).
