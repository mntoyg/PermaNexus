#pragma once

#include "actuator.grpc.pb.h"
#include "process_instance.h"

#include <memory>

namespace permanexus {
std::unique_ptr<actuator::v1::HardwareActuator::Service>
make_actuator_service(const InstanceId& instance_id);
}  // namespace permanexus
