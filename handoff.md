# Agent handoff

Last updated: 2026-10-25, Asia/Bangkok.

## Context

Current stage (2026-10-25): allowlisted synthetic read-only telemetry is implemented. See the fixture table in `proto/README.md`; all readings are deterministic RAM simulation data. Each component owns a separate locked ACTIVE/revision-zero snapshot with the process's OS-random 16-byte instance ID. Startup fails before binding if the random source fails. Register RPCs and wear-planner state remain independent; ApplyCommand is still UNIMPLEMENTED. No wire-contract change was required (one implementation-status comment was corrected).

Contributor changes were already present in AGENTS.md, feature.md, progress.md, handoff.md, and proto/README.md. Baseline copies are preserved locally in ignored `build/telemetry-baseline/`; no reset, stash, or overwrite of unrelated work was performed. The owner subsequently requested committing and publishing the completed stage, including the reviewed architecture/budget records.

Publication authorization: the owner explicitly requested committing and pushing this completed stage to origin/main. The combined implementation and related architecture/budget records were reviewed, existing tests remain applicable, and origin/main was fetched with no divergence. Commit message: `Implement allowlisted synthetic read-only telemetry`. Use `git log -1`, `git status`, and `git ls-remote origin refs/heads/main` to verify actual publication state. There are no remaining implementation/test blockers for the approved read-only stage.

Read `AGENTS.md`, `README.md`, `feature.md`, and `progress.md`, then inspect Git status. Conversations with the owner use Thai; all committed Markdown, identifiers, and code comments use English.

Repository: `https://github.com/mntoyg/PermaNexus`, branch `main`. Persistence reference: `5b09472598f0dda625bcfcf88fa73c0daabf840e`; atomic backend reference: `bf08902c83f6372332343a5a8b201cbb94020ec8`; feature-inventory reference: `d2dc951c28cf39b3d93ec01f195056a701fd8661`. The latest addition extends the protocol with telemetry and targeted Micro-State contract scaffolding plus verification. Use `git log -1` and `git ls-remote origin refs/heads/main` for current commit/publication state.

## Implementation

- `feature.md` tracks F001-F011 and P001-P011. F010 defines the wire contract; F011 implements synthetic ReadTelemetry only. Command execution remains design-dependent. Keep IDs stable and update evidence/status with future work.

- `proto/actuator.proto`: register and telemetry/command contracts with unchanged field numbers/signatures. C++ `telemetry_store.cpp` implements fixture selection and validation; `actuator_service.cpp` maps telemetry statuses to gRPC while preserving register handlers. Only ApplyCommand retains the generated UNIMPLEMENTED default. Read `proto/README.md` for fixture/error semantics and the remaining command-stage review gates.
- C++: five in-memory registers on `simulated-0`; immutable descriptors, aligned per-register strong CAS, preserved validation/read-only policy, bounded messages, and loopback-only gRPC server.
- HPP: three C++17 `.hpp` headers for bit validation, owned RAM atomics, and spin hints; optional benchmark and CMake atomic-link compatibility probe. Read `src/layer1_actuator/HPP.md` before interpreting timings or adding hardware-oriented primitives.
- Rust: generated contracts, safe client wrapper with deadlines and no write retries, plus an increment-and-verify demo.
- Rust `wear_leveling.rs`: deterministic normalized-budget selection, exact u128 comparisons, one exclusive lease per planner, conservative unknown-outcome quarantine, proven-no-write refunds, and permanent retirement. The independent `wear_simulator` CLI performs no RPC or hardware access.
- Rust `wear_journal.rs`: exclusive file locking, flushed append-only reservations/completions, sequence/CRC/transition validation, bounded replay, and poisoning on write failures. The CLI supports `--state PATH` and explicit create-new `--init`. Read `src/layer2_permacore/JOURNAL_FORMAT.md` before changing the format.
- Go/Python remain placeholders. Hardware-backed wear-leveling, register/slot mapping, data migration, persistent actuator values, telemetry prediction, and orchestration are not implemented.
- Cargo dependencies are locked; generated files and build products are ignored.
- `CLAUDE.md` imports the shared rules; Aider configuration loads shared rules and editable daily records. Cursor uses root `AGENTS.md`. These are guidance/context files, not hard enforcement or a runtime sandbox.

## Verification

Telemetry-stage evidence: four C++ CTest checks (register bank, HPP, new telemetry backend, new service status mapping), 22 ordinary Rust tests, and two dedicated real C++/Rust integration tests passed with the existing dependency setup. Coverage includes invalid/missing IDs, unknown/cross-component pairs, zero presence, unavailable/invalid samples, finite/unit checks, oversized RPC rejection, 16 concurrent backend readers, 32 concurrent RPC readers, fresh instance ID after restart, and independence from all five registers. The ordinary Rust run deliberately ignores both transport tests before the separate run. Logs are local/ignored at `build/test-results/2026-10-25-telemetry-final.log`; an initial namespace-extraction compile failure is retained separately and was corrected before passing verification.

Windows BCrypt randomness and startup success were exercised. Random-source failure was injected at the identity-generator unit boundary; a forced OS failure in a server subprocess was not executed. Linux/macOS random-source branches remain unexecuted. No real sensors, hardware access, caller authentication, command transitions, persistent Micro-State, or new Rust telemetry wrapper are claimed.

Owner-requested rerun at 13:37 Bangkok on source `fa1be05436e7a67dd84cb72283654ccf993d3673`: Rust formatting/check passed; actuator CTest 2/2, Rust 22/22, dedicated gRPC integration 1/1, and existing HPP CTest 4/4 passed. Ordinary Cargo ignored integration by design before the separate successful execution. HPP reused its existing build, including benchmark smoke/invalid-argument checks; no new timing sample is claimed. Raw output is local/ignored at `build/test-results/2026-10-11-1337-tests.log`. No source or runtime journal changed.

Latest protocol checks: regenerated/compiled C++ and Rust bindings, Rust formatting/check, two CTest backend checks, all 22 Rust tests, and the dedicated real C++/Rust integration test passed. New wire tests preserve zero-versus-absence, unknown enum values for later validation, and full uint64 revision width. Integration verifies new endpoints return `UNIMPLEMENTED` and all five registers remain unchanged; existing RPC validation/concurrency and the demo still pass. Required validation in new message comments is a future backend contract, not enforcement by protobuf decoding.

Follow platform commands in `README.md`. Required checks: `make check`, `make test`, and `make test-grpc` (`mingw32-make` on Windows). Integration testing starts an ephemeral C++ server and checks validation, all widths, 32 concurrent writers, stale writes, and the Rust demo. Backend-only success does not prove interoperability.

Latest HPP checks passed on Windows: four CTest checks in the gRPC-free benchmark build, 20 Rust regression tests, and the dedicated real C++/Rust integration test after the atomic backend change. A final benchmark/test rebuild also covered parallel independent registers and lock-free bank reporting. Two agent-run samples and one owner-reported sample are recorded in `HPP.md`. The owner reported matching checksums, atomic single at 6.776 ns/op versus mutex single at 13.391 ns/op, and atomic contended at 42.573 ns/op versus mutex contended at 40.331 ns/op. Keep the backend unchanged: one run does not justify tuning, and lock-free does not guarantee higher contended throughput. This documentation-only update did not rerun executable tests or mutate the journal. ARM/MSVC/libatomic fallback, Linux/macOS execution, and physical power-loss behavior remain unverified.

The owner independently resumed `state/demo.wearlog` and reported 500 writes per slot / 2000 remaining budget. This HPP continuation did not run or mutate that journal. To continue the local simulation when desired:

```powershell
.\src\layer2_permacore\target\release\wear_simulator.exe 1000 --state state/demo.wearlog
```

Do not add `--init` to an existing journal. A fresh clone needs explicit initialization with its own new path; the local runtime journal is not published.

Run the new local benchmark without Protobuf/gRPC:

```powershell
mingw32-make benchmark-hpp 'CMAKE_FLAGS=-G "MinGW Makefiles"' BENCHMARK_ITERATIONS=100000 BENCHMARK_THREADS=4
.\build\hpp\hpp_benchmark.exe 100000 4
```

This machine uses existing GCC 14.2.0, Rust 1.98.0, CMake, and GitHub CLI. Missing C++ dependencies were downloaded to an isolated, signature-checked prefix: gRPC 1.82.0 and Protobuf 36.2, with matching runtime dependencies. The failed old-package transaction installed nothing, and installed MSYS2 packages were not upgraded. Scoped escalation may be needed for toolchain/credential/network access; sandbox failures are not proof of bad credentials.

Reproduce this machine's verification from the repository root in PowerShell:

```powershell
$dependencyPrefix = (Join-Path (Get-Location) 'build/msys-prefix/ucrt64').Replace('\', '/')
$env:PATH = "$dependencyPrefix/bin;C:\msys64\ucrt64\bin;$env:PATH"
$env:PROTOC = "$dependencyPrefix/bin/protoc.exe"
mingw32-make check test test-grpc "CMAKE_FLAGS=-G `"MinGW Makefiles`" -DCMAKE_PREFIX_PATH=$dependencyPrefix"
```

This prefix is local and ignored, not part of a fresh clone. A fresh clone needs compatible dependencies installed or independently provisioned as described in `README.md`. Do not reuse archives without verifying them.

## Limits and decisions

- No hardware registers are touched. Preserve this boundary until an owner-authorized, reviewed design exists.
- HPP atomic cells are ordinary RAM, not device memory. Alignment and lock-free observations do not establish MMIO safety, hardware ordering, gRPC speedups, or lifespan improvements. Keep conservative memory ordering and all request validation.
- Loopback insecure gRPC does not authenticate local callers. Authentication/TLS must precede any network exposure.
- Actuator register values and the RAM-only planner reset on restart. Journaled planner counters/health persist; hardware atomicity and 100-year longevity are not established.
- A timed-out write may have completed; never retry it automatically.
- Reservation immediately charges/quarantines its slot; journaled reservation is flushed before a lease is exposed. `NotWritten` requires evidence. Unknown, dropped, and forgotten leases retain their charge and quarantine after restart. No public quarantine reactivation exists; private replay completion is only for validated records.
- Corruption is fatal, not an excuse to initialize. CRC is not authentication/anti-rollback; the journal assumes a trusted operator-owned directory and a filesystem honoring flushes. There is no compaction or device power-loss qualification yet. Metadata I/O is not modeled as physical wear.
- The owner has not selected a project license. Public visibility alone does not establish license terms.
- `AGENTS.md` discovery and third-party quota enforcement cannot be guaranteed by Markdown.

## Budget

Current implementation-session observations on 2026-10-25: 21:13 Bangkok, 6% weekly / 3% five-hour; 21:28, 7% weekly / 10% five-hour, with identical reset timestamps 1793520713 / 1792955130. Observed weekly increase: one percentage point in the interval. Full-day/shared-account attribution and other-provider metrics remain incomplete; reaching the daily 14-point threshold is not established. Preserve this uncertainty and stop feature work immediately if later evidence reaches the cap.

The owner now requires a daily workflow limit of 14% of each full weekly allowance: halt feature work when observed daily consumption or a supported conservative estimate reaches that threshold, then report completed work and tomorrow's tasks. Shared-account agents share the allowance. This does not configure provider limits. Earlier window changes and unavailable metrics prevent an exact full-day total; preserve that uncertainty and never invent a quota-triggered halt.

Latest test-session observations: 13:37 Bangkok, 5% weekly / 32% five-hour; closing observation 13:38 Bangkok, 5% weekly / 33% five-hour. Reset timestamps differ by one second, so no exact interval usage delta or actual reset is asserted. Available evidence does not establish that the daily 14% threshold was reached. Other-provider metrics and complete daily attribution remain unavailable; earlier observations are retained in `progress.md`.

## Next tasks

1. **Next focus — remaining P001 command-stage review:** the approved synthetic read-only telemetry stage is complete (F011); retain its allowlist, separate state authority, OS-random identity, validation, and register/planner independence. Keep ApplyCommand UNIMPLEMENTED until the owner approves an explicit per-component transition/permission table. Future transition tests must cover atomic preconditions, revision exhaustion, instance mismatch, single-winner contention, and unknown outcomes before commands are enabled. Preserve loopback-only simulation and avoid hardware/caller-authentication claims. At the 14% daily cap, every AI contributor must stop feature work and update this handoff immediately as required by AGENTS.md.
2. Validate HPP portability and representative latency distributions on intended targets before changing alignment, memory order, or deployment defaults.
3. Review journal durability on target platforms and design authenticated/monotonic metadata and compaction before hardware integration.
4. Design evidence-based quarantine reconciliation; do not silently reconstruct quarantined slots as healthy.
5. Define logical-data placement/migration invariants before actual storage wear-leveling or hardware integration.
6. Obtain the owner's license decision and add the selected license.

Update progress and handoff records after each substantive session, including mistakes and their fixes.
