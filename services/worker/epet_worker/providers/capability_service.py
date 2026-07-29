from __future__ import annotations

from dataclasses import replace
import importlib.util
import os
from pathlib import Path
from typing import Any

from .contracts import GenerationPlan, ProviderError
from .hardware_probe import HardwareProbe
from .model_manager import ModelManager
from .openvino_probe import OpenVinoProbe
from .openvino_gpu_provider import OpenVinoGpuProvider
from .planner import HardwarePlanner, build_capabilities
from .registry import ProviderRegistry


class CapabilityService:
    def __init__(self) -> None:
        worker_root = Path(__file__).resolve().parents[2]
        cache = Path(
            os.environ.get(
                "EPET_MODEL_CACHE_DIR",
                str(worker_root / ".model-cache"),
            )
        )
        self.models = ModelManager(worker_root / "model-manifest.json", cache)
        self.registry = ProviderRegistry()
        openvino_model = self.models.models["epet-portrait-openvino-v1"]
        openvino_marker = self.models.model_path(openvino_model)
        assert openvino_marker is not None
        self.registry.register(OpenVinoGpuProvider(openvino_marker.parent))
        self.snapshot = HardwareProbe().probe()
        self.openvino_probe = OpenVinoProbe().run()
        self._enrich_openvino_probe()
        self.planner = HardwarePlanner()
        self.last_actual_plan: GenerationPlan | None = None
        self._rebuild_capabilities()

    def _rebuild_capabilities(self) -> None:
        self.capabilities = build_capabilities(
            self.snapshot,
            self.models.all_statuses(),
            openvino_cpu=importlib.util.find_spec("openvino") is not None,
            installed_provider_ids=self.registry.installed_provider_ids(),
            openvino_probe=self.openvino_probe,
        )

    def refresh_models(self) -> None:
        self._rebuild_capabilities()

    def refresh_hardware(self) -> None:
        self.snapshot = HardwareProbe().probe()
        self.openvino_probe = OpenVinoProbe().run()
        self._enrich_openvino_probe()
        self._rebuild_capabilities()

    def _enrich_openvino_probe(self) -> None:
        if self.openvino_probe.driver_version != "unknown":
            return
        intel_gpu = next(
            (device for device in self.snapshot.gpus if device.vendor == "intel"),
            None,
        )
        if intel_gpu and intel_gpu.driver_version != "unknown":
            self.openvino_probe = replace(
                self.openvino_probe,
                driver_version=intel_gpu.driver_version,
            )

    def plan(
        self,
        provider_mode: str,
        requested_provider: str | None,
        requested_device_id: str | None,
    ) -> GenerationPlan:
        # The configured default remains Mock so existing local development keeps working.
        configured = os.environ.get("EPET_GENERATION_PROVIDER", "mock")
        if provider_mode == "configured" and not requested_provider:
            requested_provider = configured
            provider_mode = "manual"
        return self.planner.plan(
            self.snapshot,
            self.capabilities,
            mode=provider_mode,
            requested_provider=requested_provider,
            requested_device_id=requested_device_id,
        )

    def payload(self, actual_plan: GenerationPlan | None = None) -> dict[str, Any]:
        if actual_plan is not None:
            self.last_actual_plan = actual_plan
        automatic: dict[str, Any] | None
        try:
            automatic = self.planner.plan(
                self.snapshot, self.capabilities, mode="auto"
            ).to_dict()
        except ProviderError as error:
            automatic = {"error": error.to_dict()}
        return {
            "schema_version": 1,
            "hardware": self.snapshot.to_dict(),
            "runtime_probes": {"openvino": self.openvino_probe.to_dict()},
            "providers": [item.to_dict() for item in self.capabilities],
            "automatic_plan": automatic,
            "configured_provider": os.environ.get("EPET_GENERATION_PROVIDER", "mock"),
            "actual_plan": self.last_actual_plan.to_dict() if self.last_actual_plan else None,
            "models": self.models.all_statuses(),
        }
