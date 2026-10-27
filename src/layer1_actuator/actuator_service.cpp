#include "actuator_service.h"
#include "register_bank.h"
#include "telemetry_store.h"

#include <utility>

namespace {
grpc::Status status_for(permanexus::RegisterStatus status) {
    using permanexus::RegisterStatus;
    switch (status) {
        case RegisterStatus::ok: return grpc::Status::OK;
        case RegisterStatus::invalid_argument:
            return {grpc::StatusCode::INVALID_ARGUMENT, "Invalid register request"};
        case RegisterStatus::not_found:
            return {grpc::StatusCode::NOT_FOUND, "Unknown simulated device or register"};
        case RegisterStatus::permission_denied:
            return {grpc::StatusCode::PERMISSION_DENIED, "Register is read-only"};
        case RegisterStatus::aborted:
            return {grpc::StatusCode::ABORTED, "Expected value did not match"};
    }
    return {grpc::StatusCode::INTERNAL, "Unexpected register status"};
}

permanexus::RegisterAddress convert(const permanexus::actuator::v1::RegisterAddress& address) {
    return {address.device_id(), address.offset_bytes(), static_cast<int>(address.width())};
}

bool request_cancelled(void* context) noexcept {
    return static_cast<grpc::ServerContext*>(context)->IsCancelled();
}

class ActuatorService final : public permanexus::actuator::v1::HardwareActuator::Service {
public:
    explicit ActuatorService(const permanexus::InstanceId& instance_id)
        : micro_state_(instance_id), telemetry_(micro_state_) {}

private:
    grpc::Status ApplyCommand(grpc::ServerContext* context,
                              const permanexus::actuator::v1::ActuatorCommand* request,
                              permanexus::actuator::v1::ActuatorCommandResult* response) override {
        const auto result = micro_state_.apply(*request, request_cancelled, context);
        using permanexus::CommandStatus;
        switch (result.status) {
            case CommandStatus::ok:
                *response = result.result;
                return grpc::Status::OK;
            case CommandStatus::invalid_argument:
                return {grpc::StatusCode::INVALID_ARGUMENT, "Invalid actuator command"};
            case CommandStatus::not_found:
                return {grpc::StatusCode::NOT_FOUND, "Unknown simulated component"};
            case CommandStatus::permission_denied:
                return {grpc::StatusCode::PERMISSION_DENIED, "Transition denied by simulator policy"};
            case CommandStatus::aborted:
                return {grpc::StatusCode::ABORTED, "Expected Micro-State snapshot did not match"};
            case CommandStatus::failed_precondition:
                return {grpc::StatusCode::FAILED_PRECONDITION,
                        "Instance mismatch or Micro-State revision exhausted"};
            case CommandStatus::cancelled:
                return {grpc::StatusCode::CANCELLED, "Request cancelled before command commit"};
        }
        return {grpc::StatusCode::INTERNAL, "Unexpected command status"};
    }

    grpc::Status ReadTelemetry(grpc::ServerContext* context,
                              const permanexus::actuator::v1::TelemetryRequest* request,
                              permanexus::actuator::v1::TelemetryData* response) override {
        if (context->IsCancelled()) {
            return {grpc::StatusCode::CANCELLED, "Request cancelled"};
        }
        auto result = telemetry_.read(*request);
        switch (result.status) {
            case permanexus::TelemetryStatus::ok:
                *response = std::move(result.data);
                return grpc::Status::OK;
            case permanexus::TelemetryStatus::invalid_argument:
                return {grpc::StatusCode::INVALID_ARGUMENT, "Invalid telemetry request"};
            case permanexus::TelemetryStatus::not_found:
                return {grpc::StatusCode::NOT_FOUND, "Unknown simulated component or sensor"};
            case permanexus::TelemetryStatus::unavailable:
                return {grpc::StatusCode::UNAVAILABLE, "Synthetic sample is unavailable or invalid"};
        }
        return {grpc::StatusCode::INTERNAL, "Unexpected telemetry status"};
    }

    grpc::Status ReadRegister(grpc::ServerContext* context,
                             const permanexus::actuator::v1::ReadRegisterRequest* request,
                             permanexus::actuator::v1::ReadRegisterResponse* response) override {
        if (context->IsCancelled()) {
            return {grpc::StatusCode::CANCELLED, "Request cancelled"};
        }
        if (!request->has_address()) {
            return {grpc::StatusCode::INVALID_ARGUMENT, "Register address is required"};
        }
        const auto result = bank_.read(convert(request->address()));
        if (result.status == permanexus::RegisterStatus::ok) {
            response->set_value(result.value);
        }
        return status_for(result.status);
    }

    grpc::Status WriteRegister(grpc::ServerContext* context,
                              const permanexus::actuator::v1::WriteRegisterRequest* request,
                              permanexus::actuator::v1::WriteRegisterResponse* response) override {
        if (context->IsCancelled()) {
            return {grpc::StatusCode::CANCELLED, "Request cancelled"};
        }
        if (!request->has_address()) {
            return {grpc::StatusCode::INVALID_ARGUMENT, "Register address is required"};
        }
        const auto expected = request->has_expected_value()
            ? std::optional<std::uint64_t>{request->expected_value()} : std::nullopt;
        const auto result = bank_.compare_write(convert(request->address()), request->value(), expected);
        if (result.status == permanexus::RegisterStatus::ok) {
            response->set_previous_value(result.value);
        }
        return status_for(result.status);
    }

    permanexus::RegisterBank bank_;
    permanexus::MicroStateStore micro_state_;
    const permanexus::TelemetryStore telemetry_;
};

}  // namespace

namespace permanexus {
std::unique_ptr<actuator::v1::HardwareActuator::Service>
make_actuator_service(const InstanceId& instance_id) {
    return std::make_unique<ActuatorService>(instance_id);
}
}  // namespace permanexus
