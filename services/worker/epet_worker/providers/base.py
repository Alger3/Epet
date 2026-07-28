from __future__ import annotations

from abc import ABC, abstractmethod

from .contracts import GenerationPlan, GenerationRequest, GenerationResult


class GenerationProvider(ABC):
    provider_id: str

    @abstractmethod
    def generate(
        self, request: GenerationRequest, plan: GenerationPlan | None = None
    ) -> GenerationResult:
        raise NotImplementedError
