#include "hpp/atomic_register.hpp"
#include "hpp/spin_wait.hpp"
#include "register_bank.h"

#include <atomic>
#include <charconv>
#include <chrono>
#include <cstdint>
#include <iomanip>
#include <iostream>
#include <mutex>
#include <stdexcept>
#include <string_view>
#include <thread>
#include <vector>

namespace {
using Clock = std::chrono::steady_clock;
using AtomicCell = permanexus::hpp::AtomicRegister<std::uint64_t>;
constexpr std::uint64_t warmup_iterations = 10000;

class alignas(64) MutexCell {
public:
    std::uint64_t read() const {
        const std::lock_guard<std::mutex> guard(mutex_);
        return value_;
    }
    AtomicCell::CompareResult compare_write(std::uint64_t expected, std::uint64_t desired) {
        const std::lock_guard<std::mutex> guard(mutex_);
        const auto observed = value_;
        if (observed == expected) { value_ = desired; return {true, observed}; }
        return {false, observed};
    }
private:
    mutable std::mutex mutex_;
    std::uint64_t value_ = 0;
};

void report(std::string_view name, std::uint64_t iterations, Clock::time_point start,
            std::uint64_t checksum) {
    const auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(Clock::now() - start).count();
    std::cout << "benchmark=" << name << " iterations=" << iterations << " elapsed_ns=" << elapsed
              << " ns_per_op=" << std::fixed << std::setprecision(3)
              << static_cast<double>(elapsed) / static_cast<double>(iterations)
              << " checksum=" << checksum << '\n';
}

template <typename Cell>
void single_thread(std::string_view name, std::uint64_t iterations) {
    Cell cell;
    for (std::uint64_t value = 0; value < warmup_iterations; ++value) {
        if (!cell.compare_write(value, value + 1).exchanged) { throw std::runtime_error("Warmup CAS failed"); }
    }
    std::uint64_t checksum = 0;
    const auto start = Clock::now();
    for (std::uint64_t index = 0; index < iterations; ++index) {
        const auto expected = warmup_iterations + index;
        const auto result = cell.compare_write(expected, expected + 1);
        if (!result.exchanged) { throw std::runtime_error("Uncontended CAS failed"); }
        checksum += result.observed;
    }
    if (cell.read() != warmup_iterations + iterations) { throw std::runtime_error("Uncontended count mismatch"); }
    report(name, iterations, start, checksum);
}

template <typename Cell>
void contended(std::string_view name, std::uint64_t iterations, unsigned threads) {
    Cell cell;
    std::atomic<unsigned> ready{0};
    std::atomic<bool> start_flag{false};
    std::vector<std::thread> workers;
    workers.reserve(threads);
    try {
        for (unsigned index = 0; index < threads; ++index) {
            const auto count = iterations / threads + (index < iterations % threads ? 1 : 0);
            workers.emplace_back([&, count] {
                ready.fetch_add(1, std::memory_order_release);
                permanexus::hpp::SpinWait wait;
                while (!start_flag.load(std::memory_order_acquire)) { wait.pause(); }
                for (std::uint64_t operation = 0; operation < count; ++operation) {
                    auto expected = cell.read();
                    wait.reset();
                    // Retrying a local RAM counter is only a benchmark workload, never an RPC write policy.
                    while (true) {
                        const auto result = cell.compare_write(expected, expected + 1);
                        if (result.exchanged) { break; }
                        expected = result.observed;
                        wait.pause();
                    }
                }
            });
        }
    } catch (...) {
        start_flag.store(true, std::memory_order_release);
        for (auto& worker : workers) { worker.join(); }
        throw;
    }
    permanexus::hpp::SpinWait wait;
    while (ready.load(std::memory_order_acquire) != threads) { wait.pause(); }
    const auto start = Clock::now();
    start_flag.store(true, std::memory_order_release);
    for (auto& worker : workers) { worker.join(); }
    if (cell.read() != iterations) { throw std::runtime_error("Contended count mismatch"); }
    report(name, iterations, start, cell.read());
}

void bank_path(std::uint64_t iterations) {
    permanexus::RegisterBank bank;
    const permanexus::RegisterAddress address{"simulated-0", 4, 3};
    std::uint64_t checksum = 0;
    const auto start = Clock::now();
    for (std::uint64_t index = 0; index < iterations; ++index) {
        const auto result = bank.compare_write(address, index + 1, index);
        if (result.status != permanexus::RegisterStatus::ok) { throw std::runtime_error("Validated bank CAS failed"); }
        checksum += result.value;
    }
    if (bank.read(address).value != iterations) { throw std::runtime_error("Validated bank count mismatch"); }
    report("validated_bank_single", iterations, start, checksum);
}

std::uint64_t parse(std::string_view value) {
    std::uint64_t result = 0;
    const auto parsed = std::from_chars(value.data(), value.data() + value.size(), result);
    if (parsed.ec != std::errc{} || parsed.ptr != value.data() + value.size()) {
        throw std::invalid_argument("Arguments must be unsigned integers");
    }
    return result;
}
}  // namespace

int main(int argc, char** argv) {
    try {
        if (argc > 3) { throw std::invalid_argument("Usage: hpp_benchmark [iterations [threads]]"); }
        const auto iterations = argc >= 2 ? parse(argv[1]) : 1000000;
        const auto thread_count = argc == 3 ? parse(argv[2]) : 4;
        if (iterations < 1000 || iterations > 5000000 || thread_count < 1 || thread_count > 32) {
            throw std::invalid_argument("Requires iterations in 1000..5000000 and threads in 1..32");
        }
        AtomicCell probe;
        permanexus::RegisterBank bank_probe;
        std::cout << "atomic64_lock_free=" << std::boolalpha << probe.is_lock_free()
                  << " alignment_bytes=" << AtomicCell::alignment_bytes
                  << " bank_lock_free=" << bank_probe.registers_are_lock_free()
                  << " threads=" << thread_count << '\n';
        single_thread<AtomicCell>("atomic_single", iterations);
        single_thread<MutexCell>("mutex_single", iterations);
        contended<AtomicCell>("atomic_contended", iterations, static_cast<unsigned>(thread_count));
        contended<MutexCell>("mutex_contended", iterations, static_cast<unsigned>(thread_count));
        bank_path(iterations);
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "Error: " << error.what() << '\n';
        return 1;
    }
}
