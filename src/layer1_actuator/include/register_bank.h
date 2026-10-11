#pragma once

#include <array>
#include <cstdint>
#include <mutex>
#include <optional>
#include <string_view>

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

private:
    struct Register {
        std::uint64_t offset;
        int width;
        bool writable;
        std::uint64_t value;
    };

    RegisterResult locate(const RegisterAddress& address, std::size_t& index) const;
    static std::uint64_t maximum_value(int width);

    std::mutex mutex_;
    std::array<Register, 5> registers_{{
        {0, 1, true, 0}, {2, 2, true, 0}, {4, 3, true, 0},
        {8, 4, true, 0}, {16, 3, false, 1},
    }};
};

}  // namespace permanexus
