from __future__ import annotations

from ..animation_renderer import render_animation
from .base import GenerationProvider
from .contracts import GenerationPlan, GenerationRequest, GenerationResult


class MockProvider(GenerationProvider):
    """Deterministic provider used by local development and contract tests."""

    provider_id = "mock"

    def generate(
        self, request: GenerationRequest, plan: GenerationPlan | None = None
    ) -> GenerationResult:
        subject_kind = (
            "human_avatar" if request.subject_kind == "human_avatar" else "pet_cat"
        )
        return GenerationResult(
            provider_id="mock",
            device_id=plan.device_id if plan else "cpu:0",
            model_id=None,
            payload=render_animation(request.photo, subject_kind),
            diagnostics={"deterministic": True, "development_only": True},
        )
