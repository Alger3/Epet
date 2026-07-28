from datetime import datetime
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field, model_validator


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
    species: Literal["cat", "human"]
    subject_kind: Literal["pet_cat", "human_avatar"] | None = None
    display_name: str = Field(min_length=1, max_length=64)
    provider_mode: Literal["configured", "auto", "manual"] = "configured"
    requested_provider: Literal[
        "mock", "cuda", "openvino-gpu", "openvino-cpu"
    ] | None = None
    requested_device_id: str | None = Field(default=None, max_length=128)

    @model_validator(mode="after")
    def validate_subject(self):
        expected = "human_avatar" if self.species == "human" else "pet_cat"
        if self.subject_kind is not None and self.subject_kind != expected:
            raise ValueError("species and subject_kind must describe the same subject")
        if self.provider_mode == "manual" and self.requested_provider is None:
            raise ValueError("manual provider mode requires requested_provider")
        return self

    def resolved_subject_kind(self) -> str:
        return self.subject_kind or (
            "human_avatar" if self.species == "human" else "pet_cat"
        )


def iso(value: datetime) -> str:
    return value.isoformat().replace("+00:00", "Z")
