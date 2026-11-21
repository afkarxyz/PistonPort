"""Compare package-backed reads with the same locally extracted game."""
import argparse
from collections import Counter
import ctypes
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "launcher"))
from pkg_reader import Package, trophy_metadata


def verify(pkg, dump, library):
    with Package(pkg, library) as archive:
        for name in ("eboot.bin", "sce_module/libc.prx", "sce_sys/param.sfo",
                     "sce_sys/pic0.png", "sce_sys/pic1.png", "sce_sys/icon0.png", "sce_sys/playgo-chunk.dat",
                     "sce_sys/trophy/trophy00.trp", "bundles/zone-cars3_fireball/_root_.zip"):
            node, info = archive.lookup(name)
            assert not info.directory and info.length == (dump / name).stat().st_size, name
            with (dump / name).open("rb") as source:
                for offset in (0, 17, max(0, info.length // 2 - 37), max(0, info.length - 329), info.length):
                    buffer = ctypes.create_string_buffer(65539)
                    size = archive.lib.pp_read(archive.handle, node, offset, buffer, len(buffer))
                    source.seek(offset)
                    assert size >= 0 and buffer.raw[:size] == source.read(len(buffer)), (name, offset)
        node, info = archive.lookup("bundles/zone-cars3_fireball")
        assert info.directory
        child = archive.lib.pp_child
        child.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p, ctypes.c_size_t]
        listing = {}
        for i in range(info.children):
            name = ctypes.create_string_buffer(256)
            kind = child(archive.handle, node, i, name, len(name))
            listing[name.value.decode()] = kind
        assert listing == {p.name: 4 if p.is_dir() else 8 for p in (dump / "bundles/zone-cars3_fireball").iterdir()}
        trophy_path = "sce_sys/trophy/trophy00.trp"
        package_text = trophy_metadata(b"".join(archive.chunks(trophy_path, 32 * 1024**2)), library)
        assert package_text == trophy_metadata((dump / trophy_path).read_bytes(), library)
        fields = dict(line.split("=", 1) for line in package_text.decode("utf-8").splitlines())
        assert set(fields) == {key for i in range(22) for key in (str(i), f"@grade.{i}")}
        assert Counter(value for key, value in fields.items() if key.startswith("@grade.")) == {"P": 1, "G": 7, "S": 14}
        assert all(fields[str(i)].partition(" | ")[0] for i in range(22))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("pkg", "dump", "library"):
        parser.add_argument(arg, type=Path)
    opt = parser.parse_args()
    verify(opt.pkg.resolve(), opt.dump.resolve(), opt.library.resolve())
    print("PKG positional reads, EOF, directory listing and trophy metadata verified.")
