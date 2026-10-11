# Agent handoff

Last updated: 2026-10-11, Asia/Bangkok.

## Context

Read `AGENTS.md`, `README.md`, and `progress.md`, then inspect Git status. Conversations with the owner use Thai; all committed Markdown, identifiers, and code comments use English.

Repository: `https://github.com/mntoyg/PermaNexus`, branch `main`. Bootstrap reference: `51909a8fcac309625a85f2e2b503a8033c21412a`; register simulator reference: `5c75f90751d25d627b13a641880c2f84ac58212f`. The latest addition is the simulated wear-leveling planner, committed as `Add conservative simulated wear-leveling policy and CLI`. Use `git log -1` and `git ls-remote origin refs/heads/main` for current commit/publication state.

## Implementation

- `proto/actuator.proto`: versioned register read and atomic compare-and-write contracts. Preserve published field numbers.
- C++: five in-memory registers on `simulated-0`; validation, read-only protection, mutex-atomic writes, bounded messages, and loopback-only gRPC server.
- Rust: generated contracts, safe client wrapper with deadlines and no write retries, plus an increment-and-verify demo.
- Rust `wear_leveling.rs`: deterministic normalized-budget selection, exact u128 comparisons, one exclusive lease per planner, conservative unknown-outcome quarantine, proven-no-write refunds, and permanent retirement. The independent `wear_simulator` CLI performs no RPC or hardware access.
- Go/Python remain placeholders. Hardware-backed wear-leveling, register/slot mapping, data migration, persistence, telemetry prediction, and orchestration are not implemented.
- Cargo dependencies are locked; generated files and build products are ignored.
- `CLAUDE.md` imports the shared rules; Aider configuration loads shared rules and editable daily records. Cursor uses root `AGENTS.md`. These are guidance/context files, not hard enforcement or a runtime sandbox.

## Verification

Follow platform commands in `README.md`. Required checks: `make check`, `make test`, and `make test-grpc` (`mingw32-make` on Windows). Integration testing starts an ephemeral C++ server and checks validation, all widths, 32 concurrent writers, stale writes, and the Rust demo. Backend-only success does not prove interoperability.

The register feature's full Windows integration suite passed at the reference above. Latest Rust-only continuation: locked Cargo tests passed (seven planner tests, two CLI tests, and the existing endpoint-policy test), `make check` passed, and Release `make simulate-wear` distributed 1000 writes evenly as 250 per slot. Ordinary Cargo tests intentionally ignored the C++ integration test; it was not rerun for this continuation because no transport or actuator code changed. Linux/macOS execution remains unverified.

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
- Simulator state resets on restart; hardware atomicity and 100-year longevity are not established.
- A timed-out write may have completed; never retry it automatically.
- Reservation immediately charges/quarantines its slot. Only a consumed completion with a definitive result can make it eligible again. `NotWritten` requires evidence; unknown, dropped, and forgotten leases retain their charge and quarantine. No quarantine recovery or persistence exists yet, and modeled budgets are not calibrated hardware endurance.
- The owner has not selected a project license. Public visibility alone does not establish license terms.
- `AGENTS.md` discovery and third-party quota enforcement cannot be guaranteed by Markdown.

## Budget

Plan at most 14% of each full weekly provider/account allowance per Bangkok day. Agents sharing a quota pool share this target. Earlier observations were 24% -> 26% weekly usage. The latest continuation started at 12:34 with 0% reported weekly usage and different reset timestamps; do not subtract that reading from the earlier window. Its final reading was 0% weekly / 3% five-hour usage. Unchanged rounded weekly percentages do not imply zero consumption. Consult `progress.md` for measurement scope. Other providers' metrics remain unavailable; this policy does not configure provider limits.

## Next tasks

1. Architect review: settle register-map/slot ownership, protobuf compatibility, and the actuator/PermaCore trust boundary before integrating the planner with actuation.
2. Design crash-consistent persistence and reviewed quarantine reconciliation for the simulated planner; do not silently reconstruct quarantined slots as healthy.
3. Define logical-data placement/migration invariants before any actual storage wear-leveling or hardware integration.
4. Obtain the owner's license decision and add the selected license.

Update progress and handoff records after each substantive session, including mistakes and their fixes.
