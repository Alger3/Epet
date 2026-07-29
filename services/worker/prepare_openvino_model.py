from __future__ import annotations

import argparse
from hashlib import sha256
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import mkdtemp
from time import perf_counter, sleep


WORKER_ROOT = Path(__file__).resolve().parent
BASE_FILES = (
    "feature_extractor/preprocessor_config.json",
    "model_index.json",
    "scheduler/scheduler_config.json",
    "text_encoder/config.json",
    "text_encoder/model.fp16.safetensors",
    "tokenizer/merges.txt",
    "tokenizer/special_tokens_map.json",
    "tokenizer/tokenizer_config.json",
    "tokenizer/vocab.json",
    "unet/config.json",
    "unet/diffusion_pytorch_model.fp16.safetensors",
    "vae/config.json",
    "vae/diffusion_pytorch_model.fp16.safetensors",
)


def file_hash(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def emit(stage: str, **details) -> None:
    print(json.dumps({"stage": stage, **details}, ensure_ascii=False), flush=True)


def download_file(
    _client,
    repository: str,
    revision: str,
    filename: str,
    target: Path,
    *,
    expected_size: int,
    expected_hash: str | None,
) -> Path:
    if (
        target.is_file()
        and target.stat().st_size == expected_size
        and (not expected_hash or file_hash(target) == expected_hash)
    ):
        return target
    target.parent.mkdir(parents=True, exist_ok=True)
    partial = target.with_name(target.name + ".partial")
    url = f"https://huggingface.co/{repository}/resolve/{revision}/{filename}"
    if partial.is_file() and partial.stat().st_size > expected_size:
        partial.unlink()
    curl = shutil.which("curl.exe") or shutil.which("curl")
    if not curl:
        raise RuntimeError("curl is required for resumable model downloads")
    failures_without_progress = 0
    while not partial.is_file() or partial.stat().st_size < expected_size:
        before = partial.stat().st_size if partial.is_file() else 0
        emit(
            "download_start",
            file=filename,
            downloaded=before,
            total=expected_size,
        )
        completed = subprocess.run(
            [
                curl,
                "--fail",
                "--location",
                "--connect-timeout",
                "30",
                "--max-time",
                "180",
                "--continue-at",
                "-",
                "--output",
                str(partial),
                url,
            ],
            check=False,
        )
        after = partial.stat().st_size if partial.is_file() else 0
        if after > before:
            failures_without_progress = 0
        else:
            failures_without_progress += 1
        if after == expected_size:
            break
        emit(
            "download_resuming",
            file=filename,
            downloaded=after,
            total=expected_size,
            curl_exit=completed.returncode,
        )
        if after > expected_size or failures_without_progress >= 8:
            raise RuntimeError(
                f"curl download failed for {filename} with "
                f"{completed.returncode}; {after} bytes retained"
            )
        sleep(2)
    if partial.stat().st_size != expected_size:
        raise RuntimeError(
            f"Size mismatch for {filename}: expected {expected_size}, "
            f"got {partial.stat().st_size}"
        )
    if expected_hash:
        actual = file_hash(partial)
        if actual != expected_hash:
            raise RuntimeError(
                f"SHA-256 mismatch for {filename}: expected {expected_hash}, got {actual}"
            )
    partial.replace(target)
    return target


def repository_files(client, repository: str, revision: str) -> dict[str, dict]:
    url = (
        f"https://huggingface.co/api/models/{repository}/revision/{revision}"
        "?blobs=true"
    )
    response = None
    for attempt in range(1, 9):
        try:
            response = client.get(url)
            response.raise_for_status()
            break
        except Exception as error:
            if attempt == 8:
                raise
            emit(
                "metadata_retry",
                repository=repository,
                attempt=attempt,
                reason=type(error).__name__,
            )
            sleep(min(attempt * 2, 10))
    if response is None:
        raise RuntimeError(f"No metadata response for {repository}")
    metadata = response.json()
    if metadata.get("sha") != revision:
        raise RuntimeError(f"Repository revision mismatch for {repository}")
    return {item["rfilename"]: item for item in metadata["siblings"]}


def download_sources(client, sources: dict, source_dir: Path) -> tuple[Path, Path]:
    lora = sources["style_lora"]
    lora_files = repository_files(client, lora["repository"], lora["revision"])
    lora_meta = lora_files[lora["filename"]]
    emit("downloading_lora", repository=lora["repository"])
    lora_path = download_file(
        client,
        lora["repository"],
        lora["revision"],
        lora["filename"],
        source_dir / "lora" / lora["filename"],
        expected_size=int(lora_meta["size"]),
        expected_hash=lora["sha256"],
    )

    base = sources["base_model"]
    base_files = repository_files(client, base["repository"], base["revision"])
    base_dir = source_dir / "base"
    for index, filename in enumerate(BASE_FILES, start=1):
        metadata = base_files[filename]
        lfs = metadata.get("lfs") or {}
        emit(
            "downloading_base_file",
            file=filename,
            index=index,
            total=len(BASE_FILES),
            size=metadata["size"],
        )
        download_file(
            client,
            base["repository"],
            base["revision"],
            filename,
            base_dir / filename,
            expected_size=int(metadata["size"]),
            expected_hash=lfs.get("sha256"),
        )
    return lora_path, base_dir


def verify_lora(path: Path, expected_hash: str) -> None:
    actual = file_hash(path)
    if actual != expected_hash:
        raise RuntimeError(
            f"LoRA SHA-256 mismatch: expected {expected_hash}, got {actual}"
        )
    from safetensors import safe_open

    with safe_open(path, framework="pt", device="cpu") as weights:
        keys = list(weights.keys())
    if not keys or not any(
        key.startswith(("lora_unet_", "unet.", "text_encoder."))
        for key in keys
    ):
        raise RuntimeError("The candidate file does not look like an SD LoRA")
    emit("lora_verified", sha256=actual, tensors=len(keys))


def write_directory_manifest(
    model_dir: Path, sources: dict, conversion_seconds: float
) -> None:
    declared = []
    for path in sorted(model_dir.rglob("*")):
        if not path.is_file() or path.name == ".epet-model.json":
            continue
        relative = path.relative_to(model_dir).as_posix()
        declared.append(
            {
                "path": relative,
                "size": path.stat().st_size,
                "sha256": file_hash(path),
            }
        )
    marker = {
        "schema_version": 1,
        "model_id": "epet-portrait-openvino-v1",
        "workflow_id": sources["workflow_id"],
        "base_model": sources["base_model"],
        "style_lora": sources["style_lora"],
        "export": sources["export"],
        "conversion_seconds": round(conversion_seconds, 3),
        "files": declared,
    }
    (model_dir / ".epet-model.json").write_text(
        json.dumps(marker, ensure_ascii=False, sort_keys=True, indent=2),
        encoding="utf-8",
    )


def prepare(cache_root: Path, keep_merged: bool) -> Path:
    from prepare_foreground_model import prepare as prepare_foreground
    from prepare_pose_model import prepare as prepare_pose

    prepare_foreground(cache_root)
    prepare_pose(cache_root)
    sources = json.loads(
        (WORKER_ROOT / "model-sources.json").read_text(encoding="utf-8")
    )
    target = (
        cache_root
        / "openvino-gpu"
        / "epet-portrait-openvino-v1"
        / "1.0.0"
    )
    staging_parent = cache_root / ".staging"
    staging_parent.mkdir(parents=True, exist_ok=True)
    work = Path(mkdtemp(prefix="openvino-img2img-", dir=staging_parent))
    source_dir = cache_root / ".downloads"
    merged_dir = work / "merged"
    exported_dir = work / "exported"
    started = perf_counter()
    try:
        import httpx

        with httpx.Client(
            follow_redirects=True,
            timeout=httpx.Timeout(45, connect=30),
        ) as client:
            lora_path, base_dir = download_sources(
                client,
                sources,
                source_dir,
            )
        lora = sources["style_lora"]
        verify_lora(lora_path, lora["sha256"])

        emit("loading_base_model", repository=sources["base_model"]["repository"])
        emit("fusing_lora", scale=lora["scale"])
        merger = WORKER_ROOT / "merge_sd15_lora.py"
        completed = subprocess.run(
            [
                sys.executable,
                str(merger),
                "--base-dir",
                str(base_dir),
                "--lora",
                str(lora_path),
                "--output-dir",
                str(merged_dir),
                "--scale",
                str(lora["scale"]),
            ],
            check=False,
        )
        if completed.returncode:
            raise RuntimeError(
                f"SD1.5 LoRA merger exited with {completed.returncode}"
            )

        emit("exporting_openvino_fp16", output=str(exported_dir))
        exported_dir.mkdir(parents=True, exist_ok=True)
        component_exporter = WORKER_ROOT / "export_openvino_component.py"
        for component in (
            "text_encoder",
            "unet",
            "vae_encoder",
            "vae_decoder",
        ):
            emit("exporting_component", component=component)
            completed = subprocess.run(
                [
                    sys.executable,
                    str(component_exporter),
                    "--model-dir",
                    str(merged_dir),
                    "--output-dir",
                    str(exported_dir),
                    "--component",
                    component,
                ],
                check=False,
            )
            if completed.returncode:
                raise RuntimeError(
                    f"OpenVINO {component} exporter exited with "
                    f"{completed.returncode}"
                )
        for asset in ("feature_extractor", "scheduler", "tokenizer"):
            source = merged_dir / asset
            if source.is_dir():
                shutil.copytree(source, exported_dir / asset)
        shutil.copy2(
            merged_dir / "model_index.json",
            exported_dir / "model_index.json",
        )
        conversion_seconds = perf_counter() - started
        write_directory_manifest(exported_dir, sources, conversion_seconds)

        target.parent.mkdir(parents=True, exist_ok=True)
        backup = target.with_name(target.name + ".previous")
        resolved_cache = cache_root.resolve()
        for candidate in (target, backup):
            resolved = candidate.resolve()
            if resolved == resolved_cache or resolved_cache not in resolved.parents:
                raise RuntimeError("Resolved model target escaped the cache root")
        if backup.exists():
            shutil.rmtree(backup)
        if target.exists():
            target.replace(backup)
        try:
            exported_dir.replace(target)
        except Exception:
            if backup.exists() and not target.exists():
                backup.replace(target)
            raise
        if backup.exists():
            shutil.rmtree(backup)
        emit(
            "ready",
            path=str(target),
            conversion_seconds=round(conversion_seconds, 3),
        )
        if keep_merged:
            kept = cache_root / "sources" / "epet-sd15-chibi-merged"
            if kept.exists():
                shutil.rmtree(kept)
            kept.parent.mkdir(parents=True, exist_ok=True)
            merged_dir.replace(kept)
        return target
    finally:
        shutil.rmtree(work, ignore_errors=True)


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
    parser.add_argument("--keep-merged", action="store_true")
    args = parser.parse_args()
    prepare(args.cache_root, args.keep_merged)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
