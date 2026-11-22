"""Prepare and run a local CUSA07027 dump with the bundled Windows runtime."""
from pathlib import Path
import argparse
from contextlib import ExitStack, nullcontext
import ctypes
import hashlib
import json
import msvcrt
import os
import platform
import re
import struct
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request

HOME = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HOME / "tools"))
try:
    from piston_prepare import (
        BASE_DMEM_MB,
        apply_graphics_patches,
        parse_fov_scale,
        parse_resolution,
        parse_shadow_resolution,
        prepare,
        required_dmem_mb,
    )
except ModuleNotFoundError:
    sys.path.insert(0, str(HOME))
    from prepare import (
        BASE_DMEM_MB,
        apply_graphics_patches,
        parse_fov_scale,
        parse_resolution,
        parse_shadow_resolution,
        prepare,
        required_dmem_mb,
    )
try:
    from pkg_reader import Package, trophy_metadata
    from save_slots import TITLE, active_root, sfo_fields
except ModuleNotFoundError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from pkg_reader import Package, trophy_metadata
    from save_slots import TITLE, active_root, sfo_fields

PATCH_VARIANT_VERSION = "v3"
GITHUB_RELEASES_API = "https://api.github.com/repos/afkarxyz/PistonPort/releases/latest"
GITHUB_RELEASES_PAGE = "https://github.com/afkarxyz/PistonPort/releases"
VALID_FPS_LIMITS = {"0", "30", "60", "100", "120", "144", "180", "240"}
VALID_ANISOTROPIES = {"0", "2", "4", "8", "16"}
VALID_COMPAT_MODES = {"auto", "safe", "standard"}
VALID_GLYPH_STYLES = {"auto", "off", "xbox", "nintendo", "keyboard"}
LAUNCHER_PRESERVED_KEYS = (
    "shadow_resolution",
    "compat_mode",
    "gpu_id",
    "fov_scale",
    "skip_intro",
    "high_lod",
    "bloom",
)
CRASH_PREFIXES = ("STOP:", "ERROR:", "Fault:", "Runtime: cannot")
CRASH_MARKERS = (
    "<Critical>",
    "Assertion Failed",
    "Failed allocating",
    "VK_ERROR_",
    "Device lost",
)


def parse_ini(text):
    values = {}
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        key, sep, value = line.partition("=")
        if sep:
            values[key.strip()] = value.strip()
    return values


def restore_launcher_keys(config_path, saved_keys):
    """Restore launcher-only INI keys if the in-game overlay rewrote pistonport.ini."""
    if not saved_keys or not config_path.is_file():
        return
    text = config_path.read_text(encoding="utf-8-sig")
    current = parse_ini(text)
    missing = [(k, v) for k, v in saved_keys.items() if k not in current]
    if not missing:
        return
    if text and not text.endswith("\n"):
        text += "\n"
    for key, value in missing:
        text += f"{key}={value}\n"
    tmp = config_path.with_name(f"{config_path.name}.{os.getpid()}.tmp")
    tmp.write_text(text, encoding="utf-8")
    tmp.replace(config_path)


def total_ram_gib():
    if os.name != "nt":
        return 0.0

    class MemoryStatusEx(ctypes.Structure):
        _fields_ = [
            ("dwLength", ctypes.c_ulong),
            ("dwMemoryLoad", ctypes.c_ulong),
            ("ullTotalPhys", ctypes.c_ulonglong),
            ("ullAvailPhys", ctypes.c_ulonglong),
            ("ullTotalPageFile", ctypes.c_ulonglong),
            ("ullAvailPageFile", ctypes.c_ulonglong),
            ("ullTotalVirtual", ctypes.c_ulonglong),
            ("ullAvailVirtual", ctypes.c_ulonglong),
            ("ullAvailExtendedVirtual", ctypes.c_ulonglong),
        ]

    status = MemoryStatusEx()
    status.dwLength = ctypes.sizeof(MemoryStatusEx)
    try:
        if not ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status)):
            return 0.0
    except OSError:
        return 0.0
    return status.ullTotalPhys / (1024**3)


def display_adapters():
    if os.name != "nt":
        return []

    class DisplayDeviceW(ctypes.Structure):
        _fields_ = [
            ("cb", ctypes.c_ulong),
            ("DeviceName", ctypes.c_wchar * 32),
            ("DeviceString", ctypes.c_wchar * 128),
            ("StateFlags", ctypes.c_ulong),
            ("DeviceID", ctypes.c_wchar * 128),
            ("DeviceKey", ctypes.c_wchar * 128),
        ]

    enum_devices = ctypes.windll.user32.EnumDisplayDevicesW
    adapters = []
    seen = set()
    for index in range(32):
        device = DisplayDeviceW()
        device.cb = ctypes.sizeof(DisplayDeviceW)
        try:
            ok = enum_devices(None, index, ctypes.byref(device), 0)
        except OSError:
            break
        if not ok:
            break
        # Skip mirroring/pseudo display drivers (DISPLAY_DEVICE_MIRRORING_DRIVER = 0x8).
        if device.StateFlags & 0x8:
            continue
        name = device.DeviceString.strip()
        if name and name not in seen:
            seen.add(name)
            adapters.append(name)
    return adapters


def vulkan_gpus():
    """Return [(index, name), ...] in the exact order Vulkan / SPARTAN_GPU_ID uses."""
    if os.name != "nt":
        return []
    try:
        vk = ctypes.WinDLL("vulkan-1.dll")
        vk.vkCreateInstance.argtypes = [
            ctypes.c_void_p,
            ctypes.c_void_p,
            ctypes.POINTER(ctypes.c_void_p),
        ]
        vk.vkEnumeratePhysicalDevices.argtypes = [
            ctypes.c_void_p,
            ctypes.POINTER(ctypes.c_uint32),
            ctypes.c_void_p,
        ]
        vk.vkGetPhysicalDeviceProperties.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        vk.vkDestroyInstance.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    except (OSError, AttributeError):
        return []

    class VkInstanceCreateInfo(ctypes.Structure):
        _fields_ = [
            ("sType", ctypes.c_uint32),
            ("pNext", ctypes.c_void_p),
            ("flags", ctypes.c_uint32),
            ("pApplicationInfo", ctypes.c_void_p),
            ("enabledLayerCount", ctypes.c_uint32),
            ("ppEnabledLayerNames", ctypes.c_void_p),
            ("enabledExtensionCount", ctypes.c_uint32),
            ("ppEnabledExtensionNames", ctypes.c_void_p),
        ]

    ci = VkInstanceCreateInfo(sType=1)
    inst = ctypes.c_void_p()
    try:
        if vk.vkCreateInstance(ctypes.byref(ci), None, ctypes.byref(inst)) != 0 or not inst.value:
            return []
        try:
            count = ctypes.c_uint32(0)
            vk.vkEnumeratePhysicalDevices(inst, ctypes.byref(count), None)
            if count.value == 0 or count.value > 16:
                return []
            devs = (ctypes.c_void_p * count.value)()
            vk.vkEnumeratePhysicalDevices(inst, ctypes.byref(count), devs)
            buf = ctypes.create_string_buffer(1024)
            names = []
            for i in range(count.value):
                vk.vkGetPhysicalDeviceProperties(devs[i], buf)
                name = bytes(buf[20:276]).split(b"\0", 1)[0].decode("utf-8", "replace").strip()
                if name:
                    names.append((i, name))
            return names
        finally:
            vk.vkDestroyInstance(inst, None)
    except OSError:
        return []


def list_monitors():
    """Return [(index, label), ...] for active displays on Windows."""
    if os.name != "nt":
        return []

    class Rect(ctypes.Structure):
        _fields_ = [
            ("left", ctypes.c_long),
            ("top", ctypes.c_long),
            ("right", ctypes.c_long),
            ("bottom", ctypes.c_long),
        ]

    class MonitorInfoExW(ctypes.Structure):
        _fields_ = [
            ("cbSize", ctypes.c_ulong),
            ("rcMonitor", Rect),
            ("rcWork", Rect),
            ("dwFlags", ctypes.c_ulong),
            ("szDevice", ctypes.c_wchar * 32),
        ]

    monitors = []
    cb_type = ctypes.WINFUNCTYPE(
        ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p, ctypes.POINTER(Rect), ctypes.c_void_p
    )

    def _cb(hmon, _hdc, _rect, _data):
        info = MonitorInfoExW()
        info.cbSize = ctypes.sizeof(MonitorInfoExW)
        if ctypes.windll.user32.GetMonitorInfoW(ctypes.c_void_p(hmon), ctypes.byref(info)):
            w = info.rcMonitor.right - info.rcMonitor.left
            h = info.rcMonitor.bottom - info.rcMonitor.top
            primary = bool(info.dwFlags & 1)
            idx = len(monitors)
            suffix = " · Primary" if primary else ""
            monitors.append((idx, f"Display {idx + 1} ({w}×{h}{suffix})"))
        return 1

    try:
        ctypes.windll.user32.GetMonitorInfoW.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        ctypes.windll.user32.EnumDisplayMonitors(None, None, cb_type(_cb), 0)
    except OSError:
        return []
    return monitors


def is_amd_only_system(adapters=None):
    if adapters is None:
        vk = [name for _, name in vulkan_gpus()]
        source = vk if vk else display_adapters()
    else:
        source = adapters
    names = [name.lower() for name in source]
    has_amd = any(("amd" in name or "radeon" in name) for name in names)
    has_nvidia = any(
        ("nvidia" in name or "geforce" in name or "quadro" in name) for name in names
    )
    return has_amd and not has_nvidia


def _clamp_int(value, default, low, high):
    try:
        parsed = int(str(value).strip())
    except (TypeError, ValueError):
        return default
    return max(low, min(high, parsed))


def resolve_runtime_env(values, base_env=None, *, ram_gib=None, adapters=None):
    rate = values.get("fps_limit", "60")
    if rate not in VALID_FPS_LIMITS:
        rate = "60"
    res_key, _ = parse_resolution(values.get("render_resolution", "native"))
    shadow = parse_shadow_resolution(values.get("shadow_resolution", "2048"))
    fov_scale = parse_fov_scale(values.get("fov_scale", "100"))
    motion_blur = values.get("motion_blur", "0") == "1"
    bloom = values.get("bloom", "1") != "0"
    skip_intro = values.get("skip_intro", "0") == "1"
    high_lod = values.get("high_lod", "0") == "1"
    anisotropy = values.get("anisotropy", "16")
    if anisotropy not in VALID_ANISOTROPIES:
        anisotropy = "16"
    compat_mode = values.get("compat_mode", "auto").lower()
    if compat_mode not in VALID_COMPAT_MODES:
        compat_mode = "auto"
    fsr1 = values.get("fsr1", "0") == "1"
    rcas = values.get("rcas", "0") == "1"
    rcas_strength = _clamp_int(values.get("rcas_strength", "75"), 75, 0, 100)
    async_shaders = values.get("async_shaders", "1") != "0"
    vibration = _clamp_int(values.get("vibration", "100"), 100, 0, 100)
    glyph_style = values.get("glyph_style", "auto").lower()
    if glyph_style not in VALID_GLYPH_STYLES:
        glyph_style = "auto"
    gpu_id = values.get("gpu_id", "auto").strip().lower()
    if not gpu_id.isdigit():
        gpu_id = "auto"
    monitor = values.get("monitor", "").strip()
    if not monitor.isdigit():
        monitor = ""

    # If the user explicitly chose a GPU adapter index, evaluate AMD auto-compat
    # against that chosen GPU instead of all adapters in the system.
    effective_adapters = adapters
    if gpu_id.isdigit():
        idx = int(gpu_id)
        if adapters is not None:
            if 0 <= idx < len(adapters):
                effective_adapters = [adapters[idx]]
        else:
            vk = dict(vulkan_gpus())
            if idx in vk:
                effective_adapters = [vk[idx]]

    dmem_mb = required_dmem_mb(res_key, shadow)
    ram = total_ram_gib() if ram_gib is None else ram_gib
    low_ram = 0 < ram <= 20.0
    safe_mode = compat_mode == "safe" or (
        compat_mode == "auto" and is_amd_only_system(effective_adapters)
    )

    env = dict(os.environ if base_env is None else base_env)
    env["SPARTAN_FPS_LIMIT"] = rate
    env["SPARTAN_VBLANK_HZ"] = "60"
    env.setdefault("SPARTAN_ANISOTROPY", anisotropy)
    env.setdefault("SPARTAN_MOTION_BLUR", "1" if motion_blur else "0")
    env.setdefault("SPARTAN_FSR1", "1" if fsr1 else "0")
    env.setdefault("SPARTAN_RCAS", "1" if rcas else "0")
    env.setdefault("SPARTAN_ASYNC_SHADERS", "1" if async_shaders else "0")
    env.setdefault("SPARTAN_VIBRATION", str(vibration))
    env.setdefault("SPARTAN_GLYPH_STYLE", glyph_style)
    if gpu_id.isdigit():
        env.setdefault("SPARTAN_GPU_ID", gpu_id)
    if monitor.isdigit():
        env.setdefault("SPARTAN_MONITOR", monitor)
    if dmem_mb > BASE_DMEM_MB:
        env.setdefault("SPARTAN_DMEM_MB", str(dmem_mb))
    if dmem_mb > BASE_DMEM_MB or safe_mode or low_ram:
        env.setdefault("SPARTAN_GUEST_COMMIT_ON_MAP", "1")
    if safe_mode:
        env.setdefault("SPARTAN_AMD_PATHS_OFF", "all")
        env.setdefault("SPARTAN_PIPELINE_BINARIES", "0")
        # Avoid SDL3 WGI / RawInput / DirectInput joystick crashes on Windows 11 + AMD handhelds.
        env.setdefault("SDL_JOYSTICK_WGI", "0")
        env.setdefault("SDL_JOYSTICK_RAWINPUT", "0")
        env.setdefault("SDL_JOYSTICK_DIRECTINPUT", "0")
        # On AMD handhelds (unified <=20 GiB RAM, e.g. ROG Ally Z1E) or explicit Safe mode,
        # also disable HIDAPI so SDL3 uses pure XInput for the embedded controller.
        if low_ram or compat_mode == "safe":
            env.setdefault("SDL_JOYSTICK_HIDAPI", "0")

    options = {
        "fps_limit": rate,
        "resolution": res_key,
        "shadow_resolution": shadow,
        "fov_scale": fov_scale,
        "motion_blur": motion_blur,
        "bloom": bloom,
        "skip_intro": skip_intro,
        "high_lod": high_lod,
        "anisotropy": anisotropy,
        "compat_mode": compat_mode,
        "safe_mode": safe_mode,
        "dmem_mb": dmem_mb,
        "fsr1": fsr1,
        "rcas": rcas,
        "rcas_strength": rcas_strength,
        "async_shaders": async_shaders,
        "vibration": vibration,
        "glyph_style": glyph_style,
        "gpu_id": gpu_id,
        "monitor": monitor,
    }
    return env, options


def prepare_variant_image(
    base_image,
    resolution,
    shadow_resolution,
    motion_blur,
    fov_scale=100,
    skip_intro=False,
    high_lod=False,
    bloom=True,
):
    prefix = base_image.stem[:16]
    name = (
        f"{prefix}-{PATCH_VARIANT_VERSION}-{resolution}"
        f"-s{shadow_resolution}-mb{int(bool(motion_blur))}-bl{int(bool(bloom))}"
        f"-fov{fov_scale}-si{int(bool(skip_intro))}-lod{int(bool(high_lod))}.bin"
    )
    variant = base_image.parent / name
    base_stat = base_image.stat()
    if (
        variant.is_file()
        and variant.stat().st_size == base_stat.st_size
        and variant.stat().st_mtime >= base_stat.st_mtime
    ):
        try:
            os.utime(variant, None)
        except OSError:
            pass
    else:
        apply_graphics_patches(
            base_image,
            variant,
            resolution=resolution,
            shadow_resolution=shadow_resolution,
            motion_blur=motion_blur,
            fov_scale=fov_scale,
            skip_intro=skip_intro,
            high_lod=high_lod,
            bloom=bloom,
        )
    variants = sorted(
        (
            path
            for path in base_image.parent.glob(f"{prefix}-v*.bin")
            if path != variant and path != base_image
        ),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    for stale in variants[5:]:
        stale.unlink(missing_ok=True)
    return variant


def parse_version_tuple(tag):
    cleaned = str(tag or "").strip()
    for prefix in ("windows-v", "linux-v", "v", "V"):
        if cleaned.startswith(prefix):
            cleaned = cleaned[len(prefix) :]
            break
    cleaned = cleaned.split("-", 1)[0]
    parts = []
    for piece in cleaned.split("."):
        if piece.isdigit():
            parts.append(int(piece))
        else:
            break
    while len(parts) < 3:
        parts.append(0)
    return tuple(parts[:3])


def check_github_update(current_version="1.1.0", api_url=None):
    url = api_url or os.environ.get("PISTONPORT_UPDATE_API_URL") or GITHUB_RELEASES_API
    req = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "User-Agent": f"PistonPort-Launcher/{current_version}",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=6) as response:
            payload = json.loads(response.read().decode("utf-8", "replace"))
    except (urllib.error.URLError, TimeoutError, ValueError, OSError) as error:
        raise ValueError(f"Could not check GitHub Releases: {error}") from error

    tag = str(payload.get("tag_name") or "").strip()
    if not tag:
        raise ValueError("GitHub Releases response is missing tag_name.")
    html_url = str(payload.get("html_url") or GITHUB_RELEASES_PAGE).strip()
    if not html_url.startswith("https://github.com/"):
        html_url = GITHUB_RELEASES_PAGE
    name = str(payload.get("name") or tag).strip().replace("\t", " ").replace("\n", " ")
    available = parse_version_tuple(tag) > parse_version_tuple(current_version)
    return {
        "available": available,
        "tag": tag,
        "url": html_url,
        "name": name,
    }


def inspect_update(current_version):
    info = check_github_update(current_version)
    line = f"UPDATE\t{int(info['available'])}\t{info['tag']}\t{info['url']}\t{info['name']}\n"
    sys.stdout.buffer.write(line.encode("utf-8"))
    return 0


def inspect_hardware():
    gpus = vulkan_gpus()
    if not gpus:
        gpus = list(enumerate(display_adapters()))
    lines = [f"GPU\t{idx}\t{name}" for idx, name in gpus]
    lines.extend(f"MON\t{idx}\t{label}" for idx, label in list_monitors())
    sys.stdout.buffer.write(("\n".join(lines) + ("\n" if lines else "")).encode("utf-8"))
    return 0


def build_crash_report(profile):
    """Build a concise diagnostic report for sharing on Reddit / GitHub issues."""
    gpus = vulkan_gpus()
    gpu_text = ", ".join(f"[{idx}] {name}" for idx, name in gpus) if gpus else ", ".join(display_adapters()) or "Unknown"
    ram = total_ram_gib()
    ini_path = profile / "pistonport.ini"
    log_path = profile / "native-last.log"
    lines = [
        "=== PistonPort Diagnostic Report ===",
        f"OS: {platform.platform()}",
        f"RAM: {ram:.1f} GiB" if ram > 0 else "RAM: Unknown",
        f"GPUs: {gpu_text}",
    ]
    if ini_path.is_file():
        ini_text = ini_path.read_text(encoding="utf-8-sig", errors="replace").strip()
        lines.append("")
        lines.append("--- pistonport.ini ---")
        lines.append(ini_text)
    if log_path.is_file():
        log_lines = log_path.read_text(encoding="utf-8", errors="replace").splitlines()
        crash = extract_crash_lines(log_lines, limit=16)
        if crash:
            lines.append("")
            lines.append("--- Detected Errors ---")
            lines.extend(crash)
        lines.append("")
        lines.append("--- native-last.log (tail) ---")
        lines.extend(log_lines[-40:])
    else:
        lines.append("")
        lines.append("(No native-last.log found yet.)")
    return "\n".join(lines) + "\n"


def extract_crash_lines(lines, limit=12):
    found = []
    for raw in lines:
        line = raw.strip()
        if not line:
            continue
        if line.startswith(CRASH_PREFIXES) or any(marker in line for marker in CRASH_MARKERS):
            if not found or found[-1] != line:
                found.append(line)
            if len(found) >= limit:
                break
    return found


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
    args.add_argument("--game", type=Path)
    args.add_argument("--profile", type=Path)
    args.add_argument("--current-version", default="1.1.0")
    action = args.add_mutually_exclusive_group()
    action.add_argument("--prepare-only", action="store_true")
    action.add_argument("--inspect-source", action="store_true")
    action.add_argument("--source-info", action="store_true")
    action.add_argument("--inspect-trophies", action="store_true")
    action.add_argument("--list-hardware", action="store_true")
    action.add_argument("--crash-report", action="store_true")
    action.add_argument("--check-update", action="store_true")
    opt = args.parse_args()
    if opt.list_hardware:
        return inspect_hardware()
    if opt.check_update:
        return inspect_update(opt.current_version)
    if opt.crash_report:
        if opt.profile is None:
            args.error("--profile is required for --crash-report")
        report = build_crash_report(opt.profile.resolve())
        sys.stdout.buffer.write(report.encode("utf-8"))
        return 0
    if opt.game is None:
        args.error("--game is required")
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
            config.write_bytes(
                legacy.read_bytes()
                if legacy.is_file()
                else b"fps_limit=60\nshow_fps=1\nanisotropy=16\nmotion_blur=0\nasync_shaders=1\n"
            )
        values = parse_ini(config.read_text(encoding="utf-8-sig"))
        preserved = {k: values[k] for k in LAUNCHER_PRESERVED_KEYS if k in values}
        env, options = resolve_runtime_env(values, os.environ)
        env.pop("PISTONPORT_PKG", None)
        if pkg_path:
            env["PISTONPORT_PKG"] = str(pkg_path)
            # The four link inputs are no longer needed. Game assets are read
            # through the package mount in the child for the entire session.
            for name in ("eboot.bin", "sce_module/libc.prx", "sce_module/libSceFios2.prx", "sce_module/libSceNpToolkit2.prx"):
                (game / name).unlink(missing_ok=True)
        image = prepare_variant_image(
            image,
            options["resolution"],
            options["shadow_resolution"],
            options["motion_blur"],
            fov_scale=options["fov_scale"],
            skip_intro=options["skip_intro"],
            high_lod=options["high_lod"],
            bloom=options["bloom"],
        )
        env["SPARTAN_CONFIG"] = str(config)
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
            summary = (
                f"PistonPort launch: res={options['resolution']} "
                f"shadow={options['shadow_resolution']} "
                f"fov={options['fov_scale']}% "
                f"motion_blur={'on' if options['motion_blur'] else 'off'} "
                f"skip_intro={int(options['skip_intro'])} "
                f"high_lod={int(options['high_lod'])} "
                f"af={options['anisotropy']}x "
                f"fsr1={int(options['fsr1'])} "
                f"rcas={int(options['rcas'])}({options['rcas_strength']}%) "
                f"async_shaders={int(options['async_shaders'])} "
                f"gpu={options['gpu_id']} "
                f"compat={options['compat_mode']}({'safe' if options['safe_mode'] else 'standard'}) "
                f"dmem={options['dmem_mb']}MiB "
                f"commit_on_map={env.get('SPARTAN_GUEST_COMMIT_ON_MAP', '0')}\n"
            )
            stream.write(summary.encode("utf-8"))
            stream.flush()
            try:
                trp = read_trophy_pack(game, package)
                trophy_text.write_bytes(trophy_metadata(trp, HOME / "bin/piston_pkg.dll"))
            except (ValueError, OSError, AttributeError) as error:
                stream.write(f"Trophy metadata unavailable: {error}\n".encode("utf-8"))
                stream.flush()
            result = subprocess.run(cmd, env=env, cwd=profile, stdout=stream, stderr=subprocess.STDOUT)
        restore_launcher_keys(config, preserved)
        if result.returncode:
            with log.open(errors="replace") as stream:
                for line in extract_crash_lines(stream):
                    print(line, file=sys.stderr)
            print(f"Game log: {log}", file=sys.stderr)
        return result.returncode


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
