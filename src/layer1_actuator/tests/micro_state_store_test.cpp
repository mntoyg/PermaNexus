#include "micro_state_store.h"

#include <atomic>
#include <cstdlib>
#include <iostream>
#include <limits>
#include <string>
#include <thread>
#include <vector>

namespace permanexus {
class MicroStateStoreTestAccess final {
public:
    static void set_snapshot(MicroStateStore& store, std::size_t index,
                             actuator::v1::MicroState state, std::uint64_t revision) {
        auto& component = store.components_.at(index);
        const std::lock_guard<std::mutex> lock(component.mutex);
        component.snapshot.state = state;
        component.snapshot.revision = revision;
    }
};
}  // namespace permanexus

namespace {
namespace wire = permanexus::actuator::v1;
using permanexus::CommandResult;
using permanexus::CommandStatus;
using permanexus::MicroStateSnapshot;
using permanexus::MicroStateStore;

void require(bool condition, const char* message) {
    if (!condition) { std::cerr << message << '\n'; std::exit(1); }
}

const permanexus::InstanceId instance_id{{0, 1, 2, 3, 4, 5, 6, 7,
                                          8, 9, 10, 11, 12, 13, 14, 255}};

wire::ActuatorCommand command_for(const MicroStateSnapshot& snapshot,
                                  std::string component,
                                  wire::MicroState desired) {
    wire::ActuatorCommand command;
    command.set_command_id(std::string(16, 'c'));
    command.mutable_target()->set_device_id("simulated-0");
    command.mutable_target()->set_component_id(std::move(component));
    command.set_desired_state(desired);
    auto* expected = command.mutable_expected_state();
    expected->set_state(snapshot.state);
    expected->set_revision(snapshot.revision);
    expected->set_instance_id(snapshot.instance_id.data(), snapshot.instance_id.size());
    return command;
}

MicroStateSnapshot read_snapshot(const MicroStateStore& store, const char* component) {
    const auto snapshot = store.snapshot("simulated-0", component);
    require(snapshot.has_value(), "Allowlisted component snapshot missing");
    return *snapshot;
}

bool same_snapshot(const MicroStateSnapshot& left, const MicroStateSnapshot& right) {
    return left.state == right.state && left.revision == right.revision &&
           left.instance_id == right.instance_id;
}

void require_unchanged(MicroStateStore& store, const MicroStateSnapshot& before,
                       CommandStatus expected, const wire::ActuatorCommand& command) {
    const auto result = store.apply(command);
    require(result.status == expected, "Command returned the wrong rejection status");
    const auto after = store.snapshot("simulated-0", "core-0");
    require(after && same_snapshot(before, *after), "Rejected command mutated Micro-State");
    require(result.result.ByteSizeLong() == 0, "Rejected command returned a success result");
}

bool cancelled(void*) noexcept { return true; }

void test_transition_table() {
    constexpr wire::MicroState states[] = {
        wire::MICRO_STATE_ACTIVE, wire::MICRO_STATE_IDLE, wire::MICRO_STATE_LOW_POWER};
    for (const char* component : {"core-0", "core-1"}) {
        for (const auto current : states) {
            for (const auto desired : states) {
                MicroStateStore store(instance_id);
                const std::size_t index = std::string_view(component) == "core-0" ? 0 : 1;
                permanexus::MicroStateStoreTestAccess::set_snapshot(store, index, current, 7);
                const auto before = read_snapshot(store, component);
                const auto command = command_for(before, component, desired);
                const auto result = store.apply(command);
                const bool denied = current == wire::MICRO_STATE_LOW_POWER &&
                                    desired == wire::MICRO_STATE_IDLE;
                require(result.status == (denied ? CommandStatus::permission_denied :
                                                   CommandStatus::ok),
                        "Transition table result changed");
                if (denied) {
                    require(same_snapshot(before, read_snapshot(store, component)),
                            "Denied transition mutated state");
                    continue;
                }
                require(result.result.command_id() == command.command_id() &&
                        result.result.target().device_id() == "simulated-0" &&
                        result.result.target().component_id() == component,
                        "Successful result identity mismatch");
                const auto& previous = result.result.previous_state();
                const auto& current_snapshot = result.result.current_state();
                require(previous.state() == current && previous.revision() == 7 &&
                        previous.instance_id().size() == 16,
                        "Previous snapshot was not captured coherently");
                require(current_snapshot.state() == desired &&
                        current_snapshot.revision() == 8 &&
                        current_snapshot.instance_id() == previous.instance_id(),
                        "Current snapshot was not captured coherently");
                require(read_snapshot(store, component).state == desired &&
                        read_snapshot(store, component).revision == 8,
                        "Successful command was not committed");
            }
        }
    }
}

void test_validation_and_status_precedence() {
    MicroStateStore store(instance_id);
    const auto before = read_snapshot(store, "core-0");
    const auto valid = command_for(before, "core-0", wire::MICRO_STATE_IDLE);

    require_unchanged(store, before, CommandStatus::invalid_argument, wire::ActuatorCommand{});
    {
        auto command = valid;
        command.clear_target();
        require_unchanged(store, before, CommandStatus::invalid_argument, command);
    }
    for (const auto size : {std::size_t{0}, std::size_t{15}, std::size_t{17}}) {
        auto command = valid;
        command.set_command_id(std::string(size, 'x'));
        require_unchanged(store, before, CommandStatus::invalid_argument, command);
    }
    for (const auto& invalid_id : {std::string{}, std::string(65, 'x'), std::string("bad/id"),
                                   std::string("bad\0id", 6)}) {
        auto command = valid;
        command.mutable_target()->set_device_id(invalid_id);
        require_unchanged(store, before, CommandStatus::invalid_argument, command);
        command = valid;
        command.mutable_target()->set_component_id(invalid_id);
        require_unchanged(store, before, CommandStatus::invalid_argument, command);
    }
    for (const auto desired : {wire::MICRO_STATE_UNSPECIFIED,
                               static_cast<wire::MicroState>(99)}) {
        auto command = valid;
        command.set_desired_state(desired);
        require_unchanged(store, before, CommandStatus::invalid_argument, command);
    }
    {
        auto command = valid;
        command.clear_expected_state();
        require_unchanged(store, before, CommandStatus::invalid_argument, command);
    }
    for (const auto state : {wire::MICRO_STATE_UNSPECIFIED,
                             static_cast<wire::MicroState>(99)}) {
        auto command = valid;
        command.mutable_expected_state()->set_state(state);
        require_unchanged(store, before, CommandStatus::invalid_argument, command);
    }
    {
        auto command = valid;
        command.mutable_expected_state()->clear_revision();
        require_unchanged(store, before, CommandStatus::invalid_argument, command);
    }
    for (const auto size : {std::size_t{0}, std::size_t{15}, std::size_t{17}}) {
        auto command = valid;
        command.mutable_expected_state()->set_instance_id(std::string(size, 'i'));
        require_unchanged(store, before, CommandStatus::invalid_argument, command);
    }

    for (const char* unknown_device : {"unknown-device"}) {
        auto command = valid;
        command.mutable_target()->set_device_id(unknown_device);
        require_unchanged(store, before, CommandStatus::not_found, command);
    }
    {
        auto command = valid;
        command.mutable_target()->set_component_id("core-9");
        require_unchanged(store, before, CommandStatus::not_found, command);
    }
    {
        auto command = valid;
        command.mutable_target()->set_component_id(std::string(64, 'x'));
        require_unchanged(store, before, CommandStatus::not_found, command);
    }

    {
        auto command = valid;
        command.mutable_expected_state()->set_instance_id(std::string(16, 'z'));
        command.mutable_expected_state()->set_state(wire::MICRO_STATE_LOW_POWER);
        command.set_desired_state(wire::MICRO_STATE_IDLE);
        require_unchanged(store, before, CommandStatus::failed_precondition, command);
    }
    {
        auto command = valid;
        command.mutable_expected_state()->set_state(wire::MICRO_STATE_LOW_POWER);
        command.set_desired_state(wire::MICRO_STATE_IDLE);
        require_unchanged(store, before, CommandStatus::aborted, command);
    }
    {
        auto command = valid;
        command.mutable_expected_state()->set_revision(before.revision + 1);
        require_unchanged(store, before, CommandStatus::aborted, command);
    }

    MicroStateStore low_power_store(instance_id);
    permanexus::MicroStateStoreTestAccess::set_snapshot(
        low_power_store, 0, wire::MICRO_STATE_LOW_POWER, 10);
    const auto low_power = read_snapshot(low_power_store, "core-0");
    const auto denied = command_for(low_power, "core-0", wire::MICRO_STATE_IDLE);
    {
        auto command = denied;
        command.mutable_expected_state()->set_instance_id(std::string(16, 'x'));
        require_unchanged(low_power_store, low_power, CommandStatus::failed_precondition, command);
    }
    {
        auto command = denied;
        command.mutable_expected_state()->set_revision(11);
        require_unchanged(low_power_store, low_power, CommandStatus::aborted, command);
    }
    require_unchanged(low_power_store, low_power, CommandStatus::permission_denied, denied);
    {
        auto command = command_for(low_power, "core-0", wire::MICRO_STATE_LOW_POWER);
        permanexus::MicroStateStoreTestAccess::set_snapshot(
            low_power_store, 0, wire::MICRO_STATE_LOW_POWER,
            std::numeric_limits<std::uint64_t>::max());
        const auto exhausted = read_snapshot(low_power_store, "core-0");
        command = command_for(exhausted, "core-0", wire::MICRO_STATE_LOW_POWER);
        require_unchanged(low_power_store, exhausted, CommandStatus::failed_precondition, command);
    }
    {
        auto command = command_for(low_power, "core-0", wire::MICRO_STATE_IDLE);
        permanexus::MicroStateStoreTestAccess::set_snapshot(
            low_power_store, 0, wire::MICRO_STATE_LOW_POWER,
            std::numeric_limits<std::uint64_t>::max());
        const auto exhausted = read_snapshot(low_power_store, "core-0");
        command = command_for(exhausted, "core-0", wire::MICRO_STATE_IDLE);
        require_unchanged(low_power_store, exhausted, CommandStatus::permission_denied, command);
    }
    {
        auto command = valid;
        require(store.apply(command, cancelled).status == CommandStatus::cancelled,
                "Cancellation before commit was not observed");
        require(same_snapshot(before, read_snapshot(store, "core-0")),
                "Cancelled command mutated state");
    }
}

void test_component_isolation_and_contention() {
    MicroStateStore store(instance_id);
    const auto before = read_snapshot(store, "core-0");
    constexpr unsigned contenders = 32;
    std::atomic<unsigned> ready{0};
    std::atomic<bool> start{false};
    std::vector<CommandStatus> statuses(contenders, CommandStatus::cancelled);
    std::vector<std::thread> workers;
    workers.reserve(contenders);
    for (unsigned index = 0; index < contenders; ++index) {
        workers.emplace_back([&, index] {
            ready.fetch_add(1, std::memory_order_release);
            while (!start.load(std::memory_order_acquire)) { std::this_thread::yield(); }
            statuses[index] = store.apply(
                command_for(before, "core-0", wire::MICRO_STATE_IDLE)).status;
        });
    }
    while (ready.load(std::memory_order_acquire) != contenders) { std::this_thread::yield(); }
    start.store(true, std::memory_order_release);
    for (auto& worker : workers) { worker.join(); }

    unsigned winners = 0;
    unsigned stale = 0;
    for (const auto status : statuses) {
        winners += status == CommandStatus::ok ? 1u : 0u;
        stale += status == CommandStatus::aborted ? 1u : 0u;
    }
    require(winners == 1 && stale == contenders - 1,
            "Identical expected snapshots did not produce exactly one winner");
    const auto committed = read_snapshot(store, "core-0");
    require(committed.state == wire::MICRO_STATE_IDLE && committed.revision == 1,
            "Contended final snapshot is inconsistent");

    const auto other_before = read_snapshot(store, "core-1");
    const auto other_result = store.apply(
        command_for(other_before, "core-1", wire::MICRO_STATE_LOW_POWER));
    require(other_result.status == CommandStatus::ok &&
            read_snapshot(store, "core-1").revision == 1 &&
            read_snapshot(store, "core-0").revision == 1,
            "Per-component states were not independent");
}
}  // namespace

int main() {
    test_transition_table();
    test_validation_and_status_precedence();
    test_component_isolation_and_contention();
    std::cout << "Micro-State transition policy, validation, atomic snapshots, and contention passed\n";
}
