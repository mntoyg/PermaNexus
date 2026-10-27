#include "telemetry_store.h"
#include "register_bank.h"

#include <atomic>
#include <cstdlib>
#include <iostream>
#include <limits>
#include <thread>
#include <vector>

namespace {
namespace wire = permanexus::actuator::v1;
void require(bool condition, const char* message) {
    if (!condition) { std::cerr << message << '\n'; std::exit(1); }
}
wire::TelemetryRequest request(std::string component = "core-0",
                               std::string sensor = "synthetic-temperature") {
    wire::TelemetryRequest result;
    result.mutable_target()->set_device_id("simulated-0");
    result.mutable_target()->set_component_id(std::move(component));
    result.set_sensor_id(std::move(sensor));
    return result;
}
bool failed_random(permanexus::InstanceId& output) {
    output[0] = 42;  // A partial result must never become a process identity.
    return false;
}
}  // namespace

int main() {
    using namespace permanexus;
    require(!generate_instance_id(failed_random) && !generate_instance_id(nullptr),
            "Random failure issued a process identity");
    const auto os_id = generate_instance_id();
    require(os_id.has_value(), "OS random source unavailable in test environment");
    const InstanceId id{{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 255}};
    const MicroStateStore state_store(id);
    const TelemetryStore store(state_store);
    const std::string expected_id(reinterpret_cast<const char*>(id.data()), id.size());
    const auto sample = store.read(request());
    require(sample.status == TelemetryStatus::ok, "Synthetic sample failed");
    require(sample.data.has_target() && sample.data.target().device_id() == "simulated-0" &&
            sample.data.target().component_id() == "core-0" &&
            sample.data.sensor_id() == "synthetic-temperature" &&
            sample.data.has_value() && sample.data.value() == 42.5 &&
            sample.data.unit() == wire::TELEMETRY_UNIT_CELSIUS &&
            sample.data.has_sampled_at_unix_ns() && sample.data.sampled_at_unix_ns() == 0,
            "Synthetic sample metadata changed");
    require(sample.data.has_observed_state() && sample.data.observed_state().has_revision() &&
            sample.data.observed_state().revision() == 0 &&
            sample.data.observed_state().state() == wire::MICRO_STATE_ACTIVE &&
            sample.data.observed_state().instance_id() == expected_id, "Snapshot mismatch");
    const auto zero = store.read(request("core-0", "synthetic-utilization"));
    require(zero.status == TelemetryStatus::ok && zero.data.has_value() && zero.data.value() == 0 &&
            zero.data.unit() == wire::TELEMETRY_UNIT_RATIO, "Zero reading lost presence");
    const auto voltage = store.read(request("core-1", "synthetic-voltage"));
    require(voltage.status == TelemetryStatus::ok && voltage.data.value() == 1.0 &&
            voltage.data.unit() == wire::TELEMETRY_UNIT_VOLTS &&
            voltage.data.observed_state().instance_id() == expected_id, "Second component failed");
    for (const auto& invalid : std::vector<std::string>{"", std::string(65, 'a'), "a/b", "a b",
            std::string("a\0b", 3), std::string(1, static_cast<char>(255))}) {
        for (int field = 0; field < 3; ++field) {
            auto input = request();
            if (field == 0) { input.mutable_target()->set_device_id(invalid); }
            if (field == 1) { input.mutable_target()->set_component_id(invalid); }
            if (field == 2) { input.set_sensor_id(invalid); }
            const auto result = store.read(input);
            require(result.status == TelemetryStatus::invalid_argument && result.data.ByteSizeLong() == 0,
                    "Invalid ID returned data");
        }
    }
    require(valid_telemetry_id(std::string(64, 'x')) && valid_telemetry_id("AZaz09_.-"),
            "Valid ID boundary rejected");
    require(store.read(wire::TelemetryRequest{}).status == TelemetryStatus::invalid_argument,
            "Missing target accepted");
    for (auto input : {request("unknown"), request("core-0", "unknown"),
                       request("core-1", "synthetic-temperature")}) {
        require(store.read(input).status == TelemetryStatus::not_found, "Unknown pair accepted");
    }
    auto unknown_device = request();
    unknown_device.mutable_target()->set_device_id("unknown");
    require(store.read(unknown_device).status == TelemetryStatus::not_found, "Unknown device accepted");
    const auto unavailable = store.read(request("core-1", "synthetic-unavailable"));
    require(unavailable.status == TelemetryStatus::unavailable && unavailable.data.ByteSizeLong() == 0,
            "Unavailable sample became a reading");
    for (const auto& invalid : std::vector<SyntheticSample>{
            {std::nullopt, wire::TELEMETRY_UNIT_RATIO, 0},
            {0.0, wire::TELEMETRY_UNIT_RATIO, std::nullopt},
            {std::numeric_limits<double>::quiet_NaN(), wire::TELEMETRY_UNIT_RATIO, 0},
            {std::numeric_limits<double>::infinity(), wire::TELEMETRY_UNIT_RATIO, 0},
            {-std::numeric_limits<double>::infinity(), wire::TELEMETRY_UNIT_RATIO, 0},
            {-0.1, wire::TELEMETRY_UNIT_RATIO, 0}, {1.1, wire::TELEMETRY_UNIT_RATIO, 0},
            {0.0, 0, 0}, {0.0, 999, 0}, {0.0, wire::TELEMETRY_UNIT_VOLTS, 0}}) {
        require(!valid_synthetic_sample(invalid, wire::TELEMETRY_UNIT_RATIO),
                "Invalid sample validation succeeded");
    }
    require(valid_synthetic_sample({1.0, wire::TELEMETRY_UNIT_RATIO, 0}, wire::TELEMETRY_UNIT_RATIO),
            "Valid ratio boundary rejected");
    RegisterBank bank;
    const std::array<RegisterAddress, 5> registers{{
        {"simulated-0", 0, 1}, {"simulated-0", 2, 2}, {"simulated-0", 4, 3},
        {"simulated-0", 8, 4}, {"simulated-0", 16, 3}}};
    std::atomic<unsigned> ready{0};
    std::atomic<bool> start{false};
    std::vector<std::thread> readers;
    for (unsigned worker = 0; worker < 16; ++worker) {
        readers.emplace_back([&, worker] {
            ready.fetch_add(1);
            while (!start.load()) { std::this_thread::yield(); }
            for (unsigned iteration = 0; iteration < 200; ++iteration) {
                const auto result = store.read(worker % 2 == 0 ? request() :
                                              request("core-1", "synthetic-voltage"));
                require(result.status == TelemetryStatus::ok &&
                        result.data.observed_state().instance_id() == expected_id &&
                        result.data.observed_state().has_revision() &&
                        result.data.observed_state().revision() == 0 &&
                        result.data.observed_state().state() == wire::MICRO_STATE_ACTIVE,
                        "Concurrent snapshot became inconsistent");
            }
        });
    }
    while (ready.load() != 16) { std::this_thread::yield(); }
    start.store(true);
    require(bank.compare_write(registers[2], 123, 0).status == RegisterStatus::ok,
            "Independent register write failed");
    for (auto& reader : readers) { reader.join(); }
    for (std::size_t index = 0; index < registers.size(); ++index) {
        require(bank.read(registers[index]).value == (index == 2 ? 123u : index == 4 ? 1u : 0u),
                "Telemetry affected the register bank");
    }
    require(store.read(request()).data.observed_state().revision() == 0,
            "Register write affected Micro-State revision");
    std::cout << "Synthetic fixtures, validation, OS identity failure, and concurrent snapshots passed\n";
}
