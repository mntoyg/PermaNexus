#pragma once

#include <thread>

#if defined(_M_IX86) || defined(_M_X64) || defined(__i386__) || defined(__x86_64__)
#include <xmmintrin.h>
#endif

namespace permanexus::hpp {

// Scheduling hint only: neither a memory barrier nor a synchronization primitive.
inline void cpu_relax() noexcept {
#if defined(_M_IX86) || defined(_M_X64) || defined(__i386__) || defined(__x86_64__)
    _mm_pause();
#else
    std::this_thread::yield();
#endif
}

class SpinWait final {
public:
    void pause() noexcept {
        if (attempts_ < 16) {
            ++attempts_;
            cpu_relax();
        } else {
            std::this_thread::yield();
        }
    }

    void reset() noexcept { attempts_ = 0; }

private:
    unsigned attempts_ = 0;
};

}  // namespace permanexus::hpp
