#include "actuator.grpc.pb.h"
#include "register_bank.h"

#include <grpcpp/grpcpp.h>

#include <atomic>
#include <charconv>
#include <chrono>
#include <csignal>
#include <iostream>
#include <string>
#include <thread>

namespace {

std::atomic<bool> stop_requested{false};
static_assert(std::atomic<bool>::is_always_lock_free, "Signal shutdown requires a lock-free flag");

void request_stop(int) { stop_requested.store(true, std::memory_order_relaxed); }

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

class ActuatorService final : public permanexus::actuator::v1::HardwareActuator::Service {
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
};

}  // namespace

int main(int argc, char** argv) {
    unsigned port = 50051;
    if (argc != 1) {
        if (argc != 3 || std::string_view(argv[1]) != "--port") {
            std::cerr << "Usage: actuator_server [--port 0..65535]\n";
            return 2;
        }
        const std::string_view input(argv[2]);
        const auto parsed = std::from_chars(input.data(), input.data() + input.size(), port);
        if (parsed.ec != std::errc{} || parsed.ptr != input.data() + input.size() || port > 65535) {
            std::cerr << "Invalid port\n";
            return 2;
        }
    }

    ActuatorService service;
    grpc::ServerBuilder builder;
    int selected_port = 0;
    builder.AddListeningPort("127.0.0.1:" + std::to_string(port),
                             grpc::InsecureServerCredentials(), &selected_port);
    builder.SetMaxReceiveMessageSize(4096);
    builder.SetMaxSendMessageSize(4096);
    builder.SetSyncServerOption(grpc::ServerBuilder::MAX_POLLERS, 4);
    builder.RegisterService(&service);
    auto server = builder.BuildAndStart();
    if (!server || selected_port == 0) {
        std::cerr << "Failed to bind loopback actuator server\n";
        return 1;
    }

    std::signal(SIGINT, request_stop);
    std::signal(SIGTERM, request_stop);
    std::thread shutdown([&server] {
        while (!stop_requested.load(std::memory_order_relaxed)) {
            std::this_thread::sleep_for(std::chrono::milliseconds(100));
        }
        server->Shutdown(std::chrono::system_clock::now() + std::chrono::seconds(2));
    });
    std::cout << "LISTENING 127.0.0.1:" << selected_port << std::endl;
    server->Wait();
    stop_requested.store(true, std::memory_order_relaxed);
    shutdown.join();
    return 0;
}
