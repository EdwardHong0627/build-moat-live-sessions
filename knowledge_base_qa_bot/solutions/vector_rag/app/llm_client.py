"""LLM Client — the only component that touches OpenAI and the only holder of the API
key. Built on LangChain (`langchain-openai`): ChatOpenAI for completions and
OpenAIEmbeddings for embeddings. Strategy B uses it for BOTH embeddings (index + query)
and the final answer. Centralises timeout/retry/graceful degradation (Availability NFR)."""
import os

from .config import Settings


class LLMError(Exception):
    """Raised when the model is unreachable/failing after retries; routes map it to 503."""


class LLMClient:
    def __init__(self, settings: Settings):
        # Imported lazily so langchain isn't required for offline tests.
        from langchain_openai import ChatOpenAI, OpenAIEmbeddings

        api_key = os.environ.get("OPENAI_API_KEY")
        if not api_key:
            raise RuntimeError("OPENAI_API_KEY is not set")
        self._chat = ChatOpenAI(
            model=settings.chat_model,
            temperature=0,
            timeout=settings.request_timeout,
            max_retries=2,
            api_key=api_key,
        )
        self._embeddings = OpenAIEmbeddings(
            model=settings.embed_model,
            timeout=settings.request_timeout,
            max_retries=2,
            api_key=api_key,
        )

    @staticmethod
    def _messages(system: str, user: str):
        from langchain_core.messages import HumanMessage, SystemMessage

        return [SystemMessage(content=system), HumanMessage(content=user)]

    def embed(self, texts: list[str]) -> list[list[float]]:
        """Batch embeddings (LangChain batches internally). Index-time vectors are
        persisted in FAISS, so at chat time only the single query is embedded (FR-015)."""
        try:
            return self._embeddings.embed_documents(texts)
        except Exception as exc:  # noqa: BLE001
            raise LLMError(str(exc)) from exc

    def complete(self, system: str, user: str) -> str:
        try:
            resp = self._chat.invoke(self._messages(system, user))
            return resp.content or ""
        except Exception as exc:  # noqa: BLE001
            raise LLMError(str(exc)) from exc

    def stream(self, system: str, user: str):
        try:
            for chunk in self._chat.stream(self._messages(system, user)):
                if chunk.content:
                    yield chunk.content
        except Exception as exc:  # noqa: BLE001
            raise LLMError(str(exc)) from exc
