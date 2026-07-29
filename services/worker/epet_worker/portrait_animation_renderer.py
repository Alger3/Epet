"""Whole-portrait animation fallback for confirmed generated artwork.

This renderer deliberately preserves the approved portrait pixels. It applies
only deterministic whole-image transforms until semantic part segmentation is
available; it must never redraw the subject with the procedural Mock template.
"""

from io import BytesIO
from math import pi, sin
from statistics import median

from PIL import Image, ImageOps

from .animation_renderer import ACTIONS, ATLAS_COLUMNS, CANVAS, CELL


def _decode_foreground(portrait: bytes) -> Image.Image:
    with Image.open(BytesIO(portrait)) as source:
        image = ImageOps.exif_transpose(source).convert("RGBA")
    image.thumbnail((768, 768), Image.Resampling.LANCZOS)

    width, height = image.size
    sample = max(2, min(width, height) // 32)
    corner_pixels = []
    for box in (
        (0, 0, sample, sample),
        (width - sample, 0, width, sample),
        (0, height - sample, sample, height),
        (width - sample, height - sample, width, height),
    ):
        corner_pixels.extend(image.crop(box).getdata())
    background = tuple(
        int(median(pixel[channel] for pixel in corner_pixels))
        for channel in range(3)
    )

    # SD1.5 is prompted for a plain light background. Convert color distance
    # from the corner estimate into a soft alpha matte while preserving any
    # alpha already present in the generated PNG.
    matte = Image.new("L", image.size)
    output_alpha = []
    for red, green, blue, alpha in image.getdata():
        distance = (
            (red - background[0]) ** 2
            + (green - background[1]) ** 2
            + (blue - background[2]) ** 2
        ) ** 0.5
        generated_alpha = max(0, min(255, round((distance - 18) * 255 / 58)))
        output_alpha.append(min(alpha, generated_alpha))
    matte.putdata(output_alpha)
    image.putalpha(matte)

    solid = matte.point(lambda value: 255 if value >= 32 else 0)
    bounds = solid.getbbox()
    if bounds is None:
        # Do not return an invisible character when a model already emitted a
        # transparent or unusually low-contrast portrait.
        with Image.open(BytesIO(portrait)) as source:
            image = ImageOps.exif_transpose(source).convert("RGBA")
        bounds = image.getbbox() or (0, 0, image.width, image.height)
    left, top, right, bottom = bounds
    padding = max(4, round(max(right - left, bottom - top) * 0.04))
    bounds = (
        max(0, left - padding),
        max(0, top - padding),
        min(image.width, right + padding),
        min(image.height, bottom + padding),
    )
    subject = image.crop(bounds)
    subject.thumbnail((224, 224), Image.Resampling.LANCZOS)
    return subject


def _frame(subject: Image.Image, action: str, phase: float) -> Image.Image:
    wave = sin(phase * 2 * pi)
    scale_x = 1.0
    scale_y = 1.0
    rotation = 0.0
    offset_x = 0
    offset_y = 12

    if action == "idle":
        scale_y = 1.0 + wave * 0.012
        offset_y += round(-wave * 1.5)
    elif action == "walk":
        rotation = wave * 2.0
        offset_y += round(-abs(wave) * 5)
    elif action == "sleep":
        rotation = -7.0
        scale_x = 1.06
        scale_y = 0.82 + wave * 0.012
        offset_y += 32
    elif action == "tap":
        impulse = sin(phase * pi)
        scale_x = 1.0 + impulse * 0.08
        scale_y = 1.0 - impulse * 0.09
        offset_y += round(impulse * 9)
    elif action == "drag":
        rotation = wave * 2.5
        offset_y -= 5
    elif action == "wake":
        remaining = 1.0 - phase
        rotation = -7.0 * remaining
        scale_x = 1.0 + 0.06 * remaining
        scale_y = 1.0 - 0.18 * remaining
        offset_y += round(30 * remaining)
    elif action == "perch":
        scale_x = 1.04
        scale_y = 1.04 + wave * 0.008
        offset_y += 46
    elif action == "perch_sleep":
        rotation = -5.0
        scale_x = 1.08
        scale_y = 0.88 + wave * 0.01
        offset_y += 58

    transformed = subject.resize(
        (
            max(1, round(subject.width * scale_x)),
            max(1, round(subject.height * scale_y)),
        ),
        Image.Resampling.LANCZOS,
    )
    if rotation:
        transformed = transformed.rotate(
            rotation,
            resample=Image.Resampling.BICUBIC,
            expand=True,
        )
    frame = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    x = (CANVAS - transformed.width) // 2 + offset_x
    y = CANVAS - transformed.height - offset_y
    frame.alpha_composite(transformed, (x, y))
    return frame


def _portrait_metadata(subject_kind: str) -> tuple[dict, dict]:
    kind = "human_avatar" if subject_kind == "human_avatar" else "pet_cat"
    rig = {
        "schema_version": 1,
        "template_id": (
            "human-portrait-warp" if kind == "human_avatar"
            else "cat-portrait-warp"
        ),
        "template_version": "1.0.0",
        "subject_kind": kind,
        "bones": [
            {"id": "root", "parent": None},
            {"id": "portrait", "parent": "root"},
            {"id": "foreground", "parent": "portrait"},
            {"id": "overlay", "parent": "portrait"},
        ],
        "anchors": {
            "foot": [0.5, 0.94],
            "drag": [0.5, 0.34],
            "center": [0.5, 0.55],
            "head": [0.5, 0.28],
        },
    }
    layers = {
        "schema_version": 1,
        "subject_kind": kind,
        "canvas": {"width": CANVAS, "height": CANVAS},
        "parts": [
            {
                "id": part,
                "bone": bone,
                "pivot": [0.5, 0.5],
                "z_index": index,
                "source": "confirmed_portrait",
            }
            for index, (part, bone) in enumerate(
                (
                    ("shadow", "root"),
                    ("portrait", "portrait"),
                    ("foreground", "foreground"),
                    ("overlay", "overlay"),
                )
            )
        ],
    }
    return rig, layers


def render_portrait_animation(portrait: bytes, subject_kind: str) -> dict:
    subject = _decode_foreground(portrait)
    frames: list[tuple[str, Image.Image]] = []
    actions: dict[str, dict] = {}
    clips: dict[str, dict] = {}
    for action_name, spec in ACTIONS.items():
        names = []
        for index in range(spec.frames):
            name = f"{action_name}_{index:03d}"
            frames.append((name, _frame(subject, action_name, index / spec.frames)))
            names.append(name)
        action = {
            "frames": names,
            "frame_duration_ms": [spec.duration_ms] * spec.frames,
            "loop": spec.loop,
            "next_action": spec.next_action,
            "fallback": None if action_name == "idle" else "idle",
            "phase_source": spec.phase_source,
        }
        if spec.stride_length is not None:
            action["stride_length"] = spec.stride_length
        actions[action_name] = action
        clips[action_name] = {
            "frame_count": spec.frames,
            "duration_ms": spec.frames * spec.duration_ms,
            "loop": spec.loop,
            "phase_source": spec.phase_source,
            **({"stride_length": spec.stride_length} if spec.stride_length else {}),
            "channels": ["portrait.transform"],
            "events": [],
        }

    rows = (len(frames) + ATLAS_COLUMNS - 1) // ATLAS_COLUMNS
    atlas_image = Image.new(
        "RGBA",
        (ATLAS_COLUMNS * CELL, rows * CELL),
        (0, 0, 0, 0),
    )
    atlas_frames = {}
    for offset, (name, image) in enumerate(frames):
        x = (offset % ATLAS_COLUMNS) * CELL
        y = (offset // ATLAS_COLUMNS) * CELL
        atlas_image.alpha_composite(image, (x, y))
        atlas_frames[name] = {
            "frame": {"x": x, "y": y, "w": CELL, "h": CELL},
            "rotated": False,
            "source_size": {"w": CELL, "h": CELL},
            "sprite_source": {"x": 0, "y": 0, "w": CELL, "h": CELL},
        }

    atlas_output = BytesIO()
    atlas_image.save(atlas_output, "PNG", optimize=False, compress_level=9)
    thumbnail_output = BytesIO()
    frames[0][1].save(thumbnail_output, "PNG", optimize=False, compress_level=9)
    rig, layers = _portrait_metadata(subject_kind)
    return {
        "atlas_png": atlas_output.getvalue(),
        "thumbnail_png": thumbnail_output.getvalue(),
        "atlas": {
            "schema_version": 1,
            "image": "pet.png",
            "size": {"w": atlas_image.width, "h": atlas_image.height},
            "frames": atlas_frames,
        },
        "actions": actions,
        "layers": layers,
        "rig": rig,
        "clips": {"schema_version": 1, "clips": clips},
        "profile": {
            "schema_version": 1,
            "profile_id": "confirmed-portrait-warp-v1",
            "canvas": {"width": CANVAS, "height": CANVAS},
            "supersampling": 1,
            "pixel_format": "rgba8_srgb",
            "atlas_order": "action_then_frame",
            "png": {"compress_level": 9, "optimize": False},
        },
    }
