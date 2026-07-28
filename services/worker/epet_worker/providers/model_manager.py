from __future__ import annotations

from dataclasses import asdict, dataclass
from hashlib import sha256
import json
from pathlib import Path
import shutil
from tempfile import NamedTemporaryFile
from typing import Any
from urllib.request import urlopen

from .contracts import ProviderError


@dataclass(frozen=True)
class ModelSpec:
    model_id: str
    provider_id: str
    revision: str
    filename: str | None
    sha256: str | None
    download_url: str | None
    built_in: bool = False
    license: str = "unknown"
    precision: str = "unknown"
    export_profile: str = "unknown"


class ModelManager:
    def __init__(self, manifest_path: Path, cache_root: Path) -> None:
        self.manifest_path = manifest_path
        self.cache_root = cache_root
        raw = json.loads(manifest_path.read_text(encoding="utf-8"))
        self.models = {
            item["model_id"]: ModelSpec(**item) for item in raw.get("models", [])
        }

    def status(self, model_id: str) -> dict[str, Any]:
        spec = self._spec(model_id)
        path = self.model_path(spec)
        downloaded = spec.built_in or (
            path is not None
            and path.is_file()
            and (spec.sha256 is None or self._hash(path) == spec.sha256)
        )
        return {
            **asdict(spec),
            "downloaded": downloaded,
            "path": str(path) if downloaded and path else None,
        }

    def all_statuses(self) -> list[dict[str, Any]]:
        return [self.status(model_id) for model_id in sorted(self.models)]

    def model_path(self, spec: ModelSpec) -> Path | None:
        if not spec.filename:
            return None
        return self.cache_root / spec.provider_id / spec.model_id / spec.revision / spec.filename

    def download(self, model_id: str) -> dict[str, Any]:
        spec = self._spec(model_id)
        if spec.built_in:
            return self.status(model_id)
        if not spec.download_url or not spec.sha256:
            raise ProviderError(
                "MODEL_DOWNLOAD_UNAVAILABLE",
                f"Model {model_id} has no configured download source",
                provider_id=spec.provider_id,
            )
        target = self.model_path(spec)
        assert target is not None
        target.parent.mkdir(parents=True, exist_ok=True)
        with urlopen(spec.download_url, timeout=60) as response, NamedTemporaryFile(
            dir=target.parent, delete=False
        ) as temporary:
            shutil.copyfileobj(response, temporary)
            temp_path = Path(temporary.name)
        try:
            actual = self._hash(temp_path)
            if actual != spec.sha256:
                raise ProviderError(
                    "MODEL_CHECKSUM_MISMATCH",
                    f"Checksum mismatch for model {model_id}",
                    provider_id=spec.provider_id,
                    details={"expected": spec.sha256, "actual": actual},
                )
            temp_path.replace(target)
        finally:
            temp_path.unlink(missing_ok=True)
        return self.status(model_id)

    def remove(self, model_id: str) -> None:
        spec = self._spec(model_id)
        if spec.built_in:
            raise ProviderError("MODEL_BUILT_IN", "Built-in models cannot be removed")
        path = self.model_path(spec)
        if path:
            path.unlink(missing_ok=True)

    def compiled_cache_dir(self, model_id: str, device_id: str) -> Path:
        spec = self._spec(model_id)
        safe_device = "".join(
            character if character.isalnum() or character in "-_." else "_"
            for character in device_id
        )
        return self.cache_root / "compiled" / spec.provider_id / spec.revision / safe_device

    def _spec(self, model_id: str) -> ModelSpec:
        try:
            return self.models[model_id]
        except KeyError as error:
            raise ProviderError("MODEL_NOT_FOUND", f"Unknown model {model_id}") from error

    @staticmethod
    def _hash(path: Path) -> str:
        digest = sha256()
        with path.open("rb") as source:
            while chunk := source.read(1024 * 1024):
                digest.update(chunk)
        return digest.hexdigest()
