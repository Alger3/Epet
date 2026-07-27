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


def test_package_is_deterministic_and_self_describing() -> None:
    first = build_epet(source_png(), "测试猫咪")
    second = build_epet(source_png(), "测试猫咪")
    assert sha256(first).digest() == sha256(second).digest()

    with ZipFile(BytesIO(first)) as archive:
        assert archive.namelist() == [
            "manifest.json",
            "atlas/pet.json",
            "atlas/pet.png",
            "license.json",
            "thumbnail.png",
        ]
        manifest = json.loads(archive.read("manifest.json"))
        assert manifest["name"] == "测试猫咪"
        assert manifest["pet_id"].startswith("pet_")
        for declared in manifest["files"]:
            content = archive.read(declared["path"])
            assert declared["size"] == len(content)
            assert declared["sha256"] == sha256(content).hexdigest()

    with ZipFile(BytesIO(build_epet(source_png(), "另一只猫"))) as archive:
        renamed = json.loads(archive.read("manifest.json"))
    assert renamed["pet_id"] != manifest["pet_id"]
