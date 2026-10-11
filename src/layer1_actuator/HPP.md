# Hardware Performance Programming in Layer 1

HPP is the owner's project label for performance-oriented C++ infrastructure. The `.hpp` headers operate on ordinary owned RAM and pure integer values. They do not provide MMIO, hardware drivers, CPU-frequency control, privileged instructions, or evidence of hardware lifespan improvement.

## Headers and contracts

| Header | Contract |
| --- | --- |
| `include/hpp/register_bits.hpp` | Width codes match protobuf values 1..4; invalid widths/alignment and values outside a mask fail without undefined shifts. `replace_bits` preserves bits outside its mask and returns an optional result. |
| `include/hpp/atomic_register.hpp` | Unsigned integer values up to 64 bits; one strong CAS, no spurious failure/retry, deleted copying, sequentially consistent loads/CAS, observed-value result, runtime lock-free query. |
| `include/hpp/spin_wait.hpp` | x86 PAUSE or scheduler-yield fallback; yields after 16 hints. This is a scheduling hint, not synchronization. |

Default atomic-cell alignment is 64 bytes. This is a configurable template layout choice, not discovery of physical cache-line size; it cannot guarantee absence of false sharing on every CPU. Cells are owned objects: never reinterpret device/MMIO addresses as these types. Native atomic64 linkage is checked by CMake, with optional `libatomic` linkage when required. Lock-free behavior remains a runtime property.

The simulator uses immutable static register descriptors and separate aligned atomic values. Validation and gRPC errors are preserved; each register's read/compare-and-write is atomic, but there is no transaction across multiple registers. CAS compares numeric values and does not detect ABA cycles; versioned transactions would need a separate contract. Removing the former global mutex does not authorize weaker validation or real hardware atomic access.

## Benchmark method

Build with `PERMANEXUS_BUILD_BENCHMARKS=ON`; `make benchmark-hpp` uses a separate `build/hpp/` tree with gRPC disabled. Normal builds keep benchmarks optional. Multi-configuration generators use the configured target-path file to locate the executable.

Arguments: `hpp_benchmark [iterations [threads]]`. Defaults: 1000000 iterations, 4 threads. Bounds: 1000..5000000 total completed operations and 1..32 threads. No CPU pinning, priority changes, `-march=native`, or system tuning is performed.

- Atomic and mutex single-thread cases have 10000 warmup operations, identical value/checksum checks, and the same timed CAS workload.
- Contended cases partition total operations across workers, release them through an acquire/release start barrier, and verify the final counter. Local CAS retries are benchmark-only; the actuator still attempts a requested write once.
- The validated bank case includes device/width/permission/value checks; it is a separate workload without a historical whole-bank baseline. Do not compare its timing directly to the bare-cell timings as a speedup claim.
- Timings use a monotonic wall clock and report average nanoseconds per completed operation. Thread scheduling/join overhead is included in contended timing. Results are not tail latencies, energy measurements, bus timing, or RPC throughput.
- Outputs consume checksums/final values so the compiler cannot remove the benchmark work. Tests validate results, not nanosecond thresholds.

GCC 14.2.0 / Windows x86_64 Release samples on 2026-10-11, 100000 iterations, four contending threads, default 64-byte alignment; atomic64 and all bank values reported lock-free:

| Case | First sample, ns/op | Final sample, ns/op | Owner-reported sample, ns/op |
| --- | ---: | ---: | ---: |
| Atomic single | 5.564 | 6.848 | 6.776 |
| Mutex single | 15.590 | 14.988 | 13.391 |
| Atomic contended | 46.692 | 56.659 | 42.573 |
| Mutex contended | 50.788 | 61.504 | 40.331 |
| Validated bank single | 12.284 | 19.223 | 18.848 |

The owner reported the third sample from `.\build\hpp\hpp_benchmark.exe 100000 4` on the same date. Its metadata reported `atomic64_lock_free=true`, `alignment_bytes=64`, and `bank_lock_free=true`. Single-thread checksums matched at 5999950000; both contended counters were 100000; the validated bank checksum was 4999950000. This is supplied execution output, not an additional agent-run measurement.

In that sample, atomic single-thread throughput was approximately 1.98 times the mutex case, while atomic contended elapsed time per operation was approximately 5.6% higher than the mutex case. A lock-free report does not guarantee better throughput under contention. Preserve the current backend and gather representative repeated measurements before changing deployment defaults; this single sample does not establish the cause of the timing difference.

These observations demonstrate workload variance, not a portable performance guarantee or whole-system speedup. ARM, MSVC, alternative cache-line sizes, and the libatomic fallback have not been executed in this session.

## Verification

CTest checks bit/width boundaries, all four atomic widths, stale-write rejection, alignment, 32 writers competing from a shared start barrier, parallel independent registers, benchmark correctness, and rejected benchmark arguments. The actual C++ server/Rust client integration suite verifies wire behavior after the backend change. Persistent Rust journal state is unrelated and is not modified by these benchmarks.
