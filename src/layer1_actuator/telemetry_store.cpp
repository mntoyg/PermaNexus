#include "telemetry_store.h"

#include <cmath>
#include <utility>

namespace permanexus {
namespace {
namespace wire = actuator::v1;
struct ComponentSpec { std::string_view device; std::string_view component; };
struct SensorSpec {
    std::size_t component_index;
    std::string_view sensor;
    int configured_unit;
    SyntheticSample sample;
};
constexpr std::array<ComponentSpec, 2> components{{
    {"simulated-0", "core-0"}, {"simulated-0", "core-1"},
}};
// All values and epoch-zero timestamps are deterministic simulation fixtures.
// No physical sensors, clock reads, register maps, or caller-provided samples.
constexpr std::array<SensorSpec, 4> sensors{{
    {0, "synthetic-temperature", wire::TELEMETRY_UNIT_CELSIUS,
        {42.5, wire::TELEMETRY_UNIT_CELSIUS, 0}},
    {0, "synthetic-utilization", wire::TELEMETRY_UNIT_RATIO,
        {0.0, wire::TELEMETRY_UNIT_RATIO, 0}},
    {1, "synthetic-voltage", wire::TELEMETRY_UNIT_VOLTS,
        {1.0, wire::TELEMETRY_UNIT_VOLTS, 0}},
    {1, "synthetic-unavailable", wire::TELEMETRY_UNIT_VOLTS,
        {std::nullopt, wire::TELEMETRY_UNIT_VOLTS, 0}},
}};
}  // namespace

bool valid_telemetry_id(std::string_view id) {
    return valid_component_id(id);
}

bool valid_synthetic_sample(const SyntheticSample& sample, int configured_unit) {
    return sample.value && std::isfinite(*sample.value) && sample.sampled_at_unix_ns &&
        wire::TelemetryUnit_IsValid(sample.unit) &&
        sample.unit != wire::TELEMETRY_UNIT_UNSPECIFIED && sample.unit == configured_unit &&
        (sample.unit != wire::TELEMETRY_UNIT_RATIO ||
         (*sample.value >= 0.0 && *sample.value <= 1.0));
}

TelemetryStore::TelemetryStore(const MicroStateStore& state_store)
    : state_store_(state_store) {}

TelemetryResult TelemetryStore::read(const wire::TelemetryRequest& request) const {
    if (!request.has_target() || !valid_telemetry_id(request.target().device_id()) ||
        !valid_telemetry_id(request.target().component_id()) ||
        !valid_telemetry_id(request.sensor_id())) {
        return {TelemetryStatus::invalid_argument, {}};
    }
    for (std::size_t index = 0; index < components.size(); ++index) {
        const auto& component = components[index];
        if (request.target().device_id() != component.device ||
            request.target().component_id() != component.component) { continue; }
        for (const auto& sensor : sensors) {
            if (sensor.component_index != index || request.sensor_id() != sensor.sensor) { continue; }
            if (!valid_synthetic_sample(sensor.sample, sensor.configured_unit)) {
                return {TelemetryStatus::unavailable, {}};
            }
            const auto snapshot = state_store_.snapshot(component.device, component.component);
            if (!snapshot || !wire::MicroState_IsValid(snapshot->state) ||
                snapshot->state == wire::MICRO_STATE_UNSPECIFIED) {
                return {TelemetryStatus::unavailable, {}};
            }
            wire::TelemetryData data;
            data.mutable_target()->set_device_id(request.target().device_id());
            data.mutable_target()->set_component_id(request.target().component_id());
            data.set_sensor_id(request.sensor_id());
            data.set_value(*sensor.sample.value);
            data.set_unit(static_cast<wire::TelemetryUnit>(sensor.sample.unit));
            data.set_sampled_at_unix_ns(*sensor.sample.sampled_at_unix_ns);
            auto* state = data.mutable_observed_state();
            state->set_state(snapshot->state);
            state->set_revision(snapshot->revision);
            state->set_instance_id(snapshot->instance_id.data(), snapshot->instance_id.size());
            return {TelemetryStatus::ok, std::move(data)};
        }
        return {TelemetryStatus::not_found, {}};
    }
    return {TelemetryStatus::not_found, {}};
}
}  // namespace permanexus
