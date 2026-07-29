from hashlib import sha256
from io import BytesIO
import json
from zipfile import ZIP_STORED, ZipFile, ZipInfo

from .animation_renderer import CANVAS
from .portrait_animation_renderer import render_portrait_animation
from .semantic_portrait_renderer import render_semantic_human_animation
from .providers.base import GenerationProvider
from .providers.contracts import GenerationPlan, GenerationRequest
from .providers.mock_provider import MockProvider


FIXED_TIMESTAMP = (2026, 1, 1, 0, 0, 0)


def canonical_json(value: object) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def build_epet(
    photo: bytes,
    display_name: str,
    subject_kind: str = "pet_cat",
    *,
    provider: GenerationProvider | None = None,
    plan: GenerationPlan | None = None,
    portrait_provider: str | None = None,
) -> bytes:
    subject_kind = "human_avatar" if subject_kind == "human_avatar" else "pet_cat"
    package_version = (
        "2.1.0"
        if portrait_provider and subject_kind == "human_avatar"
        else "2.0.1"
        if portrait_provider
        else "2.0.0"
    )
    if portrait_provider:
        rendered = (
            render_semantic_human_animation(photo)
            if subject_kind == "human_avatar"
            else render_portrait_animation(photo, subject_kind)
        )
    else:
        result = (provider or MockProvider()).generate(
            GenerationRequest(
                photo=photo,
                display_name=display_name,
                subject_kind=subject_kind,
            ),
            plan,
        )
        rendered = result.payload
    safe_name = display_name.strip()[:64] or "自定义桌宠"
    identity_hash = sha256(
        photo + b"\0" + safe_name.encode("utf-8") + b"\0" + subject_kind.encode()
    ).hexdigest()
    atlas = canonical_json(rendered["atlas"])
    license_data = {
        "license": "Private local test asset",
        "source": "User-provided photo palette processed by Epet rigged Mock Worker",
    }
    if portrait_provider:
        license_data.update(
            {
                "source": "Locally generated portrait adapted by Epet rigged Worker",
                "portrait_provider": portrait_provider,
                "model_license": "CreativeML OpenRAIL-M",
                "usage_scope": (
                    "Private local technical spike; style LoRA provenance "
                    "review is incomplete"
                ),
            }
        )
    license_json = canonical_json(license_data)
    declared_files = {
        "animation/clips.json": canonical_json(rendered["clips"]),
        "animation/layers.json": canonical_json(rendered["layers"]),
        "animation/render-profile.json": canonical_json(rendered["profile"]),
        "animation/rig.json": canonical_json(rendered["rig"]),
        "atlas/pet.json": atlas,
        "atlas/pet.png": rendered["atlas_png"],
        "license.json": license_json,
        "thumbnail.png": rendered["thumbnail_png"],
    }
    if rendered.get("pose"):
        declared_files["animation/pose.json"] = canonical_json(rendered["pose"])
    manifest = canonical_json(
        {
            "schema_version": 2,
            "package_version": package_version,
            "min_runtime_version": "0.2.0",
            "pet_id": f"pet_{identity_hash[:16]}",
            "name": safe_name,
            "species": "human" if subject_kind == "human_avatar" else "cat",
            "subject_kind": subject_kind,
            "renderer": "sprite_atlas",
            "created_at": "2026-01-01T00:00:00Z",
            "canvas": {"width": CANVAS, "height": CANVAS},
            "atlas": {
                "image": "atlas/pet.png",
                "data": "atlas/pet.json",
                "max_texture_size": 4096,
            },
            "default_scale": 0.8,
            "anchors": {
                "foot": rendered["rig"]["anchors"]["foot"],
                "drag": rendered["rig"]["anchors"]["drag"],
            },
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
            "actions": rendered["actions"],
            "animation": {
                "layers": "animation/layers.json",
                "rig": "animation/rig.json",
                "clips": "animation/clips.json",
                "render_profile": "animation/render-profile.json",
            },
            "generation": {
                "pipeline_version": package_version,
                "template_version": rendered["rig"]["template_id"] + "-1.0.0",
                **(
                    {"portrait_provider": portrait_provider}
                    if portrait_provider
                    else {}
                ),
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
