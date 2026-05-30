"""LLM Client — the only component that touches OpenAI and the only holder of the API
key. Built on LangChain (`langchain-openai`). Centralises timeout + retry + graceful
degradation (Availability NFR). The Markdown KB strategy uses the LLM for the final
answer only — there are no embedding calls."""
import os

from .config import Settings


class LLMError(Exception):
    """Raised when the model is unreachable/failing after retries; routes map it to 503."""


class LLMClient:
    def __init__(self, settings: Settings):
        # Imported lazily so langchain isn't required for offline tests.
        from langchain_openai import ChatOpenAI

        api_key = os.environ.get("OPENAI_API_KEY")
        if not api_key:
            raise RuntimeError("OPENAI_API_KEY is not set")
        # LangChain handles retries/backoff internally via max_retries.
        self._chat = ChatOpenAI(
            model=settings.chat_model,
            temperature=0,
            timeout=settings.request_timeout,
            max_retries=2,
            api_key=api_key,
        )

    @staticmethod
    def _messages(system: str, user: str):
        from langchain_core.messages import HumanMessage, SystemMessage

        return [SystemMessage(content=system), HumanMessage(content=user)]

    def complete(self, system: str, user: str) -> str:
        try:
            resp = self._chat.invoke(self._messages(system, user))
            return resp.content or ""
        except Exception as exc:  # noqa: BLE001 - degrade gracefully on any upstream error
            raise LLMError(str(exc)) from exc

    def stream(self, system: str, user: str):
        try:
            for chunk in self._chat.stream(self._messages(system, user)):
                if chunk.content:
                    yield chunk.content
        except Exception as exc:  # noqa: BLE001
            raise LLMError(str(exc)) from exc
