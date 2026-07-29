from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import threading

import numpy as np
from PIL import Image


OPENPOSE_KEYPOINTS = (
    "nose",
    "neck",
    "shoulder_r",
    "elbow_r",
    "wrist_r",
    "shoulder_l",
    "elbow_l",
    "wrist_l",
    "hip_r",
    "knee_r",
    "ankle_r",
    "hip_l",
    "knee_l",
    "ankle_l",
    "eye_r",
    "eye_l",
    "ear_r",
    "ear_l",
)


@dataclass(frozen=True)
class PosePoint:
    x: float
    y: float
    confidence: float
    detected: bool

    def to_dict(self) -> dict:
        return {
            "x": round(self.x, 6),
            "y": round(self.y, 6),
            "confidence": round(self.confidence, 6),
            "detected": self.detected,
        }


@dataclass(frozen=True)
class HumanPose:
    points: dict[str, PosePoint]
    source: str
    detected_count: int
    mean_confidence: float

    def to_dict(self) -> dict:
        return {
            "schema_version": 1,
            "subject_kind": "human_avatar",
            "model": "human-pose-estimation-0001",
            "source": self.source,
            "detected_count": self.detected_count,
            "mean_confidence": round(self.mean_confidence, 6),
            "keypoints": {
                name: point.to_dict() for name, point in self.points.items()
            },
        }


class HumanPoseEstimator:
    def __init__(
        self,
        model_path: Path,
        *,
        device: str = "CPU",
        confidence_threshold: float = 0.08,
    ) -> None:
        self.model_path = model_path
        self.device = device
        self.confidence_threshold = confidence_threshold
        self._compiled = None
        self._lock = threading.Lock()

    def _load(self):
        if self._compiled is not None:
            return self._compiled
        if not self.model_path.is_file():
            return None
        try:
            import openvino as ov

            self._compiled = ov.Core().compile_model(
                self.model_path,
                self.device,
                {"PERFORMANCE_HINT": "LATENCY"},
            )
        except Exception:
            return None
        return self._compiled

    @staticmethod
    def _input(image: Image.Image) -> np.ndarray:
        background = Image.new("RGB", image.size, "white")
        rgba = image.convert("RGBA")
        background.paste(rgba.convert("RGB"), mask=rgba.getchannel("A"))
        resized = background.resize((456, 256), Image.Resampling.BILINEAR)
        # The OMZ model expects BGR pixels in the 0..255 range.
        values = np.asarray(resized, dtype=np.float32)[..., ::-1]
        return np.transpose(values, (2, 0, 1))[None]

    @staticmethod
    def _template(image: Image.Image) -> dict[str, tuple[float, float]]:
        alpha = image.convert("RGBA").getchannel("A")
        bounds = alpha.point(lambda value: 255 if value >= 24 else 0).getbbox()
        if bounds is None:
            bounds = (0, 0, image.width, image.height)
        left, top, right, bottom = bounds
        width = max(1, right - left)
        height = max(1, bottom - top)

        def point(x: float, y: float) -> tuple[float, float]:
            return (
                (left + width * x) / image.width,
                (top + height * y) / image.height,
            )

        return {
            "nose": point(0.50, 0.16),
            "neck": point(0.50, 0.31),
            "shoulder_r": point(0.66, 0.34),
            "elbow_r": point(0.72, 0.50),
            "wrist_r": point(0.75, 0.66),
            "shoulder_l": point(0.34, 0.34),
            "elbow_l": point(0.28, 0.50),
            "wrist_l": point(0.25, 0.66),
            "hip_r": point(0.59, 0.61),
            "knee_r": point(0.61, 0.76),
            "ankle_r": point(0.62, 0.94),
            "hip_l": point(0.41, 0.61),
            "knee_l": point(0.39, 0.76),
            "ankle_l": point(0.38, 0.94),
            "eye_r": point(0.55, 0.14),
            "eye_l": point(0.45, 0.14),
            "ear_r": point(0.61, 0.16),
            "ear_l": point(0.39, 0.16),
        }

    def estimate(self, image: Image.Image) -> HumanPose:
        template = self._template(image)
        compiled = self._load()
        detected: dict[str, PosePoint] = {}
        if compiled is not None:
            with self._lock:
                result = compiled([self._input(image)])
            heatmaps = None
            for output in compiled.outputs:
                candidate = np.asarray(result[output])
                if candidate.ndim == 4 and candidate.shape[1] == 19:
                    heatmaps = candidate[0, :18]
                    break
            if heatmaps is not None:
                heatmap_height, heatmap_width = heatmaps.shape[1:]
                for index, name in enumerate(OPENPOSE_KEYPOINTS):
                    flat_index = int(np.argmax(heatmaps[index]))
                    y, x = np.unravel_index(flat_index, heatmaps[index].shape)
                    confidence = float(heatmaps[index, y, x])
                    if confidence >= self.confidence_threshold:
                        detected[name] = PosePoint(
                            x=x / max(1, heatmap_width - 1),
                            y=y / max(1, heatmap_height - 1),
                            confidence=confidence,
                            detected=True,
                        )

        points = {}
        for name in OPENPOSE_KEYPOINTS:
            if name in detected:
                points[name] = detected[name]
            else:
                x, y = template[name]
                points[name] = PosePoint(
                    x=x,
                    y=y,
                    confidence=0.0,
                    detected=False,
                )
        confidences = [point.confidence for point in detected.values()]
        count = len(detected)
        source = (
            "openvino"
            if count >= 12
            else "hybrid"
            if count >= 6
            else "template_fallback"
        )
        return HumanPose(
            points=points,
            source=source,
            detected_count=count,
            mean_confidence=(
                sum(confidences) / len(confidences) if confidences else 0.0
            ),
        )
