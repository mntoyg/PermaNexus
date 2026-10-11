.DEFAULT_GOAL := help
.PHONY: help all build configure-actuator proto build-actuator build-permacore check

CMAKE ?= cmake
CARGO ?= cargo
BUILD_DIR ?= build
CMAKE_CONFIG ?= Release
CMAKE_FLAGS ?=

help:
	@echo "PermaNexus Day 1 targets:"
	@echo "  build             Build C++ and Rust gRPC contract libraries"
	@echo "  proto             Generate C++ bindings and a protobuf descriptor"
	@echo "  build-actuator    Build the C++ gRPC contract library"
	@echo "  build-permacore   Build the Rust gRPC contract library"
	@echo "  check             Check Rust formatting and compilation"

all: build

build: build-actuator build-permacore

configure-actuator:
	$(CMAKE) -S src/layer1_actuator -B "$(BUILD_DIR)/actuator" $(CMAKE_FLAGS)

proto: configure-actuator
	$(CMAKE) --build "$(BUILD_DIR)/actuator" --config $(CMAKE_CONFIG) --target actuator_proto_generated

build-actuator: configure-actuator
	$(CMAKE) --build "$(BUILD_DIR)/actuator" --config $(CMAKE_CONFIG) --parallel

build-permacore:
	$(CARGO) build --locked --release --manifest-path src/layer2_permacore/Cargo.toml

check:
	$(CARGO) fmt --manifest-path src/layer2_permacore/Cargo.toml -- --check
	$(CARGO) check --locked --manifest-path src/layer2_permacore/Cargo.toml
