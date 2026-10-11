# Agent handoff

Last updated: 2026-10-11, Asia/Bangkok.

## Context

Read `AGENTS.md`, `README.md`, and `progress.md`, then inspect Git status. Conversations with the owner use Thai; all committed Markdown, identifiers, and code comments use English.

Repository: `https://github.com/mntoyg/PermaNexus`, branch `main`. Persistence reference: `5b09472598f0dda625bcfcf88fa73c0daabf840e`. Latest source reference: `bf08902c83f6372332343a5a8b201cbb94020ec8`, HPP RAM primitives and the per-register atomic backend. The latest documentation update records the owner's independent benchmark output. Use `git log -1` and `git ls-remote origin refs/heads/main` for current commit/publication state.

## Implementation

- `proto/actuator.proto`: versioned register read and atomic compare-and-write contracts. Preserve published field numbers.
- C++: five in-memory registers on `simulated-0`; immutable descriptors, aligned per-register strong CAS, preserved validation/read-only policy, bounded messages, and loopback-only gRPC server.
- HPP: three C++17 `.hpp` headers for bit validation, owned RAM atomics, and spin hints; optional benchmark and CMake atomic-link compatibility probe. Read `src/layer1_actuator/HPP.md` before interpreting timings or adding hardware-oriented primitives.
- Rust: generated contracts, safe client wrapper with deadlines and no write retries, plus an increment-and-verify demo.
- Rust `wear_leveling.rs`: deterministic normalized-budget selection, exact u128 comparisons, one exclusive lease per planner, conservative unknown-outcome quarantine, proven-no-write refunds, and permanent retirement. The independent `wear_simulator` CLI performs no RPC or hardware access.
- Rust `wear_journal.rs`: exclusive file locking, flushed append-only reservations/completions, sequence/CRC/transition validation, bounded replay, and poisoning on write failures. The CLI supports `--state PATH` and explicit create-new `--init`. Read `src/layer2_permacore/JOURNAL_FORMAT.md` before changing the format.
- Go/Python remain placeholders. Hardware-backed wear-leveling, register/slot mapping, data migration, persistent actuator values, telemetry prediction, and orchestration are not implemented.
- Cargo dependencies are locked; generated files and build products are ignored.
- `CLAUDE.md` imports the shared rules; Aider configuration loads shared rules and editable daily records. Cursor uses root `AGENTS.md`. These are guidance/context files, not hard enforcement or a runtime sandbox.

## Verification

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

Plan at most 14% of each full weekly provider/account allowance per Bangkok day. Agents sharing a quota pool share this target. The HPP continuation started at 13:01 with 2% weekly / 14% five-hour usage and finished with 3% weekly / 21% five-hour usage, with identical reset timestamps: one observed weekly percentage point during that interval. This does not establish full-day use or exclusive chat attribution. Earlier windows and timestamp differences remain recorded in `progress.md`. Other providers' metrics are unavailable; this policy does not configure provider limits.

Latest documentation session usage observations: 13:17 Bangkok, 4% weekly / 23% five-hour; 13:19 Bangkok, 4% weekly / 24% five-hour. Reported reset timestamps shifted by one second, so no exact interval usage delta or actual reset is asserted. See `progress.md` for the raw window observations.

## Next tasks

1. Architect review: settle register-map/slot ownership, protobuf compatibility, and the actuator/PermaCore trust boundary before integrating the planner with actuation.
2. Validate HPP portability and representative latency distributions on intended targets before changing alignment, memory order, or deployment defaults.
3. Review journal durability on target platforms and design authenticated/monotonic metadata and compaction before hardware integration.
4. Design evidence-based quarantine reconciliation; do not silently reconstruct quarantined slots as healthy.
5. Define logical-data placement/migration invariants before actual storage wear-leveling or hardware integration.
6. Obtain the owner's license decision and add the selected license.

Update progress and handoff records after each substantive session, including mistakes and their fixes.
