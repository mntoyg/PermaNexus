#pragma once

#include <cstdint>
#include <limits>
#include <optional>

namespace permanexus::hpp {

// Width codes match the versioned protobuf enum. Invalid codes never shift bits.
constexpr unsigned width_bytes(int width) noexcept {
    switch (width) {
        case 1: return 1;
        case 2: return 2;
        case 3: return 4;
        case 4: return 8;
        default: return 0;
    }
}

constexpr std::optional<std::uint64_t> value_mask(int width) noexcept {
    switch (width) {
        case 1: return 0xffULL;
        case 2: return 0xffffULL;
        case 3: return 0xffffffffULL;
        case 4: return std::numeric_limits<std::uint64_t>::max();
        default: return std::nullopt;
    }
}

constexpr bool value_fits(std::uint64_t value, int width) noexcept {
    const auto mask = value_mask(width);
    return mask && (value & ~*mask) == 0;
}

constexpr bool aligned_offset(std::uint64_t offset, int width) noexcept {
    const auto bytes = width_bytes(width);
    return bytes != 0 && offset % bytes == 0;
}

// Pure value transformation; this does not read or write a hardware register.
constexpr std::optional<std::uint64_t> replace_bits(std::uint64_t current,
                                                 std::uint64_t desired,
                                                 std::uint64_t mask,
                                                 int width) noexcept {
    if (!value_fits(current, width) || !value_fits(desired, width) || !value_fits(mask, width)) {
        return std::nullopt;
    }
    return (current & ~mask) | (desired & mask);
}

}  // namespace permanexus::hpp
