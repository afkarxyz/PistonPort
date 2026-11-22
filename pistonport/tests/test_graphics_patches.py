"""Exercise graphics patching, ultrawide/FOV/LOD/intro patches, compat env, and crash reports."""
import math
import struct
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "launcher"))

import boot
import prepare as piston_prepare


def build_synthetic_probe_image():
    """Construct a minimal valid G3PROBE5 image with CUSA07027 patch sites populated."""
    image_size = 0xEA0000
    image = bytearray(image_size)

    w0 = struct.pack("<I", 1920)
    h0 = struct.pack("<I", 1080)
    wh0 = w0 + h0
    w0_16 = struct.pack("<H", 1920)
    h0_16 = struct.pack("<H", 1080)
    ar0_f = struct.pack("<f", 16.0 / 9.0)
    iar0_f = struct.pack("<f", 9.0 / 16.0)
    for offset, payload in (
        (0x4467EB, wh0),
        (0x5649C, wh0),
        (0x56DB8, w0),
        (0x56DBF, h0),
        (0xDE0370, wh0),
        (0xDE0380, wh0),
        (0x5A970, w0),
        (0x5A976, h0),
        (0x5A98B, w0),
        (0x5A991, h0),
        (0x8BD17, w0_16),
        (0x8BD1D, h0_16),
        (0x8BD3D, w0_16),
        (0x8BD58, h0_16),
        (0x564E2, ar0_f),
        (0xDE0958, iar0_f + ar0_f),
        (0xDE0968, iar0_f + ar0_f),
        (0xE84630, bytes.fromhex("00 00 00 20 c7 71 fc 3f")),
        (0x2EA99A, bytes.fromhex("76 48")),
        (0x36A487, struct.pack("<f", 1920.0)),
        (0x2EA3A7, bytes.fromhex("48 8d 05 7a 32 af 00")),
        (0x2EA3DC, bytes.fromhex("90 90 90 90")),
        (0x8B0562, bytes.fromhex("45 31 e4 4c 8d 7d 90")),
        (0x69F3E7, bytes.fromhex("81 fb b7 0b 00 00")),
        (0x8C7BF1, bytes.fromhex("e8 6a b6 fe ff")),
        (0xE32710, struct.pack("<f", 2048.0)),
        (0x7B1C9D, bytes.fromhex("74 0e")),
        (0x7BE789, bytes.fromhex("0f 84 7c 01 00 00")),
        (0x7CA4F4, bytes.fromhex("41 8b 4c 24 04")),
        (0x7CA52D, bytes.fromhex("41 8b 4c 24 04")),
        (0xE3C4AE, b'size="2.2G"'),
    ):
        image[offset : offset + len(payload)] = payload

    header = struct.pack("<8s6Q", b"G3PROBE5", image_size, 0, 0, 0, 0, 0)
    header += bytes(96 - len(header))
    header += struct.pack("<Q", 0)
    header += struct.pack("<Q", 0)
    return header + image, len(header)


class GraphicsPatchTest(unittest.TestCase):
    def test_resolutions_ultrawide_shadows_fov_intro_lod_and_heap(self):
        raw, image_start = build_synthetic_probe_image()
        cases = [
            ("720p", 1280, 720, 1024, False, 100, False, False, b'size="2.2G"'),
            ("1280x800", 1280, 800, 2048, False, 110, True, False, b'size="2.2G"'),
            ("native", 1920, 1080, 3072, True, 100, False, True, b'size="2.8G"'),
            ("3440x1440", 3440, 1440, 2048, False, 120, True, True, b'size="2.8G"'),
            ("5120x1440", 5120, 1440, 3072, False, 130, True, True, b'size="3.6G"'),
            ("4K", 3840, 2160, 4096, False, 100, False, False, b'size="3.6G"'),
        ]
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "base.bin"
            base.write_bytes(raw)
            for res, w, h, shadow, mb, fov, skip_intro, high_lod, expected_xml in cases:
                with self.subTest(res=res, shadow=shadow, fov=fov):
                    out = Path(tmp) / f"out-{res}-{shadow}-{fov}.bin"
                    piston_prepare.apply_graphics_patches(
                        base,
                        out,
                        resolution=res,
                        shadow_resolution=shadow,
                        motion_blur=mb,
                        fov_scale=fov,
                        skip_intro=skip_intro,
                        high_lod=high_lod,
                    )
                    patched = out.read_bytes()[image_start:]
                    wh = struct.pack("<II", w, h)
                    w_bytes = struct.pack("<I", w)
                    h_bytes = struct.pack("<I", h)
                    for off in (0x4467EB, 0x5649C, 0xDE0370, 0xDE0380):
                        self.assertEqual(patched[off : off + 8], wh)
                    for off_w, off_h in ((0x56DB8, 0x56DBF), (0x5A970, 0x5A976), (0x5A98B, 0x5A991)):
                        self.assertEqual(patched[off_w : off_w + 4], w_bytes)
                        self.assertEqual(patched[off_h : off_h + 4], h_bytes)
                    for off_w16, off_h16 in ((0x8BD17, 0x8BD1D), (0x8BD3D, 0x8BD58)):
                        self.assertEqual(struct.unpack_from("<H", patched, off_w16)[0], w)
                        self.assertEqual(struct.unpack_from("<H", patched, off_h16)[0], h)

                    aspect = w / h
                    self.assertAlmostEqual(
                        struct.unpack_from("<f", patched, 0x564E2)[0], aspect, places=5
                    )
                    self.assertAlmostEqual(
                        struct.unpack_from("<d", patched, 0xE84630)[0], aspect, places=5
                    )
                    if aspect > (16.0 / 9.0) + 1e-4:
                        self.assertEqual(patched[0x2EA99A : 0x2EA99C], bytes.fromhex("eb 48"))
                    else:
                        self.assertEqual(patched[0x2EA99A : 0x2EA99C], bytes.fromhex("76 48"))

                    self.assertEqual(
                        struct.unpack_from("<f", patched, 0xE32710)[0], float(shadow)
                    )
                    if fov != 100:
                        self.assertEqual(
                            patched[0x2EA3A7 : 0x2EA3AE], bytes.fromhex("48 8d 05 2e 00 00 00")
                        )
                        self.assertAlmostEqual(
                            struct.unpack_from("<f", patched, 0x2EA3DC)[0],
                            (math.pi / 180.0) * (fov / 100.0),
                            places=6,
                        )
                    else:
                        self.assertEqual(
                            patched[0x2EA3A7 : 0x2EA3AE], bytes.fromhex("48 8d 05 7a 32 af 00")
                        )

                    if skip_intro:
                        self.assertEqual(
                            patched[0x8B0562 : 0x8B0569], bytes.fromhex("e9 da 01 00 00 90 90")
                        )
                        self.assertEqual(
                            patched[0x69F3E7 : 0x69F3ED], bytes.fromhex("81 fb 00 00 00 00")
                        )
                    if high_lod:
                        self.assertEqual(
                            patched[0x8C7BF1 : 0x8C7BF6], bytes.fromhex("b0 01 90 90 90")
                        )
                    self.assertEqual(patched[0x7B1C9D : 0x7B1C9F], bytes.fromhex("eb 0e"))
                    self.assertEqual(patched[0xE3C4AE : 0xE3C4AE + 11], expected_xml)

    def test_unexpected_bytes_raise_value_error(self):
        raw, image_start = build_synthetic_probe_image()
        corrupt = bytearray(raw)
        corrupt[image_start + 0x7B1C9D] = 0x90
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "corrupt.bin"
            base.write_bytes(corrupt)
            with self.assertRaises(ValueError):
                piston_prepare.apply_graphics_patches(base, Path(tmp) / "out.bin")


class BootHelpersTest(unittest.TestCase):
    def test_amd_handheld_auto_compat_and_fsr_rcas(self):
        env, opts = boot.resolve_runtime_env(
            {
                "fps_limit": "60",
                "render_resolution": "720p",
                "fsr1": "1",
                "rcas": "1",
                "rcas_strength": "80",
                "glyph_style": "xbox",
                "vibration": "70",
            },
            base_env={},
            ram_gib=16.0,
            adapters=["AMD Radeon Graphics"],
        )
        self.assertTrue(opts["safe_mode"])
        self.assertEqual(env["SPARTAN_AMD_PATHS_OFF"], "all")
        self.assertEqual(env["SPARTAN_PIPELINE_BINARIES"], "0")
        self.assertEqual(env["SPARTAN_GUEST_COMMIT_ON_MAP"], "1")
        self.assertEqual(env["SPARTAN_FSR1"], "1")
        self.assertEqual(env["SPARTAN_RCAS"], "1")
        self.assertEqual(env["SPARTAN_ASYNC_SHADERS"], "1")
        self.assertEqual(env["SPARTAN_GLYPH_STYLE"], "xbox")
        self.assertEqual(env["SPARTAN_VIBRATION"], "70")

    def test_selected_gpu_and_ultrawide_dmem(self):
        env, opts = boot.resolve_runtime_env(
            {
                "fps_limit": "120",
                "render_resolution": "3440x1440",
                "shadow_resolution": "4096",
                "fov_scale": "120",
                "skip_intro": "1",
                "high_lod": "1",
                "gpu_id": "1",
                "monitor": "0",
                "compat_mode": "auto",
            },
            base_env={},
            ram_gib=32.0,
            adapters=["NVIDIA GeForce RTX 4070 SUPER", "AMD Radeon(TM) Graphics"],
        )
        # User explicitly picked GPU #1 (AMD), so auto compat engages safe mode!
        self.assertTrue(opts["safe_mode"])
        self.assertEqual(env["SPARTAN_GPU_ID"], "1")
        self.assertEqual(env["SPARTAN_MONITOR"], "0")
        self.assertEqual(env["SPARTAN_DMEM_MB"], "6144")
        self.assertEqual(opts["fov_scale"], 120)
        self.assertTrue(opts["skip_intro"])
        self.assertTrue(opts["high_lod"])

    def test_restore_launcher_keys_after_overlay_save(self):
        with tempfile.TemporaryDirectory() as tmp:
            ini = Path(tmp) / "pistonport.ini"
            ini.write_text("fps_limit=120\nshow_fps=0\n", encoding="utf-8")
            boot.restore_launcher_keys(
                ini,
                {
                    "shadow_resolution": "4096",
                    "compat_mode": "safe",
                    "fov_scale": "120",
                    "skip_intro": "1",
                    "high_lod": "1",
                    "gpu_id": "0",
                },
            )
            parsed = boot.parse_ini(ini.read_text(encoding="utf-8"))
            self.assertEqual(parsed["shadow_resolution"], "4096")
            self.assertEqual(parsed["fov_scale"], "120")
            self.assertEqual(parsed["skip_intro"], "1")
            self.assertEqual(parsed["high_lod"], "1")
            self.assertEqual(parsed["gpu_id"], "0")

    def test_variant_cache_and_pruning(self):
        raw, _ = build_synthetic_probe_image()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / "0123456789abcdef0123456789abcdef.bin"
            base.write_bytes(raw)
            created = []
            for res in ("720p", "900p", "native", "3440x1440"):
                for si in (False, True):
                    created.append(
                        boot.prepare_variant_image(base, res, 2048, False, 110, si, True)
                    )
            remaining = list(Path(tmp).glob("0123456789abcdef-v*.bin"))
            self.assertLessEqual(len(remaining), 6)
            self.assertIn(created[-1], remaining)

    def test_crash_report_and_extract_crash_lines(self):
        with tempfile.TemporaryDirectory() as tmp:
            prof = Path(tmp)
            (prof / "pistonport.ini").write_text("render_resolution=3440x1440\n", encoding="utf-8")
            (prof / "native-last.log").write_text(
                "PistonPort launch: res=3440x1440\n"
                "[Render.Vulkan] <Critical> Failed allocating buffer VK_ERROR_OUT_OF_DEVICE_MEMORY\n"
                "Assertion Failed!\n",
                encoding="utf-8",
            )
            report = boot.build_crash_report(prof)
            self.assertIn("PistonPort Diagnostic Report", report)
            self.assertIn("render_resolution=3440x1440", report)
            self.assertIn("VK_ERROR_OUT_OF_DEVICE_MEMORY", report)

    def test_version_parsing_and_update_comparison(self):
        self.assertEqual(boot.parse_version_tuple("v1.0.0"), (1, 0, 0))
        self.assertEqual(boot.parse_version_tuple("windows-v1.2.3-test1"), (1, 2, 3))
        self.assertGreater(boot.parse_version_tuple("v1.2.0"), boot.parse_version_tuple("1.1.0"))
        self.assertFalse(boot.parse_version_tuple("v1.0.0") > boot.parse_version_tuple("1.1.0"))


if __name__ == "__main__":
    unittest.main()
