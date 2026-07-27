from dataclasses import dataclass
import os
from urllib.parse import urlparse


def _env(name: str, default: str) -> str:
    return os.environ.get(name, default)


@dataclass(frozen=True)
class Settings:
    api_base_url: str = _env("EPET_API_BASE_URL", "http://127.0.0.1:8000").rstrip("/")
    database_url: str = _env(
        "EPET_DATABASE_URL",
        "postgresql://epet:epet_local_only@127.0.0.1:5432/epet",
    )
    redis_url: str = _env("EPET_REDIS_URL", "redis://127.0.0.1:6379/0")
    object_endpoint: str = _env("EPET_OBJECT_ENDPOINT", "http://127.0.0.1:9000")
    object_bucket: str = _env("EPET_OBJECT_BUCKET", "epet-development")
    object_access_key: str = _env("EPET_OBJECT_ACCESS_KEY", "local-development")
    object_secret_key: str = _env(
        "EPET_OBJECT_SECRET_KEY", "local-development-only"
    )

    @property
    def minio_endpoint(self) -> str:
        parsed = urlparse(self.object_endpoint)
        return parsed.netloc or parsed.path

    @property
    def minio_secure(self) -> bool:
        return self.object_endpoint.startswith("https://")


settings = Settings()
