cmake_minimum_required(VERSION 3.24)
project(permanexus_actuator VERSION 0.1.0 LANGUAGES CXX)

find_package(Protobuf CONFIG REQUIRED)
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
