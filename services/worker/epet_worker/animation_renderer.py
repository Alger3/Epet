"""Deterministic procedural rig renderer used before real model providers exist."""

from dataclasses import dataclass
from io import BytesIO
from math import cos, pi, sin

from PIL import Image, ImageDraw, ImageOps

from .rig_templates import layer_bundle, rig_template


CANVAS = 256
SUPERSAMPLING = 2
CELL = CANVAS
ATLAS_COLUMNS = 8


@dataclass(frozen=True)
class ActionSpec:
    frames: int
    duration_ms: int
    loop: bool
    phase_source: str = "time"
    stride_length: float | None = None
    next_action: str | None = None


ACTIONS: dict[str, ActionSpec] = {
    "idle": ActionSpec(12, 120, True),
    "walk": ActionSpec(8, 90, True, "distance", 48),
    "sleep": ActionSpec(12, 180, True),
    "tap": ActionSpec(6, 75, False, next_action="idle"),
    "drag": ActionSpec(6, 100, True),
    "wake": ActionSpec(8, 90, False, next_action="idle"),
    "perch": ActionSpec(8, 160, True),
}


def _palette(photo: bytes) -> tuple[tuple[int, int, int], ...]:
    with Image.open(BytesIO(photo)) as source:
        image = ImageOps.exif_transpose(source).convert("RGBA")
        image.thumbnail((64, 64), Image.Resampling.LANCZOS)
        pixels = [
            (red, green, blue)
            for red, green, blue, alpha in image.getdata()
            if alpha >= 64
        ]
    if not pixels:
        pixels = [(220, 130, 80)]
    pixels.sort(key=lambda value: sum(value))
    primary = pixels[len(pixels) // 2]
    dark = tuple(max(20, int(channel * 0.48)) for channel in primary)
    light = tuple(min(245, int(channel * 0.55 + 112)) for channel in primary)
    accent_source = pixels[(len(pixels) * 3) // 4]
    accent = (
        min(245, accent_source[0] + 35),
        max(45, accent_source[1] - 10),
        max(45, accent_source[2] - 20),
    )
    return primary, dark, light, accent


def _line(draw: ImageDraw.ImageDraw, points, fill, width: int) -> None:
    draw.line(points, fill=fill, width=width, joint="curve")
    radius = width // 2
    for x, y in (points[0], points[-1]):
        draw.ellipse((x - radius, y - radius, x + radius, y + radius), fill=fill)


def _cat_frame(
    action: str, phase: float, palette: tuple[tuple[int, int, int], ...]
) -> Image.Image:
    if action == "perch":
        base = _cat_frame("idle", phase, palette)
        frame = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
        head = base.crop((55, 24, 190, 142))
        frame.alpha_composite(head, (60, 82))
        draw = ImageDraw.Draw(frame)
        _, dark, light, _ = palette
        for paw in ((72, 190, 116, 224), (150, 190, 194, 224)):
            draw.ellipse(paw, fill=light + (255,), outline=dark + (255,), width=3)
        return frame

    primary, dark, light, accent = palette
    scale = SUPERSAMPLING
    frame = Image.new("RGBA", (CANVAS * scale, CANVAS * scale), (0, 0, 0, 0))
    draw = ImageDraw.Draw(frame)
    p = lambda value: int(round(value * scale))
    wave = sin(phase * 2 * pi)
    blink = action == "sleep" or (action == "idle" and 0.43 < phase < 0.57)

    if action == "sleep":
        breath = sin(phase * 2 * pi) * 2
        body = (48, 137 - breath, 192, 204 + breath)
        head = (139, 126 - breath, 211, 188 + breath)
        tail_points = [(62, 172), (37, 164 + wave * 2), (30, 188), (52, 199)]
        paws = [(113, 184, 151, 204), (144, 180, 179, 201)]
    else:
        body_y = sin(phase * 4 * pi) * 3 if action == "walk" else wave * 1.5
        if action == "tap":
            squash = sin(min(1, phase) * pi)
            body_y += squash * 7
        elif action == "drag":
            body_y = -5 + wave
        elif action == "wake":
            body_y = (1 - phase) * 9
        body = (56, 93 + body_y, 190, 190 + body_y)
        head = (72, 43 + body_y, 174, 132 + body_y)
        tail_points = [
            (177, 125 + body_y),
            (207, 117 + wave * 8),
            (220, 88 + wave * 11),
            (211, 62 + wave * 8),
        ]
        paws = []

    _line(
        draw,
        [(p(x), p(y)) for x, y in tail_points],
        dark + (255,),
        p(14),
    )

    if action != "sleep":
        leg_swing = sin(phase * 2 * pi) * 13 if action == "walk" else 0
        if action == "drag":
            leg_swing = 7
        baseline = 220 if action == "drag" else 213
        for x, swing, back in [
            (82, leg_swing, True),
            (112, -leg_swing, False),
            (143, -leg_swing, True),
            (169, leg_swing, False),
        ]:
            top = (x, 155)
            knee = (x + swing * 0.45, 181)
            foot = (x + swing, baseline - abs(swing) * 0.14)
            color = dark if back else primary
            _line(
                draw,
                [(p(a), p(b)) for a, b in (top, knee, foot)],
                color + (255,),
                p(18),
            )

    draw.ellipse(tuple(p(v) for v in body), fill=primary + (255,), outline=dark + (255,), width=p(3))
    for paw in paws:
        draw.ellipse(tuple(p(v) for v in paw), fill=light + (255,), outline=dark + (255,), width=p(2))

    ear_y = head[1] + 8
    draw.polygon(
        [(p(82), p(ear_y + 15)), (p(89), p(ear_y - 25)), (p(112), p(ear_y + 8))],
        fill=primary + (255,),
        outline=dark + (255,),
    )
    draw.polygon(
        [(p(137), p(ear_y + 7)), (p(160), p(ear_y - 24)), (p(169), p(ear_y + 18))],
        fill=primary + (255,),
        outline=dark + (255,),
    )
    draw.ellipse(tuple(p(v) for v in head), fill=primary + (255,), outline=dark + (255,), width=p(3))
    eye_y = (head[1] + head[3]) / 2 - 3
    for eye_x in (105, 143):
        if blink:
            draw.arc(
                (p(eye_x - 8), p(eye_y - 2), p(eye_x + 8), p(eye_y + 7)),
                10,
                170,
                fill=dark + (255,),
                width=p(3),
            )
        else:
            draw.ellipse(
                (p(eye_x - 6), p(eye_y - 8), p(eye_x + 6), p(eye_y + 8)),
                fill=dark + (255,),
            )
            draw.ellipse(
                (p(eye_x - 2), p(eye_y - 5), p(eye_x + 1), p(eye_y - 2)),
                fill=(255, 255, 255, 235),
            )
    nose_y = eye_y + 20
    draw.polygon(
        [(p(119), p(nose_y)), (p(129), p(nose_y)), (p(124), p(nose_y + 6))],
        fill=accent + (255,),
    )
    return frame.resize((CANVAS, CANVAS), Image.Resampling.LANCZOS)


def _human_frame(
    action: str, phase: float, palette: tuple[tuple[int, int, int], ...]
) -> Image.Image:
    if action == "perch":
        base = _human_frame("idle", phase, palette)
        frame = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
        head = base.crop((68, 18, 188, 132))
        frame.alpha_composite(head, (68, 72))
        draw = ImageDraw.Draw(frame)
        _, dark, light, _ = palette
        for hand in ((73, 184, 112, 218), (154, 184, 193, 218)):
            draw.ellipse(hand, fill=light + (255,), outline=dark + (255,), width=3)
        return frame

    primary, dark, light, accent = palette
    scale = SUPERSAMPLING
    frame = Image.new("RGBA", (CANVAS * scale, CANVAS * scale), (0, 0, 0, 0))
    draw = ImageDraw.Draw(frame)
    p = lambda value: int(round(value * scale))
    wave = sin(phase * 2 * pi)
    blink = action == "sleep" or (action == "idle" and 0.44 < phase < 0.58)

    if action == "sleep":
        breath = wave * 2
        draw.ellipse(
            (p(54), p(158 - breath), p(204), p(211 + breath)),
            fill=primary + (255,),
            outline=dark + (255,),
            width=p(3),
        )
        head_box = (56, 111 - breath, 132, 181 - breath)
        draw.ellipse(tuple(p(v) for v in head_box), fill=light + (255,), outline=dark + (255,), width=p(3))
        draw.pieslice(tuple(p(v) for v in head_box), 180, 360, fill=dark + (255,))
        for x in (80, 105):
            draw.arc((p(x - 7), p(143), p(x + 7), p(151)), 10, 170, fill=dark + (255,), width=p(3))
        return frame.resize((CANVAS, CANVAS), Image.Resampling.LANCZOS)

    bounce = sin(phase * 4 * pi) * 3 if action == "walk" else wave
    if action == "tap":
        bounce -= sin(phase * pi) * 9
    elif action == "drag":
        bounce = -6 + wave
    elif action == "wake":
        bounce = (1 - phase) * 8
    hip_y = 154 + bounce
    limb = sin(phase * 2 * pi) * 15 if action == "walk" else 0

    for x, swing, back in [(111, limb, True), (145, -limb, False)]:
        color = dark if back else primary
        _line(
            draw,
            [
                (p(x), p(hip_y)),
                (p(x + swing * 0.45), p(185 + bounce)),
                (p(x + swing), p(222)),
            ],
            color + (255,),
            p(18),
        )
        draw.ellipse(
            (p(x + swing - 11), p(214), p(x + swing + 14), p(227)),
            fill=dark + (255,),
        )

    arm_swing = -limb * 0.75
    for x, direction in ((96, 1), (160, -1)):
        swing = arm_swing * direction
        if action == "drag":
            swing = 8 * direction
        _line(
            draw,
            [
                (p(x), p(101 + bounce)),
                (p(x + swing), p(137 + bounce)),
                (p(x + swing * 0.7), p(163 + bounce)),
            ],
            light + (255,),
            p(15),
        )

    draw.rounded_rectangle(
        (p(91), p(91 + bounce), p(166), p(171 + bounce)),
        radius=p(18),
        fill=primary + (255,),
        outline=dark + (255,),
        width=p(3),
    )
    draw.polygon(
        [(p(91), p(142 + bounce)), (p(75), p(187 + bounce)), (p(181), p(187 + bounce)), (p(166), p(142 + bounce))],
        fill=accent + (255,),
        outline=dark + (255,),
    )
    hair_sway = wave * 3 + (limb * 0.08)
    draw.ellipse(
        (p(77 + hair_sway), p(28 + bounce), p(179 + hair_sway), p(126 + bounce)),
        fill=dark + (255,),
    )
    draw.ellipse(
        (p(85), p(34 + bounce), p(171), p(119 + bounce)),
        fill=light + (255,),
        outline=dark + (255,),
        width=p(3),
    )
    draw.pieslice(
        (p(78 + hair_sway), p(24 + bounce), p(178 + hair_sway), p(112 + bounce)),
        180,
        355,
        fill=dark + (255,),
    )
    for eye_x in (111, 145):
        eye_y = 80 + bounce
        if blink:
            draw.arc((p(eye_x - 7), p(eye_y - 2), p(eye_x + 7), p(eye_y + 7)), 10, 170, fill=dark + (255,), width=p(3))
        else:
            draw.ellipse((p(eye_x - 5), p(eye_y - 7), p(eye_x + 5), p(eye_y + 7)), fill=dark + (255,))
            draw.ellipse((p(eye_x - 2), p(eye_y - 5), p(eye_x + 1), p(eye_y - 2)), fill=(255, 255, 255, 235))
    draw.arc((p(120), p(91 + bounce), p(136), p(105 + bounce)), 15, 165, fill=accent + (255,), width=p(2))
    return frame.resize((CANVAS, CANVAS), Image.Resampling.LANCZOS)


def render_animation(photo: bytes, subject_kind: str) -> dict:
    subject_kind = "human_avatar" if subject_kind == "human_avatar" else "pet_cat"
    palette = _palette(photo)
    frames: list[tuple[str, Image.Image]] = []
    actions: dict[str, dict] = {}
    clips: dict[str, dict] = {}
    for action_name, spec in ACTIONS.items():
        names = []
        for index in range(spec.frames):
            frame_name = f"{action_name}_{index:03d}"
            phase = index / spec.frames
            renderer = _human_frame if subject_kind == "human_avatar" else _cat_frame
            frames.append((frame_name, renderer(action_name, phase, palette)))
            names.append(frame_name)
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
        channels = ["root.position", "torso.rotation"]
        events = []
        if action_name == "idle":
            channels += ["torso.scale", "eyes.visibility", "tail.rotation" if subject_kind == "pet_cat" else "hair_back.rotation"]
            events = ["blink"]
        elif action_name == "walk":
            channels += ["legs.rotation", "body.height", "secondary_motion.rotation"]
            events = ["foot_contact_l", "foot_contact_r"]
        elif action_name == "sleep":
            channels += ["pose.sleep", "eyes.visibility", "torso.scale"]
            events = ["eyes_close"]
        elif action_name == "perch":
            channels += ["pose.perch", "eyes.visibility", "hands.position"]
            events = ["edge_dock"]
        clips[action_name] = {
            "frame_count": spec.frames,
            "duration_ms": spec.frames * spec.duration_ms,
            "loop": spec.loop,
            "phase_source": spec.phase_source,
            **({"stride_length": spec.stride_length} if spec.stride_length else {}),
            "channels": channels,
            "events": events,
        }

    rows = (len(frames) + ATLAS_COLUMNS - 1) // ATLAS_COLUMNS
    atlas_image = Image.new("RGBA", (ATLAS_COLUMNS * CELL, rows * CELL), (0, 0, 0, 0))
    atlas_frames: dict[str, dict] = {}
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
        "layers": layer_bundle(subject_kind),
        "rig": rig_template(subject_kind),
        "clips": {"schema_version": 1, "clips": clips},
        "profile": {
            "schema_version": 1,
            "profile_id": "offline-rigged-atlas-v1",
            "canvas": {"width": CANVAS, "height": CANVAS},
            "supersampling": SUPERSAMPLING,
            "pixel_format": "rgba8_srgb",
            "atlas_order": "action_then_frame",
            "png": {"compress_level": 9, "optimize": False},
        },
    }
