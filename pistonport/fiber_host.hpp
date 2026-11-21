// Freestanding support for the shadPS4 fiber implementation in the Cars 3
// image.
#include <stddef.h>
#include <stdint.h>
using u8 = uint8_t;
using s8 = int8_t;
using u16 = uint16_t;
using u32 = uint32_t;
using s32 = int32_t;
using u64 = uint64_t;
#define PS4_SYSV_ABI
#define ORBIS_OK 0
#define LOG_WARNING(...) ((void)0)
#define UNREACHABLE_MSG(...) __builtin_trap()
namespace Common::ElfInfo {
constexpr u32 FW_350 = 0x3500000;
}

template <typename T> struct GuestAtomic {
  T value;
  constexpr GuestAtomic(T initial = T{}) : value(initial) {}
  operator T() const { return __atomic_load_n(&value, __ATOMIC_SEQ_CST); }
  void operator=(T desired) {
    __atomic_store_n(&value, desired, __ATOMIC_SEQ_CST);
  }
  bool compare_exchange_strong(T &expected, T desired) {
    return __atomic_compare_exchange_n(&value, &expected, desired, false,
                                       __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST);
  }
};
extern "C" {
extern uint32_t cars3_fiber_key;
extern void *(*cars3_key_get)(uint32_t);
extern int (*cars3_key_set)(uint32_t, void *);
}
static char *strncpy(char *dst, const char *src, size_t n) {
  size_t i = 0;
  for (; i < n && src[i]; ++i)
    dst[i] = src[i];
  for (; i < n; ++i)
    dst[i] = 0;
  return dst;
}
