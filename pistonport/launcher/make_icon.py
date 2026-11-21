"""Export Windows icons for the launcher and the game executable."""
import argparse
from io import BytesIO
from pathlib import Path

from PIL import Image

SIZES = [(size, size) for size in (16, 20, 24, 32, 40, 48, 64, 96, 128, 256)]


def icon_canvas(path):
    with Image.open(path) as source:
        image = source.convert("RGBA")
    image.thumbnail((256, 256), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (256, 256))
    canvas.alpha_composite(image, ((256 - image.width) // 2, (256 - image.height) // 2))
    return canvas


def save_ico(image, path):
    data = BytesIO()
    image.save(data, format="ICO", sizes=SIZES)
    contents = data.getvalue()
    if not path.is_file() or path.read_bytes() != contents:
        path.write_bytes(contents)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game-icon", nargs=2, type=Path, metavar=("PNG", "ICO"))
    options = parser.parse_args()
    if options.game_icon:
        source, target = options.game_icon
        save_ico(icon_canvas(source), target)
    else:
        directory = Path(__file__).resolve().parent
        image = icon_canvas(directory / "PistonCup.png")
        image.save(directory / "pistonport.png")
        save_ico(image, directory / "pistonport.ico")
