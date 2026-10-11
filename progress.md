# Daily progress

Dates use Asia/Bangkok. Records distinguish implementation, verification, and plans; corrected mistakes remain documented.

## 2026-10-11 — Day 1

### Plan and acceptance criteria

1. Bootstrap the four-layer monorepo, Makefile, and versioned C++/Rust protobuf contract; publish to GitHub.
2. Make all Markdown English and establish shared context, security, and daily budget rules.
3. Implement a loopback C++ register simulator and Rust client/demo. Validate requests, preserve state on rejection, and permit exactly one winner among concurrent writers using the same expected value.
4. Verify the backend and real C++/Rust transport; record checks, limitations, corrections, and usage before handoff.

### Completed bootstrap

- Created `proto/` and all four language-layer directories, shared Make targets, CMake contract generation, Rust bindings, `.gitignore`, and `Cargo.lock`.
- Added `ReadRegister` and atomic `WriteRegister` RPC contracts. Compiled the protobuf descriptor and checked Rust formatting and Make dry-runs.
- Created public repository `https://github.com/mntoyg/PermaNexus`; pushed bootstrap commit `51909a8fcac309625a85f2e2b503a8033c21412a` and verified local/remote `main` matched.

### First feature

- Added a mutex-protected C++ register bank, five allowlisted registers, validation, read-only permissions, and a loopback gRPC server with bounded messages and shutdown.
- Added a Rust loopback client, two-second RPC deadlines, no write retries, and an increment-and-verify demo.
- Added backend/endpoint-policy tests and an integration test covering all widths, rejected requests, 32 concurrent writers, stale writes, and the CLI demo.
- Added `test`, `test-grpc`, backend-only builds, English collaboration records, and LF normalization.
- Added a Claude rule import and Aider context configuration; Cursor uses root `AGENTS.md`. Local Aider histories and private Claude context stay ignored.

### Mistakes and fixes

| Observation | Correction | Status |
| --- | --- | --- |
| Original README was in Thai, limiting international collaboration. | Rewrite committed Markdown in English; add the language rule. | Fixed |
| Initial PowerShell profile failed in the tool environment. | Use profile-free shell execution. | Fixed |
| A failed CMake configure could leave a cache file that skipped subsequent configuration. | Use a phony configure prerequisite. | Fixed |
| Sandbox restrictions blocked Cargo toolchain/network access and made GitHub CLI appear unauthenticated. | Recheck through scoped escalation; existing credentials were valid. No token rotation was needed. | Fixed for completed checks |
| Initial C++ dependencies were absent; the installed MSYS2 database referenced removed 2024 packages and downloads returned 404. | Failed transaction installed nothing. Downloaded and verified current dependencies into an isolated build prefix; the installed toolchain was not upgraded. | Fixed and verified |
| The isolated pacman downloader stalled; its lock file was read-only after stopping the task-owned process. | Downloaded official database URLs and signatures with PowerShell, removed only the task's lock file with an explicit path, and used pacman to verify cached package integrity. | Fixed and verified |
| Windows Git defaults could convert text files to CRLF. | Add `.gitattributes` with LF normalization. | Fixed |
| A draft volatile shutdown flag crossed thread boundaries without synchronization. | Replace it before publication with a statically verified lock-free atomic flag. | Fixed before testing |
| One documentation patch attempted delete/add of the same path and was rejected. | Split the edit and retain the original until its replacement was ready. | Fixed |

### Environment and checks

- Bootstrap protobuf validation installed `grpcio-tools` only in ignored `build/proto-validation/`. It did not replace the global Python environment. Pip warned about pre-existing Aider dependency mismatches; that warning does not prove this targeted install changed those packages.
- Existing Git identity/keyring credentials were reused; secrets were not committed.
- Dependencies live only in ignored `build/msys-prefix/ucrt64/`, with an isolated package database/cache. All 12 package archives passed pacman keyring/integrity checks before extraction; package installation scripts were not executed.
- Verified environment: existing GCC 14.2.0, Rust 1.98.0, isolated gRPC 1.82.0 / Protobuf 36.2 / OpenSSL 3.6.5. Dependency runtime DLLs come from the same isolated package set. No installed MSYS2 packages were upgraded.
- Rust was first checked with the existing target-local Python protoc wrapper, then rebuilt and tested with the native isolated protoc.

| Check | Result |
| --- | --- |
| Backend-only C++ build and CTest | Passed: validation, overflow, read-only protection, stale/no-mutation checks, all widths, and 32-writer atomicity |
| C++ gRPC server Release build | Passed |
| Locked Rust Release build | Passed |
| `mingw32-make check` | Passed: Rust formatting and compilation |
| `mingw32-make test` | Passed: CTest and Rust endpoint-policy test; integration test explicitly ignored in this ordinary run |
| `mingw32-make test-grpc` | Passed: actual C++/Rust wire contract, all widths, error codes, 32 concurrent writers, stale-write rejection, and CLI demo |
| English Markdown scan | Passed for all five committed Markdown files |
| `git diff --check` | Passed |

The combined Make invocation used the isolated dependency prefix documented in `handoff.md`. Runtime/build verification was performed on Windows; Linux/macOS commands are documented but were not executed in this session.

The feature and collaboration records are committed together with the English message `Implement loopback actuator transactions and shared agent guidance`. Git history and the remote `main` ref identify the exact published commit without a self-referential hash inside this commit.

### Usage observations

| Provider pool | Observation | Scope |
| --- | --- | --- |
| Codex | Weekly window: 24% used at 09:42 Bangkok; five-hour window: 14% used. | Mid-day baseline; earlier daily consumption unknown. These are different windows. |
| Codex final verification observation | Weekly window: 26% used; five-hour window: 23% used. | Same weekly reset window as the baseline: +2 percentage points during the observed interval. |
| Claude / Cursor / Aider providers | Unavailable in this session. | No percentage inferred or setting changed. |

Target: at most 14% of each full weekly provider/account allowance per Bangkok day, shared by agents in that quota pool. The observed +2 weekly percentage points are below the 14-point planning target, but this does not establish total consumption for the entire day or attribute all shared-account usage to this session. No third-party quota setting or recurring monitor was created.

### Remaining work

- Obtain the owner's project license decision.
- Architect review before real hardware access, identity/TLS, or hardware-backed wear-leveling implementation.

### Day 1 continuation — simulated wear-leveling

Plan: implement a Rust-only mock policy with explicit budget and uncertainty invariants, a runnable simulation, focused tests, and updated records. Acceptance criteria: deterministic selection by normalized budget use, no over-budget writes, no selection of quarantined/retired slots, no reservation replay, and safe accounting for ambiguous outcomes.

Completed:

- Added `wear_leveling.rs` with validated configuration, immutable snapshots, exact u128 utilization comparisons, deterministic ID tie-breaking, and bounded slot count.
- Added an exclusive, consumed write lease. Reservation charges/quarantines immediately; successful completion retains the charge, proven non-writes refund it, and uncertain/dropped/forgotten leases remain quarantined. Explicit retirement preserves accounting.
- Added a bounded, standalone `wear_simulator` CLI and `make simulate-wear`; no RPC/hardware integration or new dependencies were added.
- Kept `permanexus-permacore` as Cargo's `default-run` to preserve the original demo after adding a second binary.
- Added seven policy tests and two CLI tests. Tests cover balanced allocation/exhaustion, mixed capacities, near-u64-max arithmetic, refunds, quarantine, retirement, configuration failures, and rejected CLI workloads.

Verification on Windows:

| Check | Result |
| --- | --- |
| Locked Cargo tests | Passed: seven policy tests, two CLI tests, and the existing endpoint-policy test |
| `mingw32-make check` | Passed: Rust formatting and compilation |
| `mingw32-make simulate-wear SIMULATION_ARGS="1000 4 1000"` | Passed: 1000 committed simulated writes, 250 per slot, 3000 remaining budget |
| C++/Rust integration | Intentionally ignored by ordinary Cargo tests; previously passed for the register feature, not rerun because transport/actuator code is unchanged |

Mistakes/fixes: no new feature-check failures occurred in this continuation. Multi-binary default-run compatibility was handled before publication; it was not an observed failure. Environment: reused the verified isolated native protoc/runtime prefix and existing Cargo dependencies; no package installation, credential change, or system upgrade occurred.

Limits: this is an in-memory allocation model, not hardware wear measurement or data migration. Quarantine is conservative but not persisted. There is no reactivation API; recovery and slot/register mapping need an architect-reviewed design before integration. The commit message for this continuation is `Add conservative simulated wear-leveling policy and CLI`.

Usage: at 12:34 Bangkok, the provider reported 0% weekly and 0% five-hour usage with reset timestamps different from the earlier observation. This is a new measurement baseline; earlier 26% must not be subtracted across the differing windows. Final verification reported 0% weekly and 3% five-hour usage within the same windows as the continuation baseline. The displayed weekly change is 0 percentage points; unchanged rounded percentages do not mean no resources were consumed. Full-day use and other providers remain unknown.

### Day 1 continuation — persistent planner journal

The owner independently ran `wear_simulator.exe 1000 4 1000` and reported the expected 250 writes per slot / 3000 remaining budget. The next plan was to preserve planner counters and quarantine across restart without introducing hardware access.

Acceptance criteria: exclusive writer ownership, flushed reservations before leases, conservative restart after interruption, refusal of damaged/missing state, no overwrite through initialization, poisoning after failed writes, and reproducible persistent CLI runs.

Completed:

- Added a versioned append-only journal with create-new initialization, native OS locking, flushed reservation/completion events, bounded CRC/sequence/semantic replay, and fail-closed load/error handling.
- Kept unknown/dropped/forgotten leases charged and quarantined on disk; added no reactivation API. Completion replay is crate-private and checks matching pending reservations.
- Extended the existing simulator with `--state PATH` / `--init`, configuration mismatch protection, and readable error messages; preserved the RAM-only interface and default gRPC executable.
- Added a journal format/durability specification and an agent security rule. Runtime `state/` and `*.wearlog` files remain ignored.
- Added `tempfile` as an explicit test dependency using its already-cached, previously transitive version; updated `Cargo.lock` offline. No new package download, system upgrade, credential change, or network RPC was needed.

Verification on Windows:

| Check | Result |
| --- | --- |
| Locked Cargo tests | Passed: 20 executed tests, including ten new persistence tests; C++ integration explicitly ignored |
| Process interruption | A child exited without Rust destructors after a flushed reservation; restart preserved its charge/quarantine |
| Locking | Another handle/process could not open the owned journal; closing the owner released it |
| Recovery validation | Rejected corruption, every partial-tail size, CRC-valid invalid transitions/sequences, oversized files, and exhausted record capacity |
| I/O failure | A failed append returned no lease, preserved the source file, and poisoned subsequent mutations |
| Persistent CLI | Second process resumed counts; mismatched configuration and repeated initialization did not modify the source |
| `mingw32-make check simulate-wear` | Passed: formatting, compilation, optimized build, and unchanged 1000-write RAM demo |
| Local persistent demonstration | Created `state/demo.wearlog` with a fresh 1000-write simulation, then reopened with zero writes: 250 per slot, 3000 remaining, 34096-byte journal |

Mistakes and fixes:

- Initial cross-process lock test failed because CLI errors used enum Debug output rather than the readable Display message. Locking itself rejected the second writer. Changed the CLI entry point to print Display errors; reran the complete suite successfully.
- A follow-up patch referenced a pre-rustfmt line and was rejected without publication. Reapplied the change against the formatted source.

The old in-memory process could not be retroactively persisted; the local demo journal recreates its reported distribution through a new simulated run. Power-loss durability, authenticated anti-rollback, compaction, and hardware-backed wear remain unverified/unimplemented. Existing source journals are never repaired or deleted automatically. Commit message: `Persist simulated wear state with a locked write-ahead journal`.

Usage baseline at 12:44 Bangkok: 1% weekly / 5% five-hour usage, with the same reported reset timestamps as the previous continuation. Final verification showed 2% weekly / 12% five-hour usage. Final reset timestamps differ from the baseline by one second; no exact interval delta or actual reset is asserted from this small timestamp difference. Full-day attribution and other providers' metrics remain unavailable. No provider limits or recurring automation were changed.

### Day 1 continuation — Hardware Performance Programming

The owner reported the persistent simulation resumed correctly at 500 writes per slot / 2000 remaining budget and requested more HPP (Hardware Performance Programming). Interpreted this as performance-oriented Layer 1 C++ infrastructure and reusable `.hpp` headers, retaining the simulator-only hardware boundary.

Plan/acceptance criteria: safe constexpr bit operations, owned RAM atomics with reported capabilities, scheduling hints with bounded backoff, per-register CAS without a global mutex, preserved errors/validation, deliberate concurrency tests, and a correctness-checked benchmark against an equivalent mutex primitive.

Completed:

- Added `register_bits.hpp`, `atomic_register.hpp`, and `spin_wait.hpp`; kept sequentially consistent strong CAS, unsigned-width constraints, default 64-byte layout alignment, and no raw hardware-address APIs.
- Replaced mutable register metadata/global mutex with immutable descriptors and separate aligned atomics. Read/compare-and-write validation and protobuf semantics are unchanged.
- Added an interface HPP target, native atomic64 link check with optional libatomic fallback, optional bounded benchmark build, and `make benchmark-hpp` without gRPC dependencies.
- Added compile-time bit/layout checks, all-width atomic checks, a barrier-started 32-writer test, parallel independent-register checks, benchmark smoke/invalid-argument checks, and benchmark/durability-scope documentation.

Verification on Windows GCC 14.2.0:

| Check | Result |
| --- | --- |
| Final `benchmark-hpp` CTest run | Four checks passed: register bank, HPP primitives, benchmark smoke, rejected arguments |
| `make test` | Two actuator checks and all 20 Rust regression tests passed; wire test explicitly ignored in the ordinary Cargo run |
| Dedicated `make test-grpc` | Passed after the backend change, including real C++/Rust validation, 32 writers, and CLI demo |
| Bounded benchmark | Two Release samples at 100000 iterations/four threads; final atomic single 6.848 ns/op vs mutex single 14.988 ns/op; runtime atomic64/bank lock-free reports true |

All sample values are retained in `src/layer1_actuator/HPP.md`; variance between samples is reported rather than hidden. Timing does not establish a whole-system speedup, energy savings, tail latency, or hardware lifetime. Benchmarks never touched `state/demo.wearlog` or hardware registers. No dependencies, compiler flags requiring native ISA features, system tuning, or credentials were added.

Mistakes/fixes: no build or test failures occurred in this continuation. Code review identified that the earlier bank concurrency check launched workers without a common release barrier; added readiness/start coordination to make contention deliberate. This was a validation gap, not an observed register-data failure. Final additional independent-register tests were rebuilt and passed after the earlier full integration run; production backend logic was unchanged by that final test refinement.

Documentation review also removed an accidentally duplicated empty prerequisites heading before publication. Final `git diff --check` and the English-language scan passed for all seven Markdown files.

Commit message: `Add HPP primitives and benchmark the atomic actuator backend`. Usage baseline at 13:01 Bangkok: 2% weekly / 14% five-hour usage; final verification: 3% weekly / 21% five-hour usage with identical reset timestamps. The observed weekly increase is one percentage point within that interval, not a claim about full-day consumption or exclusive attribution to this chat. Other providers' metrics remain unavailable; no quota settings or automation changed.

### Day 1 observation — Owner HPP benchmark

Plan/acceptance criteria: preserve the owner's supplied output with its provenance, check timing ratios and checksum consistency, update the handoff, and publish a documentation-only change. No source tuning is justified by one timing sample.

The owner ran `.\build\hpp\hpp_benchmark.exe 100000 4` and reported atomic64/bank lock-free status true and 64-byte alignment. All five timings are preserved in `src/layer1_actuator/HPP.md`. Atomic/mutex single checksums both equal 5999950000; both contended counters equal 100000; validated bank checksum equals 4999950000. These observations are consistent with the benchmark's expected results, not a new agent-executed test run.

Atomic single throughput was approximately 1.98 times the mutex case. Atomic contended time per operation was approximately 5.6% higher than the mutex case. Retained the existing backend and documented that lock-free status does not guarantee a timing win. No hardware, runtime journal, dependencies, environment settings, or provider limits changed. Existing executable checks were not rerun for this documentation-only update; verification consists of diff review, whitespace checking, and English-language checking.

Mistakes/fixes: the initial conversational interpretation described the mutex as about 5.6% faster. More precisely, atomic elapsed time is 5.6% higher relative to the mutex baseline (the mutex time is about 5.3% lower relative to atomic). The recorded comparison uses mutex time as the denominator: `(42.573 / 40.331 - 1) * 100`.

Usage observations: baseline 13:17 Bangkok, weekly 4% (reset 1792301671), five-hour 23% (reset 1791714871); final observation 13:19 Bangkok, weekly 4% (reset 1792301670), five-hour 24% (reset 1791714870). The one-second reset timestamp difference is recorded without asserting a reset or exact interval delta. Full-day attribution and other-provider metrics remain unavailable. Commit message: `Record owner HPP benchmark results and contention variance`.

### Day 1 documentation — Feature inventory and roadmap

Plan/acceptance criteria: create English `feature.md` from implemented capabilities and recorded next work, distinguish delivered features from design-dependent plans, attach evidence/limits and stable IDs, connect contributor entry points, and publish the documentation update.

Completed: recorded implemented items F001-F009 and roadmap items P001-P011 with scope, verification, dependencies, and acceptance criteria. Linked the inventory from README; AGENTS now requires reading and maintaining it; Aider loads it as an editable record. Updated handoff context. Go/Python remain placeholders, real hardware remains outside current implementation, and dates/owners for future delivery remain unassigned. No source, runtime journal, toolchain, credentials, provider settings, or executable behavior changed.

Verification: review inventory against repository paths and existing verification records; check local Markdown links, English language, stable-ID uniqueness, and Git whitespace. Executable tests are not rerun for documentation/configuration records; earlier test evidence is identified explicitly.

Mistakes/fixes: the first patch used a sentence fragment as a full-line handoff match and was rejected without changing files. Verified that no partial edits were applied, then reapplied against the actual line. No build/test failure occurred.

Usage: baseline 13:22 Bangkok, 4% weekly / 25% five-hour; closing observation 13:24 Bangkok, 4% weekly / 27% five-hour. Both observations reported weekly reset 1792301671 and five-hour reset 1791714871. Displayed weekly usage was unchanged in this interval; full-day consumption, finer-grained usage, and exclusive chat attribution cannot be inferred. Other providers' metrics remain unavailable. Commit message: `Document implemented features and the project roadmap`.

### Day 1 protocol extension — Telemetry and targeted Micro-State commands

Plan/acceptance criteria: extend the existing v1 schema additively with sensor telemetry and component-targeted commands; preserve published register fields/RPC semantics; specify presence, units, safety/concurrency preconditions, and unknown-outcome rules; compile both language bindings; verify unsupported new endpoints cannot mutate current registers; prepare architect review context.

Completed:

- Added `ComponentTarget`, `TelemetryUnit`, `MicroState`, `MicroStateSnapshot`, `TelemetryRequest`, `TelemetryData`, `ActuatorCommand`, and `ActuatorCommandResult`, plus unary `ReadTelemetry`/`ApplyCommand` RPCs. Existing field numbers and RPC names are unchanged.
- Defined finite readings, explicit units, zero-versus-absence, configured logical targets, command correlation without automatic retries, and state/revision/instance preconditions. Comments specify validation; decoding alone does not enforce it.
- Added `proto/README.md` with endpoint status, future backend acceptance gates, direct-register-write ownership questions, and completion/recovery requirements. Current new endpoints return generated `UNIMPLEMENTED`; no sensors, Micro-State executor, revision store, or new safe Rust wrapper was added.
- Added two Rust wire tests and extended real C++/Rust integration to verify both unsupported endpoints and unchanged values for all five registers. Updated feature F010, README, and handoff.
- Updated the prior soft budget guidance to the owner's explicit 14% daily workflow limit and halt/report requirement. This changes agent instructions, not subscription/provider settings or recurring jobs.

Verification: `mingw32-make check test test-grpc` with the existing isolated dependency prefix passed. C++/Rust bindings regenerated and compiled, two CTest backend checks passed, 22 Rust tests passed, and the dedicated integration test passed. Ordinary Cargo execution explicitly ignored that integration test before the separate run. New bindings and build products remain ignored. No journal, hardware, dependency lockfile, or installed toolchain changed.

Mistakes/fixes: no build or test failure occurred. Documentation review moved the new protocol paragraph into its own README section before publication; the initial placement under wear-leveling was misleading.

Usage: baseline 13:30 Bangkok, weekly 4% (reset 1792301670), five-hour 28% (reset 1791714870); closing observation 13:33 Bangkok, weekly 5% (reset 1792301671), five-hour 30% (reset 1791714871). The one-second timestamp difference is recorded without asserting a reset or exact interval delta. Earlier daily use/shared-account attribution and other-provider metrics remain incomplete. Evidence does not establish reaching 14% daily usage; no quota-triggered halt is claimed.

Tomorrow's target: architect review of component/sensor policies, state transitions, instance/revision lifecycle, direct register-write interaction, and definitive/unknown completion evidence before implementing simulated new handlers or planner integration. Commit message: `Add telemetry and targeted micro-state protocol contracts`.
