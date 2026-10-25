#pragma once

#include <array>
#include <cstdint>
#include <optional>

namespace permanexus {
using InstanceId = std::array<std::uint8_t, 16>;
using RandomFill = bool (*)(InstanceId&);

// No fallback to clocks, counters, PRNGs, or a previous process identity.
bool fill_os_random(InstanceId& output);
std::optional<InstanceId> generate_instance_id(RandomFill fill = fill_os_random);
}  // namespace permanexus
