"""Exercise native save import and the separation of progress slots."""
import struct
import sys
import tempfile
import unittest
import zipfile
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "launcher"))
import save_slots as slots


class SaveSlotTest(unittest.TestCase):
    def test_builtin_is_seeded_once_and_resumes_after_switching_slots(self):
        for slot, filename in slots.BUILTIN_SAVES.items():
            with self.subTest(slot=slot), tempfile.TemporaryDirectory() as tmp:
                self.check_builtin(Path(tmp), slot, filename)

    def check_builtin(self, profile, slot, filename):
        seed_path = Path(slots.__file__).with_name("saves") / filename
        self.assertEqual(slots.active_root(profile), profile)
        slots.select(profile, slot)
        builtin = slots.active_root(profile)
        player = Path("savedata/1") / slots.TITLE / slots.TITLE / "P0"
        with zipfile.ZipFile(seed_path) as seed:
            initial = seed.read(slots.TITLE + "/P0")
            trophies = seed.read("trophies.dat")
        self.assertEqual((builtin / player).read_bytes(), initial)
        self.assertEqual((builtin / "trophies.dat").read_bytes(), trophies)
        imported_zip = slots.import_save(profile, seed_path)
        self.assertEqual((slots.active_root(profile) / "trophies.dat").read_bytes(), trophies)
        self.assertNotEqual(imported_zip, slot)
        personal_slot = slots.import_save(profile, builtin / player)
        personal = slots.active_root(profile)
        self.assertNotEqual(personal, builtin)
        restored = (personal / "trophies.dat").read_bytes()
        self.assertEqual(int.from_bytes(restored, "little").bit_count(), 22)
        (builtin / player).write_bytes(b"later progress")
        slots.select(profile, "default")
        slots.select(profile, slot)
        self.assertEqual((slots.active_root(profile) / player).read_bytes(), b"later progress")
        slots.select(profile, personal_slot)
        self.assertEqual((slots.active_root(profile) / player).read_bytes(), initial)
        with zipfile.ZipFile(seed_path) as seed:
            self.assertEqual(seed.read(slots.TITLE + "/P0"), initial)

    def test_retired_selection_uses_full_preset_and_preserves_old_data(self):
        for retired in slots.RETIRED_BUILTINS:
            with self.subTest(retired=retired), tempfile.TemporaryDirectory() as tmp:
                profile = Path(tmp)
                previous = profile / "save-slots" / retired / slots.PLAYER
                previous.parent.mkdir(parents=True)
                previous.write_bytes(b"previous progress")
                slots.write_selected(profile, retired)
                full = slots.active_root(profile)
                self.assertEqual(full.name, slots.COMPLETE_BUILTIN)
                self.assertEqual(int.from_bytes((full / "trophies.dat").read_bytes(), "little").bit_count(), 22)
                self.assertEqual(previous.read_bytes(), b"previous progress")
                self.assertEqual((profile / slots.SELECTED).read_text(), slots.COMPLETE_BUILTIN)

    def test_old_import_recovers_condition_and_keeps_existing_trophy(self):
        with tempfile.TemporaryDirectory() as tmp:
            profile = Path(tmp)
            slot = "a" * 32
            root = profile / "save-slots" / slot
            player = root / slots.PLAYER
            player.parent.mkdir(parents=True)
            stats = (
                b"CGameSettings =\r\n"
                b"HannahStats =\r\n{\r\n\tPLAYER =\r\n\t{\r\n"
                b"\t\tACH_DRIVING_BACKWARDS_SEC = 900.0\r\n"
                b"\t\tACH_TT_LAPS_COMPLETED = 99.0\r\n\t}\r\n}\r\n"
            )
            payload = zlib.compress(stats)
            header = struct.pack("<II", 0x207, 84 + len(payload)).ljust(64, b"\0")
            player.write_bytes(header + struct.pack("<4s4I", b"CMP1", len(stats), len(payload), 0, 0) + payload)
            trophy_file = root / "trophies.dat"
            trophy_file.write_bytes((1 << 5).to_bytes(16, "little"))
            slots.select(profile, slot)
            self.assertEqual(int.from_bytes(trophy_file.read_bytes(), "little"), (1 << 4) | (1 << 5))
            self.assertEqual(slots.active_root(profile), root)

    def test_round_trip_and_rejected_archive_leave_progress_intact(self):
        with tempfile.TemporaryDirectory() as tmp:
            profile = Path(tmp)
            original = profile / "savedata/1" / slots.TITLE / slots.TITLE
            original.mkdir(parents=True)
            payload = struct.pack("<3I", 0x207, 12, 1)
            (original / "P0").write_bytes(payload)
            meta = original.with_name(slots.TITLE + ".sce_sys")
            meta.mkdir()
            (meta / "param.bin").write_bytes(bytes(1328))
            new = slots.import_save(profile, original / "P0")
            imported = slots.active_root(profile)
            self.assertEqual(imported.name, new)
            self.assertEqual((imported / (original / "P0").relative_to(profile)).read_bytes(), payload)
            bad = profile / "other-region.zip"
            with zipfile.ZipFile(bad, "w") as z:
                z.writestr("CUSA07044/P0", payload)
            with self.assertRaises(ValueError):
                slots.import_save(profile, bad)
            self.assertEqual(slots.active_root(profile), imported)
            self.assertEqual(list((profile / "save-slots").iterdir()), [imported])
            slots.select(profile, "default")
            self.assertEqual(slots.active_root(profile), profile)
            self.assertEqual((original / "P0").read_bytes(), payload)


if __name__ == "__main__":
    unittest.main()
