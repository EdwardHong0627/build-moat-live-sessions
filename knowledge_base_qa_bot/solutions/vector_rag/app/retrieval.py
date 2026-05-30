"""Vector RAG retrieval strategy: OpenAI embeddings + FAISS ANN search.

Implements the same interface as the Markdown KB strategy:
    index(chunks) -> stats
    retrieve(query, top_k) -> [(Chunk, score)]
Cosine similarity via an inner-product index over L2-normalised vectors."""
import numpy as np

from .indexer import Chunk
from .llm_client import LLMClient, LLMError


class VectorRAGStrategy:
    def __init__(self, llm: LLMClient | None = None):
        self.llm = llm
        self.chunks: list[Chunk] = []
        self.dim: int | None = None
        self._index = None  # faiss.Index; named _index so it won't shadow index()

    @property
    def faiss_index(self):
        return self._index

    def _require_llm(self) -> LLMClient:
        if self.llm is None:
            raise LLMError("LLM client is not configured (missing OPENAI_API_KEY)")
        return self.llm

    def index(self, chunks: list[Chunk]) -> int:
        import faiss

        vectors = np.asarray(self._require_llm().embed([c.text for c in chunks]), dtype="float32")
        faiss.normalize_L2(vectors)
        idx = faiss.IndexFlatIP(vectors.shape[1])
        idx.add(vectors)
        self._index = idx
        self.chunks = chunks
        self.dim = int(vectors.shape[1])
        return len(chunks)

    def load(self, index, chunks: list[Chunk]) -> None:
        self._index = index
        self.chunks = chunks
        self.dim = index.d

    def retrieve(self, query: str, top_k: int) -> list[tuple[Chunk, float]]:
        import faiss

        if self._index is None or not self.chunks:
            return []
        q = np.asarray(self._require_llm().embed([query]), dtype="float32")
        faiss.normalize_L2(q)
        k = min(top_k, len(self.chunks))
        scores, ids = self._index.search(q, k)
        out: list[tuple[Chunk, float]] = []
        for score, i in zip(scores[0].tolist(), ids[0].tolist()):
            if i < 0:
                continue
            out.append((self.chunks[i], float(score)))
        return out
