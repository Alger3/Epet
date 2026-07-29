from __future__ import annotations

import ctypes
import json
import os
import platform
import subprocess
from typing import Any

from .contracts import HardwareDevice, HardwareSnapshot, UNKNOWN


def _vendor(name: str) -> str:
    lowered = name.lower()
    if "nvidia" in lowered:
        return "nvidia"
    if "intel" in lowered:
        return "intel"
    if "amd" in lowered or "radeon" in lowered:
        return "amd"
    return UNKNOWN


class HardwareProbe:
    """Best-effort hardware discovery. A failed field is represented as unknown."""

    def probe(self) -> HardwareSnapshot:
        warnings: list[str] = []
        cpu_name = platform.processor() or os.environ.get("PROCESSOR_IDENTIFIER") or UNKNOWN
        computer_model = UNKNOWN
        gpu_rows: list[dict[str, Any]] = []
        if platform.system() == "Windows":
            computer_model, gpu_rows, windows_warnings = self._windows_inventory()
            warnings.extend(windows_warnings)
        memory_mb = self._system_memory_mb()
        gpus = self._merge_runtime_information(gpu_rows, warnings)
        cpu = HardwareDevice(
            id="cpu:0",
            kind="cpu",
            name=cpu_name,
            vendor=_vendor(cpu_name),
            memory_mb=memory_mb,
            runtime="native",
            available=True,
        )
        return HardwareSnapshot(
            platform=f"{platform.system()} {platform.release()}".strip() or UNKNOWN,
            computer_model=computer_model,
            cpu=cpu,
            gpus=tuple(gpus),
            system_memory_mb=memory_mb,
            warnings=tuple(warnings),
        )

    @staticmethod
    def _windows_inventory() -> tuple[str, list[dict[str, Any]], list[str]]:
        script = (
            "$c=Get-CimInstance Win32_ComputerSystem;"
            "$g=Get-CimInstance Win32_VideoController | "
            "Select-Object Name,AdapterRAM,PNPDeviceID,DriverVersion;"
            "[pscustomobject]@{Model=$c.Model;Gpu=@($g)} | ConvertTo-Json -Depth 4 -Compress"
        )
        flags = getattr(subprocess, "CREATE_NO_WINDOW", 0)
        try:
            result = subprocess.run(
                ["powershell", "-NoProfile", "-NonInteractive", "-Command", script],
                capture_output=True,
                text=True,
                timeout=6,
                check=True,
                creationflags=flags,
            )
            value = json.loads(result.stdout)
            rows = value.get("Gpu") or []
            if isinstance(rows, dict):
                rows = [rows]
            return value.get("Model") or UNKNOWN, rows, []
        except Exception as error:
            return UNKNOWN, [], [f"Windows hardware inventory unavailable: {type(error).__name__}"]

    @staticmethod
    def _system_memory_mb() -> int | str:
        try:
            if platform.system() == "Windows":
                class MemoryStatus(ctypes.Structure):
                    _fields_ = [
                        ("length", ctypes.c_ulong),
                        ("memory_load", ctypes.c_ulong),
                        ("total_phys", ctypes.c_ulonglong),
                        ("avail_phys", ctypes.c_ulonglong),
                        ("total_page_file", ctypes.c_ulonglong),
                        ("avail_page_file", ctypes.c_ulonglong),
                        ("total_virtual", ctypes.c_ulonglong),
                        ("avail_virtual", ctypes.c_ulonglong),
                        ("avail_extended_virtual", ctypes.c_ulonglong),
                    ]

                status = MemoryStatus()
                status.length = ctypes.sizeof(status)
                if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status)):
                    return int(status.total_phys // (1024 * 1024))
            return UNKNOWN
        except Exception:
            return UNKNOWN

    def _merge_runtime_information(
        self, rows: list[dict[str, Any]], warnings: list[str]
    ) -> list[HardwareDevice]:
        cuda_names = self._cuda_devices(warnings)
        openvino_names = self._openvino_devices(warnings)
        known_names = [str(row.get("Name") or UNKNOWN) for row in rows]
        for runtime, names in (("cuda", cuda_names), ("openvino", openvino_names)):
            for name in names:
                if any(self._same_device(name, known) for known in known_names):
                    continue
                rows.append({"Name": name, "AdapterRAM": None, "_runtime": runtime})
                known_names.append(name)
        enriched: list[HardwareDevice] = []
        for index, row in enumerate(rows):
            name = str(row.get("Name") or UNKNOWN)
            raw_memory = row.get("AdapterRAM")
            memory: int | str = UNKNOWN
            if isinstance(raw_memory, int) and raw_memory > 0:
                memory = raw_memory // (1024 * 1024)
            runtimes = []
            explicit_runtime = str(row.get("_runtime") or "")
            if explicit_runtime == "cuda" or any(
                self._same_device(name, runtime_name) for runtime_name in cuda_names
            ):
                runtimes.append("cuda")
            if explicit_runtime == "openvino" or any(
                self._same_device(name, runtime_name)
                for runtime_name in openvino_names
            ):
                runtimes.append("openvino")
            enriched.append(
                HardwareDevice(
                    id=f"gpu:{index}",
                    kind="gpu",
                    name=name,
                    vendor=_vendor(name),
                    memory_mb=memory,
                    runtime=",".join(runtimes) if runtimes else UNKNOWN,
                    driver_version=str(row.get("DriverVersion") or UNKNOWN),
                    available=True,
                )
            )
        return enriched

    @staticmethod
    def _same_device(left: str, right: str) -> bool:
        left_words = {word for word in left.lower().split() if len(word) > 2}
        right_words = {word for word in right.lower().split() if len(word) > 2}
        return bool(left_words & right_words)

    @staticmethod
    def _cuda_devices(warnings: list[str]) -> list[str]:
        try:
            import torch

            if not torch.cuda.is_available():
                return []
            return [torch.cuda.get_device_name(index) for index in range(torch.cuda.device_count())]
        except ImportError:
            return []
        except Exception as error:
            warnings.append(f"CUDA probe failed: {type(error).__name__}")
            return []

    @staticmethod
    def _openvino_devices(warnings: list[str]) -> list[str]:
        try:
            from openvino import Core

            core = Core()
            names = []
            for device_id in core.available_devices:
                if not device_id.upper().startswith("GPU"):
                    continue
                try:
                    names.append(str(core.get_property(device_id, "FULL_DEVICE_NAME")))
                except Exception:
                    names.append(device_id)
            return names
        except ImportError:
            return []
        except Exception as error:
            warnings.append(f"OpenVINO probe failed: {type(error).__name__}")
            return []
