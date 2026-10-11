# PermaNexus

PermaNexus explores a polyglot architecture for extending hardware service life. A 100-year lifetime is a research goal; the current implementation does not demonstrate that lifetime.

## Architecture

| Directory | Language | Responsibility | Current status |
| --- | --- | --- | --- |
| `proto/` | Protocol Buffers | Shared versioned contracts | Register read and compare-and-write |
| `src/layer1_actuator/` | C++ / Assembly | Hardware access and low-latency actuation | Loopback register simulator; no hardware access |
| `src/layer2_permacore/` | Rust | Memory safety and wear-leveling | Safe gRPC client, write-budget planner, and demos |
| `src/layer3_omni_agent/` | Go | Concurrent orchestration | Reserved directory |
| `src/layer4_telemetry_ai/` | Python | Predictive telemetry and AI | Reserved directory |

Read [AGENTS.md](AGENTS.md) before changing the project. [progress.md](progress.md) records daily work, checks, and corrections; [handoff.md](handoff.md) provides context for the next contributor.

## First feature: simulated register transactions

The C++ server binds only to `127.0.0.1`, stores registers in memory, and permits device `simulated-0`. A mutex protects each complete compare-and-write operation. Validation covers identity, width, alignment, overflow, expected-value presence, and write permissions. Rejected writes leave state unchanged.

| Byte offset | Width | Initial value | Access |
| --- | --- | --- | --- |
| 0 | 8 bits | 0 | Read / compare-and-write |
| 2 | 16 bits | 0 | Read / compare-and-write |
| 4 | 32 bits | 0 | Read / compare-and-write |
| 8 | 64 bits | 0 | Read / compare-and-write |
| 16 | 32 bits | 1 | Read only |

State resets on restart. The Rust client only accepts `http://127.0.0.1:<port>`, sets two-second RPC deadlines, and does not automatically retry writes. A timeout does not prove a write failed. TLS, caller authentication, persistence, real MMIO, and hardware-backed wear-leveling are not implemented. Loopback does not authenticate other local processes.

## Simulated wear-leveling policy

`wear_leveling::WearLeveler` is an independent, in-memory write-budget planner. It selects a healthy, non-exhausted slot with the lowest `charged_writes / write_limit` utilization, breaking ties by numeric slot ID. Exact u128 cross-products avoid floating-point rounding and u64 multiplication overflow. Selection is O(number of slots), with no allocation per reservation and at most 4096 configured slots.

Each reservation charges one write immediately and quarantines the selected slot. An exclusive Rust borrow allows one outstanding lease per planner; completing the lease consumes it so it cannot be replayed. Immutable snapshots do not allow callers to modify internal counters.

| Completion | Accounting | Slot eligibility |
| --- | --- | --- |
| `Committed` | Keep the charged write | Healthy, eligible while budget remains |
| `NotWritten` | Refund the reserved write | Healthy; requires authoritative proof that no write occurred |
| `Unknown` | Keep the charged write | Quarantined |
| Dropped or forgotten lease | Keep the charged write | Quarantined |
| Explicit retirement | Preserve existing charges | Permanently excluded from this planner |

Never classify a timeout or uncertain transport failure as `NotWritten`. Quarantined slots have no reactivation API in this version. The planner does not execute writes, retry operations, move data, or map slots to actuator registers. State and quarantine are not persisted across process restart. Counts model configured write attempts; they are not measured endurance, physical wear, or evidence of lifetime improvement.

Run a self-contained simulation without starting the actuator:

```bash
make simulate-wear SIMULATION_ARGS="1000 4 1000"
# Arguments: requested writes, slot count, write limit per slot.
cargo run --locked --release --manifest-path src/layer2_permacore/Cargo.toml --bin wear_simulator -- 1000 4 1000
```

On Windows, use `mingw32-make simulate-wear 'SIMULATION_ARGS=1000 4 1000'` with the same PATH/PROTOC setup as below. The CLI allows up to 100000 requested writes, 1..256 slots, and positive u64 limits. Exhaustion stops the simulation and reports the committed count without exceeding any slot limit. The original gRPC demo remains Cargo's default executable.

## Prerequisites

- GNU Make, CMake >= 3.24, a C++17 compiler, and Rust/Cargo.
- Protobuf with proto3 optional support (`protoc` >= 3.15) and gRPC C++ development packages, including CMake configs and `grpc_cpp_plugin`.
- Compiler, Protobuf, and gRPC binaries from a compatible toolchain.

`Cargo.lock` is committed. Generated bindings, local environments, and build products are ignored. Set `PROTOC` for Rust if the compiler is outside `PATH`. CMake accepts dependency locations through `CMAKE_FLAGS`, such as `-DCMAKE_PREFIX_PATH=...` or a toolchain file.

## Build and test

On Linux/macOS with dependencies installed:

```bash
make build
make check
make test
make test-grpc
```

On Windows with MSYS2 UCRT64 and compatible dependencies installed, use PowerShell:

```powershell
$env:PATH = "C:\msys64\ucrt64\bin;$env:PATH"
$env:PROTOC = "C:\msys64\ucrt64\bin\protoc.exe"
mingw32-make build 'CMAKE_FLAGS=-G "MinGW Makefiles" -DCMAKE_PREFIX_PATH=C:/msys64/ucrt64'
mingw32-make check
mingw32-make test 'CMAKE_FLAGS=-G "MinGW Makefiles" -DCMAKE_PREFIX_PATH=C:/msys64/ucrt64'
mingw32-make test-grpc 'CMAKE_FLAGS=-G "MinGW Makefiles" -DCMAKE_PREFIX_PATH=C:/msys64/ucrt64'
```

`test` checks the C++ backend, Rust endpoint policy, simulated wear-leveling invariants, and simulation CLI. `test-grpc` starts the C++ server on an ephemeral loopback port and checks the Rust client, canonical errors, all widths, 32 concurrent writers, rejected stale writes, and the CLI demo. Ordinary `cargo test` explicitly ignores this integration test because it needs a C++ executable.

To test the C++ backend without gRPC installed, use a separate build directory:

```bash
cmake -S src/layer1_actuator -B build/backend -DPERMANEXUS_WITH_GRPC=OFF
cmake --build build/backend --parallel
ctest --test-dir build/backend --output-on-failure
```

With a multi-configuration generator, add `--config Release` when building and `-C Release` when running CTest. On Windows, add `-G "MinGW Makefiles"` when using the UCRT64 compiler.

## Run the demo

Start `build/actuator/actuator_server` in one terminal (`actuator_server.exe` on Windows; multi-configuration generators may put it under `Release/`). Default port: 50051. `--port 0` chooses an ephemeral port and prints its address.

In another terminal:

```bash
cargo run --locked --manifest-path src/layer2_permacore/Cargo.toml -- http://127.0.0.1:50051
```

The demo reads the 32-bit register at offset 4, compares the current value, writes the next value once, and verifies it. Each successful run increments the register. Concurrent callers can cause an `ABORTED` response or verification failure; the demo does not retry writes.

## Collaboration and budget

All committed Markdown, identifiers, and code comments use English. The owner may request Thai conversations.

Agents should plan daily use below 14% of their provider/account's full weekly allowance, preserving 2% across seven days. This repository policy is a planning target, not an enforced subscription setting. Shared usage and unavailable metrics must be recorded honestly. See [AGENTS.md](AGENTS.md).

Cursor can load the root `AGENTS.md` directly. `CLAUDE.md` imports the shared rules for Claude Code. `.aider.conf.yml` loads shared instructions as read-only context and the two daily records as editable files. Existing model choices and credentials are not changed. These files guide agents; they are not a security sandbox or guaranteed quota enforcement. Configuration references: [Cursor rules](https://cursor.com/docs/rules), [Claude project memory](https://code.claude.com/docs/en/memory), and [Aider YAML configuration](https://aider.chat/docs/config/aider_conf.html).

No project license has been selected. Public repository visibility alone does not grant an open-source license; license selection remains an owner decision.
