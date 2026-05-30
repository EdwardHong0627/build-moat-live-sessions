"""Configuration — env-driven. The OpenAI key is read from the environment only,
never stored in code or returned by the API (CON-001, Security)."""
import os
from dataclasses import dataclass, field

# Load environment from .env files (wrapped so offline tests run without python-dotenv).
# Precedence: an optional per-solution .env wins (override=False keeps first-loaded
# values), then the shared solutions/.env supplies the OPENAI_API_KEY.
try:
    from dotenv import load_dotenv

    _solution_dir = os.path.dirname(os.path.dirname(__file__))
    load_dotenv(os.path.join(_solution_dir, ".env"))              # optional local overrides
    load_dotenv(os.path.join(_solution_dir, os.pardir, ".env"))    # shared solutions/.env (key)
except ImportError:
    pass


@dataclass
class Settings:
    docs_dir: str = field(default_factory=lambda: os.environ.get("DOCS_DIR", "docs"))
    kb_dir: str = field(default_factory=lambda: os.environ.get("KB_DIR", ".kb"))
    wiki_dir: str = field(default_factory=lambda: os.environ.get("WIKI_DIR", "wiki"))
    chat_model: str = field(default_factory=lambda: os.environ.get("CHAT_MODEL", "gpt-4o-mini"))
    top_k: int = field(default_factory=lambda: int(os.environ.get("TOP_K", "4")))
    # BM25 scores are unbounded; 0.0 keeps any positive keyword overlap and lets the
    # LLM's own grounding handle the rest. Raise to be stricter (FR-016).
    score_threshold: float = field(default_factory=lambda: float(os.environ.get("SCORE_THRESHOLD", "0.0")))
    request_timeout: float = field(default_factory=lambda: float(os.environ.get("OPENAI_TIMEOUT", "30")))
    max_query_len: int = field(default_factory=lambda: int(os.environ.get("MAX_QUERY_LEN", "2000")))

    @property
    def index_path(self) -> str:
        return os.path.join(self.kb_dir, "index.json")


def get_settings() -> Settings:
    return Settings()
