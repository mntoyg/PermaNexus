#pragma once

#include "actuator.pb.h"
#include "process_instance.h"

#include <array>
#include <mutex>
#include <optional>
#include <string_view>

namespace permanexus {
enum class TelemetryStatus { ok, invalid_argument, not_found, unavailable };

struct SyntheticSample {
    std::optional<double> value;
    int unit;
    std::optional<std::uint64_t> sampled_at_unix_ns;
};

bool valid_telemetry_id(std::string_view id);
bool valid_synthetic_sample(const SyntheticSample& sample, int configured_unit);

struct TelemetryResult {
    TelemetryStatus status;
    actuator::v1::TelemetryData data;
};

// Separate from RegisterBank and the Rust wear planner. No transition API.
class TelemetryStore final {
public:
    explicit TelemetryStore(const InstanceId& instance_id);
    TelemetryResult read(const actuator::v1::TelemetryRequest& request) const;

private:
    struct Snapshot {
        int state;
        std::uint64_t revision;
        InstanceId instance_id;
    };
    struct ComponentState {
        mutable std::mutex mutex;
        const Snapshot snapshot;
    };
    const std::array<ComponentState, 2> states_;
};
}  // namespace permanexus
