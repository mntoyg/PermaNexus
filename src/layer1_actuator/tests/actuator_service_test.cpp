#include "actuator_service.h"

#include <cstdlib>
#include <iostream>

namespace {
namespace wire = permanexus::actuator::v1;
wire::ActuatorCommand command(const char* component, wire::MicroState desired,
                              const wire::MicroStateSnapshot& expected) {
    wire::ActuatorCommand result;
    result.set_command_id(std::string(16, 'x'));
    result.mutable_target()->set_device_id("simulated-0");
    result.mutable_target()->set_component_id(component);
    result.set_desired_state(desired);
    *result.mutable_expected_state() = expected;
    return result;
}
void require(bool condition, const char* message) {
    if (!condition) { std::cerr << message << '\n'; std::exit(1); }
}
}  // namespace

int main() {
    permanexus::InstanceId id{};
    for (std::size_t index = 0; index < id.size(); ++index) {
        id[index] = static_cast<std::uint8_t>(index + 1);
    }
    auto service = permanexus::make_actuator_service(id);
    grpc::ServerContext context;
    wire::ReadRegisterRequest register_request;
    register_request.mutable_address()->set_device_id("simulated-0");
    register_request.mutable_address()->set_offset_bytes(4);
    register_request.mutable_address()->set_width(wire::REGISTER_WIDTH_32_BITS);
    wire::ReadRegisterResponse register_response;
    require(service->ReadRegister(&context, &register_request, &register_response).ok() &&
            register_response.value() == 0, "Initial register value changed");
    wire::WriteRegisterRequest register_write;
    *register_write.mutable_address() = register_request.address();
    register_write.set_value(77);
    register_write.set_expected_value(0);
    wire::WriteRegisterResponse register_write_response;
    require(service->WriteRegister(&context, &register_write, &register_write_response).ok(),
            "Could not establish register independence sentinel");
    wire::TelemetryRequest request;
    wire::TelemetryData response;
    require(service->ReadTelemetry(&context, &request, &response).error_code() ==
            grpc::StatusCode::INVALID_ARGUMENT, "Missing target status changed");
    require(response.ByteSizeLong() == 0, "Rejected request returned telemetry");
    request.mutable_target()->set_device_id("simulated-0");
    request.mutable_target()->set_component_id("core-0");
    request.set_sensor_id("synthetic-temperature");
    require(service->ReadTelemetry(&context, &request, &response).ok() &&
            response.value() == 42.5 && response.observed_state().instance_id().size() == 16,
            "Service did not return a valid synthetic reading");
    request.set_sensor_id("unknown");
    response.Clear();
    require(service->ReadTelemetry(&context, &request, &response).error_code() ==
            grpc::StatusCode::NOT_FOUND && response.ByteSizeLong() == 0,
            "Unknown sensor status changed");
    request.mutable_target()->set_component_id("core-1");
    request.set_sensor_id("synthetic-unavailable");
    require(service->ReadTelemetry(&context, &request, &response).error_code() ==
            grpc::StatusCode::UNAVAILABLE && response.ByteSizeLong() == 0,
            "Unavailable fixture status changed");
    wire::TelemetryRequest state_request;
    state_request.mutable_target()->set_device_id("simulated-0");
    state_request.mutable_target()->set_component_id("core-0");
    state_request.set_sensor_id("synthetic-temperature");
    require(service->ReadTelemetry(&context, &state_request, &response).ok(),
            "Could not read initial command snapshot");
    const auto initial = response.observed_state();
    wire::ActuatorCommandResult result;
    auto command_request = command("core-0", wire::MICRO_STATE_IDLE, initial);
    require(service->ApplyCommand(&context, &command_request, &result).ok() &&
            result.previous_state().state() == wire::MICRO_STATE_ACTIVE &&
            result.current_state().state() == wire::MICRO_STATE_IDLE &&
            result.current_state().revision() == 1,
            "Valid RAM-only command did not commit");
    result.Clear();
    require(service->ApplyCommand(&context, &command_request, &result).error_code() ==
            grpc::StatusCode::ABORTED && result.ByteSizeLong() == 0,
            "Stale command status changed");

    state_request.mutable_target()->set_component_id("missing-component");
    command_request = command("missing-component", wire::MICRO_STATE_IDLE,
                              initial);
    require(service->ApplyCommand(&context, &command_request, &result).error_code() ==
            grpc::StatusCode::NOT_FOUND, "Unknown component status changed");

    state_request.mutable_target()->set_component_id("core-0");
    response.Clear();
    require(service->ReadTelemetry(&context, &state_request, &response).ok(),
            "Could not read updated command snapshot");
    auto current = response.observed_state();
    auto wake_command = command("core-0", wire::MICRO_STATE_LOW_POWER, current);
    require(service->ApplyCommand(&context, &wake_command, &result).ok(),
            "Allowed transition to LOW_POWER failed");
    current = result.current_state();
    auto denied_command = command("core-0", wire::MICRO_STATE_IDLE, current);
    result.Clear();
    require(service->ApplyCommand(&context, &denied_command, &result).error_code() ==
            grpc::StatusCode::PERMISSION_DENIED && result.ByteSizeLong() == 0,
            "Denied transition status changed");

    auto invalid_command = command("core-1", wire::MICRO_STATE_IDLE, initial);
    invalid_command.set_command_id("short");
    require(service->ApplyCommand(&context, &invalid_command, &result).error_code() ==
            grpc::StatusCode::INVALID_ARGUMENT, "Invalid command status changed");
    auto wrong_instance = initial;
    wrong_instance.set_instance_id(std::string(16, 'z'));
    auto instance_command = command("core-1", wire::MICRO_STATE_IDLE, wrong_instance);
    require(service->ApplyCommand(&context, &instance_command, &result).error_code() ==
            grpc::StatusCode::FAILED_PRECONDITION, "Instance mismatch status changed");

    response.Clear();
    require(service->ReadTelemetry(&context, &state_request, &response).ok() &&
            response.observed_state().revision() == 2 &&
            response.observed_state().state() == wire::MICRO_STATE_LOW_POWER,
            "Rejected command changed committed state");
    require(service->ReadRegister(&context, &register_request, &register_response).ok() &&
            register_response.value() == 77,
            "Micro-State command changed the independent register bank");
    std::cout << "C++ telemetry and ApplyCommand service status mapping passed\n";
}
