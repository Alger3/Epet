from __future__ import annotations

from .base import GenerationProvider
from .contracts import ProviderError
from .mock_provider import MockProvider


class ProviderRegistry:
    """Runtime adapter registry. Real adapters are registered only when implemented."""

    def __init__(self) -> None:
        self._providers: dict[str, GenerationProvider] = {"mock": MockProvider()}

    def register(self, provider: GenerationProvider) -> None:
        self._providers[provider.provider_id] = provider

    def get(self, provider_id: str) -> GenerationProvider:
        try:
            return self._providers[provider_id]
        except KeyError as error:
            raise ProviderError(
                "PROVIDER_NOT_IMPLEMENTED",
                f"Provider {provider_id} has no installed inference adapter",
                provider_id=provider_id,
            ) from error

    def installed_provider_ids(self) -> tuple[str, ...]:
        return tuple(sorted(self._providers))
