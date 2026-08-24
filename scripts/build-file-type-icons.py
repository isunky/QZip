from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
SOURCE_DIR = ROOT / "PIC" / "file-types" / "source"
PNG_DIR = ROOT / "PIC" / "file-types" / "png"
README_PNG_DIR = ROOT / "PIC" / "file-types" / "readme"
ICO_DIR = ROOT / "apps" / "desktop" / "public" / "file-types"
NATIVE_ICO_DIR = ROOT / "apps" / "desktop" / "src-tauri" / "icons" / "file-types"

CANVAS_SIZE = 1024
CONTENT_SIZE = 932
README_SIZE = 128
ICO_SIZES = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]


def visible_bounds(image: Image.Image) -> tuple[int, int, int, int]:
    alpha = image.getchannel("A")
    width, height = image.size

    row_density = alpha.resize((1, height), Image.Resampling.BOX)
    column_density = alpha.resize((width, 1), Image.Resampling.BOX)
    rows = [index for index, value in enumerate(row_density.get_flattened_data()) if value >= 5]
    columns = [index for index, value in enumerate(column_density.get_flattened_data()) if value >= 5]

    if not rows or not columns:
        raise ValueError("Image has no visible content")

    return columns[0], rows[0], columns[-1] + 1, rows[-1] + 1


def normalize_icon(source: Path) -> Image.Image:
    image = Image.open(source).convert("RGBA")
    cropped = image.crop(visible_bounds(image))
    scale = min(CONTENT_SIZE / cropped.width, CONTENT_SIZE / cropped.height)
    target = (
        max(1, round(cropped.width * scale)),
        max(1, round(cropped.height * scale)),
    )
    resized = cropped.resize(target, Image.Resampling.LANCZOS)

    canvas = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    offset = ((CANVAS_SIZE - target[0]) // 2, (CANVAS_SIZE - target[1]) // 2)
    canvas.alpha_composite(resized, offset)
    return canvas


def main() -> None:
    PNG_DIR.mkdir(parents=True, exist_ok=True)
    README_PNG_DIR.mkdir(parents=True, exist_ok=True)
    ICO_DIR.mkdir(parents=True, exist_ok=True)
    NATIVE_ICO_DIR.mkdir(parents=True, exist_ok=True)
    sources = sorted(SOURCE_DIR.glob("*.png"))
    if not sources:
        raise SystemExit(f"No PNG sources found in {SOURCE_DIR}")

    for source in sources:
        icon = normalize_icon(source)
        png_path = PNG_DIR / source.name
        readme_png_path = README_PNG_DIR / source.name
        ico_path = ICO_DIR / f"{source.stem}.ico"
        native_ico_path = NATIVE_ICO_DIR / ico_path.name
        icon.save(png_path, "PNG", optimize=True)
        icon.resize((README_SIZE, README_SIZE), Image.Resampling.LANCZOS).save(
            readme_png_path,
            "PNG",
            optimize=True,
        )
        icon.save(ico_path, "ICO", sizes=ICO_SIZES)
        icon.save(native_ico_path, "ICO", sizes=ICO_SIZES)
        print(
            f"built {source.stem}: {png_path.name}, readme/{readme_png_path.name}, "
            f"{ico_path.name}, native/{native_ico_path.name}"
        )


if __name__ == "__main__":
    main()
