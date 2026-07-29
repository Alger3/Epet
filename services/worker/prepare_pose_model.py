from __future__ import annotations

import argparse
from hashlib import sha256
import os
from pathlib import Path
import shutil
import subprocess


WORKER_ROOT = Path(__file__).resolve().parent
MODEL_BASE_URL = (
    "https://storage.openvinotoolkit.org/repositories/open_model_zoo/"
    "2022.1/models_bin/3/human-pose-estimation-0001/FP16/"
    "human-pose-estimation-0001"
)
FILES = {
    "human-pose-estimation-0001.xml": {
        "size": 248_520,
        "sha256": "2907250931ecb23a9d321278ac58fc1fff1d0c14bf056c0fe6206aa862ab234f",
    },
    "human-pose-estimation-0001.bin": {
        "size": 8_197_356,
        "sha256": "1085b20f87b69de129a0dcac2dbb4aba7c18e044eaa4fb298239ab05ebd56efe",
    },
}


def file_hash(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def prepare(cache_root: Path) -> Path:
    target = (
        cache_root
        / "pose"
        / "human-pose-estimation-0001"
        / "1.0.0"
    )
    target.mkdir(parents=True, exist_ok=True)
    curl = shutil.which("curl.exe") or shutil.which("curl")
    if not curl:
        raise RuntimeError("curl is required to download the pose model")
    for filename, expected in FILES.items():
        destination = target / filename
        if (
            destination.is_file()
            and destination.stat().st_size == expected["size"]
            and file_hash(destination) == expected["sha256"]
        ):
            continue
        partial = destination.with_suffix(destination.suffix + ".partial")
        completed = subprocess.run(
            [
                curl,
                "--fail",
                "--location",
                "--connect-timeout",
                "30",
                "--max-time",
                "180",
                "--output",
                str(partial),
                f"{MODEL_BASE_URL}{destination.suffix}",
            ],
            check=False,
        )
        if completed.returncode:
            raise RuntimeError(
                f"Pose model download exited with {completed.returncode}"
            )
        if partial.stat().st_size != expected["size"]:
            raise RuntimeError(f"Pose model size mismatch for {filename}")
        actual = file_hash(partial)
        if actual != expected["sha256"]:
            raise RuntimeError(
                f"Pose model SHA-256 mismatch for {filename}: {actual}"
            )
        partial.replace(destination)
    print(f"Pose model ready: {target}")
    return target


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--cache-root",
        type=Path,
        default=Path(
            os.environ.get(
                "EPET_MODEL_CACHE_DIR",
                str(WORKER_ROOT / ".model-cache"),
            )
        ),
    )
    args = parser.parse_args()
    prepare(args.cache_root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
