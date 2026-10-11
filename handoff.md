# Agent handoff

Last updated: 2026-10-11, Asia/Bangkok.

## Context

Read `AGENTS.md`, `README.md`, and `progress.md`, then inspect Git status. Conversations with the owner use Thai; all committed Markdown, identifiers, and code comments use English.

Repository: `https://github.com/mntoyg/PermaNexus`, branch `main`. Register simulator reference: `5c75f90751d25d627b13a641880c2f84ac58212f`; in-memory policy reference: `864a7d63010b98a68bd9e7f5f5968c8023c1dff8`. The latest addition is append-only planner persistence, committed as `Persist simulated wear state with a locked write-ahead journal`. Use `git log -1` and `git ls-remote origin refs/heads/main` for current commit/publication state.

## Implementation

- `proto/actuator.proto`: versioned register read and atomic compare-and-write contracts. Preserve published field numbers.
- C++: five in-memory registers on `simulated-0`; validation, read-only protection, mutex-atomic writes, bounded messages, and loopback-only gRPC server.
- Rust: generated contracts, safe client wrapper with deadlines and no write retries, plus an increment-and-verify demo.
- Rust `wear_leveling.rs`: deterministic normalized-budget selection, exact u128 comparisons, one exclusive lease per planner, conservative unknown-outcome quarantine, proven-no-write refunds, and permanent retirement. The independent `wear_simulator` CLI performs no RPC or hardware access.
- Rust `wear_journal.rs`: exclusive file locking, flushed append-only reservations/completions, sequence/CRC/transition validation, bounded replay, and poisoning on write failures. The CLI supports `--state PATH` and explicit create-new `--init`. Read `src/layer2_permacore/JOURNAL_FORMAT.md` before changing the format.
- Go/Python remain placeholders. Hardware-backed wear-leveling, register/slot mapping, data migration, persistent actuator values, telemetry prediction, and orchestration are not implemented.
- Cargo dependencies are locked; generated files and build products are ignored.
- `CLAUDE.md` imports the shared rules; Aider configuration loads shared rules and editable daily records. Cursor uses root `AGENTS.md`. These are guidance/context files, not hard enforcement or a runtime sandbox.

## Verification

Follow platform commands in `README.md`. Required checks: `make check`, `make test`, and `make test-grpc` (`mingw32-make` on Windows). Integration testing starts an ephemeral C++ server and checks validation, all widths, 32 concurrent writers, stale writes, and the Rust demo. Backend-only success does not prove interoperability.

The register feature's full Windows integration suite passed at its reference above. Latest Rust continuation: all 20 executed tests passed, including ten new persistence checks (process interruption, cross-process locking, replay/corruption, incomplete tails, failed writes, capacity bounds, and CLI restart/configuration protection). `make check` and the existing Release RAM simulation passed. The C++ integration test remained explicitly ignored; transport/actuator code did not change. Linux/macOS and physical power-loss behavior remain unverified.

The owner independently ran the previous 1000-write RAM demo and reported 250 writes per slot. This continuation reproduced that distribution in a new ignored `state/demo.wearlog` and reopened it with zero additional writes in a second process. To continue the prepared local journal:

```powershell
.\src\layer2_permacore\target\release\wear_simulator.exe 1000 --state state/demo.wearlog
```

Do not add `--init` to an existing journal. A fresh clone needs explicit initialization with its own new path; the local runtime journal is not published.

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
- Loopback insecure gRPC does not authenticate local callers. Authentication/TLS must precede any network exposure.
- Actuator register values and the RAM-only planner reset on restart. Journaled planner counters/health persist; hardware atomicity and 100-year longevity are not established.
- A timed-out write may have completed; never retry it automatically.
- Reservation immediately charges/quarantines its slot; journaled reservation is flushed before a lease is exposed. `NotWritten` requires evidence. Unknown, dropped, and forgotten leases retain their charge and quarantine after restart. No public quarantine reactivation exists; private replay completion is only for validated records.
- Corruption is fatal, not an excuse to initialize. CRC is not authentication/anti-rollback; the journal assumes a trusted operator-owned directory and a filesystem honoring flushes. There is no compaction or device power-loss qualification yet. Metadata I/O is not modeled as physical wear.
- The owner has not selected a project license. Public visibility alone does not establish license terms.
- `AGENTS.md` discovery and third-party quota enforcement cannot be guaranteed by Markdown.

## Budget

Plan at most 14% of each full weekly provider/account allowance per Bangkok day. Agents sharing a quota pool share this target. Earlier observations were 24% -> 26% weekly usage, followed by a different reported window at 0%. The persistence continuation started at 12:44 with 1% weekly / 5% five-hour usage and ended with 2% weekly / 12% five-hour usage. Reset timestamps shifted by one second; do not assert an actual reset or precise interval attribution from that difference. Consult `progress.md` for scope and earlier observations. Other providers' metrics remain unavailable; this policy does not configure provider limits.

## Next tasks

1. Architect review: settle register-map/slot ownership, protobuf compatibility, and the actuator/PermaCore trust boundary before integrating the planner with actuation.
2. Review journal durability on target platforms and design authenticated/monotonic metadata and compaction before hardware integration.
3. Design evidence-based quarantine reconciliation; do not silently reconstruct quarantined slots as healthy.
4. Define logical-data placement/migration invariants before actual storage wear-leveling or hardware integration.
5. Obtain the owner's license decision and add the selected license.

Update progress and handoff records after each substantive session, including mistakes and their fixes.
