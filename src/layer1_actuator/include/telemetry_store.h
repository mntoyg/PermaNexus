#pragma once

#include "actuator.pb.h"
#include "micro_state_store.h"

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
    explicit TelemetryStore(const MicroStateStore& state_store);
    TelemetryResult read(const actuator::v1::TelemetryRequest& request) const;

private:
    const MicroStateStore& state_store_;
};
}  // namespace permanexus
