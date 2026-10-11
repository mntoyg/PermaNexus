#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <string_view>
#include "hpp/atomic_register.hpp"

namespace permanexus {

enum class RegisterStatus { ok, invalid_argument, not_found, permission_denied, aborted };

struct RegisterResult {
    RegisterStatus status;
    std::uint64_t value = 0;
};

struct RegisterAddress {
    std::string_view device_id;
    std::uint64_t offset_bytes;
    int width;
};

// This backend only stores simulated registers. It never maps hardware memory.
class RegisterBank final {
public:
    RegisterResult read(const RegisterAddress& address);
    RegisterResult compare_write(const RegisterAddress& address, std::uint64_t value,
                                 std::optional<std::uint64_t> expected_value);
    bool registers_are_lock_free() const noexcept;

private:
    struct RegisterSpec {
        std::uint64_t offset;
        int width;
        bool writable;
    };

    RegisterResult locate(const RegisterAddress& address, std::size_t& index) const;
    inline static constexpr std::array<RegisterSpec, 5> registers_{{
        {0, 1, true}, {2, 2, true}, {4, 3, true}, {8, 4, true}, {16, 3, false},
    }};
    using RegisterValue = hpp::AtomicRegister<std::uint64_t>;
    std::array<RegisterValue, 5> values_{{
        RegisterValue{0}, RegisterValue{0}, RegisterValue{0}, RegisterValue{0}, RegisterValue{1},
    }};
};

}  // namespace permanexus
