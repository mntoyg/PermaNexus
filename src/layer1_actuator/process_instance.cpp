#include "process_instance.h"

#if defined(_WIN32)
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <bcrypt.h>
#elif defined(__linux__)
#include <cerrno>
#include <sys/random.h>
#elif defined(__APPLE__)
#include <unistd.h>
#endif

namespace permanexus {
bool fill_os_random(InstanceId& output) {
#if defined(_WIN32)
    return BCryptGenRandom(nullptr, output.data(), static_cast<ULONG>(output.size()),
                           BCRYPT_USE_SYSTEM_PREFERRED_RNG) == 0;
#elif defined(__linux__)
    std::size_t filled = 0;
    while (filled < output.size()) {
        const auto count = getrandom(output.data() + filled, output.size() - filled, 0);
        if (count < 0 && errno == EINTR) { continue; }
        if (count <= 0) { return false; }
        filled += static_cast<std::size_t>(count);
    }
    return true;
#elif defined(__APPLE__)
    return getentropy(output.data(), output.size()) == 0;
#else
    (void)output;
    return false;
#endif
}

std::optional<InstanceId> generate_instance_id(RandomFill fill) {
    InstanceId output{};
    if (fill == nullptr || !fill(output)) { return std::nullopt; }
    return output;
}
}  // namespace permanexus
