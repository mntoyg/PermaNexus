# PermaNexus feature inventory and roadmap

Last updated: 2026-10-11, Asia/Bangkok.

This file tracks delivered capabilities and future work. Daily actions, mistakes, and usage observations belong in [progress.md](progress.md); reproduction commands and contributor context belong in [handoff.md](handoff.md). Security rules are defined in [AGENTS.md](AGENTS.md).

## Status definitions

- **Implemented**: code or infrastructure exists; available verification and limits are recorded below.
- **Planned**: next work with acceptance criteria; implementation has not started.
- **Design required**: contracts, scope, or safety decisions must be settled before implementation.
- **Research**: a long-term direction without demonstrated feasibility or a delivery commitment.

The current system simulates owned RAM registers and storage write budgets. A 100-year hardware lifetime remains a research goal. Listing a roadmap item does not authorize privileged hardware operations or network exposure.

## Implemented features

All entries below were delivered on 2026-10-11. Verification distinguishes implemented endpoints from contract scaffolding and records the relevant test runs.

| ID | Feature | Implementation and scope | Verification and limits |
| --- | --- | --- | --- |
| F001 | Monorepo and build infrastructure | Four layer directories, shared contracts, [Makefile](Makefile), CMake, locked Cargo dependencies; build, generation, checks, tests, simulation, and benchmark targets. | C++/Rust builds exercised on Windows. Go/Python remain placeholders. |
| F002 | Versioned register RPC contract | [actuator.proto](proto/actuator.proto): reads and atomic compare-and-write with expected-value validation. | Real C++/Rust integration passed; preserve published field numbers and error semantics. |
| F003 | C++ actuator simulator | [register_bank.cpp](src/layer1_actuator/register_bank.cpp) and [server.cpp](src/layer1_actuator/server.cpp): five registers, immutable descriptors, per-register strong CAS, request validation, loopback binding, bounded messages/shutdown. | Checks cover rejected-write immutability, all widths, stale writes, 32 concurrent writers, and independent registers. No MMIO, caller authentication, or persistent register values. |
| F004 | Safe Rust client and demo | [lib.rs](src/layer2_permacore/src/lib.rs) and [main.rs](src/layer2_permacore/src/main.rs): loopback endpoint policy, two-second deadlines, increment-and-verify demo, no automatic write retries. | Endpoint tests and real C++/Rust integration passed. A timeout can have an unknown write outcome. |
| F005 | Simulated wear-leveling planner | [wear_leveling.rs](src/layer2_permacore/src/wear_leveling.rs): exact normalized-budget selection, exclusive charged reservations, quarantine, evidence-based refunds, permanent retirement. | Rust invariant tests passed. No register/slot mapping, data migration, or physical endurance measurement. |
| F006 | Persistent journal and simulation CLI | [wear_journal.rs](src/layer2_permacore/src/wear_journal.rs) and [wear_simulator.rs](src/layer2_permacore/src/bin/wear_simulator.rs): exclusive file lock, flushed append-only events, bounded CRC/sequence/semantic replay, explicit initialization, resume/configuration checks, I/O-failure poisoning. | Tests cover interruption, locking, corruption, incomplete tails, capacity, and I/O errors. Owner resumed 500 writes per slot / 2000 remaining. See [journal specification](src/layer2_permacore/JOURNAL_FORMAT.md); no compaction, authenticated anti-rollback, or quarantine reactivation. |
| F007 | HPP primitives and atomic backend | Three C++17 headers for integer bit validation, aligned owned RAM atomics, and bounded spin hints; optional atomic-link fallback. See [HPP.md](src/layer1_actuator/HPP.md). | Bit-boundary, all-width, alignment, stale-write, and concurrency checks passed on Windows x86_64 GCC. ARM/MSVC/libatomic fallback execution remains unverified. |
| F008 | HPP benchmark | [hpp_benchmark.cpp](src/layer1_actuator/benchmarks/hpp_benchmark.cpp): bounded atomic/mutex single-thread and contended workloads, separate validated-bank workload, checksums, lock-free reporting. | Four benchmark-build CTest checks passed. Two agent-run samples and one owner-reported sample are in HPP.md. No RPC latency, energy, or lifespan measurement; lock-free does not guarantee higher throughput. |
| F009 | Contributor rules and records | AGENTS, CLAUDE, Aider configuration, README, feature inventory, progress, and handoff. English documentation; owner-required daily 14% weekly-allowance workflow limit and halt/report rule. | Guidance does not enforce a sandbox or provider quota settings. Daily totals must not be invented when metrics are incomplete. |
| F010 | Telemetry and targeted Micro-State protocol scaffolding | Additive `ReadTelemetry`/`ApplyCommand` RPCs and `TelemetryData`/`ActuatorCommand` messages; typed units, presence-bearing readings/revisions, component targeting, instance/state/revision preconditions. See [protocol contract](proto/README.md). | C++/Rust bindings compiled; two wire tests passed; real integration verified `UNIMPLEMENTED` and unchanged registers. No sensors, Micro-State executor, revision store, or new safe Rust wrapper. |

Latest protocol verification: 22 Rust tests, two C++ backend checks, and the dedicated C++/Rust integration test passed on Windows after regenerating bindings. The earlier gRPC-free HPP benchmark build passed four checks. Ordinary Cargo tests deliberately ignore the integration test; it is run separately. Linux/macOS execution and physical power-loss durability remain unverified.

## Planned work and design dependencies

Prioritize contract review and simulation safety before hardware integration. Delivery dates and implementation ownership remain unassigned. Items requiring design are not approved implementation specifications.

| ID | Status | Feature or milestone | Dependencies and acceptance criteria |
| --- | --- | --- | --- |
| P001 | Planned | Architect review of actuator/planner integration | Review the new protocol, component/sensor policy, instance/revision lifecycle, and direct-register-write interaction; settle register-map/slot ownership, trust boundary, compatibility, and completion evidence. Preserve simulation boundaries and prohibit ambiguous write retries. |
| P002 | Planned | HPP portability and representative measurements | Execute checks on intended platforms, including ARM/MSVC and required atomic-link fallback; gather repeated representative timing distributions before changing alignment, memory order, or defaults. |
| P003 | Design required | Journal durability, integrity, and compaction | Review target filesystem/device flush behavior; design authenticated metadata, a trusted monotonic reference, bounded compaction, and interruption recovery preserving charges/quarantine. CRC alone is insufficient. |
| P004 | Design required | Quarantine reconciliation | Specify authoritative evidence and auditable transitions. Unknown outcomes remain charged and quarantined until evidence supports a reviewed decision; never silently reactivate slots. |
| P005 | Design required | Logical placement and wear-aware migration | After P001/P003/P004, define ownership, movement, consistency, and recovery invariants; validate in simulation before device-backed writes. |
| P006 | Design required | Authenticated transport | Define local caller identity and TLS/authentication policy. Verify unauthorized requests cannot mutate state before any endpoint is exposed beyond loopback. |
| P007 | Design required | Go Omni Agent orchestration | After P001, define bounded concurrency, cancellation, deadlines, ownership, and unknown-outcome handling. Require integration coverage before orchestrated actuation. |
| P008 | Design required | Python telemetry and prediction | Define schema, provenance, retention, and evaluation data; demonstrate reproducible prediction quality. Predictive control requires a separate reviewed actuation policy. |
| P009 | Design required | Hardware actuator backend | Requires explicit owner authorization and reviewed device-specific safety design. Validate register side effects, supported atomicity, ordering, and recovery on approved targets before access. |
| P010 | Planned | Project license decision | Obtain the owner's choice, then add exact license terms and contributor guidance. Public visibility alone does not grant an open-source license. |
| P011 | Research | Long-term hardware service-life validation | Establish measurable endurance, energy, reliability, and maintenance evidence on intended devices. Simulation and benchmarks do not demonstrate 100-year longevity. |

## Maintenance rules

Keep IDs stable; never reuse removed IDs. Record completion dates and evidence when changing status. Update this file whenever capabilities, limitations, or agreed plans change, together with progress and handoff. Do not mark a feature implemented solely because a directory, plan, or generated interface exists. Preserve AGENTS.md security rules when revising the roadmap.
