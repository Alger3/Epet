from __future__ import annotations

from .contracts import (
    GenerationPlan,
    HardwareSnapshot,
    ProviderCapability,
    ProviderError,
)


AUTO_ORDER = ("cuda", "openvino-gpu", "openvino-cpu")


class HardwarePlanner:
    def plan(
        self,
        snapshot: HardwareSnapshot,
        capabilities: list[ProviderCapability],
        *,
        mode: str = "auto",
        requested_provider: str | None = None,
        requested_device_id: str | None = None,
    ) -> GenerationPlan:
        by_id = {capability.provider_id: capability for capability in capabilities}
        manual = mode == "manual" or requested_provider is not None
        candidates = (
            (requested_provider,) if requested_provider else AUTO_ORDER
        )
        for provider_id in candidates:
            capability = by_id.get(provider_id)
            if not capability or not capability.available:
                continue
            device_id = requested_device_id or (
                capability.device_ids[0] if capability.device_ids else "cpu:0"
            )
            if requested_device_id and requested_device_id not in capability.device_ids:
                raise ProviderError(
                    "DEVICE_UNAVAILABLE",
                    f"Device {requested_device_id} is unavailable for {provider_id}",
                    provider_id=provider_id,
                    details={"device_id": requested_device_id},
                )
            warnings = (
                ("CPU generation is supported but may be significantly slower.",)
                if provider_id == "openvino-cpu"
                else ()
            )
            return GenerationPlan(
                selection_mode="manual" if manual else "auto",
                provider_id=provider_id,
                device_id=device_id,
                model_id=capability.model_id,
                estimated_speed=capability.estimated_speed,
                reason=(
                    "Selected by the user."
                    if manual
                    else f"Highest-priority available provider: {provider_id}."
                ),
                warnings=warnings,
            )
        reasons = {
            capability.provider_id: capability.unavailable_reason
            for capability in capabilities
            if capability.unavailable_reason
        }
        if requested_provider:
            capability = by_id.get(requested_provider)
            raise ProviderError(
                "PROVIDER_UNAVAILABLE",
                (
                    capability.unavailable_reason
                    if capability and capability.unavailable_reason
                    else f"Provider {requested_provider} is unavailable"
                ),
                provider_id=requested_provider,
                retryable=False,
                details={"reasons": reasons},
            )
        raise ProviderError(
            "NO_COMPATIBLE_PROVIDER",
            "No compatible generation provider is currently available",
            retryable=False,
            details={"reasons": reasons, "hardware": snapshot.to_dict()},
        )


def build_capabilities(
    snapshot: HardwareSnapshot,
    model_statuses: list[dict],
    openvino_cpu: bool,
    installed_provider_ids: tuple[str, ...] = ("mock",),
) -> list[ProviderCapability]:
    status_by_provider = {
        status["provider_id"]: status for status in model_statuses
    }
    cuda_devices = tuple(
        device.id for device in snapshot.gpus if "cuda" in device.runtime
    )
    openvino_devices = tuple(
        device.id for device in snapshot.gpus if "openvino" in device.runtime
    )

    def real_capability(
        provider_id: str,
        display_name: str,
        devices: tuple[str, ...],
        speed: str,
        runtime_available: bool,
    ) -> ProviderCapability:
        model = status_by_provider.get(provider_id)
        downloaded = bool(model and model["downloaded"])
        reason = None
        adapter_installed = provider_id in installed_provider_ids
        if not adapter_installed:
            reason = f"{display_name} 推理适配器尚未安装。"
        elif not runtime_available:
            reason = f"未检测到 {display_name} 运行时或兼容设备。"
        elif not downloaded:
            reason = "所需模型尚未下载。"
        return ProviderCapability(
            provider_id=provider_id,
            display_name=display_name,
            available=adapter_installed and runtime_available and downloaded,
            device_ids=devices,
            model_id=model["model_id"] if model else None,
            model_downloaded=downloaded,
            estimated_speed=speed,
            unavailable_reason=reason,
        )

    mock = status_by_provider.get("mock")
    return [
        real_capability("cuda", "NVIDIA CUDA", cuda_devices, "fast", bool(cuda_devices)),
        real_capability(
            "openvino-gpu",
            "Intel GPU / OpenVINO",
            openvino_devices,
            "fast",
            bool(openvino_devices),
        ),
        real_capability(
            "openvino-cpu",
            "CPU / OpenVINO",
            ("cpu:0",) if openvino_cpu else (),
            "slow",
            openvino_cpu,
        ),
        ProviderCapability(
            provider_id="mock",
            display_name="Mock（本地测试）",
            available=True,
            device_ids=("cpu:0",),
            model_id=mock["model_id"] if mock else None,
            model_downloaded=True,
            estimated_speed="fast",
            development_only=True,
        ),
    ]
