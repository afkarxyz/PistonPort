// Read back the same local metadata that SpartanPort writes on mount and save.
// Cars 3 uses one directory (CUSA07027) for player one.
#include <stddef.h>
#include <stdint.h>

int (*cars3_open)(const char *, int, int);
int64_t (*cars3_read)(int, void *, uint64_t);
int (*cars3_close)(int);
int64_t (*cars3_seek)(int, int64_t, int);
extern int (*cars3_kernel_stat)(const char *, void *);

int64_t (*cars3_get_direntries)(int, char *, uint64_t, int64_t *);
int64_t cars3_getdents(int fd, char *buffer, uint64_t size) {
  return cars3_get_direntries(fd, buffer, size, NULL);
}
enum {
  BAD_PARAM = (int)0x809f0000,
  NO_MOUNT = (int)0x809f0004,
  NOT_FOUND = (int)0x809f0008,
  IO_ERROR = (int)0x809f000b
};
static int mounted(const char *point) {
  if (!point)
    return 0;
  const char prefix[] = "/savedata";
  for (unsigned i = 0; i < 9; ++i)
    if (point[i] != prefix[i])
      return 0;
  if (point[9] < '0' || point[9] > '9')
    return 0;
  if (point[10] &&
      !(point[9] == '1' && point[10] >= '0' && point[10] <= '5' && !point[11]))
    return 0;
  uint64_t st[32];
  return cars3_kernel_stat(point, st) == 0;
}
static int read_at(const char *file, size_t offset, void *destination,
                   size_t size) {
  int fd = cars3_open(file, 0, 0);
  if (fd < 0)
    return NOT_FOUND;
  int result = 0;
  if (cars3_seek(fd, (int64_t)offset, 0) < 0)
    result = IO_ERROR;
  size_t done = 0;
  while (!result && done < size) {
    int64_t n = cars3_read(fd, (char *)destination + done, size - done);
    if (n <= 0)
      result = IO_ERROR;
    else
      done += (size_t)n;
  }
  cars3_close(fd);
  return result;
}
int cars3_get_param(const char *point, unsigned type, void *dst,
                    size_t capacity, size_t *got) {
  static const unsigned offsets[] = {0, 0, 128, 256, 1280, 1288};
  static const unsigned sizes[] = {1328, 128, 128, 1024, 4, 8};
  if (!dst || type >= 6 || !capacity)
    return BAD_PARAM;
  if (!mounted(point))
    return NO_MOUNT;
  size_t size = sizes[type];
  int string = type >= 1 && type <= 3;
  if (!string && capacity < size)
    return BAD_PARAM;
  if (string && capacity < size)
    size = capacity;
  int result = read_at("/cars3meta/param.bin", offsets[type], dst, size);
  if (result)
    return result;
  if (string) {
    char *s = dst;
    s[size - 1] = 0;
    size_t used = 0;
    while (s[used])
      ++used;
    size = used + 1;
  }
  if (got)
    *got = size;
  return 0;
}
int cars3_load_icon(const char *point, void *data) {
  struct Icon {
    void *buffer;
    uint64_t capacity, size;
  };
  struct Icon *icon = data;
  if (!icon || (!icon->buffer && icon->capacity))
    return BAD_PARAM;
  if (!mounted(point))
    return NO_MOUNT;
  int fd = cars3_open("/cars3meta/icon0.png", 0, 0);
  if (fd < 0)
    return NOT_FOUND;
  int64_t length = cars3_seek(fd, 0, 2);
  cars3_close(fd);
  if (length < 0)
    return IO_ERROR;
  icon->size = length;
  size_t amount = icon->capacity < icon->size ? icon->capacity : icon->size;
  return amount ? read_at("/cars3meta/icon0.png", 0, icon->buffer, amount) : 0;
}
static int directory_bytes(const char *path, uint64_t *total) {
  int dir = cars3_open(path, 0x20000, 0);
  if (dir < 0)
    return NOT_FOUND;
  unsigned char entries[512];
  char child[300];
  size_t prefix = 0;
  while (path[prefix]) {
    child[prefix] = path[prefix];
    ++prefix;
  }
  child[prefix++] = '/';
  int result = 0;
  for (;;) {
    int64_t n =
        cars3_get_direntries(dir, (char *)entries, sizeof(entries), NULL);
    if (n < 0)
      result = IO_ERROR;
    if (n <= 0)
      break;
    for (size_t i = 0; i < (size_t)n;) {
      if (n - i < 8) {
        result = IO_ERROR;
        break;
      }
      size_t record = entries[i + 4] | ((unsigned)entries[i + 5] << 8);
      size_t length = entries[i + 7];
      if (record < 8 + length || i + record > (size_t)n ||
          prefix + length >= sizeof(child)) {
        result = IO_ERROR;
        break;
      }
      if (entries[i + 6] == 8) {
        for (size_t j = 0; j < length; ++j)
          child[prefix + j] = entries[i + 8 + j];
        child[prefix + length] = 0;
        uint64_t st[15]; // PS4 stat, st_size at offset 72
        if (cars3_kernel_stat(child, st)) {
          result = IO_ERROR;
          break;
        }
        *total += st[9];
      }
      i += record;
    }
    if (result)
      break;
  }
  cars3_close(dir);
  return result;
}
int cars3_mount_info(const char *point, void *info) {
  if (!info)
    return BAD_PARAM;
  if (!mounted(point))
    return NO_MOUNT;
  uint64_t used = 0;
  int error = directory_bytes(point, &used);
  if (!error)
    error = directory_bytes("/cars3meta", &used);
  if (error)
    return error;
  // The PS4 save created by this title reserves 96 blocks of 32 KiB.
  const uint64_t blocks = 96;
  uint64_t *values = info;
  values[0] = blocks;
  values[1] = used / 32768 < blocks ? blocks - used / 32768 : 0;
  for (unsigned i = 2; i < 6; ++i)
    values[i] = 0;
  return 0;
}
