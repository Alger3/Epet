from datetime import datetime
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field


class StrictModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


class CreateUploadRequest(StrictModel):
    role: Literal["primary", "side", "detail"]
    size: int = Field(ge=1, le=20 * 1024 * 1024)
    mime_type: Literal["image/jpeg", "image/png", "image/webp"]
    sha256: str = Field(pattern=r"^[a-f0-9]{64}$")


class CreateGenerationRequest(StrictModel):
    primary_upload_id: str = Field(pattern=r"^upl_[A-Za-z0-9_-]{8,64}$")
    additional_upload_ids: list[str] = Field(max_length=2)
    style_id: str = Field(min_length=1, max_length=64)
    species: Literal["cat"]
    display_name: str = Field(min_length=1, max_length=64)


def iso(value: datetime) -> str:
    return value.isoformat().replace("+00:00", "Z")
