"""Import playable Cars 3 saves into independent slots. The original profile is the default."""
import argparse
import calendar
from contextlib import contextmanager
import os
from pathlib import Path, PurePosixPath
import re
import struct
import sys
import tempfile
import uuid
import zipfile

from save_trophies import NON_PLATINUM, earned_trophies

TITLE = "CUSA07027"
SELECTED = "selected-save.txt"
COMPLETE_BUILTIN = "builtin-complete-v2"
RETIRED_BUILTINS = {"builtin-smokey", "builtin-complete"}
BUILTIN_SAVES = {
    COMPLETE_BUILTIN: "All Skill Checks.zip",
}
PAYLOAD = re.compile(r"(?:P[0-9]+|(?:E|EH|S|SH|SC)C?3[0-9][A-Z0-9]*)\Z")
LIMIT = 32 * 1024**2
IMPORTED_SLOT = re.compile(r"[a-f0-9]{32}\Z")
PLAYER = Path("savedata/1") / TITLE / TITLE / "P0"


def slot_root(profile, slot):
    if slot == "default":
        return profile
    if slot not in BUILTIN_SAVES and not IMPORTED_SLOT.fullmatch(slot):
        raise ValueError("Invalid save slot.")
    result = profile / "save-slots" / slot
    if not (result / PLAYER).is_file():
        raise ValueError("This save slot is missing. Select another save in Saves.")
    return result


def active_root(profile):
    selected = profile / SELECTED
    slot = selected.read_text(encoding="utf-8").strip() if selected.exists() else "default"
    if slot in RETIRED_BUILTINS:
        select(profile, slot)
        slot = COMPLETE_BUILTIN
    return slot_root(profile, slot)


def select(profile, slot):
    if slot in RETIRED_BUILTINS:
        slot = COMPLETE_BUILTIN
    if slot in BUILTIN_SAVES and not (profile / "save-slots" / slot).exists():
        create_slot(profile, Path(__file__).with_name("saves") / BUILTIN_SAVES[slot], slot)
    root = slot_root(profile, slot)
    if IMPORTED_SLOT.fullmatch(slot):
        restore_trophies(root)
    write_selected(profile, slot)


def write_selected(profile, slot):
    with tempfile.NamedTemporaryFile(dir=profile, delete=False) as f:
        f.write(slot.encode("ascii"))
    try:
        os.replace(f.name, profile / SELECTED)
    finally:
        Path(f.name).unlink(missing_ok=True)


def restore_trophies(root):
    player = root / PLAYER
    trophy_file = root / "trophies.dat"
    with player.open("rb") as source:
        unlocked = earned_trophies(source.read(LIMIT))
    try:
        with trophy_file.open("rb") as source:
            data = source.read(16)
    except FileNotFoundError:
        data = b""
    existing = int.from_bytes(data, "little") if len(data) == 16 else 0
    merged = existing | unlocked
    if merged & NON_PLATINUM == NON_PLATINUM:
        merged |= 1
    if merged == existing:
        return
    with tempfile.NamedTemporaryFile(dir=root, delete=False) as staged:
        staged.write(merged.to_bytes(16, "little"))
    try:
        os.replace(staged.name, trophy_file)
    finally:
        Path(staged.name).unlink(missing_ok=True)


def sfo_fields(raw):
    if len(raw) < 20:
        raise ValueError("Incomplete PS4 metadata.")
    magic, _, keys, values, count = struct.unpack_from("<4s4I", raw)
    if magic != b"\0PSF" or count > 256 or not 20 + count * 16 <= keys <= values <= len(raw):
        raise ValueError("Invalid PS4 metadata.")
    fields = {}
    for i in range(count):
        key, _, size, capacity, offset = struct.unpack_from("<HHIII", raw, 20 + i * 16)
        end = raw.find(b"\0", keys + key, values)
        if end < 0 or size > capacity or values + offset + size > len(raw):
            raise ValueError("Invalid PS4 metadata field.")
        fields[raw[keys + key:end]] = raw[values + offset:values + offset + size]
    return fields


def sfo_param(raw, timestamp):
    fields = sfo_fields(raw)
    if any(fields.get(k, b"").rstrip(b"\0") != TITLE.encode() for k in (b"TITLE_ID", b"SAVEDATA_DIRECTORY")):
        raise ValueError("This save belongs to another title or region. Select a CUSA07027 save.")
    return struct.pack("<128s128s1024sI4xq32x", fields.get(b"MAINTITLE", b"")[:127],
                       fields.get(b"SUBTITLE", b"")[:127], fields.get(b"DETAIL", b"")[:1023],
                       int.from_bytes(fields.get(b"SAVEDATA_LIST_PARAM", b"\0")[:4], "little"), timestamp)


@contextmanager
def save_source(source):
    if source.suffix.lower() == ".zip":
        with zipfile.ZipFile(source) as archive:
            files = {}
            for entry in archive.infolist():
                path = PurePosixPath(entry.filename.replace("\\", "/"))
                if entry.is_dir() or "__MACOSX" in path.parts:
                    continue
                if path.is_absolute() or ".." in path.parts or path in files:
                    raise ValueError("Invalid or duplicate save archive path.")
                files[path] = entry
            profiles = [p for p in files if p.name == "P0"]
            if len(profiles) != 1:
                raise ValueError("Choose a ZIP with one decrypted CUSA07027 save containing P0 and save metadata.")
            base = profiles[0].parent
            def read(path, limit):
                entry = files.get(path)
                if entry is None:
                    return None
                if entry.file_size > limit:
                    raise ValueError("Save file exceeds the size limit.")
                return archive.read(entry)
            stamp = calendar.timegm(files[profiles[0]].date_time)
            yield base, [p.name for p in files if p.parent == base], read, stamp
    else:
        if source.name != "P0" or not source.is_file():
            raise ValueError("Select a save ZIP or the P0 file inside a decrypted save folder.")
        base = source.parent
        def read(path, limit):
            if not path.is_file():
                return None
            if path.stat().st_size > limit:
                raise ValueError("Save file exceeds the size limit.")
            with path.open("rb") as f:
                data = f.read(limit + 1)
            if len(data) > limit:
                raise ValueError("Save file exceeds the size limit.")
            return data
        yield base, [p.name for p in base.iterdir() if p.is_file()], read, int(source.stat().st_mtime)


def create_slot(profile, source, slot):
    slots = profile / "save-slots"
    slots.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(dir=slots) as tmp, save_source(source) as (base, names, read, stamp):
        staged = Path(tmp) / "slot"
        body = staged / "savedata/1" / TITLE / TITLE
        body.mkdir(parents=True)
        meta = body.with_name(TITLE + ".sce_sys")
        meta.mkdir()
        original_meta = base / "sce_sys"
        sfo = read(original_meta / "param.sfo", 512 * 1024)
        if sfo is not None:
            param = sfo_param(sfo, stamp)
        elif base.name == TITLE:
            original_meta = base.with_name(TITLE + ".sce_sys")
            param = read(original_meta / "param.bin", 1328)
        else:
            param = None
        if param is None or len(param) != 1328:
            raise ValueError("Save metadata is missing. Keep sce_sys or the native .sce_sys folder beside the save files.")
        (meta / "param.bin").write_bytes(param)
        icon = read(original_meta / "icon0.png", 8 * 1024**2)
        if icon:
            (meta / "icon0.png").write_bytes(icon)
        if sfo is None and source.suffix.lower() == ".zip":
            trophies = read(base.parent / "trophies.dat", 16)
            if trophies is not None:
                if len(trophies) != 16:
                    raise ValueError("Invalid PistonPort trophy data.")
                (staged / "trophies.dat").write_bytes(trophies)
        remaining = LIMIT
        for name in names:
            if not PAYLOAD.fullmatch(name):
                continue
            data = read(base / name, remaining)
            if data is None or len(data) < 12 or data[:4] != b"\x07\x02\0\0" or not 12 <= int.from_bytes(data[4:8],"little") <= len(data):
                raise ValueError(f"Invalid Cars 3 progress file: {name}")
            remaining -= len(data)
            (body / name).write_bytes(data)
        if not (body / "P0").is_file():
            raise ValueError("The save has no player profile.")
        if IMPORTED_SLOT.fullmatch(slot):
            restore_trophies(staged)
        label = source.stem if source.suffix.lower() == ".zip" else base.name
        (staged / "name.txt").write_text(label, encoding="utf-8")
        staged.rename(slots / slot)


def import_save(profile, source):
    slot = uuid.uuid4().hex
    create_slot(profile, source, slot)
    write_selected(profile, slot)
    return slot


def export_save(profile, slot, destination):
    """Export a save slot as a re-importable PistonPort ZIP archive."""
    if slot in BUILTIN_SAVES and not (profile / "save-slots" / slot).exists():
        create_slot(profile, Path(__file__).with_name("saves") / BUILTIN_SAVES[slot], slot)
    root = slot_root(profile, slot)
    body = root / "savedata/1" / TITLE / TITLE
    if not (body / "P0").is_file():
        raise ValueError("This save slot has no saved progress to export yet.")
    meta = body.with_name(TITLE + ".sce_sys")
    param = meta / "param.bin"
    if not param.is_file() or param.stat().st_size != 1328:
        raise ValueError("Save metadata (param.bin) is missing for this save slot.")
    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    tmp = destination.with_name(f"{destination.name}.{os.getpid()}.tmp")
    try:
        with zipfile.ZipFile(tmp, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
            for item in sorted(body.iterdir()):
                if item.is_file() and PAYLOAD.fullmatch(item.name):
                    archive.write(item, f"{TITLE}/{item.name}")
            archive.write(param, f"{TITLE}.sce_sys/param.bin")
            icon = meta / "icon0.png"
            if icon.is_file():
                archive.write(icon, f"{TITLE}.sce_sys/icon0.png")
            trophies = root / "trophies.dat"
            if trophies.is_file() and trophies.stat().st_size == 16:
                archive.write(trophies, "trophies.dat")
        os.replace(tmp, destination)
    finally:
        tmp.unlink(missing_ok=True)
    return destination


if __name__ == "__main__":
    import msvcrt
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", type=Path, required=True)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--import-save", type=Path)
    action.add_argument("--select")
    action.add_argument("--export-save", nargs=2, metavar=("SLOT", "DEST"))
    args = parser.parse_args()
    try:
        profile = args.profile.resolve()
        profile.mkdir(parents=True, exist_ok=True)
        with (profile / ".pistonport-session.lock").open("a+b") as lock:
            lock.seek(0)
            try:
                msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
            except OSError:
                raise ValueError("Close the game before changing saves.")
            if args.import_save:
                print(import_save(profile, args.import_save.resolve()))
            elif args.export_save:
                slot_id, dest = args.export_save
                print(export_save(profile, slot_id, Path(dest).resolve()))
            else:
                select(profile, args.select)
    except (ValueError, OSError, zipfile.BadZipFile, RuntimeError, OverflowError) as e:
        sys.exit(str(e))
