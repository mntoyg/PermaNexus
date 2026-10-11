cmake_minimum_required(VERSION 3.24)
project(permanexus_actuator VERSION 0.1.0 LANGUAGES CXX)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE Release CACHE STRING "Build type" FORCE)
endif()
option(PERMANEXUS_WITH_GRPC "Build the loopback gRPC simulator" ON)
include(CTest)
find_package(Threads REQUIRED)
add_library(register_bank STATIC register_bank.cpp)
target_include_directories(register_bank PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_compile_features(register_bank PUBLIC cxx_std_17)
target_link_libraries(register_bank PUBLIC Threads::Threads)
if(MSVC)
    target_compile_options(register_bank PRIVATE /W4 /WX)
else()
    target_compile_options(register_bank PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()
if(BUILD_TESTING)
    add_executable(register_bank_test tests/register_bank_test.cpp)
    target_link_libraries(register_bank_test PRIVATE register_bank)
    add_test(NAME register_bank_test COMMAND register_bank_test)
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

add_executable(actuator_server server.cpp)
target_link_libraries(actuator_server PRIVATE register_bank actuator_proto)
file(GENERATE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/server_path-$<CONFIG>.txt"
    CONTENT "$<TARGET_FILE:actuator_server>")
