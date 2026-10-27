#include "micro_state_store.h"

#include <cstring>
#include <limits>
#include <utility>

namespace permanexus {
namespace {
namespace wire = actuator::v1;

struct ComponentSpec {
    std::string_view device_id;
    std::string_view component_id;
};

constexpr std::array<ComponentSpec, 2> components{{
    {"simulated-0", "core-0"},
    {"simulated-0", "core-1"},
}};

bool valid_state(wire::MicroState state) {
    return wire::MicroState_IsValid(static_cast<int>(state)) &&
           state != wire::MICRO_STATE_UNSPECIFIED;
}

bool transition_allowed(wire::MicroState current, wire::MicroState desired) {
    if (current == desired) { return true; }
    return !(current == wire::MICRO_STATE_LOW_POWER && desired == wire::MICRO_STATE_IDLE);
}

void write_snapshot(const MicroStateSnapshot& source, wire::MicroStateSnapshot* target) {
    target->set_state(source.state);
    target->set_revision(source.revision);
    target->set_instance_id(source.instance_id.data(), source.instance_id.size());
}

bool same_instance(const std::string& expected, const InstanceId& actual) {
    return expected.size() == actual.size() &&
           std::memcmp(expected.data(), actual.data(), actual.size()) == 0;
}

CommandResult failure(CommandStatus status) {
    return {status, {}};
}
}  // namespace

bool valid_component_id(std::string_view id) {
    if (id.empty() || id.size() > 64) { return false; }
    for (unsigned char character : id) {
        if (!((character >= 'A' && character <= 'Z') ||
              (character >= 'a' && character <= 'z') ||
              (character >= '0' && character <= '9') ||
              character == '_' || character == '.' || character == '-')) {
            return false;
        }
    }
    return true;
}

MicroStateStore::MicroStateStore(const InstanceId& instance_id)
    : components_{{{{}, {wire::MICRO_STATE_ACTIVE, 0, instance_id}},
                   {{}, {wire::MICRO_STATE_ACTIVE, 0, instance_id}}}} {}

std::optional<std::size_t> MicroStateStore::component_index(
    std::string_view device_id, std::string_view component_id) const {
    for (std::size_t index = 0; index < components.size(); ++index) {
        if (components[index].device_id == device_id &&
            components[index].component_id == component_id) {
            return index;
        }
    }
    return std::nullopt;
}

std::optional<MicroStateSnapshot> MicroStateStore::snapshot(
    std::string_view device_id, std::string_view component_id) const {
    const auto index = component_index(device_id, component_id);
    if (!index) { return std::nullopt; }
    const std::lock_guard<std::mutex> lock(components_[*index].mutex);
    return components_[*index].snapshot;
}

CommandResult MicroStateStore::apply(const wire::ActuatorCommand& command,
                                     CancellationCheck is_cancelled,
                                     void* cancellation_context) {
    if (!command.has_target() || !command.has_expected_state() ||
        command.command_id().size() != 16 ||
        !valid_component_id(command.target().device_id()) ||
        !valid_component_id(command.target().component_id()) ||
        !valid_state(command.desired_state()) ||
        !command.expected_state().has_revision() ||
        command.expected_state().instance_id().size() != 16 ||
        !valid_state(command.expected_state().state())) {
        return failure(CommandStatus::invalid_argument);
    }

    const auto index = component_index(command.target().device_id(),
                                       command.target().component_id());
    if (!index) { return failure(CommandStatus::not_found); }

    auto& component = components_[*index];
    const std::lock_guard<std::mutex> lock(component.mutex);
    const auto& expected = command.expected_state();

    if (!same_instance(expected.instance_id(), component.snapshot.instance_id)) {
        return failure(CommandStatus::failed_precondition);
    }
    if (expected.state() != component.snapshot.state ||
        expected.revision() != component.snapshot.revision) {
        return failure(CommandStatus::aborted);
    }
    if (!transition_allowed(component.snapshot.state, command.desired_state())) {
        return failure(CommandStatus::permission_denied);
    }
    if (component.snapshot.revision == std::numeric_limits<std::uint64_t>::max()) {
        return failure(CommandStatus::failed_precondition);
    }
    const auto previous = component.snapshot;
    const auto next_revision = previous.revision + 1;
    wire::ActuatorCommandResult result;
    result.set_command_id(command.command_id());
    result.mutable_target()->set_device_id(command.target().device_id());
    result.mutable_target()->set_component_id(command.target().component_id());
    write_snapshot(previous, result.mutable_previous_state());
    write_snapshot(previous, result.mutable_current_state());

    if (is_cancelled && is_cancelled(cancellation_context)) {
        return failure(CommandStatus::cancelled);
    }

    component.snapshot.state = command.desired_state();
    component.snapshot.revision = next_revision;
    result.mutable_current_state()->set_state(component.snapshot.state);
    result.mutable_current_state()->set_revision(component.snapshot.revision);
    return {CommandStatus::ok, std::move(result)};
}
}  // namespace permanexus
