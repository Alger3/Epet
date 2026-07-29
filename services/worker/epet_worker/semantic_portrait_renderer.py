from __future__ import annotations

from dataclasses import dataclass
from io import BytesIO
from math import pi, sin
import os
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

from .animation_renderer import ACTIONS, ATLAS_COLUMNS, CANVAS, CELL
from .human_pose_estimator import HumanPose, HumanPoseEstimator
from .portrait_animation_renderer import _decode_foreground


PART_ORDER = ("leg_l", "arm_l", "torso", "leg_r", "arm_r", "head")


@dataclass(frozen=True)
class SemanticParts:
    images: dict[str, Image.Image]
    pivots: dict[str, tuple[float, float]]
    coverage: dict[str, float]
    pose: HumanPose
    source: Image.Image


def _canvas(portrait: bytes) -> Image.Image:
    subject = _decode_foreground(portrait)
    subject.thumbnail((198, 226), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    canvas.alpha_composite(
        subject,
        ((CANVAS - subject.width) // 2, CANVAS - subject.height - 10),
    )
    return canvas


def _point(pose: HumanPose, name: str) -> tuple[float, float]:
    value = pose.points[name]
    return value.x * CANVAS, value.y * CANVAS


def _segment_distance(
    xx: np.ndarray,
    yy: np.ndarray,
    start: tuple[float, float],
    end: tuple[float, float],
) -> np.ndarray:
    ax, ay = start
    bx, by = end
    dx = bx - ax
    dy = by - ay
    denominator = max(1e-6, dx * dx + dy * dy)
    position = np.clip(((xx - ax) * dx + (yy - ay) * dy) / denominator, 0, 1)
    closest_x = ax + position * dx
    closest_y = ay + position * dy
    return np.sqrt((xx - closest_x) ** 2 + (yy - closest_y) ** 2)


def split_human_portrait(
    portrait: bytes,
    estimator: HumanPoseEstimator,
) -> SemanticParts:
    source = _canvas(portrait)
    pose = estimator.estimate(source)
    alpha = np.asarray(source.getchannel("A"), dtype=np.uint8)
    opaque = alpha >= 18
    if int(np.count_nonzero(opaque)) < 700:
        raise ValueError("semantic portrait foreground is too small")

    yy, xx = np.indices((CANVAS, CANVAS), dtype=np.float32)
    neck = _point(pose, "neck")
    nose = _point(pose, "nose")
    shoulder_l = _point(pose, "shoulder_l")
    shoulder_r = _point(pose, "shoulder_r")
    hip_l = _point(pose, "hip_l")
    hip_r = _point(pose, "hip_r")
    mid_hip = (
        (hip_l[0] + hip_r[0]) / 2,
        (hip_l[1] + hip_r[1]) / 2,
    )
    shoulder_width = max(
        28.0,
        abs(shoulder_r[0] - shoulder_l[0]),
    )
    head_center = (
        (nose[0] + neck[0]) / 2,
        nose[1] - shoulder_width * 0.08,
    )
    head_rx = max(25.0, shoulder_width * 0.62)
    head_ry = max(28.0, head_rx * 1.05)
    head_score = np.sqrt(
        ((xx - head_center[0]) / head_rx) ** 2
        + ((yy - head_center[1]) / head_ry) ** 2
    )

    arm_width = max(9.0, shoulder_width * 0.20)
    leg_width = max(11.0, shoulder_width * 0.24)
    scores = {
        "head": head_score,
        "arm_l": np.minimum(
            _segment_distance(
                xx,
                yy,
                shoulder_l,
                _point(pose, "elbow_l"),
            ),
            _segment_distance(
                xx,
                yy,
                _point(pose, "elbow_l"),
                _point(pose, "wrist_l"),
            ),
        )
        / arm_width,
        "arm_r": np.minimum(
            _segment_distance(
                xx,
                yy,
                shoulder_r,
                _point(pose, "elbow_r"),
            ),
            _segment_distance(
                xx,
                yy,
                _point(pose, "elbow_r"),
                _point(pose, "wrist_r"),
            ),
        )
        / arm_width,
        "leg_l": np.minimum(
            _segment_distance(
                xx,
                yy,
                hip_l,
                _point(pose, "knee_l"),
            ),
            _segment_distance(
                xx,
                yy,
                _point(pose, "knee_l"),
                _point(pose, "ankle_l"),
            ),
        )
        / leg_width,
        "leg_r": np.minimum(
            _segment_distance(
                xx,
                yy,
                hip_r,
                _point(pose, "knee_r"),
            ),
            _segment_distance(
                xx,
                yy,
                _point(pose, "knee_r"),
                _point(pose, "ankle_r"),
            ),
        )
        / leg_width,
    }
    torso_width = max(18.0, shoulder_width * 0.56)
    torso_score = (
        _segment_distance(xx, yy, neck, mid_hip) / torso_width
        + np.where(yy < neck[1] - 5, 1.8, 0)
        + np.where(yy > mid_hip[1] + leg_width, 1.2, 0)
    )
    scores["torso"] = torso_score

    names = list(PART_ORDER)
    stack = np.stack([scores[name] for name in names])
    assignment = np.argmin(stack, axis=0)
    # Preserve chibi hair/face at the top and force lower extremities to the
    # nearest leg. These guards make template fallback deterministic.
    assignment[(yy < neck[1]) & opaque] = names.index("head")
    below_hips = (yy > mid_hip[1] + 2) & opaque
    left_leg_closer = scores["leg_l"] <= scores["leg_r"]
    assignment[below_hips & left_leg_closer] = names.index("leg_l")
    assignment[below_hips & ~left_leg_closer] = names.index("leg_r")

    images = {}
    coverage = {}
    foreground_pixels = max(1, int(np.count_nonzero(opaque)))
    rgba = np.asarray(source).copy()
    for index, name in enumerate(names):
        part_mask = opaque & (assignment == index)
        # Articulated cutouts need a small shared seam allowance around every
        # joint. Expand only inside the original foreground alpha so adjacent
        # layers overlap without inventing pixels outside the character.
        expanded = np.asarray(
            Image.fromarray(np.where(part_mask, 255, 0).astype(np.uint8)).filter(
                ImageFilter.MaxFilter(13)
            )
        )
        part = rgba.copy()
        part[..., 3] = np.where(expanded >= 16, rgba[..., 3], 0)
        images[name] = Image.fromarray(part)
        coverage[name] = round(
            int(np.count_nonzero(part_mask)) / foreground_pixels,
            6,
        )

    required = ("head", "torso", "arm_l", "arm_r", "leg_l", "leg_r")
    if any(coverage[name] < 0.015 for name in required):
        missing = [name for name in required if coverage[name] < 0.015]
        raise ValueError(f"semantic parts too small: {','.join(missing)}")

    pivots = {
        "head": neck,
        "torso": mid_hip,
        "arm_l": shoulder_l,
        "arm_r": shoulder_r,
        "leg_l": hip_l,
        "leg_r": hip_r,
    }
    return SemanticParts(images, pivots, coverage, pose, source)


def _layer(
    image: Image.Image,
    angle: float,
    pivot: tuple[float, float],
    translate: tuple[float, float] = (0, 0),
) -> Image.Image:
    return image.rotate(
        angle,
        resample=Image.Resampling.BICUBIC,
        center=pivot,
        translate=translate,
    )


def _blink(frame: Image.Image, parts: SemanticParts) -> None:
    draw = ImageDraw.Draw(frame)
    nose = _point(parts.pose, "nose")
    eye_l = _point(parts.pose, "eye_l")
    eye_r = _point(parts.pose, "eye_r")
    sample_x = max(0, min(CANVAS - 1, round(nose[0])))
    sample_y = max(0, min(CANVAS - 1, round(nose[1])))
    skin = parts.source.getpixel((sample_x, sample_y))
    fill = skin[:3] + (255,) if skin[3] >= 20 else (224, 178, 150, 255)
    eye_width = max(4, round(abs(eye_r[0] - eye_l[0]) * 0.28))
    for x, y in (eye_l, eye_r):
        draw.ellipse(
            (x - eye_width, y - 3, x + eye_width, y + 4),
            fill=fill,
        )
        draw.arc(
            (x - eye_width, y - 2, x + eye_width, y + 3),
            8,
            172,
            fill=(45, 35, 35, 255),
            width=2,
        )


def _articulated_frame(
    parts: SemanticParts,
    action: str,
    phase: float,
) -> Image.Image:
    wave = sin(phase * 2 * pi)
    stride = sin(phase * 2 * pi)
    bob = 0.0
    angles = {name: 0.0 for name in PART_ORDER}
    if action == "idle":
        bob = -max(0, wave) * 1.5
        angles["head"] = wave * 1.2
        angles["arm_l"] = wave * 0.8
        angles["arm_r"] = -wave * 0.8
    elif action == "walk":
        bob = -abs(stride) * 4
        angles.update(
            {
                "arm_l": stride * 8,
                "arm_r": -stride * 8,
                "leg_l": -stride * 7,
                "leg_r": stride * 7,
                "head": -stride * 1.5,
            }
        )
    elif action == "tap":
        impulse = sin(phase * pi)
        bob = impulse * 6
        angles["head"] = -impulse * 4
        angles["arm_r"] = -impulse * 18
    elif action == "drag":
        bob = -6 + wave
        angles["arm_l"] = wave * 4
        angles["arm_r"] = -wave * 4
    elif action == "wake":
        bob = (1 - phase) * 8
        angles["head"] = -(1 - phase) * 8

    frame = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    for name in PART_ORDER:
        translated = (0.0, bob)
        transformed = _layer(
            parts.images[name],
            angles[name],
            parts.pivots[name],
            translated,
        )
        frame.alpha_composite(transformed)

    blink = action == "sleep" or (
        action == "idle" and 0.42 <= phase <= 0.58
    )
    if blink:
        _blink(frame, parts)
    return frame


def _posture(
    frame: Image.Image,
    action: str,
    phase: float,
) -> Image.Image:
    wave = sin(phase * 2 * pi)
    if action not in {"sleep", "perch", "perch_sleep"}:
        return frame
    if action == "sleep":
        scale_x, scale_y, rotation, bottom = 1.05, 0.72 + wave * 0.01, -10, 236
    elif action == "perch":
        scale_x, scale_y, rotation, bottom = 1.0, 0.92 + wave * 0.008, 0, 248
    else:
        scale_x, scale_y, rotation, bottom = 1.04, 0.70 + wave * 0.01, -7, 250
    bounds = frame.getbbox()
    if bounds is None:
        return frame
    subject = frame.crop(bounds)
    subject = subject.resize(
        (
            max(1, round(subject.width * scale_x)),
            max(1, round(subject.height * scale_y)),
        ),
        Image.Resampling.LANCZOS,
    ).rotate(
        rotation,
        resample=Image.Resampling.BICUBIC,
        expand=True,
    )
    output = Image.new("RGBA", frame.size, (0, 0, 0, 0))
    output.alpha_composite(
        subject,
        ((CANVAS - subject.width) // 2, bottom - subject.height),
    )
    return output


def _metadata(parts: SemanticParts) -> tuple[dict, dict, dict]:
    pose = parts.pose.to_dict()
    pose["part_coverage"] = parts.coverage
    pose["quality"] = {
        "required_parts_present": True,
        "semantic_split": True,
        "rig_bound": True,
    }
    origins = {
        "root": parts.pivots["torso"],
        "torso": parts.pivots["torso"],
        "head": parts.pivots["head"],
        "arm_l": parts.pivots["arm_l"],
        "arm_r": parts.pivots["arm_r"],
        "leg_l": parts.pivots["leg_l"],
        "leg_r": parts.pivots["leg_r"],
        "eyes": _point(parts.pose, "nose"),
    }
    parents = {
        "root": None,
        "torso": "root",
        "head": "torso",
        "arm_l": "torso",
        "arm_r": "torso",
        "leg_l": "root",
        "leg_r": "root",
        "eyes": "head",
    }
    rig = {
        "schema_version": 1,
        "template_id": "human-semantic-cutout-v1",
        "template_version": "1.0.0",
        "subject_kind": "human_avatar",
        "pose_source": parts.pose.source,
        "bones": [
            {
                "id": name,
                "parent": parents[name],
                "origin": [
                    round(origins[name][0] / CANVAS, 6),
                    round(origins[name][1] / CANVAS, 6),
                ],
                "length": 0,
                "angle": 0,
                "angle_limit": [-45, 45],
            }
            for name in parents
        ],
        "anchors": {
            "foot": [0.5, 0.94],
            "drag": [0.5, 0.34],
            "center": [0.5, 0.55],
            "head": [
                round(parts.pivots["head"][0] / CANVAS, 6),
                round(parts.pivots["head"][1] / CANVAS, 6),
            ],
        },
    }
    z = {"leg_l": -3, "arm_l": -2, "torso": 0, "leg_r": 1, "arm_r": 2, "head": 3}
    layers = {
        "schema_version": 1,
        "subject_kind": "human_avatar",
        "canvas": {"width": CANVAS, "height": CANVAS},
        "parts": [
            {
                "id": name,
                "bone": name,
                "pivot": [
                    round(parts.pivots[name][0] / CANVAS, 6),
                    round(parts.pivots[name][1] / CANVAS, 6),
                ],
                "z_index": z[name],
                "source": "semantic_portrait",
                "coverage": parts.coverage[name],
            }
            for name in PART_ORDER
        ]
        + [
            {
                "id": "eyes_open",
                "bone": "eyes",
                "pivot": rig["anchors"]["head"],
                "z_index": 4,
                "source": "confirmed_portrait",
            },
            {
                "id": "eyes_closed",
                "bone": "eyes",
                "pivot": rig["anchors"]["head"],
                "z_index": 5,
                "source": "generated_blink_overlay",
            },
        ],
    }
    return pose, rig, layers


def render_semantic_human_animation(portrait: bytes) -> dict:
    worker_root = Path(__file__).resolve().parents[1]
    cache_root = Path(
        os.environ.get(
            "EPET_MODEL_CACHE_DIR",
            str(worker_root / ".model-cache"),
        )
    )
    estimator = HumanPoseEstimator(
        cache_root
        / "pose"
        / "human-pose-estimation-0001"
        / "1.0.0"
        / "human-pose-estimation-0001.xml",
        device=os.environ.get("EPET_POSE_DEVICE", "CPU"),
    )
    parts = split_human_portrait(portrait, estimator)
    frames: list[tuple[str, Image.Image]] = []
    actions = {}
    clips = {}
    for action_name, spec in ACTIONS.items():
        names = []
        for index in range(spec.frames):
            name = f"{action_name}_{index:03d}"
            phase = index / spec.frames
            base_action = "idle" if action_name in {"sleep", "perch", "perch_sleep"} else action_name
            frame = _articulated_frame(parts, base_action, phase)
            if action_name in {"sleep", "perch_sleep"}:
                _blink(frame, parts)
            frame = _posture(frame, action_name, phase)
            frames.append((name, frame))
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
            "channels": [
                "head.rotation",
                "arm_l.rotation",
                "arm_r.rotation",
                "leg_l.rotation",
                "leg_r.rotation",
                "torso.translation",
            ],
            "events": (
                ["eyes_close"]
                if action_name in {"sleep", "perch_sleep"}
                else []
            ),
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
    pose, rig, layers = _metadata(parts)
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
        "pose": pose,
        "clips": {"schema_version": 1, "clips": clips},
        "profile": {
            "schema_version": 1,
            "profile_id": "semantic-human-rig-v1",
            "canvas": {"width": CANVAS, "height": CANVAS},
            "supersampling": 1,
            "pixel_format": "rgba8_srgb",
            "atlas_order": "action_then_frame",
            "png": {"compress_level": 9, "optimize": False},
        },
    }
