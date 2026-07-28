from pathlib import Path
import unittest

from epet_worker.providers.contracts import (
    HardwareDevice,
    HardwareSnapshot,
    ProviderCapability,
    ProviderError,
)
from epet_worker.providers.model_manager import ModelManager
from epet_worker.providers.planner import HardwarePlanner
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

    def test_model_manager_verifies_cached_model(self) -> None:
        fixture = Path(__file__).parent / "fixtures" / "model-manager"
        manager = ModelManager(fixture / "manifest.json", fixture / "cache")
        self.assertTrue(manager.status("test-model")["downloaded"])
        from dataclasses import replace

        manager.models["test-model"] = replace(
            manager.models["test-model"], sha256="0" * 64
        )
        self.assertFalse(manager.status("test-model")["downloaded"])


if __name__ == "__main__":
    unittest.main()
