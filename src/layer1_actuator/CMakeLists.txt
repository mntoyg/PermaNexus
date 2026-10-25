cmake_minimum_required(VERSION 3.24)
project(permanexus_actuator VERSION 0.1.0 LANGUAGES CXX)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE Release CACHE STRING "Build type" FORCE)
endif()
option(PERMANEXUS_WITH_GRPC "Build the loopback gRPC simulator" ON)
option(PERMANEXUS_BUILD_BENCHMARKS "Build bounded RAM performance benchmarks" OFF)
include(CTest)
find_package(Threads REQUIRED)
add_library(permanexus_hpp INTERFACE)
target_include_directories(permanexus_hpp INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_compile_features(permanexus_hpp INTERFACE cxx_std_17)
add_library(register_bank STATIC register_bank.cpp)
target_include_directories(register_bank PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_compile_features(register_bank PUBLIC cxx_std_17)
target_link_libraries(register_bank PUBLIC permanexus_hpp Threads::Threads)

include(CheckCXXSourceCompiles)
set(ATOMIC_PROBE "#include <atomic>\n#include <cstdint>\nint main() { std::atomic<std::uint64_t> cell{0}; std::uint64_t expected=0; cell.compare_exchange_strong(expected,1); return cell.is_lock_free() ? 0 : 1; }")
check_cxx_source_compiles("${ATOMIC_PROBE}" PERMANEXUS_NATIVE_ATOMIC64)
if(NOT PERMANEXUS_NATIVE_ATOMIC64)
    set(SAVED_ATOMIC_REQUIRED_LIBRARIES "${CMAKE_REQUIRED_LIBRARIES}")
    list(APPEND CMAKE_REQUIRED_LIBRARIES atomic)
    check_cxx_source_compiles("${ATOMIC_PROBE}" PERMANEXUS_LIBATOMIC64)
    set(CMAKE_REQUIRED_LIBRARIES "${SAVED_ATOMIC_REQUIRED_LIBRARIES}")
    unset(SAVED_ATOMIC_REQUIRED_LIBRARIES)
    if(NOT PERMANEXUS_LIBATOMIC64)
        message(FATAL_ERROR "The toolchain must support std::atomic<uint64_t>, natively or via libatomic")
    endif()
    target_link_libraries(permanexus_hpp INTERFACE atomic)
endif()
if(MSVC)
    target_compile_options(register_bank PRIVATE /W4 /WX)
else()
    target_compile_options(register_bank PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()
if(BUILD_TESTING)
    add_executable(register_bank_test tests/register_bank_test.cpp)
    target_link_libraries(register_bank_test PRIVATE register_bank)
    add_test(NAME register_bank_test COMMAND register_bank_test)
    add_executable(hpp_test tests/hpp_test.cpp)
    target_link_libraries(hpp_test PRIVATE register_bank)
    add_test(NAME hpp_test COMMAND hpp_test)
    set_tests_properties(register_bank_test hpp_test PROPERTIES TIMEOUT 30)
endif()
if(PERMANEXUS_BUILD_BENCHMARKS)
    add_executable(hpp_benchmark benchmarks/hpp_benchmark.cpp)
    target_link_libraries(hpp_benchmark PRIVATE register_bank)
    file(GENERATE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/hpp_benchmark_path-$<CONFIG>.txt"
        CONTENT "$<TARGET_FILE:hpp_benchmark>")
    if(BUILD_TESTING)
        add_test(NAME hpp_benchmark_smoke COMMAND hpp_benchmark 10000 2)
        set_tests_properties(hpp_benchmark_smoke PROPERTIES TIMEOUT 30)
        add_test(NAME hpp_benchmark_invalid COMMAND hpp_benchmark 999 0)
        set_tests_properties(hpp_benchmark_invalid PROPERTIES WILL_FAIL TRUE TIMEOUT 5)
    endif()
endif()
if(NOT PERMANEXUS_WITH_GRPC)
    return()
endif()

find_package(Protobuf CONFIG QUIET)
if(NOT Protobuf_FOUND)
    find_package(Protobuf REQUIRED)
endif()
find_package(gRPC CONFIG REQUIRED)

set(PROTO_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../proto")
set(PROTO_FILE "${PROTO_ROOT}/actuator.proto")
set(GENERATED_DIR "${CMAKE_CURRENT_BINARY_DIR}/generated")
set(GENERATED_FILES
    "${GENERATED_DIR}/actuator.pb.cc"
    "${GENERATED_DIR}/actuator.pb.h"
    "${GENERATED_DIR}/actuator.grpc.pb.cc"
    "${GENERATED_DIR}/actuator.grpc.pb.h"
    "${GENERATED_DIR}/actuator.pb")

add_custom_command(
    OUTPUT ${GENERATED_FILES}
    COMMAND "${CMAKE_COMMAND}" -E make_directory "${GENERATED_DIR}"
    COMMAND protobuf::protoc
        "--proto_path=${PROTO_ROOT}"
        "--cpp_out=${GENERATED_DIR}"
        "--grpc_out=${GENERATED_DIR}"
        "--plugin=protoc-gen-grpc=$<TARGET_FILE:gRPC::grpc_cpp_plugin>"
        "--descriptor_set_out=${GENERATED_DIR}/actuator.pb"
        --include_imports
        "${PROTO_FILE}"
    DEPENDS "${PROTO_FILE}" protobuf::protoc gRPC::grpc_cpp_plugin
    VERBATIM)

add_custom_target(actuator_proto_generated DEPENDS ${GENERATED_FILES})
add_library(actuator_proto STATIC
    "${GENERATED_DIR}/actuator.pb.cc"
    "${GENERATED_DIR}/actuator.grpc.pb.cc")
add_dependencies(actuator_proto actuator_proto_generated)
target_compile_features(actuator_proto PUBLIC cxx_std_17)
target_include_directories(actuator_proto PUBLIC "${GENERATED_DIR}")
target_link_libraries(actuator_proto PUBLIC protobuf::libprotobuf gRPC::grpc++)

add_library(telemetry_store STATIC telemetry_store.cpp process_instance.cpp)
target_include_directories(telemetry_store PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(telemetry_store PUBLIC actuator_proto Threads::Threads)
if(WIN32)
    target_link_libraries(telemetry_store PRIVATE bcrypt)
endif()
if(MSVC)
    target_compile_options(telemetry_store PRIVATE /W4 /WX)
else()
    target_compile_options(telemetry_store PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()
if(BUILD_TESTING)
    add_executable(telemetry_store_test tests/telemetry_store_test.cpp)
    target_link_libraries(telemetry_store_test PRIVATE telemetry_store register_bank)
    add_test(NAME telemetry_store_test COMMAND telemetry_store_test)
    set_tests_properties(telemetry_store_test PROPERTIES TIMEOUT 30)
endif()
add_library(actuator_service STATIC actuator_service.cpp)
target_link_libraries(actuator_service PUBLIC register_bank telemetry_store)
if(BUILD_TESTING)
    add_executable(actuator_service_test tests/actuator_service_test.cpp)
    target_link_libraries(actuator_service_test PRIVATE actuator_service)
    add_test(NAME actuator_service_test COMMAND actuator_service_test)
    set_tests_properties(actuator_service_test PROPERTIES TIMEOUT 30)
endif()
add_executable(actuator_server server.cpp)
target_link_libraries(actuator_server PRIVATE actuator_service)
file(GENERATE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/server_path-$<CONFIG>.txt"
    CONTENT "$<TARGET_FILE:actuator_server>")
