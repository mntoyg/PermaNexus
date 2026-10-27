#pragma once

#include "actuator.pb.h"
#include "process_instance.h"

#include <array>
#include <cstddef>
#include <cstdint>
#include <mutex>
#include <optional>
#include <string_view>

namespace permanexus {
enum class CommandStatus {
    ok,
    invalid_argument,
    not_found,
    permission_denied,
    aborted,
    failed_precondition,
    cancelled
};

struct MicroStateSnapshot {
    actuator::v1::MicroState state;
    std::uint64_t revision;
    InstanceId instance_id;
};

struct CommandResult {
    CommandStatus status;
    actuator::v1::ActuatorCommandResult result;
};

using CancellationCheck = bool (*)(void*) noexcept;

bool valid_component_id(std::string_view id);

// Stores simulator state independently from registers and wear-planner slots.
class MicroStateStore final {
public:
    explicit MicroStateStore(const InstanceId& instance_id);

    std::optional<MicroStateSnapshot> snapshot(
        std::string_view device_id, std::string_view component_id) const;
    CommandResult apply(const actuator::v1::ActuatorCommand& command,
                        CancellationCheck is_cancelled = nullptr,
                        void* cancellation_context = nullptr);

private:
    struct ComponentState {
        mutable std::mutex mutex;
        MicroStateSnapshot snapshot;
    };

    std::optional<std::size_t> component_index(
        std::string_view device_id, std::string_view component_id) const;

    std::array<ComponentState, 2> components_;

    friend class MicroStateStoreTestAccess;
};
}  // namespace permanexus
