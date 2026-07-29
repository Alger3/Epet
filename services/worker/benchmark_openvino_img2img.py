from __future__ import annotations

import argparse
from hashlib import sha256
from io import BytesIO
import json
from pathlib import Path
import threading
from time import perf_counter

from PIL import Image, ImageDraw

from epet_worker.providers.contracts import GenerationRequest, ProviderError
from epet_worker.providers.openvino_gpu_provider import OpenVinoGpuProvider


WORKER_ROOT = Path(__file__).resolve().parent
DEFAULT_MODEL = (
    WORKER_ROOT
    / ".model-cache"
    / "openvino-gpu"
    / "epet-portrait-openvino-v1"
    / "1.0.0"
)


def synthetic_reference() -> bytes:
    image = Image.new("RGB", (640, 640), "#e8ded0")
    draw = ImageDraw.Draw(image)
    draw.ellipse((150, 115, 490, 475), fill="#d2813d")
    draw.polygon(((180, 180), (210, 50), (290, 170)), fill="#d2813d")
    draw.polygon(((350, 170), (430, 50), (460, 180)), fill="#d2813d")
    draw.ellipse((230, 240, 270, 285), fill="#202124")
    draw.ellipse((370, 240, 410, 285), fill="#202124")
    draw.polygon(((310, 300), (330, 300), (320, 315)), fill="#bd5b63")
    encoded = BytesIO()
    image.save(encoded, "PNG")
    return encoded.getvalue()


def run_once(
    provider: OpenVinoGpuProvider,
    photo: bytes,
    *,
    subject_kind: str = "pet_cat",
    cancel_check=None,
) -> tuple[dict, bytes, float]:
    started = perf_counter()
    result = provider.generate(
        GenerationRequest(
            photo=photo,
            display_name="Arc 140V benchmark",
            subject_kind=subject_kind,
            seed=20260728,
            cancellation_check=cancel_check,
        )
    )
    elapsed_ms = round((perf_counter() - started) * 1000, 3)
    return result.diagnostics, result.payload["portrait_png"], elapsed_ms


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path)
    parser.add_argument("--model-dir", type=Path, default=DEFAULT_MODEL)
    parser.add_argument("--output-dir", type=Path, default=WORKER_ROOT / "benchmarks")
    parser.add_argument(
        "--subject-kind",
        choices=("pet_cat", "human_avatar"),
        default="pet_cat",
    )
    parser.add_argument("--cancel-after-seconds", type=float, default=2.0)
    args = parser.parse_args()

    photo = args.input.read_bytes() if args.input else synthetic_reference()
    provider = OpenVinoGpuProvider(args.model_dir)
    args.output_dir.mkdir(parents=True, exist_ok=True)

    first_metrics, first_png, first_elapsed = run_once(
        provider,
        photo,
        subject_kind=args.subject_kind,
    )
    second_metrics, second_png, second_elapsed = run_once(
        provider,
        photo,
        subject_kind=args.subject_kind,
    )
    first_hash = sha256(first_png).hexdigest()
    second_hash = sha256(second_png).hexdigest()
    (args.output_dir / "openvino-arc-first.png").write_bytes(first_png)
    (args.output_dir / "openvino-arc-repeat.png").write_bytes(second_png)

    canceled = threading.Event()
    timer = threading.Timer(args.cancel_after_seconds, canceled.set)
    cancel_started = perf_counter()
    timer.start()
    cancellation = {
        "requested_after_seconds": args.cancel_after_seconds,
        "observed": False,
        "latency_ms": None,
        "error_code": None,
    }
    try:
        run_once(
            provider,
            photo,
            subject_kind=args.subject_kind,
            cancel_check=canceled.is_set,
        )
    except ProviderError as error:
        cancellation["observed"] = error.code == "GENERATION_CANCELLED"
        cancellation["error_code"] = error.code
        cancellation["latency_ms"] = round(
            (perf_counter() - cancel_started) * 1000, 3
        )
    finally:
        timer.cancel()

    report = {
        "schema_version": 1,
        "device": "GPU",
        "workload": {
            "task": "image-to-image",
            "width": 512,
            "height": 512,
            "batch_size": 1,
            "seed": 20260728,
            "input": str(args.input) if args.input else "synthetic-reference",
        },
        "cold_run": {**first_metrics, "total_time_ms": first_elapsed},
        "repeat_run": {**second_metrics, "total_time_ms": second_elapsed},
        "repeatability": {
            "first_sha256": first_hash,
            "second_sha256": second_hash,
            "byte_identical": first_hash == second_hash,
        },
        "cancellation": cancellation,
        "memory_note": (
            "peak_process_memory_mb is peak host RSS sampled during the call; "
            "OpenVINO/Level Zero GPU allocation is not exposed by psutil."
        ),
    }
    report_path = args.output_dir / "openvino-arc-140v.json"
    report_path.write_text(
        json.dumps(report, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(json.dumps({"report": str(report_path), **report}, ensure_ascii=False))
    return 0 if cancellation["observed"] and first_hash == second_hash else 2


if __name__ == "__main__":
    raise SystemExit(main())
