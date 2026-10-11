#pragma once

#include <atomic>
#include <cstddef>
#include <type_traits>

namespace permanexus::hpp {

// Alignment is a layout choice, not a claim about the host's physical cache line.
template <typename UInt, std::size_t Alignment = 64>
class alignas(Alignment) AtomicRegister final {
    static_assert(std::is_integral_v<UInt> && std::is_unsigned_v<UInt> &&
                  !std::is_same_v<UInt, bool> && sizeof(UInt) <= 8,
                  "AtomicRegister requires an unsigned integer of at most 64 bits");
    static_assert(Alignment >= alignof(std::atomic<UInt>) &&
                  (Alignment & (Alignment - 1)) == 0,
                  "Alignment must be a supported power of two");

public:
    struct CompareResult {
        bool exchanged;
        UInt observed;
    };

    explicit AtomicRegister(UInt initial = 0) noexcept : value_(initial) {}
    AtomicRegister(const AtomicRegister&) = delete;
    AtomicRegister& operator=(const AtomicRegister&) = delete;

    UInt read() const noexcept {
        return value_.load(std::memory_order_seq_cst);
    }

    // One strong CAS, no retry loop, no spurious failure, no raw address access.
    CompareResult compare_write(UInt expected, UInt desired) noexcept {
        const bool exchanged = value_.compare_exchange_strong(
            expected, desired, std::memory_order_seq_cst, std::memory_order_seq_cst);
        return {exchanged, expected};
    }

    bool is_lock_free() const noexcept { return value_.is_lock_free(); }
    static constexpr std::size_t alignment_bytes = Alignment;
    static constexpr bool always_lock_free = std::atomic<UInt>::is_always_lock_free;

private:
    // Ordinary owned RAM only. Never cast MMIO or device memory to this type.
    std::atomic<UInt> value_;
};

}  // namespace permanexus::hpp
