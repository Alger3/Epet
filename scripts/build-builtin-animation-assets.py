"""Regenerate the two checked-in 5.4 development Atlas assets."""

import json
from math import pi, sin
from pathlib import Path
import sys

from PIL import Image, ImageOps


ROOT = Path(__file__).resolve().parents[1]
WORKER = ROOT / "services" / "worker"
if str(WORKER) not in sys.path:
    sys.path.insert(0, str(WORKER))

from epet_worker.animation_renderer import ACTIONS, ATLAS_COLUMNS, CANVAS, render_animation


TARGETS = (
    (
        "pet_cat",
        ROOT / "assets" / "builtin-pet" / "cat-idle.png",
        ROOT / "assets" / "builtin-pet" / "cat-walk-sheet.png",
        ROOT / "assets" / "builtin-pet" / "cat-sleep-sheet.png",
        ROOT / "assets" / "builtin-pet" / "cat-perch-sheet.png",
        ROOT / "assets" / "builtin-pet" / "cat-perch-sleep-sheet.png",
        ROOT / "assets" / "builtin-pet",
    ),
    (
        "human_avatar",
        ROOT / "assets" / "builtin-character" / "human-avatar.png",
        ROOT / "assets" / "builtin-character" / "human-walk-sheet.png",
        ROOT / "assets" / "builtin-character" / "human-sleep-sheet.png",
        ROOT / "assets" / "builtin-character" / "human-perch-sheet.png",
        ROOT / "assets" / "builtin-character" / "human-perch-sleep-sheet.png",
        ROOT / "assets" / "builtin-character",
    ),
)


def runtime_definition(rendered: dict) -> dict:
    return {
        "canvas": {"width": CANVAS, "height": CANVAS},
        "frames": {
            name: {
                "frame": value["frame"],
                "sourceSize": value["source_size"],
                "spriteSource": value["sprite_source"],
            }
            for name, value in rendered["atlas"]["frames"].items()
        },
        "actions": {
            name: {
                "frames": value["frames"],
                "frameDurationMs": value["frame_duration_ms"],
                "loop": value["loop"],
                "fallback": value["fallback"] or value["next_action"],
                "phaseSource": value["phase_source"],
                **(
                    {"strideLength": value["stride_length"]}
                    if "stride_length" in value
                    else {}
                ),
            }
            for name, value in rendered["actions"].items()
        },
}


def normalize(image: Image.Image) -> Image.Image:
    rgba = image.convert("RGBA")
    bounds = rgba.getchannel("A").getbbox()
    if bounds:
        rgba = rgba.crop(bounds)
    fitted = ImageOps.contain(rgba, (238, 226), Image.Resampling.LANCZOS)
    frame = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    frame.alpha_composite(fitted, ((CANVAS - fitted.width) // 2, 248 - fitted.height))
    return frame


def sheet_frames(path: Path) -> list[Image.Image]:
    with Image.open(path) as source:
        source = source.convert("RGBA")
        cell_width = source.width // 4
        cell_height = source.height // 2
        return [
            normalize(
                source.crop(
                    (
                        column * cell_width,
                        row * cell_height,
                        (column + 1) * cell_width,
                        (row + 1) * cell_height,
                    )
                )
            )
            for row in range(2)
            for column in range(4)
        ]


def transform(
    image: Image.Image,
    *,
    scale_x: float = 1,
    scale_y: float = 1,
    angle: float = 0,
    offset_y: int = 0,
) -> Image.Image:
    width = max(1, round(CANVAS * scale_x))
    height = max(1, round(CANVAS * scale_y))
    changed = image.resize((width, height), Image.Resampling.LANCZOS)
    if angle:
        changed = changed.rotate(angle, Image.Resampling.BICUBIC, expand=True)
    frame = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    frame.alpha_composite(
        changed,
        ((CANVAS - changed.width) // 2, CANVAS - changed.height + offset_y),
    )
    return frame


def curated_frames(
    source_path: Path,
    walk_path: Path,
    sleep_path: Path,
    perch_path: Path,
    perch_sleep_path: Path,
) -> list[tuple[str, Image.Image]]:
    with Image.open(source_path) as source:
        idle = normalize(source)
    walk = sheet_frames(walk_path)
    sleep_cells = sheet_frames(sleep_path)
    sleep = sleep_cells[0]
    perch = sheet_frames(perch_path)
    perch_sleep = sheet_frames(perch_sleep_path)
    frames: list[tuple[str, Image.Image]] = []
    for action_name, spec in ACTIONS.items():
        for index in range(spec.frames):
            phase = index / spec.frames
            wave = sin(phase * 2 * pi)
            if action_name == "walk":
                frame = walk[index % len(walk)]
            elif action_name == "sleep":
                frame = transform(sleep, scale_y=1 + wave * 0.012, offset_y=round(wave))
            elif action_name == "perch":
                frame = perch[index % len(perch)]
            elif action_name == "perch_sleep":
                frame = perch_sleep[index % len(perch_sleep)]
            elif action_name == "tap":
                amount = sin(phase * pi)
                frame = transform(
                    idle,
                    scale_x=1 + amount * 0.035,
                    scale_y=1 - amount * 0.055,
                    offset_y=round(amount * 5),
                )
            elif action_name == "drag":
                frame = transform(idle, angle=wave * 1.6, offset_y=-4)
            elif action_name == "wake":
                waking = sleep if phase < 0.5 else idle
                frame = transform(waking, offset_y=round((1 - phase) * 3))
            else:
                frame = transform(
                    idle,
                    scale_x=1 - wave * 0.004,
                    scale_y=1 + wave * 0.012,
                    offset_y=round(-wave),
                )
            frames.append((f"{action_name}_{index:03d}", frame))
    return frames


def pack_atlas(frames: list[tuple[str, Image.Image]]) -> tuple[bytes, dict]:
    rows = (len(frames) + ATLAS_COLUMNS - 1) // ATLAS_COLUMNS
    atlas = Image.new(
        "RGBA", (ATLAS_COLUMNS * CANVAS, rows * CANVAS), (0, 0, 0, 0)
    )
    definition = {}
    for offset, (name, frame) in enumerate(frames):
        x = (offset % ATLAS_COLUMNS) * CANVAS
        y = (offset // ATLAS_COLUMNS) * CANVAS
        atlas.alpha_composite(frame, (x, y))
        definition[name] = {
            "frame": {"x": x, "y": y, "w": CANVAS, "h": CANVAS},
            "rotated": False,
            "source_size": {"w": CANVAS, "h": CANVAS},
            "sprite_source": {"x": 0, "y": 0, "w": CANVAS, "h": CANVAS},
        }
    from io import BytesIO

    output = BytesIO()
    atlas.save(output, "PNG", optimize=False, compress_level=9)
    return output.getvalue(), definition


def main() -> None:
    for subject_kind, source, walk, sleep, perch, perch_sleep, destination in TARGETS:
        rendered = render_animation(source.read_bytes(), subject_kind)
        atlas_png, atlas_frames = pack_atlas(
            curated_frames(source, walk, sleep, perch, perch_sleep)
        )
        rendered["atlas"]["frames"] = atlas_frames
        (destination / "animation-atlas.png").write_bytes(atlas_png)
        (destination / "animation.json").write_text(
            json.dumps(
                runtime_definition(rendered),
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n",
            encoding="utf-8",
        )
        print(f"generated {subject_kind}: {len(rendered['atlas']['frames'])} frames")


if __name__ == "__main__":
    main()
