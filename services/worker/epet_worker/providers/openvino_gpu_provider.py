from __future__ import annotations

from hashlib import sha256
from io import BytesIO
import inspect
import os
from pathlib import Path
import threading
from time import perf_counter, sleep
from typing import Any

import numpy as np
from PIL import Image, ImageOps

from ..foreground_segmenter import ForegroundSegmenter
from .base import GenerationProvider
from .contracts import (
    GenerationPlan,
    GenerationRequest,
    GenerationResult,
    ProviderError,
)


PET_PROMPT = (
    "full body, chibi, blind box figurine of the same pet, super-deformed, "
    "oversized head and tiny body, standing full body from ears to paws, "
    "centered, clean silhouette, large expressive eyes, simple shapes, "
    "smooth PVC style, isolated on a pure white background"
)
HUMAN_PROMPT = (
    "full body, chibi, blind box figurine of the same authorized adult person, "
    "super-deformed, oversized head and tiny body, one complete standing character, "
    "full body visible from hair to shoes, centered, large expressive eyes, "
    "simplified proportions, smooth PVC style, isolated on a pure white background"
)
NEGATIVE_PROMPT = (
    "child, minor, photorealistic, realistic face, portrait photo, headshot, close-up, "
    "bust shot, cropped body, 3d render, multiple subjects, background scenery, "
    "abstract, sketch, doodle, floating limbs, detached limbs, border, frame, "
    "text, handwriting, watermark, logo, extra limbs, missing limbs, deformed hands, "
    "blurry, low quality"
)


class _PeakRssSampler:
    def __init__(self) -> None:
        self._stopped = threading.Event()
        self.peak_bytes = 0
        self._thread: threading.Thread | None = None

    def __enter__(self):
        import psutil

        process = psutil.Process()

        def sample() -> None:
            while not self._stopped.is_set():
                self.peak_bytes = max(self.peak_bytes, process.memory_info().rss)
                sleep(0.02)

        self._thread = threading.Thread(target=sample, daemon=True)
        self._thread.start()
        return self

    def __exit__(self, *_args) -> None:
        self._stopped.set()
        if self._thread:
            self._thread.join(timeout=1)


class OpenVinoGpuProvider(GenerationProvider):
    provider_id = "openvino-gpu"

    def __init__(self, model_dir: Path, *, device: str = "GPU") -> None:
        self.model_dir = model_dir
        self.device = device
        worker_root = Path(__file__).resolve().parents[2]
        cache_root = Path(
            os.environ.get(
                "EPET_MODEL_CACHE_DIR",
                str(worker_root / ".model-cache"),
            )
        )
        self._foreground = ForegroundSegmenter(
            cache_root / "foreground" / "u2netp" / "1.0.0" / "u2netp.onnx",
            device=os.environ.get("EPET_FOREGROUND_DEVICE", "CPU"),
        )
        self._pipeline: Any = None
        self._load_time_ms: float | None = None
        self._lock = threading.Lock()

    def _ensure_not_cancelled(self, request: GenerationRequest) -> None:
        if request.cancellation_check and request.cancellation_check():
            raise ProviderError(
                "GENERATION_CANCELLED",
                "生成任务已取消。",
                provider_id=self.provider_id,
                retryable=False,
            )

    def _load(self) -> tuple[Any, float]:
        if self._pipeline is not None:
            return self._pipeline, 0.0
        marker = self.model_dir / ".epet-model.json"
        if not marker.is_file():
            raise ProviderError(
                "MODEL_NOT_DOWNLOADED",
                "OpenVINO FP16 模型尚未准备完成。",
                provider_id=self.provider_id,
            )
        try:
            from optimum.intel import OVStableDiffusionImg2ImgPipeline
        except Exception as error:
            raise ProviderError(
                "OPENVINO_PROVIDER_IMPORT_FAILED",
                f"无法加载 Optimum Intel：{type(error).__name__}",
                provider_id=self.provider_id,
                details={"message": str(error)[:240]},
            ) from error
        started = perf_counter()
        try:
            pipeline = OVStableDiffusionImg2ImgPipeline.from_pretrained(
                self.model_dir,
                device=self.device,
                compile=False,
                dynamic_shapes=False,
                ov_config={
                    "PERFORMANCE_HINT": "LATENCY",
                    "CACHE_DIR": str(self.model_dir / ".compiled-cache"),
                },
            )
            pipeline.reshape(
                batch_size=1,
                height=512,
                width=512,
                num_images_per_prompt=1,
            )
            pipeline.compile()
        except Exception as error:
            raise ProviderError(
                "OPENVINO_MODEL_LOAD_FAILED",
                "OpenVINO FP16 模型加载或编译失败。",
                provider_id=self.provider_id,
                retryable=True,
                details={"message": str(error)[:240]},
            ) from error
        self._pipeline = pipeline
        self._load_time_ms = round((perf_counter() - started) * 1000, 3)
        return pipeline, self._load_time_ms

    def _prepare_image(self, photo: bytes, subject_kind: str) -> Image.Image:
        try:
            with Image.open(BytesIO(photo)) as source:
                image = ImageOps.exif_transpose(source).convert("RGBA")
        except Exception as error:
            raise ProviderError(
                "INPUT_IMAGE_INVALID",
                "无法解码用于 img2img 的照片。",
                provider_id="openvino-gpu",
            ) from error
        # Never feed a rectangular photo/card into img2img: SD preserves that
        # boundary and later background removal sees the whole card as the
        # salient object. Segment the source first, then compose only the
        # authorized subject over the model's requested white background.
        image, _metrics = self._foreground.remove_background(image)
        limit = (336, 336) if subject_kind == "human_avatar" else (384, 384)
        reference = ImageOps.contain(image, limit, Image.Resampling.LANCZOS)
        canvas = Image.new("RGB", (512, 512), (255, 255, 255))
        x = (512 - reference.width) // 2
        y = (512 - reference.height) // 2
        canvas.paste(reference.convert("RGB"), (x, y), reference.getchannel("A"))
        return canvas

    def _cancellation_arguments(
        self, pipeline: Any, request: GenerationRequest
    ) -> dict[str, Any]:
        call_class = getattr(pipeline, "auto_model_class", pipeline.__class__)
        parameters = inspect.signature(call_class.__call__).parameters
        if "callback_on_step_end" in parameters:
            def on_step_end(_pipe, _step, _timestep, callback_kwargs):
                self._ensure_not_cancelled(request)
                return callback_kwargs

            return {"callback_on_step_end": on_step_end}
        if "callback" in parameters:
            def callback(_step, _timestep, _latents):
                self._ensure_not_cancelled(request)

            return {"callback": callback, "callback_steps": 1}
        return {}

    def cutout_portrait(
        self, image: Image.Image
    ) -> tuple[Image.Image, dict[str, Any]]:
        rgba = image.convert("RGBA")
        alpha = np.asarray(rgba.getchannel("A"), dtype=np.uint8)
        if float(np.count_nonzero(alpha < 32)) / alpha.size >= 0.02:
            # The preview already passed this exact segmenter before user
            # confirmation. Preserve it byte-for-byte at the visual layer
            # instead of running a second mask that could erode hair or fur.
            return rgba, {"foreground_already_transparent": True}
        cutout, metrics = self._foreground.remove_background(rgba)
        if metrics.touches_edge:
            raise ProviderError(
                "FOREGROUND_CROPPED",
                "生成角色碰到画布边缘，无法制作完整桌宠，请重新生成。",
                provider_id=self.provider_id,
                retryable=True,
                details=metrics.to_dict(),
            )
        return cutout, metrics.to_dict()

    def generate(
        self, request: GenerationRequest, plan: GenerationPlan | None = None
    ) -> GenerationResult:
        self._ensure_not_cancelled(request)
        seed = request.seed
        if seed is None:
            seed = int.from_bytes(
                sha256(
                    request.photo
                    + b"\0"
                    + request.subject_kind.encode("utf-8")
                    + b"\0epet-openvino-img2img-v1"
                ).digest()[:8],
                "big",
            ) % (2**31)
        image = self._prepare_image(request.photo, request.subject_kind)
        prompt = HUMAN_PROMPT if request.subject_kind == "human_avatar" else PET_PROMPT
        with self._lock, _PeakRssSampler() as memory:
            self._ensure_not_cancelled(request)
            pipeline, load_ms = self._load()
            try:
                import torch

                generator = torch.Generator(device="cpu").manual_seed(seed)
                started = perf_counter()
                output = pipeline(
                    prompt=prompt,
                    negative_prompt=NEGATIVE_PROMPT,
                    image=image,
                    strength=float(os.environ.get("EPET_IMG2IMG_STRENGTH", "0.72")),
                    guidance_scale=float(
                        os.environ.get("EPET_IMG2IMG_GUIDANCE_SCALE", "8.0")
                    ),
                    num_inference_steps=int(
                        os.environ.get("EPET_IMG2IMG_STEPS", "24")
                    ),
                    generator=generator,
                    output_type="pil",
                    **self._cancellation_arguments(pipeline, request),
                )
                inference_ms = round((perf_counter() - started) * 1000, 3)
            except ProviderError:
                raise
            except Exception as error:
                raise ProviderError(
                    "OPENVINO_INFERENCE_FAILED",
                    "OpenVINO img2img 推理失败。",
                    provider_id=self.provider_id,
                    retryable=True,
                    details={"message": str(error)[:240]},
                ) from error
        self._ensure_not_cancelled(request)
        portrait, foreground_metrics = self.cutout_portrait(output.images[0])
        encoded = BytesIO()
        portrait.save(encoded, "PNG", optimize=False)
        return GenerationResult(
            provider_id="openvino-gpu",
            device_id=plan.device_id if plan else self.device,
            model_id=plan.model_id if plan else "epet-portrait-openvino-v1",
            payload={
                "portrait_png": encoded.getvalue(),
                "width": portrait.width,
                "height": portrait.height,
            },
            diagnostics={
                "seed": seed,
                "load_time_ms": load_ms,
                "cold_load_time_ms": self._load_time_ms,
                "inference_time_ms": inference_ms,
                "peak_process_memory_mb": round(
                    memory.peak_bytes / (1024 * 1024), 2
                ),
                "device": self.device,
                "width": 512,
                "height": 512,
                "batch_size": 1,
                "identity_conditioning": "segmented-img2img-reference-v1",
                **foreground_metrics,
            },
        )
