.DEFAULT_GOAL := help
.PHONY: help all build configure-actuator proto build-actuator build-permacore check test test-grpc simulate-wear

CMAKE ?= cmake
CARGO ?= cargo
CTEST ?= ctest
BUILD_DIR ?= build
CMAKE_CONFIG ?= Release
CMAKE_FLAGS ?=
SIMULATION_ARGS ?= 1000 4 1000

help:
	@echo "PermaNexus Day 1 targets:"
	@echo "  build             Build C++ and Rust gRPC contract libraries"
	@echo "  proto             Generate C++ bindings and a protobuf descriptor"
	@echo "  build-actuator    Build the C++ gRPC contract library"
	@echo "  build-permacore   Build the Rust gRPC contract library"
	@echo "  check             Check Rust formatting and compilation"
	@echo "  test              Test C++ register safety and Rust endpoint policy"
	@echo "  test-grpc         Test the C++ server with the Rust client and demo"
	@echo "  simulate-wear     Run the in-memory Rust write-budget simulation"

all: build

build: build-actuator build-permacore

configure-actuator:
	$(CMAKE) -S src/layer1_actuator -B "$(BUILD_DIR)/actuator" -DCMAKE_BUILD_TYPE=$(CMAKE_CONFIG) $(CMAKE_FLAGS)

proto: configure-actuator
	$(CMAKE) --build "$(BUILD_DIR)/actuator" --config $(CMAKE_CONFIG) --target actuator_proto_generated

build-actuator: configure-actuator
	$(CMAKE) --build "$(BUILD_DIR)/actuator" --config $(CMAKE_CONFIG) --parallel

build-permacore:
	$(CARGO) build --locked --release --manifest-path src/layer2_permacore/Cargo.toml

check:
	$(CARGO) fmt --manifest-path src/layer2_permacore/Cargo.toml -- --check
	$(CARGO) check --locked --manifest-path src/layer2_permacore/Cargo.toml

test: build-actuator
	$(CTEST) --test-dir "$(BUILD_DIR)/actuator" -C $(CMAKE_CONFIG) --output-on-failure
	$(CARGO) test --locked --manifest-path src/layer2_permacore/Cargo.toml

test-grpc: build
	$(CMAKE) -E env "PERMANEXUS_ACTUATOR_PATH_FILE=$(abspath $(BUILD_DIR)/actuator)/server_path-$(CMAKE_CONFIG).txt" $(CARGO) test --locked --manifest-path src/layer2_permacore/Cargo.toml --test grpc_contract -- --ignored

simulate-wear:
	$(CARGO) run --locked --release --manifest-path src/layer2_permacore/Cargo.toml --bin wear_simulator -- $(SIMULATION_ARGS)
