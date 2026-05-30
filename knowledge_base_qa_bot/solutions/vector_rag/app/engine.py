"""Engine — orchestrates indexer + vector strategy + persistence + answerer and holds the
in-memory index state. Same flow shape as the Markdown KB engine; the difference is
embeddings at index and query time."""
import logging
import time
from dataclasses import dataclass

from . import answerer, citation, indexer, persistence, wiki
from .config import Settings
from .llm_client import LLMClient, LLMError
from .retrieval import VectorRAGStrategy

log = logging.getLogger("kb")


@dataclass
class Prepared:
    indexed: bool
    has_context: bool
    sources: list[dict]
    context: str


class Engine:
    def __init__(self, settings: Settings, llm: LLMClient | None = None):
        self.settings = settings
        self.llm = llm
        self.strategy = VectorRAGStrategy(llm=llm)
        self.indexed = False

    def attach_llm(self, llm: LLMClient | None) -> None:
        self.llm = llm
        self.strategy.llm = llm

    # --- lifecycle -------------------------------------------------------
    def load_on_startup(self) -> bool:
        loaded = persistence.load(self.settings.faiss_dir)
        if loaded:
            index, chunks, meta = loaded
            self.strategy.load(index, chunks)
            self.indexed = True
            log.info(
                "Loaded %d chunks from %s (embed_model=%s)",
                len(chunks), self.settings.faiss_dir, meta.get("embed_model"),
            )
            if meta.get("embed_model") and meta["embed_model"] != self.settings.embed_model:
                log.warning(
                    "Persisted embed_model %s != configured %s — re-index recommended.",
                    meta["embed_model"], self.settings.embed_model,
                )
        return self.indexed

    def build_index(self) -> tuple[int, int]:
        if self.llm is None:
            raise LLMError("LLM client is not configured (missing OPENAI_API_KEY)")
        started = time.time()
        sections, files_indexed = indexer.read_sections(self.settings.docs_dir)
        chunks = indexer.chunk_sections(
            sections, self.settings.chunk_max_chars, self.settings.chunk_overlap
        )
        self.strategy.index(chunks)
        persistence.save(
            self.settings.faiss_dir, self.strategy.faiss_index, chunks,
            self.settings.embed_model, self.strategy.dim,
        )
        self.indexed = True
        log.info(
            "Indexed %d files, %d chunks in %.2fs",
            files_indexed, len(chunks), time.time() - started,
        )
        # 'sections_indexed' reports the retrieval-unit count (chunks) for Vector RAG.
        return files_indexed, len(chunks)

    def generate_wiki(self) -> dict | None:
        """Regenerate wiki/index.md from the indexed chunks (FR-021). Duplicate chunks of
        the same section collapse to one topic entry."""
        if not self.indexed:
            return None
        path, files, topics = wiki.generate_index(
            self.strategy.chunks, self.settings.wiki_dir, self.settings.faiss_dir + "/metadata.json"
        )
        log.info("Wrote wiki index %s (%d topics, %d files)", path, topics, files)
        return {"path": path, "files": files, "topics": topics}

    # --- retrieval + grounding ------------------------------------------
    def prepare(self, query: str) -> Prepared:
        if not self.indexed:
            return Prepared(False, False, [], "")
        retrieved = self.strategy.retrieve(query, self.settings.top_k)  # may raise LLMError
        retrieved = [(c, sc) for c, sc in retrieved if sc >= self.settings.score_threshold]
        has_context = bool(retrieved)
        return Prepared(
            indexed=True,
            has_context=has_context,
            sources=citation.dedupe_sources(retrieved),
            context=answerer.build_context(retrieved),
        )

    def chat(self, query: str) -> dict:
        prepared = self.prepare(query)
        if not prepared.indexed:
            return {"answer": answerer.NOT_INDEXED, "sources": [], "grounded": False}
        if not prepared.has_context:
            return {"answer": answerer.CANNOT_CONFIRM, "sources": [], "grounded": False}
        if self.llm is None:
            raise LLMError("LLM client is not configured (missing OPENAI_API_KEY)")
        text = self.llm.complete(
            answerer.SYSTEM_PROMPT, answerer.build_user_prompt(query, prepared.context)
        )
        grounded = not answerer.is_refusal(text)
        return {
            "answer": text,
            "sources": prepared.sources if grounded else [],
            "grounded": grounded,
        }
