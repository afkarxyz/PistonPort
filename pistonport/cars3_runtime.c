// Cars 3 uses this PS4 libc interface during its allocator initialization.
// Disabled tracing still exposes writable storage for the lifetime of the
// process.
#include <stdatomic.h>
#include <stdint.h>

struct HeapTraceInfo {
  uint64_t size;
  uint32_t flags;
  uint32_t get_segment_info;
  uint64_t *arena_mask;
  uint64_t *arena_states;
};

static uint64_t arena_mask;
static uint64_t arena_states[64];

uint32_t cars3_fiber_key;
int (*cars3_key_create)(uint32_t *, void (*)(void *));
int (*cars3_key_set)(uint32_t, void *);
void *(*cars3_key_get)(uint32_t);

int cars3_init_events(void);
int cars3_module_start(void) {
  int error = cars3_key_create(&cars3_fiber_key, 0);
  return error ? error : cars3_init_events();
}

int (*cars3_kernel_stat)(const char *, void *);
struct Zone {
  int minutes_west, dst;
};
struct TimeInfo {
  int64_t utc;
  int32_t offset, daylight;
};
int (*cars3_zone)(struct Zone *);
int cars3_utc_to_local(int64_t utc, int64_t *local, struct TimeInfo *info,
                       uint64_t *dst) {
  struct Zone tz;
  int error = cars3_zone(&tz);
  if (error)
    return error;
  int offset = -tz.minutes_west * 60;
  if (local)
    *local = utc + offset;
  if (info)
    *info = (struct TimeInfo){utc, offset, 0};
  if (dst)
    *dst = 0;
  return 0;
}
int cars3_local_to_utc(int64_t local, int64_t hint, int64_t *utc,
                       struct Zone *zone, int *dst) {
  (void)hint;
  struct Zone tz;
  int error = cars3_zone(&tz);
  if (error)
    return error;
  if (utc)
    *utc = local + tz.minutes_west * 60;
  if (zone)
    *zone = tz;
  if (dst)
    *dst = 0;
  return 0;
}
int (*cars3_pad_info)(int, void *);

// SpartanPort's virtual pad supplies buttons and axes without motion sampling.
int scePadSetMotionSensorState(int handle, _Bool enable) {
  uint64_t info[16];
  int result = cars3_pad_info(handle, info);
  if (result != 0)
    return result;
  return enable ? (int)0x80920101 : 0;
}

int sceKernelIsNeoMode(void) {
  return 0; // Base PS4 hardware profile.
}

// Local installation. The SpartanPort service validates handles and chunk IDs
// against the extracted PlayGo manifest. No downloads are outstanding.
int (*cars3_chunk_locus)(int, const uint16_t *, uint32_t, int8_t *);
static uint64_t language_mask = UINT64_C(1) << 62; // en-US
static int installed(int handle, const uint16_t *ids, uint32_t count) {
  if (!ids)
    return (int)0x80b2000a;
  if (!count)
    return (int)0x80b2000b;
  for (uint32_t i = 0; i < count; ++i) {
    int8_t locus;
    int result = cars3_chunk_locus(handle, ids + i, 1, &locus);
    if (result)
      return result;
    if (locus != 3)
      return (int)0x80b20011;
  }
  return 0;
}
int scePlayGoGetToDoList(int handle, void *list, uint32_t capacity,
                         uint32_t *count) {
  uint16_t first = 0;
  int result = installed(handle, &first, 1);
  if (result)
    return result;
  if (!list || !count)
    return (int)0x80b2000a;
  if (!capacity)
    return (int)0x80b2000b;
  *count = 0;
  return 0;
}
int scePlayGoGetEta(int handle, const uint16_t *ids, uint32_t count,
                    int64_t *eta) {
  if (!eta)
    return (int)0x80b2000a;
  int result = installed(handle, ids, count);
  if (!result)
    *eta = 0;
  return result;
}
int scePlayGoPrefetch(int handle, const uint16_t *ids, uint32_t count,
                      int8_t locus) {
  if (locus != 0 && locus != 2 && locus != 3)
    return (int)0x80b20010;
  return installed(handle, ids, count);
}
int scePlayGoGetLanguageMask(int handle, uint64_t *mask) {
  if (!mask)
    return (int)0x80b2000a;
  uint16_t first = 0;
  int result = installed(handle, &first, 1);
  if (!result)
    *mask = __atomic_load_n(&language_mask, __ATOMIC_RELAXED);
  return result;
}
int scePlayGoSetLanguageMask(int handle, uint64_t mask) {
  uint16_t first = 0;
  int result = installed(handle, &first, 1);
  if (!result)
    __atomic_store_n(&language_mask, mask, __ATOMIC_RELAXED);
  return result;
}

// This profile has one local user and remains signed out of PSN. There are no
// sign-in transitions to dispatch, but each toolkit subscription has a
// lifetime.
struct Subscription {
  void *function, *argument;
};
static struct Subscription user_events, np_events;
static atomic_flag subscription_lock = ATOMIC_FLAG_INIT;
static void set_subscription(struct Subscription *slot, void *function,
                             void *argument) {
  while (atomic_flag_test_and_set(&subscription_lock))
    __builtin_ia32_pause();
  *slot = (struct Subscription){function, argument};
  atomic_flag_clear(&subscription_lock);
}
int cars3_user_subscribe(void *function, void *argument) {
  if (!function)
    return (int)0x80960005;
  set_subscription(&user_events, function, argument);
  return 0;
}
int cars3_user_unsubscribe(void) {
  set_subscription(&user_events, 0, 0);
  return 0;
}
int cars3_np_subscribe(void *function, void *argument) {
  if (!function)
    return (int)0x80550003;
  set_subscription(&np_events, function, argument);
  return 0;
}
int cars3_np_unsubscribe(void) {
  set_subscription(&np_events, 0, 0);
  return 0;
}

// Cars uses only the local initialize/terminate pairs for these services.
// Initializing the service does not open a stream or a network session.
static atomic_uint local_services;
int cars3_share_initialize(void) {
  atomic_fetch_or(&local_services, 1u);
  return 0;
}
int cars3_share_terminate(void) {
  atomic_fetch_and(&local_services, ~1u);
  return 0;
}
int cars3_stream_initialize(void) {
  atomic_fetch_or(&local_services, 2u);
  return 0;
}
int cars3_stream_terminate(void) {
  atomic_fetch_and(&local_services, ~2u);
  return 0;
}

extern int (*cars3_thread_priority)(void *, int *);
int (*cars3_set_priority)(void *, int);
int cars3_get_sched(void *thread, int *policy, int *priority) {
  if (!thread || !policy || !priority)
    return 22;
  int error = cars3_thread_priority(thread, priority);
  if (!error)
    *policy = 1; // SpartanPort default FIFO guest policy
  return error & 0xffff;
}
int cars3_set_sched(void *thread, int policy, const int *priority) {
  if (!thread || !priority || policy < 1 || policy > 3)
    return 22;
  if (policy != 1)
    return 45;
  return cars3_set_priority(thread, *priority) & 0xffff;
}

struct SaveMount2 {
  int user;
  uint32_t padding;
  const void *directory;
  uint64_t blocks;
  uint32_t mode;
  uint8_t reserved[32];
};
struct SaveMount1 {
  int user;
  uint32_t padding;
  const void *title, *directory, *fingerprint;
  uint64_t blocks;
  uint32_t mode;
  uint8_t reserved[32];
};
_Static_assert(sizeof(struct SaveMount1) == 80 &&
                   sizeof(struct SaveMount2) == 64,
               "Unexpected save mount ABI");
int (*cars3_mount)(const struct SaveMount1 *, void *);
int cars3_mount2(const struct SaveMount2 *mount, void *result) {
  if (!mount || !result)
    return (int)0x809f0000;
  const struct SaveMount1 request = {.user = mount->user,
                                     .directory = mount->directory,
                                     .blocks = mount->blocks,
                                     .mode = mount->mode};
  return cars3_mount(&request, result);
}

int sceKernelCheckReachability(const char *path) {
  if (!path)
    return (int)0x8002000e;
  for (unsigned i = 0; i <= 255; ++i) {
    if (path[i])
      continue;
    if (i == 1 && path[0] == '/')
      return 0;
    uint64_t stat_buffer[32];
    return cars3_kernel_stat(path, stat_buffer);
  }
  return (int)0x8002003f;
}

void sceLibcHeapGetTraceInfo(struct HeapTraceInfo *info) {
  info->get_segment_info = 0;
  info->arena_mask = &arena_mask;
  info->arena_states = arena_states;
}
