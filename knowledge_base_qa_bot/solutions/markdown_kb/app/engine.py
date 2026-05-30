"""Engine — orchestrates indexer + strategy + persistence + answerer and holds the
in-memory index state. Keeps the retrieval/grounding flow in one place so /chat and
/chat/stream share it."""
import logging
import time
from dataclasses import dataclass

from . import answerer, citation, indexer, persistence, wiki
from .config import Settings
from .indexer import Section
from .llm_client import LLMClient, LLMError
from .retrieval import MarkdownKBStrategy

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
        self.strategy = MarkdownKBStrategy()
        self.indexed = False

    # --- lifecycle -------------------------------------------------------
    def load_on_startup(self) -> bool:
        sections = persistence.load_sections(self.settings.index_path)
        if sections:
            self.strategy.index(sections)
            self.indexed = True
            log.info("Loaded %d sections from %s", len(sections), self.settings.index_path)
        return self.indexed

    def build_index(self) -> tuple[int, int]:
        started = time.time()
        sections, files_indexed = indexer.read_sections(self.settings.docs_dir)
        self.strategy.index(sections)
        persistence.save_sections(self.settings.index_path, sections)
        self.indexed = True
        log.info(
            "Indexed %d files, %d sections in %.2fs",
            files_indexed, len(sections), time.time() - started,
        )
        return files_indexed, len(sections)

    def generate_wiki(self) -> dict | None:
        """Regenerate wiki/index.md from the current section index (FR-021)."""
        if not self.indexed:
            return None
        path, files, topics = wiki.generate_index(
            self.strategy.sections, self.settings.wiki_dir, self.settings.index_path
        )
        log.info("Wrote wiki index %s (%d topics, %d files)", path, topics, files)
        return {"path": path, "files": files, "topics": topics}

    # --- retrieval + grounding ------------------------------------------
    def prepare(self, query: str) -> Prepared:
        if not self.indexed:
            return Prepared(False, False, [], "")
        retrieved = self.strategy.retrieve(query, self.settings.top_k)
        retrieved = [(s, sc) for s, sc in retrieved if sc >= self.settings.score_threshold]
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
