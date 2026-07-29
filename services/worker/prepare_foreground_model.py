from __future__ import annotations

import argparse
from hashlib import sha256
import os
from pathlib import Path
import shutil
import subprocess


WORKER_ROOT = Path(__file__).resolve().parent
MODEL_URL = (
    "https://github.com/danielgatis/rembg/releases/download/v0.0.0/"
    "u2netp.onnx"
)
MODEL_SIZE = 4_574_861
MODEL_SHA256 = (
    "309c8469258dda742793dce0ebea8e6dd393174f89934733ecc8b14c76f4ddd8"
)


def file_hash(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def prepare(cache_root: Path) -> Path:
    target = cache_root / "foreground" / "u2netp" / "1.0.0" / "u2netp.onnx"
    if (
        target.is_file()
        and target.stat().st_size == MODEL_SIZE
        and file_hash(target) == MODEL_SHA256
    ):
        print(f"Foreground model ready: {target}")
        return target

    target.parent.mkdir(parents=True, exist_ok=True)
    partial = target.with_suffix(".onnx.partial")
    curl = shutil.which("curl.exe") or shutil.which("curl")
    if not curl:
        raise RuntimeError("curl is required to download the foreground model")
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
            MODEL_URL,
        ],
        check=False,
    )
    if completed.returncode:
        raise RuntimeError(
            f"Foreground model download exited with {completed.returncode}"
        )
    if partial.stat().st_size != MODEL_SIZE:
        raise RuntimeError(
            f"Foreground model size mismatch: {partial.stat().st_size}"
        )
    actual = file_hash(partial)
    if actual != MODEL_SHA256:
        raise RuntimeError(
            f"Foreground model SHA-256 mismatch: expected {MODEL_SHA256}, got {actual}"
        )
    partial.replace(target)
    print(f"Foreground model ready: {target}")
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
