from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import threading

import numpy as np
from PIL import Image, ImageFilter

from .providers.contracts import ProviderError


@dataclass(frozen=True)
class ForegroundMetrics:
    coverage: float
    bounds: tuple[int, int, int, int]
    bounding_box_fill: float
    touches_edge: bool

    def to_dict(self) -> dict:
        return {
            "foreground_coverage": round(self.coverage, 4),
            "foreground_bounds": list(self.bounds),
            "foreground_bounding_box_fill": round(self.bounding_box_fill, 4),
            "foreground_touches_edge": self.touches_edge,
        }


class ForegroundSegmenter:
    """Small OpenVINO U²-NetP foreground extractor.

    The model is deliberately separate from the diffusion pipeline so the
    preview shown to the user is the same transparent cutout later packaged
    into the desktop character.
    """

    def __init__(self, model_path: Path, *, device: str = "CPU") -> None:
        self.model_path = model_path
        self.device = device
        self._compiled = None
        self._lock = threading.Lock()

    def _load(self):
        if self._compiled is not None:
            return self._compiled
        if not self.model_path.is_file():
            raise ProviderError(
                "FOREGROUND_MODEL_NOT_DOWNLOADED",
                "前景分割模型尚未准备，请先运行 npm run prepare:model:foreground。",
                provider_id="openvino-gpu",
                retryable=True,
            )
        try:
            import openvino as ov

            self._compiled = ov.Core().compile_model(
                self.model_path,
                self.device,
                {"PERFORMANCE_HINT": "LATENCY"},
            )
        except Exception as error:
            raise ProviderError(
                "FOREGROUND_MODEL_LOAD_FAILED",
                "前景分割模型加载失败。",
                provider_id="openvino-gpu",
                retryable=True,
                details={"message": str(error)[:240]},
            ) from error
        return self._compiled

    @staticmethod
    def _input(image: Image.Image) -> np.ndarray:
        resized = image.convert("RGB").resize(
            (320, 320),
            Image.Resampling.LANCZOS,
        )
        values = np.asarray(resized, dtype=np.float32) / 255.0
        values = (values - np.asarray((0.485, 0.456, 0.406))) / np.asarray(
            (0.229, 0.224, 0.225)
        )
        return np.transpose(values, (2, 0, 1))[None].astype(np.float32)

    def remove_background(
        self, image: Image.Image
    ) -> tuple[Image.Image, ForegroundMetrics]:
        with self._lock:
            compiled = self._load()
            try:
                prediction = np.asarray(compiled([self._input(image)])[0])
            except Exception as error:
                raise ProviderError(
                    "FOREGROUND_INFERENCE_FAILED",
                    "角色前景分割失败。",
                    provider_id="openvino-gpu",
                    retryable=True,
                    details={"message": str(error)[:240]},
                ) from error

        mask_values = np.squeeze(prediction).astype(np.float32)
        low = float(mask_values.min())
        high = float(mask_values.max())
        if high - low < 1e-6:
            raise ProviderError(
                "FOREGROUND_EMPTY",
                "没有从生成图中识别到可用角色。",
                provider_id="openvino-gpu",
                retryable=True,
            )
        mask_values = np.clip((mask_values - low) / (high - low), 0, 1)
        mask = Image.fromarray(
            np.rint(mask_values * 255).astype(np.uint8)
        ).resize(image.size, Image.Resampling.LANCZOS)
        # Suppress U²-Net's faint full-canvas haze, retain hair/fur softness,
        # then feather one pixel to avoid a hard sticker edge.
        mask = mask.point(
            lambda value: max(0, min(255, round((value - 18) * 255 / 205)))
        ).filter(ImageFilter.GaussianBlur(0.8))

        solid = mask.point(lambda value: 255 if value >= 32 else 0)
        bounds = solid.getbbox()
        if bounds is None:
            raise ProviderError(
                "FOREGROUND_EMPTY",
                "生成图只有背景，没有识别到完整角色。",
                provider_id="openvino-gpu",
                retryable=True,
            )
        coverage = sum(1 for value in solid.getdata() if value) / (
            image.width * image.height
        )
        bounds_area = max(1, (bounds[2] - bounds[0]) * (bounds[3] - bounds[1]))
        bounding_box_fill = (
            sum(1 for value in solid.crop(bounds).getdata() if value)
            / bounds_area
        )
        margin = max(2, min(image.size) // 100)
        touches_edge = (
            bounds[0] <= margin
            or bounds[1] <= margin
            or bounds[2] >= image.width - margin
            or bounds[3] >= image.height - margin
        )
        if coverage < 0.015 or coverage > 0.82 or bounding_box_fill > 0.88:
            raise ProviderError(
                "FOREGROUND_QUALITY_FAILED",
                "生成图仍像矩形照片或没有形成独立桌宠前景，请重新生成。",
                provider_id="openvino-gpu",
                retryable=True,
                details={
                    "coverage": round(coverage, 4),
                    "bounding_box_fill": round(bounding_box_fill, 4),
                },
            )

        output = image.convert("RGBA")
        output.putalpha(mask)
        return output, ForegroundMetrics(
            coverage,
            bounds,
            bounding_box_fill,
            touches_edge,
        )
