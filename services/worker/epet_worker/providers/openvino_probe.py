from __future__ import annotations

from dataclasses import asdict
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
from time import perf_counter
from typing import Any, Callable

from .contracts import RuntimeProbe, UNKNOWN


def _property(core: Any, device_id: str, name: str) -> Any:
    try:
        return core.get_property(device_id, name)
    except Exception:
        return None


def _text(value: Any) -> str:
    return str(value) if value not in (None, "") else UNKNOWN


def execute_openvino_probe() -> RuntimeProbe:
    """Compile and execute a tiny in-memory graph on the first Intel GPU."""
    try:
        import numpy as np
        from openvino import Core, Model, Type, get_version
        from openvino import opset13 as ops
    except Exception as error:
        return RuntimeProbe(
            runtime_id="openvino",
            error_code="OPENVINO_IMPORT_FAILED",
            error_message=f"{type(error).__name__}: {str(error)[:240]}",
        )

    try:
        core = Core()
        devices = tuple(str(item) for item in core.available_devices)
        target = next(
            (item for item in devices if item.upper().startswith("GPU")),
            None,
        )
        if target is None:
            return RuntimeProbe(
                runtime_id="openvino",
                detected=True,
                runtime_available=True,
                runtime_version=_text(get_version()),
                available_devices=devices,
                error_code="OPENVINO_GPU_NOT_FOUND",
                error_message="OpenVINO Runtime 可用，但没有发现 Intel GPU 设备。",
            )

        full_name = _text(_property(core, target, "FULL_DEVICE_NAME"))
        driver = _text(_property(core, target, "DRIVER_VERSION"))
        architecture = _text(_property(core, target, "DEVICE_ARCHITECTURE"))
        raw_precisions = _property(core, target, "OPTIMIZATION_CAPABILITIES") or ()
        precisions = tuple(sorted(str(item) for item in raw_precisions))

        parameter = ops.parameter([1, 4], Type.f32, name="input")
        bias = ops.constant(np.array([[1.0, 2.0, 3.0, 4.0]], dtype=np.float32))
        output = ops.add(parameter, bias)
        model = Model([output], [parameter], "epet_openvino_probe")

        started = perf_counter()
        compiled = core.compile_model(model, target, {"PERFORMANCE_HINT": "LATENCY"})
        compile_ms = round((perf_counter() - started) * 1000, 3)

        input_data = np.array([[4.0, 3.0, 2.0, 1.0]], dtype=np.float32)
        started = perf_counter()
        result = compiled([input_data])
        inference_ms = round((perf_counter() - started) * 1000, 3)
        actual = next(iter(result.values()))
        expected = np.array([[5.0, 5.0, 5.0, 5.0]], dtype=np.float32)
        verified = bool(np.allclose(actual, expected))
        return RuntimeProbe(
            runtime_id="openvino",
            detected=True,
            runtime_available=True,
            compile_verified=True,
            inference_verified=verified,
            runtime_version=_text(get_version()),
            available_devices=devices,
            target_device=target,
            full_device_name=full_name,
            driver_version=driver,
            device_architecture=architecture,
            supported_precisions=precisions,
            compile_time_ms=compile_ms,
            inference_time_ms=inference_ms,
            error_code=None if verified else "OPENVINO_INFERENCE_MISMATCH",
            error_message=None if verified else "测试推理输出与预期不一致。",
        )
    except Exception as error:
        return RuntimeProbe(
            runtime_id="openvino",
            detected=True,
            runtime_available=True,
            error_code="OPENVINO_PROBE_FAILED",
            error_message=f"{type(error).__name__}: {str(error)[:240]}",
        )


class OpenVinoProbe:
    def __init__(
        self,
        *,
        timeout_seconds: float | None = None,
        runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
        installed: Callable[[], bool] | None = None,
    ) -> None:
        self.timeout_seconds = timeout_seconds or float(
            os.environ.get("EPET_OPENVINO_PROBE_TIMEOUT", "30")
        )
        self.runner = runner
        self.installed = installed or (
            lambda: importlib.util.find_spec("openvino") is not None
        )

    def run(self) -> RuntimeProbe:
        if not self.installed():
            return RuntimeProbe(
                runtime_id="openvino",
                error_code="OPENVINO_NOT_INSTALLED",
                error_message="OpenVINO Runtime 尚未安装。",
            )
        flags = getattr(subprocess, "CREATE_NO_WINDOW", 0)
        worker_root = str(Path(__file__).resolve().parents[2])
        environment = os.environ.copy()
        existing_path = environment.get("PYTHONPATH")
        environment["PYTHONPATH"] = (
            worker_root
            if not existing_path
            else worker_root + os.pathsep + existing_path
        )
        try:
            process = self.runner(
                [
                    sys.executable,
                    "-m",
                    "epet_worker.providers.openvino_probe",
                    "--child",
                ],
                capture_output=True,
                text=True,
                timeout=self.timeout_seconds,
                check=False,
                creationflags=flags,
                env=environment,
            )
        except subprocess.TimeoutExpired:
            return RuntimeProbe(
                runtime_id="openvino",
                detected=True,
                runtime_available=True,
                error_code="OPENVINO_PROBE_TIMEOUT",
                error_message=f"OpenVINO 探针超过 {self.timeout_seconds:g} 秒。",
            )
        except Exception as error:
            return RuntimeProbe(
                runtime_id="openvino",
                detected=True,
                error_code="OPENVINO_PROBE_PROCESS_FAILED",
                error_message=f"{type(error).__name__}: {str(error)[:240]}",
            )
        try:
            payload = json.loads(process.stdout)
            return RuntimeProbe(**payload)
        except Exception:
            message = process.stderr.strip() or process.stdout.strip()
            return RuntimeProbe(
                runtime_id="openvino",
                detected=True,
                error_code="OPENVINO_PROBE_INVALID_OUTPUT",
                error_message=message[:240] or f"探针进程退出码：{process.returncode}",
            )


def _main() -> int:
    result = execute_openvino_probe()
    print(json.dumps(asdict(result), ensure_ascii=False))
    return 0 if result.inference_verified else 1


if __name__ == "__main__":
    raise SystemExit(_main())
