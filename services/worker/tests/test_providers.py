from pathlib import Path
import subprocess
import unittest
from io import BytesIO
from types import SimpleNamespace

import numpy as np
from PIL import Image

from epet_worker.foreground_segmenter import ForegroundSegmenter
from epet_worker.human_pose_estimator import HumanPoseEstimator
from epet_worker.providers.contracts import (
    GenerationRequest,
    HardwareDevice,
    HardwareSnapshot,
    ProviderCapability,
    ProviderError,
    RuntimeProbe,
)
from epet_worker.providers.model_manager import ModelManager
from epet_worker.providers.openvino_gpu_provider import OpenVinoGpuProvider
from epet_worker.providers.openvino_probe import OpenVinoProbe
from epet_worker.providers.planner import HardwarePlanner, build_capabilities
from epet_worker.providers.registry import ProviderRegistry


def snapshot() -> HardwareSnapshot:
    return HardwareSnapshot(
        platform="test",
        computer_model="test-computer",
        cpu=HardwareDevice(
            id="cpu:0", kind="cpu", name="Test CPU", available=True
        ),
        gpus=(
            HardwareDevice(
                id="gpu:0",
                kind="gpu",
                name="Test NVIDIA",
                vendor="nvidia",
                runtime="cuda",
                available=True,
            ),
        ),
    )


def capability(
    provider_id: str, available: bool, speed: str, device_id: str
) -> ProviderCapability:
    return ProviderCapability(
        provider_id=provider_id,
        display_name=provider_id,
        available=available,
        device_ids=(device_id,),
        model_downloaded=available,
        estimated_speed=speed,
        unavailable_reason=None if available else "not available",
    )


class ProviderTests(unittest.TestCase):
    def test_auto_planner_is_deterministic_and_prefers_cuda(self) -> None:
        capabilities = [
            capability("openvino-cpu", True, "slow", "cpu:0"),
            capability("openvino-gpu", True, "fast", "gpu:1"),
            capability("cuda", True, "fast", "gpu:0"),
            capability("mock", True, "fast", "cpu:0"),
        ]
        first = HardwarePlanner().plan(snapshot(), capabilities)
        second = HardwarePlanner().plan(snapshot(), list(reversed(capabilities)))
        self.assertEqual(first, second)
        self.assertEqual(first.provider_id, "cuda")
        self.assertEqual(first.device_id, "gpu:0")

    def test_manual_selection_never_silently_falls_back(self) -> None:
        capabilities = [
            capability("cuda", False, "fast", "gpu:0"),
            capability("openvino-cpu", True, "slow", "cpu:0"),
        ]
        with self.assertRaises(ProviderError) as caught:
            HardwarePlanner().plan(
                snapshot(),
                capabilities,
                mode="manual",
                requested_provider="cuda",
            )
        self.assertEqual(caught.exception.code, "PROVIDER_UNAVAILABLE")

    def test_cpu_plan_exposes_slow_warning(self) -> None:
        plan = HardwarePlanner().plan(
            snapshot(),
            [capability("openvino-cpu", True, "slow", "cpu:0")],
        )
        self.assertEqual(plan.provider_id, "openvino-cpu")
        self.assertEqual(plan.estimated_speed, "slow")
        self.assertTrue(plan.warnings)

    def test_registry_only_claims_installed_adapters(self) -> None:
        registry = ProviderRegistry()
        self.assertEqual(registry.installed_provider_ids(), ("mock",))
        with self.assertRaises(ProviderError) as caught:
            registry.get("cuda")
        self.assertEqual(caught.exception.code, "PROVIDER_NOT_IMPLEMENTED")

    def test_openvino_gpu_requires_verified_probe_before_becoming_available(
        self,
    ) -> None:
        intel = HardwareSnapshot(
            platform="test",
            cpu=snapshot().cpu,
            gpus=(
                HardwareDevice(
                    id="gpu:intel:0",
                    kind="gpu",
                    name="Intel Arc",
                    vendor="intel",
                    runtime="openvino",
                    available=True,
                ),
            ),
        )
        model = [
            {
                "provider_id": "openvino-gpu",
                "model_id": "test",
                "downloaded": True,
            }
        ]
        failed = build_capabilities(
            intel,
            model,
            openvino_cpu=False,
            installed_provider_ids=("mock", "openvino-gpu"),
            openvino_probe=RuntimeProbe(
                runtime_id="openvino",
                detected=True,
                runtime_available=True,
            ),
        )
        self.assertFalse(
            next(item for item in failed if item.provider_id == "openvino-gpu").available
        )
        passed = build_capabilities(
            intel,
            model,
            openvino_cpu=False,
            installed_provider_ids=("mock", "openvino-gpu"),
            openvino_probe=RuntimeProbe(
                runtime_id="openvino",
                detected=True,
                runtime_available=True,
                compile_verified=True,
                inference_verified=True,
            ),
        )
        self.assertTrue(
            next(item for item in passed if item.provider_id == "openvino-gpu").available
        )

    def test_model_manager_verifies_cached_model(self) -> None:
        fixture = Path(__file__).parent / "fixtures" / "model-manager"
        manager = ModelManager(fixture / "manifest.json", fixture / "cache")
        self.assertTrue(manager.status("test-model")["downloaded"])
        from dataclasses import replace

        manager.models["test-model"] = replace(
            manager.models["test-model"], sha256="0" * 64
        )
        self.assertFalse(manager.status("test-model")["downloaded"])

    def test_openvino_probe_reports_missing_runtime(self) -> None:
        result = OpenVinoProbe(installed=lambda: False).run()
        self.assertEqual(result.error_code, "OPENVINO_NOT_INSTALLED")
        self.assertFalse(result.runtime_available)

    def test_openvino_probe_parses_verified_child_result(self) -> None:
        import json

        payload = {
            "runtime_id": "openvino",
            "detected": True,
            "runtime_available": True,
            "compile_verified": True,
            "inference_verified": True,
            "runtime_version": "test",
            "available_devices": ["CPU", "GPU.0"],
            "target_device": "GPU.0",
            "full_device_name": "Test Intel GPU",
            "driver_version": "1",
            "supported_precisions": ["FP16"],
            "compile_time_ms": 10.0,
            "inference_time_ms": 1.0,
            "error_code": None,
            "error_message": None,
        }

        def runner(*_args, **_kwargs):
            return subprocess.CompletedProcess([], 0, json.dumps(payload), "")

        result = OpenVinoProbe(installed=lambda: True, runner=runner).run()
        self.assertTrue(result.inference_verified)
        self.assertEqual(result.target_device, "GPU.0")

    def test_openvino_probe_timeout_is_stable(self) -> None:
        def runner(*_args, **_kwargs):
            raise subprocess.TimeoutExpired("probe", 0.01)

        result = OpenVinoProbe(
            installed=lambda: True,
            runner=runner,
            timeout_seconds=0.01,
        ).run()
        self.assertEqual(result.error_code, "OPENVINO_PROBE_TIMEOUT")

    def test_openvino_gpu_provider_returns_static_preview_and_metrics(self) -> None:
        class Signature:
            def __call__(
                self,
                prompt=None,
                image=None,
                callback_on_step_end=None,
                **_kwargs,
            ):
                del prompt, image, callback_on_step_end

        class Pipeline:
            auto_model_class = Signature

            def __call__(self, **kwargs):
                callback = kwargs.get("callback_on_step_end")
                if callback:
                    callback(self, 0, 0, {"latents": None})
                return SimpleNamespace(
                    images=[Image.new("RGB", (512, 512), (100, 150, 200))]
                )

        source = BytesIO()
        Image.new("RGB", (640, 480), (200, 120, 80)).save(source, "PNG")
        provider = OpenVinoGpuProvider(Path("."))
        provider._pipeline = Pipeline()
        provider._load_time_ms = 12.0
        provider._foreground = SimpleNamespace(
            remove_background=lambda image: (image.convert("RGBA"), None)
        )
        provider.cutout_portrait = lambda image: (
            image.convert("RGBA"),
            {"foreground_coverage": 0.25},
        )
        result = provider.generate(
            GenerationRequest(
                photo=source.getvalue(),
                display_name="test",
                subject_kind="human_avatar",
                seed=42,
            )
        )
        self.assertEqual(result.provider_id, "openvino-gpu")
        self.assertEqual(result.payload["width"], 512)
        self.assertTrue(result.payload["portrait_png"].startswith(b"\x89PNG"))
        self.assertEqual(result.diagnostics["seed"], 42)
        self.assertEqual(result.diagnostics["foreground_coverage"], 0.25)
        self.assertIn("peak_process_memory_mb", result.diagnostics)

    def test_foreground_segmenter_produces_transparent_character(self) -> None:
        mask = np.zeros((1, 1, 320, 320), dtype=np.float32)
        yy, xx = np.ogrid[:320, :320]
        mask[0, 0][((xx - 160) / 70) ** 2 + ((yy - 160) / 130) ** 2 <= 1] = 1

        class Compiled:
            def __call__(self, _inputs):
                return [mask]

        segmenter = ForegroundSegmenter(Path("unused.onnx"))
        segmenter._compiled = Compiled()
        image = Image.new("RGB", (512, 512), "white")
        output, metrics = segmenter.remove_background(image)
        self.assertEqual(output.getpixel((0, 0))[3], 0)
        self.assertGreater(output.getpixel((256, 256))[3], 240)
        self.assertLess(metrics.bounding_box_fill, 0.88)

    def test_foreground_segmenter_rejects_rectangular_photo(self) -> None:
        mask = np.zeros((1, 1, 320, 320), dtype=np.float32)
        mask[0, 0, 40:280, 80:240] = 1

        class Compiled:
            def __call__(self, _inputs):
                return [mask]

        segmenter = ForegroundSegmenter(Path("unused.onnx"))
        segmenter._compiled = Compiled()
        with self.assertRaises(ProviderError) as caught:
            segmenter.remove_background(Image.new("RGB", (512, 512), "white"))
        self.assertEqual(caught.exception.code, "FOREGROUND_QUALITY_FAILED")

    def test_human_pose_estimator_decodes_heatmaps_and_fills_missing(self) -> None:
        heatmaps = np.zeros((1, 19, 32, 57), dtype=np.float32)
        for index in range(12):
            heatmaps[0, index, 4 + index, 10 + index] = 0.9

        class Compiled:
            outputs = ("heatmaps",)

            def __call__(self, _inputs):
                return {"heatmaps": heatmaps}

        estimator = HumanPoseEstimator(Path("unused.xml"))
        estimator._compiled = Compiled()
        pose = estimator.estimate(Image.new("RGBA", (256, 256), "white"))
        self.assertEqual(pose.source, "openvino")
        self.assertEqual(pose.detected_count, 12)
        self.assertTrue(pose.points["nose"].detected)
        self.assertFalse(pose.points["ankle_l"].detected)

    def test_openvino_gpu_provider_honors_preflight_cancellation(self) -> None:
        provider = OpenVinoGpuProvider(Path("."))
        with self.assertRaises(ProviderError) as caught:
            provider.generate(
                GenerationRequest(
                    photo=b"unused",
                    display_name="test",
                    cancellation_check=lambda: True,
                )
            )
        self.assertEqual(caught.exception.code, "GENERATION_CANCELLED")


if __name__ == "__main__":
    unittest.main()
