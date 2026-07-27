from hashlib import sha256
from io import BytesIO
import json
from zipfile import ZIP_STORED, ZipFile, ZipInfo

from PIL import Image, ImageOps


CANVAS = 256
FIXED_TIMESTAMP = (2026, 1, 1, 0, 0, 0)


def canonical_json(value: object) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def render_sprite(photo: bytes) -> bytes:
    with Image.open(BytesIO(photo)) as source:
        source = ImageOps.exif_transpose(source).convert("RGBA")
        fitted = ImageOps.contain(source, (CANVAS, CANVAS), Image.Resampling.LANCZOS)
        canvas = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
        canvas.alpha_composite(
            fitted, ((CANVAS - fitted.width) // 2, CANVAS - fitted.height)
        )
        output = BytesIO()
        canvas.save(output, format="PNG", optimize=False, compress_level=9)
        return output.getvalue()


def frame() -> dict:
    return {
        "frame": {"x": 0, "y": 0, "w": CANVAS, "h": CANVAS},
        "rotated": False,
        "source_size": {"w": CANVAS, "h": CANVAS},
        "sprite_source": {"x": 0, "y": 0, "w": CANVAS, "h": CANVAS},
    }


def build_epet(photo: bytes, display_name: str) -> bytes:
    sprite = render_sprite(photo)
    safe_name = display_name.strip()[:64] or "自定义桌宠"
    identity_hash = sha256(photo + b"\0" + safe_name.encode("utf-8")).hexdigest()
    action_names = ("idle", "sleep", "tap", "walk")
    atlas = canonical_json(
        {
            "schema_version": 1,
            "image": "pet.png",
            "size": {"w": CANVAS, "h": CANVAS},
            "frames": {f"{name}_000": frame() for name in action_names},
        }
    )
    license_json = canonical_json(
        {
            "license": "Private local test asset",
            "source": "User-provided photo processed by Epet Mock Worker",
        }
    )
    declared_files = {
        "atlas/pet.json": atlas,
        "atlas/pet.png": sprite,
        "license.json": license_json,
        "thumbnail.png": sprite,
    }
    actions = {
        name: {
            "frames": [f"{name}_000"],
            "frame_duration_ms": [500 if name == "sleep" else 160],
            "loop": name != "tap",
            "next_action": "idle" if name == "tap" else None,
            "fallback": None if name == "idle" else "idle",
        }
        for name in action_names
    }
    manifest = canonical_json(
        {
            "schema_version": 1,
            "package_version": "1.0.0",
            "min_runtime_version": "0.2.0",
            "pet_id": f"pet_{identity_hash[:16]}",
            "name": safe_name,
            "species": "cat",
            "renderer": "sprite_atlas",
            "created_at": "2026-01-01T00:00:00Z",
            "canvas": {"width": CANVAS, "height": CANVAS},
            "atlas": {
                "image": "atlas/pet.png",
                "data": "atlas/pet.json",
                "max_texture_size": 4096,
            },
            "default_scale": 0.8,
            "anchors": {"foot": [0.5, 0.95], "drag": [0.5, 0.3]},
            "hitboxes": [
                {
                    "id": "body",
                    "shape": "ellipse",
                    "x": 0.1,
                    "y": 0.08,
                    "w": 0.8,
                    "h": 0.9,
                }
            ],
            "actions": actions,
            "generation": {
                "pipeline_version": "1.0.0",
                "template_version": "local-mock-v1",
            },
            "files": [
                {
                    "path": path,
                    "size": len(content),
                    "sha256": sha256(content).hexdigest(),
                }
                for path, content in sorted(declared_files.items())
            ],
        }
    )
    output = BytesIO()
    with ZipFile(output, "w", compression=ZIP_STORED) as archive:
        for path, content in [
            ("manifest.json", manifest),
            *sorted(declared_files.items()),
        ]:
            info = ZipInfo(path, date_time=FIXED_TIMESTAMP)
            info.compress_type = ZIP_STORED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, content)
    return output.getvalue()
