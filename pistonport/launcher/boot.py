"""Prepare and run a local CUSA07027 dump with the bundled Windows runtime."""
from pathlib import Path
import argparse
from contextlib import ExitStack, nullcontext
import hashlib
import json
import msvcrt
import os
import re
import struct
import subprocess
import sys
import tempfile

HOME = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HOME / "tools"))
from piston_prepare import prepare
from pkg_reader import Package, trophy_metadata
from save_slots import TITLE, active_root, sfo_fields


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def read_trophy_pack(game, package):
    name = "sce_sys/trophy/trophy00.trp"
    limit = 32 * 1024**2
    if package:
        return b"".join(package.chunks(name, limit))
    else:
        with (game / name).open("rb") as source:
            trp = source.read(limit + 1)
    if len(trp) > limit:
        raise ValueError("The trophy pack exceeds the size limit.")
    return trp


def inspect_trophies(game):
    library = HOME / "bin/piston_pkg.dll"
    with ExitStack() as resources:
        package = None
        if game.suffix.lower() == ".pkg":
            package = resources.enter_context(Package(game, library))
        trp = read_trophy_pack(game, package)
        texts = trophy_metadata(trp, library)
        sys.stdout.buffer.write(struct.pack("<I", len(texts)))
        sys.stdout.buffer.write(texts)
        sys.stdout.buffer.write(trp)
    return 0


def inspect_source(game, include_info=False):
    """Validate the launcher selection and return optional cover art on stdout."""
    expected = json.loads((HOME / "image.json").read_text(encoding="utf-8"))["inputs"]["eboot.bin"]
    if game.suffix.lower() == ".pkg":
        if not game.is_file():
            raise ValueError("The selected PKG file could not be found.")
        with Package(game, HOME / "bin/piston_pkg.dll") as package:
            sfo = b"".join(package.chunks("sce_sys/param.sfo", 512 * 1024))
            h = hashlib.sha256()
            for chunk in package.chunks("eboot.bin", 64 * 1024**2):
                h.update(chunk)
            actual = h.hexdigest()
            try:
                art = b"".join(package.chunks("sce_sys/pic1.png", 8 * 1024**2))
            except ValueError:
                art = b""
    else:
        if not ((game / "eboot.bin").is_file() and (game / "sce_sys/param.sfo").is_file()
                and (game / "assets").is_dir() and (game / "bundles").is_dir()):
            raise ValueError("Choose the game folder containing eboot.bin, assets, bundles and sce_sys.")
        actual = digest(game / "eboot.bin")
        with (game / "sce_sys/param.sfo").open("rb") as metadata:
            sfo = metadata.read(512 * 1024 + 1)
        if len(sfo) > 512 * 1024:
            raise ValueError("The PS4 metadata exceeds the size limit.")
        cover = game / "sce_sys/pic1.png"
        art = cover.read_bytes() if cover.is_file() and cover.stat().st_size <= 8 * 1024**2 else b""
    fields = sfo_fields(sfo)
    title_id = fields.get(b"TITLE_ID", b"").rstrip(b"\0")
    if not re.fullmatch(rb"CUSA[0-9]{5}", title_id):
        raise ValueError("The game metadata is missing a valid PS4 Title ID.")
    versions = (fields.get(key, b"").rstrip(b"\0") for key in (b"APP_VER", b"VERSION"))
    version = next((value for value in versions if re.fullmatch(rb"[0-9]{2}\.[0-9]{2}", value)), b"")
    if actual != expected or title_id != TITLE.encode("ascii"):
        detected = title_id.decode("ascii") + (f" v{version.decode('ascii')}" if version else "")
        raise ValueError(f"Detected {detected}. This release supports Cars 3: Driven to Win, CUSA07027 v01.00.")
    if include_info:
        sys.stdout.buffer.write(title_id + b"\t" + version + b"\n")
    sys.stdout.buffer.write(art)
    return 0


def build_image(game, profile, package=None):
    manifest = json.loads((HOME / "image.json").read_text(encoding="utf-8"))
    cache = profile / "cache" / "images"
    cache.mkdir(parents=True, exist_ok=True)
    target = cache / (manifest["image_sha256"] + ".bin")
    cached = target.is_file() and digest(target) == manifest["image_sha256"]
    for name, expected in manifest["inputs"].items():
        file = game / name
        if package:
            h = hashlib.sha256()
            file.parent.mkdir(exist_ok=True, parents=True)
            with (nullcontext(None) if cached else file.open("wb")) as dest:
                for chunk in package.chunks(name, 64 * 1024**2):
                    h.update(chunk)
                    if dest: dest.write(chunk)
            actual = h.hexdigest()
        else:
            actual = digest(file) if file.is_file() else None
        if actual != expected:
            raise ValueError(f"Unsupported or incomplete game ({name}). Expected CUSA07027 v1.00.")
    for directory in ("assets", "bundles"):
        present = package.lookup(directory)[1].directory if package else (game / directory).is_dir()
        if not present:
            raise ValueError(f"The selected game is missing {directory}.")
    if package:
        # The host consumes param.sfo through its ordinary file API at startup.
        sysdir = game / "sce_sys"
        sysdir.mkdir(exist_ok=True)
        for name in ("param.sfo", "icon0.png"):
            (sysdir / name).write_bytes(b"".join(package.chunks("sce_sys/" + name, 8 * 1024**2)))
    if cached:
        return target
    with tempfile.TemporaryDirectory(dir=cache) as tmp:
        tmp = Path(tmp)
        prepare(game, HOME / "upstream", None, tmp, None, prebuilt=HOME / "tools" / "cars3-runtime.elf")
        image = tmp / "cars3-native.bin"
        if digest(image) != manifest["image_sha256"]:
            raise ValueError("The prepared image does not match this build. Check the dump and extract the entire PistonPort ZIP.")
        os.replace(image, target)
    return target


def main():
    args = argparse.ArgumentParser()
    args.add_argument("--game", type=Path, required=True)
    args.add_argument("--profile", type=Path)
    action = args.add_mutually_exclusive_group()
    action.add_argument("--prepare-only", action="store_true")
    action.add_argument("--inspect-source", action="store_true")
    action.add_argument("--source-info", action="store_true")
    action.add_argument("--inspect-trophies", action="store_true")
    opt = args.parse_args()
    if opt.inspect_source or opt.source_info:
        return inspect_source(opt.game.resolve(), opt.source_info)
    if opt.inspect_trophies:
        return inspect_trophies(opt.game.resolve())
    if opt.profile is None:
        args.error("--profile is required to play or prepare the game")
    game, profile = opt.game.resolve(), opt.profile.resolve()
    profile.mkdir(parents=True, exist_ok=True)
    with ExitStack() as resources:
        session = resources.enter_context((profile / ".pistonport-session.lock").open("a+b"))
        session.seek(0)
        try:
            msvcrt.locking(session.fileno(), msvcrt.LK_NBLCK, 1)
        except OSError:
            raise ValueError("PistonPort is already using this profile.")
        pkg_path = None
        package = None
        if game.is_file() and game.suffix.lower() == ".pkg":
            pkg_path = game
            package = resources.enter_context(Package(game, HOME / "bin/piston_pkg.dll"))
            game = Path(resources.enter_context(tempfile.TemporaryDirectory(prefix="pkg-session-", dir=profile)))
        image = build_image(game, profile, package)
        if opt.prepare_only:
            return 0
        config = profile / "pistonport.ini"
        if not config.exists():
            legacy = profile / "spartanport.ini"
            config.write_bytes(legacy.read_bytes() if legacy.is_file() else b"fps_limit=60\nshow_fps=1\n")
        values = {}
        for line in config.read_text(encoding="utf-8-sig").splitlines():
            key, sep, value = line.partition("=")
            if sep:
                values[key.strip()] = value.strip()
        rate = values.get("fps_limit", "60")
        if rate not in {"0", "30", "60", "100", "120", "144", "180", "240"}:
            rate = "60"
        env = os.environ.copy()
        env.pop("PISTONPORT_PKG", None)
        if pkg_path:
            env["PISTONPORT_PKG"] = str(pkg_path)
            # The four link inputs are no longer needed. Game assets are read
            # through the package mount in the child for the entire session.
            for name in ("eboot.bin", "sce_module/libc.prx", "sce_module/libSceFios2.prx", "sce_module/libSceNpToolkit2.prx"):
                (game / name).unlink(missing_ok=True)
        env.update(SPARTAN_CONFIG=str(config), SPARTAN_FPS_LIMIT=rate, SPARTAN_VBLANK_HZ="60")
        mapping = profile / "controller.map"
        if mapping.is_file():
            env["SPARTAN_PAD_MAP"] = mapping.read_text(encoding="utf-8-sig").strip()
        progress = active_root(profile)
        trophy_dir = game if package else Path(resources.enter_context(tempfile.TemporaryDirectory(dir=profile)))
        trophy_text = trophy_dir / "trophy_metadata.txt"
        env["PISTONPORT_TROPHY_METADATA"] = str(trophy_text)
        meta = progress / "savedata/1/CUSA07027/CUSA07027.sce_sys"
        cmd = [HOME / "bin/PistonPort.Game.exe", image, "--app0", game,
               "--user", progress, "--mount", f"/cars3meta={meta}"]
        log = profile / "native-last.log"
        with log.open("wb") as stream:
            try:
                trp = read_trophy_pack(game, package)
                trophy_text.write_bytes(trophy_metadata(trp, HOME / "bin/piston_pkg.dll"))
            except (ValueError, OSError, AttributeError) as error:
                stream.write(f"Trophy metadata unavailable: {error}\n".encode("utf-8"))
                stream.flush()
            result = subprocess.run(cmd, env=env, cwd=profile, stdout=stream, stderr=subprocess.STDOUT)
        if result.returncode:
            with log.open(errors="replace") as stream:
                for line in stream:
                    if line.startswith(("STOP:", "ERROR:", "Fault:")):
                        print(line.rstrip(), file=sys.stderr)
            print(f"Game log: {log}", file=sys.stderr)
        return result.returncode


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
