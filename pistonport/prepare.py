"""Prepare the CUSA07027 SpartanPort image from an existing local game dump."""

import argparse
import hashlib
import struct
import subprocess
import sys
from pathlib import Path

from elftools.elf.elffile import ELFFile
from fibers import compile_fibers

TARGET_HASH = "56d27cc83b7fe34921fb07413dd7c8fb255daf357a3f81c2d882969f001ff654"
# The toolkit and public NetCtl callbacks use the same PS4 calling convention.
# Both use the offline network provider in this build.
ALIASES = {
    "wIsKy+TfeLs#libSceNetCtlForNpToolkit": "UJ+Z7Q+4ck0#libSceNetCtl",
    "TywrFKCoLGY#libSceSaveData": "ZkZhskCPXFw#libSceSaveData",
    "JELHf4xPufo#libSceNpManagerForToolkit": "3Zl8BePTh9Y#libSceNpManager",
    "u5oqtlIP+Fw#libSceNetCtlForNpToolkit": "iQw3iQPhvUQ#libSceNetCtl",
    "2oUqKR5odGc#libSceNetCtlForNpToolkit": "Rqm2OnZMCz0#libSceNetCtl",
}
EXPORTS = {
    "NWtTN10cJzE#libSceLibcInternalExt": "sceLibcHeapGetTraceInfo",
    "uWyW3v98sU4#libkernel": "sceKernelCheckReachability",
    "clVvL4ZDntw#libScePad": "scePadSetMotionSensorState",
    "WslcK1FQcGI#libkernel": "sceKernelIsNeoMode",
    "0z45PIH+SNI#libSceSaveData": "cars3_mount2",
    "j2AIqSqJP0w#libkernel": "cars3_getdents",
    "-o5uEDpN+oY#libkernel": "cars3_utc_to_local",
    "0NTHN1NKONI#libkernel": "cars3_local_to_utc",
    "XgvSuIdnMlw#libSceSaveData": "cars3_get_param",
    "65VH0Qaaz6s#libSceSaveData": "cars3_mount_info",
    "cGjO3wM3V28#libSceSaveData": "cars3_load_icon",
    "FIs3-UQT9sg#libScePosix": "cars3_get_sched",
    "Xs9hdiD7sAA#libScePosix": "cars3_set_sched",
    "isruqthpYcw#libSceSharePlay": "cars3_share_initialize",
    "UaLjloJinow#libSceSharePlay": "cars3_share_terminate",
    "kvYEw2lBndk#libSceGameLiveStreaming": "cars3_stream_initialize",
    "9yK6Fk8mKOQ#libSceGameLiveStreaming": "cars3_stream_terminate",
    "wuI7c7UNk0A#libSceUserServiceForNpToolkit": "cars3_user_subscribe",
    "spW--yoLQ9o#libSceUserServiceForNpToolkit": "cars3_user_unsubscribe",
    "0c7HbXRKUt4#libSceNpManagerForToolkit": "cars3_np_subscribe",
    "YIvqqvJyjEc#libSceNpManagerForToolkit": "cars3_np_unsubscribe",
    "v6EZ-YWRdMs#libScePlayGo": "scePlayGoGetEta",
    "3OMbYZBaa50#libScePlayGo": "scePlayGoGetLanguageMask",
    "Nn7zKwnA5q0#libScePlayGo": "scePlayGoGetToDoList",
    "-Q1-u1a7p0g#libScePlayGo": "scePlayGoPrefetch",
    "LosLlHOpNqQ#libScePlayGo": "scePlayGoSetLanguageMask",
    "BpFoboUJoZU#libkernel": "sceKernelCreateEventFlag",
    "8mql9OcQnd4#libkernel": "sceKernelDeleteEventFlag",
    "IOnSvHzqu6A#libkernel": "sceKernelSetEventFlag",
    "7uhBFWRAS60#libkernel": "sceKernelClearEventFlag",
    "JTvBflhYazQ#libkernel": "sceKernelWaitEventFlag",
    "9lvj5DjHZiA#libkernel": "sceKernelPollEventFlag",
    "hVYD7Ou2pCQ#libSceFiber": "sceFiberInitialize",
    "JeNX5F-NzQU#libSceFiber": "sceFiberFinalize",
    "p+zLIOg27zU#libSceFiber": "sceFiberGetSelf",
    "B0ZX2hx9DMw#libSceFiber": "sceFiberReturnToThread",
    "a0LLrZWac0M#libSceFiber": "sceFiberRun",
    "PFT2S-tJ7Uk#libSceFiber": "sceFiberSwitch",
}
IMPORTS = {
    "IWIBBdTHit4#libkernel": "cars3_map_flag_page",
    "eV9wAD2riIA#libkernel": "cars3_kernel_stat",
    "geDaqgH9lTg#libkernel": "cars3_key_create",
    "gjP9-KQzoUk#libScePad": "cars3_pad_info",
    "32HQAQdwM2o#libSceSaveData": "cars3_mount",
    "1G3lF1Gg1k8#libkernel": "cars3_open",
    "Cg4srZ6TKbU#libkernel": "cars3_read",
    "UK2Tl2DWUns#libkernel": "cars3_close",
    "oib76F-12fk#libkernel": "cars3_seek",
    "taRWhTJFTgE#libkernel": "cars3_get_direntries",
    "kOcnerypnQA#libkernel": "cars3_zone",
    "uWIYLFkkwqk#libScePlayGo": "cars3_chunk_locus",
    "188x57JYp0g#libkernel": "cars3_sem_create",
    "R1Jvn8bSCW8#libkernel": "cars3_sem_delete",
    "Zxa0VhQVTsk#libkernel": "cars3_sem_wait",
    "4czppHBiriw#libkernel": "cars3_sem_signal",
    "1tKyG7RlMJo#libkernel": "cars3_thread_priority",
    "aI+OeCz8xrQ#libkernel": "cars3_thread_self",
    "W0Hpm2X0uPE#libkernel": "cars3_set_priority",
    "+BzXYkqYeLE#libkernel": "cars3_key_set",
    "eoht7mQOCmo#libkernel": "cars3_key_get",
}


def set_60fps_presentation(image):
    # CUSA07027 01.00 requests two vblanks in the base-PS4 mode. Select one
    # both when opening VideoOut (zero-based rate) and when restoring the
    # renderer's setting after a transition (one-based interval).
    changes = [
        (0x5A8C5, "0f b6 f1 41 89 74 24 50", "31 f6 90 41 89 74 24 50"),
        (0x58D333, "be 02 00 00 00 55", "be 01 00 00 00 55"),
        # If a pixel counter's valid bit is still clear, Octane already has
        # a timeout path. Poll for at most 500 us instead of 20 ms. Leave the
        # separate frame-ready wait, the result checks and the simulation
        # timestep untouched.
        (0x78037, "48 05 20 4e 00 00", "48 05 f4 01 00 00"),
    ]
    for offset, original, replacement in changes:
        expected = bytes.fromhex(original)
        if image[offset : offset + len(expected)] != expected:
            raise ValueError(f"Unexpected presentation code at {offset:#x}")
        image[offset : offset + len(expected)] = bytes.fromhex(replacement)


def prepare(game, reference, llvm, out, shad, prebuilt=None):
    if hashlib.sha256((game / "eboot.bin").read_bytes()).hexdigest() != TARGET_HASH:
        raise ValueError("Unsupported eboot.bin. This profile targets CUSA07027 01.00.")
    out.mkdir(parents=True, exist_ok=True)
    for tool in ("prepare.py", "link_modules.py"):
        command = [sys.executable, reference / "scripts" / tool, game, "--out", out]
        if tool == "link_modules.py":
            command += [
                "--modules",
                "libc.prx",
                "libSceFios2.prx",
                "libSceNpToolkit2.prx",
            ]
        subprocess.run(command, check=True)
    source = Path(__file__).resolve().parent
    original = (out / "boot-linked.bin").read_bytes()
    header = struct.Struct("<8s6Q")
    magic, size, entry, ns, nr, ni, flags = header.unpack_from(original)
    if magic != b"G3PROBE5":
        raise ValueError("Unsupported SpartanPort image version")
    nmods = struct.unpack_from("<Q", original, 96)[0]
    binding_start = 104 + 56 * nmods
    nb = struct.unpack_from("<Q", original, binding_start)[0]
    segment_start = binding_start + 8 + nb * 24
    name_start = segment_start + ns * 24
    relocation_start = name_start + ni * 128
    image_start = relocation_start + nr * 32
    if image_start + size != len(original):
        raise ValueError("Unexpected probe image layout")

    base = (size + 0xFFFF) & ~0xFFFF
    if prebuilt is not None:
        elf = prebuilt
    else:
        obj = out / "cars3-runtime.o"
        elf = out / "cars3-runtime.elf"
        subprocess.run(
            [
                llvm / "clang.exe",
                "-target",
                "x86_64-none-elf",
                "-O2",
                "-ffreestanding",
                "-fPIC",
                "-fno-stack-protector",
                "-c",
                source / "cars3_runtime.c",
                "-o",
                obj,
            ],
            check=True,
        )
        fibers = compile_fibers(shad, source, llvm, out)
        events = out / "cars3-events.o"
        saves = out / "cars3-save.o"
        subprocess.run(
            [
                llvm / "clang.exe",
                "-target",
                "x86_64-none-elf",
                "-O2",
                "-ffreestanding",
                "-fPIC",
                "-fno-stack-protector",
                "-c",
                source / "save_info.c",
                "-o",
                saves,
            ],
            check=True,
        )
        subprocess.run(
            [
                llvm / "clang.exe",
                "-target",
                "x86_64-none-elf",
                "-O2",
                "-ffreestanding",
                "-fPIC",
                "-fno-stack-protector",
                "-c",
                source / "event_flags.c",
                "-o",
                events,
            ],
            check=True,
        )
        subprocess.run(
            [
                llvm / "ld.lld.exe",
                "-T",
                source / "runtime.ld",
                f"--defsym=CARS3_BASE={base}",
                "-e",
                "cars3_module_start",
                obj,
                fibers,
                events,
                saves,
                "-o",
                elf,
            ],
            check=True,
        )
    image = bytearray(original[image_start:])
    set_60fps_presentation(image)
    segments = []
    bindings = []
    relocations = []
    names = [
        original[pos : pos + 128].split(b"\0", 1)[0].decode("ascii")
        for pos in range(name_start, relocation_start, 128)
    ]
    names.extend(name for name in IMPORTS if name not in names)
    bound = {
        struct.unpack_from("<Q", original, pos)[0]
        for pos in range(binding_start + 8, segment_start, 24)
    }
    with elf.open("rb") as f:
        module = ELFFile(f)
        for seg in module.iter_segments("PT_LOAD"):
            addr, length, perms = seg["p_vaddr"], seg["p_memsz"], seg["p_flags"]
            if not length or addr < len(image) or addr + length > 512 * 1024**2:
                raise ValueError("Invalid native runtime segment")
            data = seg.data()
            image.extend(bytes(addr + length - len(image)))
            image[addr : addr + len(data)] = data
            segments.append((addr, length, perms))
        symbols = module.get_section_by_name(".symtab")
        initializer = symbols.get_symbol_by_name("cars3_module_start")[0]["st_value"]
        for nid, name in EXPORTS.items():
            index = names.index(nid)
            address = symbols.get_symbol_by_name(name)[0]["st_value"]
            if index in bound or not any(
                p & 1 and a <= address < a + n for a, n, p in segments
            ):
                raise ValueError(f"Invalid binding for {name}")
            bindings.append((index, address, 1))
        for nid, name in IMPORTS.items():
            slot = symbols.get_symbol_by_name(name)[0]["st_value"]
            if not any(
                p & 2 and a <= slot and slot + 8 <= a + n for a, n, p in segments
            ):
                raise ValueError(f"Invalid runtime import slot {name}")
            relocations.append((slot, 1, names.index(nid), 0))
    path = out / "cars3-native.bin"
    tmp = path.with_suffix(".tmp")
    with tmp.open("wb") as f:
        f.write(
            header.pack(
                magic,
                len(image),
                entry,
                ns + len(segments),
                nr + len(relocations),
                len(names),
                flags,
            )
        )
        f.write(original[header.size : 96])
        f.write(struct.pack("<Q", nmods + 1))
        f.write(original[104:binding_start])
        f.write(struct.pack("<7Q", base, len(image) - base, initializer, 0, 0, 0, 0))
        f.write(struct.pack("<Q", nb + len(bindings)))
        f.write(original[binding_start + 8 : segment_start])
        for row in bindings:
            f.write(struct.pack("<3Q", *row))
        f.write(original[segment_start:name_start])
        for row in segments:
            f.write(struct.pack("<3Q", *row))
        for name in names:
            f.write(ALIASES.get(name, name).encode("ascii").ljust(128, b"\0"))
        f.write(original[relocation_start:image_start])
        for row in relocations:
            f.write(struct.pack("<4Q", *row))
        f.write(image)
    tmp.replace(path)
    print(path)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("game", "reference", "llvm", "out", "shad"):
        parser.add_argument(arg, type=Path)
    args = parser.parse_args()
    prepare(
        *(
            path.resolve()
            for path in (args.game, args.reference, args.llvm, args.out, args.shad)
        )
    )
