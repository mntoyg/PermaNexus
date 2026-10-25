#include "actuator_service.h"

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

    const auto instance_id = permanexus::generate_instance_id();
    if (!instance_id) {
        std::cerr << "Failed to obtain process instance ID from the OS random source\n";
        return 1;
    }
    auto service = permanexus::make_actuator_service(*instance_id);
    grpc::ServerBuilder builder;
    int selected_port = 0;
    builder.AddListeningPort("127.0.0.1:" + std::to_string(port),
                             grpc::InsecureServerCredentials(), &selected_port);
    builder.SetMaxReceiveMessageSize(4096);
    builder.SetMaxSendMessageSize(4096);
    builder.SetSyncServerOption(grpc::ServerBuilder::MAX_POLLERS, 4);
    builder.RegisterService(service.get());
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
