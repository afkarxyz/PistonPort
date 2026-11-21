"""Read-only access to selected files in a PistonPort game package."""
import ctypes as c
from pathlib import Path


class Info(c.Structure):
    _fields_ = [("length", c.c_uint64), ("mtime", c.c_uint64),
                ("children", c.c_uint32), ("directory", c.c_uint32)]


def trophy_metadata(trp, library):
    """Get the overlay's English names and grades from the local trophy pack."""
    lib = c.CDLL(str(library))
    read = lib.pp_trophy_texts
    read.argtypes = [c.c_char_p, c.c_size_t, c.c_void_p, c.c_size_t]
    read.restype = c.c_int
    output = c.create_string_buffer(32 * 1024)
    length = read(trp, len(trp), output, len(output))
    if length < 0:
        raise ValueError("Unrecognized Cars 3 trophy metadata.")
    return output.raw[:length]


class Package:
    def __init__(self, path, library):
        self.lib = lib = c.CDLL(str(library))
        lib.pp_open.argtypes = [c.c_wchar_p, c.c_size_t, c.c_void_p, c.c_size_t]
        lib.pp_open.restype = c.c_void_p
        lib.pp_close.argtypes = [c.c_void_p]
        lib.pp_close.restype = None
        lib.pp_find.argtypes = [c.c_void_p, c.c_char_p, c.POINTER(Info)]
        lib.pp_read.argtypes = [c.c_void_p, c.c_uint32, c.c_uint64, c.c_void_p, c.c_size_t]
        lib.pp_read.restype = c.c_int64
        error = c.create_string_buffer(512)
        path = str(Path(path).resolve())
        self.handle = lib.pp_open(path, len(path.encode("utf-16-le")) // 2, error, len(error))
        if not self.handle:
            raise ValueError("Cannot open PKG: " + error.value.decode("utf-8", errors="replace"))

    def __enter__(self):
        return self

    def __exit__(self, *_):
        if self.handle:
            self.lib.pp_close(self.handle)
            self.handle = None

    def lookup(self, name):
        info = Info()
        node = self.lib.pp_find(self.handle, name.encode("utf-8"), c.byref(info))
        if node < 0:
            raise ValueError(f"The game package is missing {name}.")
        return node, info

    def chunks(self, name, limit):
        node, info = self.lookup(name)
        if info.directory or info.length > limit:
            raise ValueError(f"Invalid size or type for {name}.")
        buffer = c.create_string_buffer(1024 * 1024)
        offset = 0
        while offset < info.length:
            count = self.lib.pp_read(self.handle, node, offset, buffer, min(len(buffer), info.length-offset))
            if count <= 0:
                raise ValueError(f"Could not read {name} from the game package.")
            yield buffer.raw[:count]
            offset += count
