#include "actuator_service.h"

#include <cstdlib>
#include <iostream>

namespace {
namespace wire = permanexus::actuator::v1;
void require(bool condition, const char* message) {
    if (!condition) { std::cerr << message << '\n'; std::exit(1); }
}
}  // namespace

int main() {
    const permanexus::InstanceId id{{1, 2, 3}};
    auto service = permanexus::make_actuator_service(id);
    grpc::ServerContext context;
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
    wire::ActuatorCommand command;
    wire::ActuatorCommandResult result;
    require(service->ApplyCommand(&context, &command, &result).error_code() ==
            grpc::StatusCode::UNIMPLEMENTED, "Command endpoint was enabled");
    std::cout << "C++ telemetry service status mapping and disabled commands passed\n";
}
