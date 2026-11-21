"""Developer shortcut for the same bundled bootstrap used by the Windows launcher."""
import os
from pathlib import Path
import subprocess
import sys

outputs = Path(__file__).resolve().parent.parent / "dist/PistonPort"
runtime = outputs / "PistonPort.Runtime"
profile = Path(os.environ.get("PISTONPORT_PROFILE") or Path(os.environ["LOCALAPPDATA"]) / "PistonPort")
library = profile / "game-folder.txt"
selected = os.environ.get("PISTONPORT_GAME") or (library.read_text(encoding="utf-8-sig").strip() if library.is_file() else "")
if not selected:
    sys.exit("Select your extracted game folder in PistonPort.exe first.")
game = Path(selected)
sys.exit(subprocess.call([runtime / "python/python.exe", "-B", runtime / "tools/boot.py",
                          "--profile", profile, "--game", game]))
