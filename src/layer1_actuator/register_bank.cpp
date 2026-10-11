#include "register_bank.h"

#include "hpp/register_bits.hpp"

namespace permanexus {

RegisterResult RegisterBank::locate(const RegisterAddress& address, std::size_t& index) const {
    if (address.device_id.empty() || hpp::width_bytes(address.width) == 0) {
        return {RegisterStatus::invalid_argument};
    }
    if (!hpp::aligned_offset(address.offset_bytes, address.width)) {
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
    std::size_t index = 0;
    const auto result = locate(address, index);
    if (result.status != RegisterStatus::ok) {
        return result;
    }
    return {RegisterStatus::ok, values_[index].read()};
}

RegisterResult RegisterBank::compare_write(const RegisterAddress& address, std::uint64_t value,
                                          std::optional<std::uint64_t> expected_value) {
    std::size_t index = 0;
    const auto result = locate(address, index);
    if (result.status != RegisterStatus::ok) {
        return result;
    }
    if (!expected_value || !hpp::value_fits(value, address.width) ||
        !hpp::value_fits(*expected_value, address.width)) {
        return {RegisterStatus::invalid_argument};
    }
    const auto& target = registers_[index];
    if (!target.writable) {
        return {RegisterStatus::permission_denied};
    }
    const auto exchange = values_[index].compare_write(*expected_value, value);
    if (!exchange.exchanged) {
        return {RegisterStatus::aborted};
    }
    return {RegisterStatus::ok, exchange.observed};
}

bool RegisterBank::registers_are_lock_free() const noexcept {
    for (const auto& value : values_) {
        if (!value.is_lock_free()) { return false; }
    }
    return true;
}

}  // namespace permanexus
