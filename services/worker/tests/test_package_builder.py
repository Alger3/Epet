from hashlib import sha256
from io import BytesIO
import json
from zipfile import ZipFile

from PIL import Image

from epet_worker.package_builder import build_epet


def source_png() -> bytes:
    output = BytesIO()
    Image.new("RGB", (80, 120), (220, 130, 80)).save(output, "PNG")
    return output.getvalue()


def confirmed_portrait_png() -> bytes:
    image = Image.new("RGB", (512, 512), (244, 241, 232))
    for x in range(150, 363):
        for y in range(80, 451):
            if ((x - 256) / 106) ** 2 + ((y - 265) / 185) ** 2 <= 1:
                image.putpixel((x, y), (42, 91, 188))
    output = BytesIO()
    image.save(output, "PNG")
    return output.getvalue()


def test_package_is_deterministic_and_self_describing() -> None:
    first = build_epet(source_png(), "测试猫咪")
    second = build_epet(source_png(), "测试猫咪")
    assert sha256(first).digest() == sha256(second).digest()
    assert sha256(first).hexdigest() == (
        "64a3ad2c3ac684b14a5e75755c0f1538a48d2e6157766940ecc04e36c6c3bbe3"
    )

    with ZipFile(BytesIO(first)) as archive:
        assert archive.namelist() == [
            "manifest.json",
            "animation/clips.json",
            "animation/layers.json",
            "animation/render-profile.json",
            "animation/rig.json",
            "atlas/pet.json",
            "atlas/pet.png",
            "license.json",
            "thumbnail.png",
        ]
        manifest = json.loads(archive.read("manifest.json"))
        assert manifest["name"] == "测试猫咪"
        assert manifest["pet_id"].startswith("pet_")
        assert manifest["schema_version"] == 2
        assert manifest["subject_kind"] == "pet_cat"
        assert set(manifest["actions"]) == {
            "idle",
            "walk",
            "sleep",
            "tap",
            "drag",
            "wake",
            "perch",
            "perch_sleep",
        }
        assert all(
            len(action["frames"]) > 1 for action in manifest["actions"].values()
        )
        assert manifest["actions"]["walk"]["phase_source"] == "distance"
        assert manifest["actions"]["walk"]["stride_length"] == 48
        atlas = json.loads(archive.read("atlas/pet.json"))
        assert sha256(archive.read("atlas/pet.png")).hexdigest() == (
            "61f736f3b08a3fdcb2198c2f64449f9a6474b12cde1a8317908eb160c0fdb6a5"
        )
        assert len(atlas["frames"]) == sum(
            len(action["frames"]) for action in manifest["actions"].values()
        )
        with Image.open(BytesIO(archive.read("atlas/pet.png"))) as atlas_image:
            assert atlas_image.size[0] <= 4096
            assert atlas_image.size[1] <= 4096
            for action in manifest["actions"].values():
                rendered = []
                for frame_name in action["frames"]:
                    rect = atlas["frames"][frame_name]["frame"]
                    rendered.append(
                        sha256(
                            atlas_image.crop(
                                (
                                    rect["x"],
                                    rect["y"],
                                    rect["x"] + rect["w"],
                                    rect["y"] + rect["h"],
                                )
                            ).tobytes()
                        ).digest()
                    )
                assert len(set(rendered)) > 1
        clips = json.loads(archive.read("animation/clips.json"))
        rig = json.loads(archive.read("animation/rig.json"))
        layers = json.loads(archive.read("animation/layers.json"))
        assert clips["clips"]["sleep"]["events"] == ["eyes_close"]
        assert any(bone["id"] == "tail_04" for bone in rig["bones"])
        assert any(part["id"] == "eyes_closed" for part in layers["parts"])
        for declared in manifest["files"]:
            content = archive.read(declared["path"])
            assert declared["size"] == len(content)
            assert declared["sha256"] == sha256(content).hexdigest()

    with ZipFile(BytesIO(build_epet(source_png(), "另一只猫"))) as archive:
        renamed = json.loads(archive.read("manifest.json"))
    assert renamed["pet_id"] != manifest["pet_id"]


def test_human_template_has_limbs_secondary_motion_and_distinct_identity() -> None:
    package = build_epet(source_png(), "测试人物", "human_avatar")
    assert sha256(package).hexdigest() == (
        "eedc57a40a2426772fd6cd0075352264e49165d6644a88a23ca3b825a998392c"
    )
    with ZipFile(BytesIO(package)) as archive:
        manifest = json.loads(archive.read("manifest.json"))
        rig = json.loads(archive.read("animation/rig.json"))
        clips = json.loads(archive.read("animation/clips.json"))
        assert sha256(archive.read("atlas/pet.png")).hexdigest() == (
            "259cedac5590c9b0968e0d70e3beca29acff2b5d21b54a9fbc9a5f0377acf580"
        )
    assert manifest["species"] == "human"
    assert manifest["subject_kind"] == "human_avatar"
    assert any(bone["id"] == "forearm_l" for bone in rig["bones"])
    assert any(bone["id"] == "hair_back" for bone in rig["bones"])
    assert "secondary_motion.rotation" in clips["clips"]["walk"]["channels"]


def test_confirmed_portrait_is_preserved_in_installed_atlas() -> None:
    package = build_epet(
        confirmed_portrait_png(),
        "OpenVINO portrait",
        "human_avatar",
        portrait_provider="openvino-gpu",
    )
    with ZipFile(BytesIO(package)) as archive:
        manifest = json.loads(archive.read("manifest.json"))
        profile = json.loads(archive.read("animation/render-profile.json"))
        layers = json.loads(archive.read("animation/layers.json"))
        with Image.open(BytesIO(archive.read("thumbnail.png"))) as thumbnail:
            rgba = thumbnail.convert("RGBA")
            center = rgba.getpixel((128, 128))
            corner = rgba.getpixel((0, 0))
    assert manifest["generation"]["portrait_provider"] == "openvino-gpu"
    assert manifest["package_version"] == "2.1.0"
    assert manifest["generation"]["pipeline_version"] == "2.1.0"
    assert manifest["generation"]["template_version"].startswith(
        "human-semantic-cutout"
    )
    assert profile["profile_id"] == "semantic-human-rig-v1"
    assert any(
        part["source"] == "semantic_portrait" for part in layers["parts"]
    )
    assert {
        "head",
        "torso",
        "arm_l",
        "arm_r",
        "leg_l",
        "leg_r",
    }.issubset({part["id"] for part in layers["parts"]})
    assert center[2] > center[0]
    assert center[3] > 200
    assert corner[3] == 0
