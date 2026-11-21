// Regression for the file-completion flags held simultaneously on Fireball Beach.
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>

int sceKernelCreateEventFlag(uint64_t *, const char *, uint32_t, uint64_t, void *);
int sceKernelSetEventFlag(uint64_t, uint64_t);
int sceKernelPollEventFlag(uint64_t, uint64_t, uint32_t, uint64_t *);
int sceKernelDeleteEventFlag(uint64_t);
extern int (*cars3_map_flag_page)(void **, uint64_t, int, int);
// These runtime slots belong to the thread-wait path. Polling never sleeps.
int (*cars3_key_create)(uint32_t *, void (*)(void *));
int (*cars3_key_set)(uint32_t, void *);
void *(*cars3_key_get)(uint32_t);

static unsigned page_count;
static int map_page(void **page, uint64_t size, int protection, int flags) {
    assert(size == 16384 && protection == 3 && !flags);
    *page = calloc(1, size);
    ++page_count;
    return *page ? 0 : (int)0x8002000c;
}

int main(void) {
    enum { requests = 1500 };
    uint64_t flags[requests], stale = 0;
    unsigned peak_pages = 0;
    cars3_map_flag_page = map_page;
    for (int round = 0; round < 2; ++round) {
        for (int i = 0; i < requests; ++i)
            assert(!sceKernelCreateEventFlag(&flags[i], "HogEventFlag", 0x10, 0, NULL));
        if (round) {
            assert(page_count == peak_pages);
            assert(sceKernelSetEventFlag(stale, 1) == (int)0x80020003);
        }
        for (int i = requests - 1; i >= 0; --i) {
            uint64_t pattern = 0;
            assert(!sceKernelSetEventFlag(flags[i], 1));
            assert(!sceKernelPollEventFlag(flags[i], 1, 0x21, &pattern) && pattern == 1);
            assert(sceKernelPollEventFlag(flags[i], 1, 1, NULL) == (int)0x80020010);
            assert(!sceKernelDeleteEventFlag(flags[i]));
        }
        peak_pages = page_count;
        stale = flags[0];
    }
    return 0;
}
