#include "hpp/atomic_register.hpp"
#include "hpp/register_bits.hpp"
#include "hpp/spin_wait.hpp"

#include <array>
#include <atomic>
#include <cstdlib>
#include <iostream>
#include <limits>
#include <thread>
#include <type_traits>
#include <vector>

namespace {
void require(bool condition, const char* message) {
    if (!condition) { std::cerr << message << '\n'; std::exit(1); }
}

template <typename UInt>
void check_atomic_width() {
    permanexus::hpp::AtomicRegister<UInt> cell;
    const auto maximum = std::numeric_limits<UInt>::max();
    const auto written = cell.compare_write(0, maximum);
    require(written.exchanged && written.observed == 0, "Atomic width write failed");
    const auto stale = cell.compare_write(0, 1);
    require(!stale.exchanged && stale.observed == maximum, "Stale CAS was accepted or lost the observed value");
    require(cell.read() == maximum, "Stale CAS changed the value");
}
}  // namespace

int main() {
    using namespace permanexus::hpp;
    static_assert(width_bytes(4) == 8 && width_bytes(-1) == 0);
    static_assert(value_fits(255, 1) && !value_fits(256, 1));
    static_assert(value_fits(std::numeric_limits<std::uint64_t>::max(), 4));
    static_assert(!value_mask(0) && !aligned_offset(0, 0));
    static_assert(replace_bits(0xa5, 0x3c, 0x0f, 1).value() == 0xac);
    static_assert(!replace_bits(0x100, 0, 0xff, 1));
    static_assert(!replace_bits(0, 0x100, 0xff, 1));
    static_assert(!replace_bits(0, 0, 0x100, 1));
    static_assert(!replace_bits(0, 0, 0, 99));
    static_assert(alignof(AtomicRegister<std::uint64_t>) == 64);
    static_assert(sizeof(AtomicRegister<std::uint64_t>) % 64 == 0);
    static_assert(!std::is_copy_constructible_v<AtomicRegister<std::uint64_t>>);
    check_atomic_width<std::uint8_t>(); check_atomic_width<std::uint16_t>();
    check_atomic_width<std::uint32_t>(); check_atomic_width<std::uint64_t>();

    AtomicRegister<std::uint64_t> counter;
    std::atomic<unsigned> ready{0}, successes{0};
    std::atomic<bool> start{false};
    std::vector<std::thread> writers;
    for (unsigned index = 0; index < 32; ++index) {
        writers.emplace_back([&] {
            ready.fetch_add(1, std::memory_order_release);
            SpinWait wait;
            while (!start.load(std::memory_order_acquire)) { wait.pause(); }
            if (counter.compare_write(0, 1).exchanged) { successes.fetch_add(1); }
        });
    }
    SpinWait wait;
    while (ready.load(std::memory_order_acquire) != 32) { wait.pause(); }
    start.store(true, std::memory_order_release);
    for (auto& writer : writers) { writer.join(); }
    require(successes == 1 && counter.read() == 1, "Concurrent CAS did not have exactly one winner");
    std::cout << "HPP bit validation, four atomic widths, alignment, and 32-writer CAS passed\n";
}
