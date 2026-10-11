#include "register_bank.h"

#include <atomic>
#include <cstdlib>
#include <iostream>
#include <limits>
#include <thread>
#include <vector>

namespace {
void require(bool condition, const char* message) {
    if (!condition) {
        std::cerr << message << '\n';
        std::exit(1);
    }
}
}  // namespace

int main() {
    using namespace permanexus;
    RegisterBank bank;
    const RegisterAddress register32{"simulated-0", 4, 3};
    require(bank.read(register32).value == 0, "Initial value must be zero");
    require(bank.compare_write(register32, 42, 0).status == RegisterStatus::ok, "Write failed");
    require(bank.compare_write(register32, 99, 0).status == RegisterStatus::aborted, "Stale write accepted");
    require(bank.read(register32).value == 42, "Stale write changed the register");
    require(bank.compare_write(register32, 1, std::nullopt).status == RegisterStatus::invalid_argument,
            "Missing expected value accepted");
    require(bank.compare_write(register32, std::uint64_t{1} << 32, 42).status == RegisterStatus::invalid_argument,
            "Overflow accepted");
    require(bank.compare_write(register32, 0, std::uint64_t{1} << 32).status == RegisterStatus::invalid_argument,
            "Expected value overflow accepted");
    require(bank.read({"", 4, 3}).status == RegisterStatus::invalid_argument, "Empty device accepted");
    require(bank.read({"unknown", 4, 3}).status == RegisterStatus::not_found, "Unknown device accepted");
    require(bank.read({"simulated-0", 3, 3}).status == RegisterStatus::invalid_argument, "Unaligned address accepted");
    require(bank.read({"simulated-0", 20, 3}).status == RegisterStatus::not_found, "Unknown register accepted");
    require(bank.read({"simulated-0", 4, 0}).status == RegisterStatus::invalid_argument, "Unspecified width accepted");
    require(bank.read({"simulated-0", 4, 5}).status == RegisterStatus::invalid_argument, "Unknown width accepted");
    require(bank.read({"simulated-0", 4, 2}).status == RegisterStatus::invalid_argument, "Wrong register width accepted");
    require(bank.compare_write({"simulated-0", 16, 3}, 2, 1).status == RegisterStatus::permission_denied,
            "Read-only register changed");

    for (const auto& address : {RegisterAddress{"simulated-0", 0, 1}, {"simulated-0", 2, 2},
                                {"simulated-0", 8, 4}}) {
        const auto maximum = address.width == 4 ? std::numeric_limits<std::uint64_t>::max()
            : (std::uint64_t{1} << (8U << (address.width - 1))) - 1;
        require(bank.compare_write(address, maximum, 0).status == RegisterStatus::ok, "Width limit rejected");
        require(bank.read(address).value == maximum, "Width limit round-trip failed");
    }

    std::atomic<unsigned> successes{0};
    std::vector<std::thread> writers;
    for (unsigned i = 0; i < 32; ++i) {
        writers.emplace_back([&bank, &successes, register32] {
            if (bank.compare_write(register32, 43, 42).status == RegisterStatus::ok) {
                ++successes;
            }
        });
    }
    for (auto& writer : writers) {
        writer.join();
    }
    require(successes == 1, "Compare-and-write was not atomic");
    require(bank.read(register32).value == 43, "Unexpected final value");
    std::cout << "Register validation and 32-writer atomicity passed\n";
}
