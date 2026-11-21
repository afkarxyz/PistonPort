// Octane workers use event flags to pass work between guest threads.
#include <stdatomic.h>
#include <stddef.h>
#include <stdint.h>

#define KERR(n) ((int32_t)(0x80020000u | (n)))
int (*cars3_sem_create)(uint32_t *, const char *, uint32_t, int, int,
                        const void *);
int (*cars3_sem_wait)(uint32_t, int, uint32_t *);
int (*cars3_sem_signal)(uint32_t, int);
int (*cars3_sem_delete)(uint32_t);
extern int (*cars3_key_create)(uint32_t *, void (*)(void *));
extern int (*cars3_key_set)(uint32_t, void *);
extern void *(*cars3_key_get)(uint32_t);
static uint32_t waiter_key;
static void drop_semaphore(void *handle) {
  cars3_sem_delete((uint32_t)(uintptr_t)handle);
}
int cars3_init_events(void) {
  return cars3_key_create(&waiter_key, drop_semaphore);
}
static int thread_semaphore(uint32_t *id) {
  *id = (uint32_t)(uintptr_t)cars3_key_get(waiter_key);
  if (*id)
    return 0;
  int error = cars3_sem_create(id, "Cars3Event", 1, 0, 1, NULL);
  if (error)
    return error;
  error = cars3_key_set(waiter_key, (void *)(uintptr_t)*id);
  if (error)
    cars3_sem_delete(*id);
  return error;
}
void *(*cars3_thread_self)(void);
int (*cars3_thread_priority)(void *, int *);

struct Waiter {
  struct Waiter *next;
  uint64_t bits, received;
  uint32_t mode, sem;
  int priority, result, done;
};
struct Event {
  struct Event *next;
  struct Waiter *first;
  uint64_t id, pattern;
  uint32_t attr;
};
// Workers keep a distinct completion flag for each outstanding file request.
// Grow by a guest page at a time and reuse entries after all waiters are woken.
enum { BUCKET_COUNT = 256, PAGE_BYTES = 16384 };
int (*cars3_map_flag_page)(void **address, uint64_t length, int protection,
                           int flags);
static struct Event *buckets[BUCKET_COUNT];
static struct Event *free_events;
static uint64_t next_id = 1;
static atomic_flag guard = ATOMIC_FLAG_INIT;
static void lock(void) {
  while (atomic_flag_test_and_set_explicit(&guard, memory_order_acquire))
    __builtin_ia32_pause();
}
static void unlock(void) {
  atomic_flag_clear_explicit(&guard, memory_order_release);
}
static struct Event **find_link(uint64_t id) {
  struct Event **link = &buckets[id % BUCKET_COUNT];
  while (*link && (*link)->id != id)
    link = &(*link)->next;
  return link;
}
static struct Event *lookup(uint64_t id) { return *find_link(id); }
static int grow_pool(void) {
  void *page = NULL;
  int error = cars3_map_flag_page(&page, PAGE_BYTES, 3, 0);
  if (error)
    return error;
  struct Event *entries = page;
  for (size_t i = 0; i < PAGE_BYTES / sizeof(*entries); ++i) {
    entries[i].next = free_events;
    free_events = &entries[i];
  }
  return 0;
}
static int consume(struct Event *e, struct Waiter *w) {
  uint64_t match = e->pattern & w->bits;
  if ((w->mode & 15) == 1 ? match != w->bits : !match)
    return 0;
  w->received = e->pattern;
  if (w->mode & 0x10)
    e->pattern = 0;
  if (w->mode & 0x20)
    e->pattern &= ~w->bits;
  return 1;
}
int sceKernelCreateEventFlag(uint64_t *out, const char *name, uint32_t attr,
                             uint64_t bits, void *opt) {
  if (!out || !name || opt || (attr & ~0x33u) || (attr & 3) == 3 ||
      (attr & 0x30) == 0x30)
    return KERR(22);
  unsigned len = 0;
  while (len < 32 && name[len])
    ++len;
  if (len == 32)
    return KERR(63);
  lock();
  int result = free_events ? 0 : grow_pool();
  if (!result) {
    uint64_t id;
    do {
      id = next_id++;
    } while (!id || lookup(id));
    struct Event **bucket = &buckets[id % BUCKET_COUNT];
    struct Event *e = free_events;
    free_events = e->next;
    *e = (struct Event){
        .next = *bucket, .id = id, .pattern = bits, .attr = attr};
    *bucket = e;
    *out = id;
  }
  unlock();
  return result;
}
int sceKernelClearEventFlag(uint64_t id, uint64_t keep) {
  lock();
  struct Event *e = lookup(id);
  if (e)
    e->pattern &= keep;
  unlock();
  return e ? 0 : KERR(3);
}
static int update(uint64_t id, uint64_t bits, int deleting) {
  lock();
  struct Event **link = find_link(id);
  struct Event *e = *link;
  if (!e) {
    unlock();
    return KERR(3);
  }
  e->pattern |= bits;
  struct Waiter **p = &e->first;
  while (*p) {
    struct Waiter *w = *p;
    if (!deleting && !consume(e, w)) {
      p = &w->next;
      continue;
    }
    *p = w->next;
    if (deleting) {
      w->received = e->pattern;
      w->result = KERR(13);
    }
    w->done = 1;
    cars3_sem_signal(w->sem, 1);
  }
  if (deleting) {
    *link = e->next;
    e->next = free_events;
    free_events = e;
  }
  unlock();
  return 0;
}
int sceKernelDeleteEventFlag(uint64_t id) { return update(id, 0, 1); }
int sceKernelSetEventFlag(uint64_t id, uint64_t bits) {
  return update(id, bits, 0);
}
int sceKernelWaitEventFlag(uint64_t id, uint64_t bits, uint32_t mode,
                           uint64_t *received, uint32_t *timeout) {
  if (!bits || ((mode & 15) != 1 && (mode & 15) != 2) || (mode >> 4) > 2)
    return KERR(22);
  struct Waiter w = {.bits = bits, .mode = mode};
  lock();
  struct Event *e = lookup(id);
  if (!e)
    w.result = KERR(3);
  else if (e->first && !(e->attr & 0x20))
    w.result = KERR(1);
  else if (!consume(e, &w)) {
    w.received = e->pattern;
    if (timeout && !*timeout)
      w.result = KERR(60);
    else if (!(w.result = thread_semaphore(&w.sem))) {
      if ((e->attr & 3) == 2)
        cars3_thread_priority(cars3_thread_self(), &w.priority);
      struct Waiter **p = &e->first;
      while (*p && (*p)->priority <= w.priority)
        p = &(*p)->next;
      w.next = *p;
      *p = &w;
      unlock();
      int status = cars3_sem_wait(w.sem, 1, timeout);
      lock();
      if (!w.done) {
        struct Waiter **link = &e->first;
        while (*link != &w)
          link = &(*link)->next;
        *link = w.next;
        w.result = status;
        w.received = e->pattern;
      }
      // A signal may acquire the registry lock just after sem_wait times out.
      // Consume that signal before the same thread reuses its semaphore.
      if (status && w.done) {
        uint32_t poll = 0;
        cars3_sem_wait(w.sem, 1, &poll);
      }
    }
  }
  unlock();
  if (received)
    *received = w.received;
  return w.result;
}
int sceKernelPollEventFlag(uint64_t id, uint64_t bits, uint32_t mode,
                           uint64_t *received) {
  uint32_t timeout = 0;
  int result = sceKernelWaitEventFlag(id, bits, mode, received, &timeout);
  return result == KERR(60) ? KERR(16) : result;
}
