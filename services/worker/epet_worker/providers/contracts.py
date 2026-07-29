from __future__ import annotations

from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from typing import Any, Callable, Literal


UNKNOWN = "unknown"
ProviderId = Literal["mock", "cuda", "openvino-gpu", "openvino-cpu"]
SelectionMode = Literal["auto", "manual"]


@dataclass(frozen=True)
class HardwareDevice:
    id: str
    kind: Literal["cpu", "gpu"]
    name: str = UNKNOWN
    vendor: str = UNKNOWN
    memory_mb: int | Literal["unknown"] = UNKNOWN
    runtime: str = UNKNOWN
    driver_version: str = UNKNOWN
    available: bool = False
    unavailable_reason: str | None = None


@dataclass(frozen=True)
class HardwareSnapshot:
    platform: str = UNKNOWN
    computer_model: str = UNKNOWN
    cpu: HardwareDevice = field(
        default_factory=lambda: HardwareDevice(id="cpu:0", kind="cpu")
    )
    gpus: tuple[HardwareDevice, ...] = ()
    system_memory_mb: int | Literal["unknown"] = UNKNOWN
    probed_at: str = field(
        default_factory=lambda: datetime.now(timezone.utc).isoformat()
    )
    warnings: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class RuntimeProbe:
    runtime_id: str
    detected: bool = False
    runtime_available: bool = False
    compile_verified: bool = False
    inference_verified: bool = False
    runtime_version: str = UNKNOWN
    available_devices: tuple[str, ...] = ()
    target_device: str = UNKNOWN
    full_device_name: str = UNKNOWN
    driver_version: str = UNKNOWN
    device_architecture: str = UNKNOWN
    supported_precisions: tuple[str, ...] = ()
    compile_time_ms: float | Literal["unknown"] = UNKNOWN
    inference_time_ms: float | Literal["unknown"] = UNKNOWN
    error_code: str | None = None
    error_message: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class ProviderCapability:
    provider_id: ProviderId
    display_name: str
    available: bool
    device_ids: tuple[str, ...] = ()
    model_id: str | None = None
    model_downloaded: bool = False
    estimated_speed: Literal["fast", "medium", "slow", "unknown"] = UNKNOWN
    unavailable_reason: str | None = None
    supports_subjects: tuple[str, ...] = ("pet_cat", "human_avatar")
    development_only: bool = False
    detected: bool = False
    runtime_available: bool = False
    compile_verified: bool = False
    inference_verified: bool = False
    runtime_version: str = UNKNOWN
    full_device_name: str = UNKNOWN
    driver_version: str = UNKNOWN
    device_architecture: str = UNKNOWN
    supported_precisions: tuple[str, ...] = ()
    compile_time_ms: float | Literal["unknown"] = UNKNOWN
    inference_time_ms: float | Literal["unknown"] = UNKNOWN

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class GenerationRequest:
    photo: bytes
    display_name: str
    subject_kind: str = "pet_cat"
    job_id: str | None = None
    provider_mode: str = "configured"
    requested_provider: str | None = None
    requested_device_id: str | None = None
    seed: int | None = None
    cancellation_check: Callable[[], bool] | None = field(
        default=None,
        repr=False,
        compare=False,
    )


@dataclass(frozen=True)
class GenerationPlan:
    selection_mode: SelectionMode
    provider_id: ProviderId
    device_id: str
    model_id: str | None
    estimated_speed: Literal["fast", "medium", "slow", "unknown"]
    reason: str
    warnings: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class GenerationResult:
    provider_id: ProviderId
    device_id: str
    model_id: str | None
    payload: dict[str, Any]
    diagnostics: dict[str, Any] = field(default_factory=dict)


class ProviderError(RuntimeError):
    def __init__(
        self,
        code: str,
        message: str,
        *,
        provider_id: str | None = None,
        retryable: bool = False,
        details: dict[str, Any] | None = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.provider_id = provider_id
        self.retryable = retryable
        self.details = details or {}

    def to_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "message": self.message,
            "provider_id": self.provider_id,
            "retryable": self.retryable,
            "details": self.details,
        }
