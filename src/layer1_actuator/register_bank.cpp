#include "register_bank.h"

#include <limits>

namespace permanexus {

std::uint64_t RegisterBank::maximum_value(int width) {
    if (width == 4) {
        return std::numeric_limits<std::uint64_t>::max();
    }
    const auto bits = 8U << (width - 1);
    return (std::uint64_t{1} << bits) - 1;
}

RegisterResult RegisterBank::locate(const RegisterAddress& address, std::size_t& index) const {
    if (address.device_id.empty() || address.width < 1 || address.width > 4) {
        return {RegisterStatus::invalid_argument};
    }
    const auto alignment = std::uint64_t{1} << (address.width - 1);
    if (address.offset_bytes % alignment != 0) {
        return {RegisterStatus::invalid_argument};
    }
    if (address.device_id != "simulated-0") {
        return {RegisterStatus::not_found};
    }
    for (index = 0; index < registers_.size(); ++index) {
        if (registers_[index].offset == address.offset_bytes) {
            return {registers_[index].width == address.width
                        ? RegisterStatus::ok : RegisterStatus::invalid_argument};
        }
    }
    return {RegisterStatus::not_found};
}

RegisterResult RegisterBank::read(const RegisterAddress& address) {
    const std::lock_guard<std::mutex> lock(mutex_);
    std::size_t index = 0;
    const auto result = locate(address, index);
    if (result.status != RegisterStatus::ok) {
        return result;
    }
    return {RegisterStatus::ok, registers_[index].value};
}

RegisterResult RegisterBank::compare_write(const RegisterAddress& address, std::uint64_t value,
                                          std::optional<std::uint64_t> expected_value) {
    const std::lock_guard<std::mutex> lock(mutex_);
    std::size_t index = 0;
    const auto result = locate(address, index);
    if (result.status != RegisterStatus::ok) {
        return result;
    }
    if (!expected_value || value > maximum_value(address.width) ||
        *expected_value > maximum_value(address.width)) {
        return {RegisterStatus::invalid_argument};
    }
    auto& target = registers_[index];
    if (!target.writable) {
        return {RegisterStatus::permission_denied};
    }
    if (target.value != *expected_value) {
        return {RegisterStatus::aborted};
    }
    const auto previous = target.value;
    target.value = value;
    return {RegisterStatus::ok, previous};
}

}  // namespace permanexus
