/* Private to runtime_file.c. The archive and its immutable node IDs live until process exit. */
typedef struct { uint64_t length, mtime; uint32_t children, directory; } PkgInfo;
static void *pkg_archive;
static int (*pkg_find)(const void *, const char *, PkgInfo *);
static int (*pkg_child)(const void *, uint32_t, uint32_t, char *, size_t);
static int64_t (*pkg_read)(const void *, uint32_t, uint64_t, void *, size_t);
static void configure_pkg(void) {
#ifdef _WIN32
    const wchar_t *path = _wgetenv(L"PISTONPORT_PKG");
    if (!path || !*path) return;
    HMODULE dll = LoadLibraryExW(L"piston_pkg.dll", NULL,
        LOAD_LIBRARY_SEARCH_APPLICATION_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
    void *(*open_pkg)(const wchar_t *, size_t, char *, size_t) = NULL;
    if (dll) {
        open_pkg = (void *)GetProcAddress(dll, "pp_open");
        pkg_find = (void *)GetProcAddress(dll, "pp_find");
        pkg_child = (void *)GetProcAddress(dll, "pp_child");
        pkg_read = (void *)GetProcAddress(dll, "pp_read");
    }
    if (!open_pkg || !pkg_find || !pkg_child || !pkg_read) {
        fputs("ERROR: PistonPort package reader is unavailable. Extract the whole release.\n", stderr);
        exit(1);
    }
    char error[512] = {0};
    pkg_archive = open_pkg(path, wcslen(path), error, sizeof(error));
    if (!pkg_archive) { fprintf(stderr,"ERROR: Could not mount game package: %s\n",error); exit(1); }
    puts("PistonPort: mounted read-only PKG on /app0 and /hostapp");
#endif
}
