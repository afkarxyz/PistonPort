"""Stage the Windows bundle. Game-derived files are regenerated on the player's PC."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile

here = Path(__file__).resolve().parent
project = here.parent
repo = project.parent
outputs = repo / "dist" / "PistonPort"
work = Path(os.environ.get("PISTONPORT_WORK", repo / "out/work"))
ps4 = work / "ps4"
game = Path(os.environ.get("PISTONPORT_GAME", ps4 / "CUSA07027"))
runtime = outputs / "PistonPort.Runtime"


def copy(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)


def tree(source, target):
    shutil.copytree(source, target, dirs_exist_ok=True,
                    ignore=shutil.ignore_patterns("__pycache__", "*.pyc", ".git"))


def sha(file):
    with file.open("rb") as f:
        h = hashlib.sha256()
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
        return h.hexdigest()


def cargo_licenses(destination, manifest, toolchain=()):
    metadata = json.loads(subprocess.check_output([
        "cargo", *toolchain, "metadata", "--locked", "--format-version=1", "--filter-platform",
        "x86_64-pc-windows-msvc", "--manifest-path", str(manifest)
    ]))
    notices = []
    for dep in metadata["packages"]:
        if not dep["source"]:
            continue
        name = dep["name"] + "-" + dep["version"]
        notices.append(f"{name} | {dep['license']} | {dep.get('repository') or ''}")
        directory = Path(dep["manifest_path"]).parent
        for file in directory.iterdir():
            if file.is_file() and file.name.upper().startswith(("LICENSE", "COPYING", "NOTICE")):
                copy(file, destination / name / file.name)
        if dep["name"] == "rune":
            for filename in ("LICENSE", "THIRD_PARTY_NOTICES.md"):
                copy(directory.parent.parent / filename, destination / f"Rune-{filename}")
    (destination / "DEPENDENCIES.txt").write_text("\n".join(notices) + "\n", encoding="utf-8")


def stage_licenses(runtime):
    licenses = runtime / "Licenses"
    tree(ps4 / "build-tools/clang64/share/licenses", licenses / "MSYS2")
    tree(ps4 / "spartan-tools/spartanport-windows/LICENSES", licenses / "SpartanPort")
    for file, name in (
        (repo / "LICENSE", "SpartanPort-GPL.txt"),
        (project / "LICENSE.txt", "shadPS4-GPL.txt"),
        (here / "lucide/LICENSE", "Lucide.txt"),
        (repo / "launcher-rust/assets/pads/LICENSE.txt", "Controller-art.txt"),
        (repo / "gpu/third_party/fonts/LICENSE-DejaVu.txt", "DejaVu.txt"),
        (repo / "gpu/third_party/imgui/LICENSE.txt", "Dear-ImGui.txt"),
    ):
        copy(file, licenses / name)
    for file in (ps4 / "python-libs/pyelftools-0.32.dist-info").rglob("*"):
        if file.is_file() and "LICENSE" in file.name.upper():
            copy(file, licenses / ("pyelftools-" + file.name))
    cargo_licenses(licenses / "PKG-reader", project / "pkg/Cargo.toml")
    cargo_licenses(licenses / "Rust-launcher", repo / "launcher-rust/pistonport/Cargo.toml", ("+1.99.1",))


def stage(runtime):
    shutil.copytree(
        ps4 / "piston-runtime",
        runtime / "bin",
        dirs_exist_ok=True,
        ignore=shutil.ignore_patterns("__pycache__", "*.pyc", ".git", "vulkan-1.dll", "VULKAN-1.DLL"),
    )
    tree(ps4 / "piston-package/python", runtime / "python")
    (runtime / "python/python313._pth").write_text(
        "python313.zip\n.\n../tools\n../upstream/scripts\n../lib\n", encoding="ascii")
    tree(ps4 / "python-libs/elftools", runtime / "lib/elftools")
    copy(project / "prepare.py", runtime / "tools/piston_prepare.py")
    copy(project / "fibers.py", runtime / "tools/fibers.py")
    copy(here / "boot.py", runtime / "tools/boot.py")
    copy(here / "pkg_reader.py", runtime / "tools/pkg_reader.py")
    copy(here / "save_slots.py", runtime / "tools/save_slots.py")
    copy(here / "save_trophies.py", runtime / "tools/save_trophies.py")
    tree(here / "saves", runtime / "tools/saves")
    copy(ps4 / "native/cars3-runtime.elf", runtime / "tools/cars3-runtime.elf")
    for script in ("prepare.py", "link_modules.py", "link_libc.py"):
        copy(repo / "scripts" / script, runtime / "upstream/scripts" / script)
    inputs = ("eboot.bin", "sce_module/libc.prx", "sce_module/libSceFios2.prx",
              "sce_module/libSceNpToolkit2.prx")
    manifest = {"inputs": {key: sha(game / key) for key in inputs},
                "image_sha256": sha(ps4 / "native/cars3-native.bin")}
    (runtime / "image.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")


def archive(runtime):
    archive_path = outputs.parent / "PistonPort-Windows-x64.zip"
    temp = archive_path.with_suffix(".zip.tmp")
    with zipfile.ZipFile(temp, "w", zipfile.ZIP_DEFLATED, compresslevel=6, strict_timestamps=False) as z:
        z.write(outputs / "PistonPort.exe", "PistonPort/PistonPort.exe")
        for file in sorted(runtime.rglob("*")):
            if file.is_file():
                z.write(file, "PistonPort/PistonPort.Runtime/" + file.relative_to(runtime).as_posix())
    temp.replace(archive_path)
    print(archive_path, archive_path.stat().st_size)


if __name__ == "__main__":
    if "--stage-only" not in sys.argv:
        with tempfile.TemporaryDirectory() as folder:
            staged = Path(folder) / runtime.name
            stage(staged)
            stage_licenses(staged)
            archive(staged)
            tree(staged, runtime)
    else:
        stage(runtime)
