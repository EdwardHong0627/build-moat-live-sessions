"""Markdown KB retrieval strategy: dependency-free BM25 over heading sections.

Implements the same interface as the Vector RAG strategy:
    index(sections) -> stats
    retrieve(query, top_k) -> [(Section, score)]
so every downstream component (citation, answerer) is strategy-agnostic."""
import math
import re
from collections import Counter

from .indexer import Section

_TOKEN_RE = re.compile(r"[a-z0-9]+")

# Small English stopword set. Dropping these keeps BM25 precise: an out-of-scope query
# like "which restaurants are nearby" shouldn't match on the filler word "are".
_STOPWORDS = {
    "a", "an", "and", "are", "as", "at", "be", "by", "can", "do", "does", "for", "from",
    "how", "i", "in", "is", "it", "its", "long", "of", "on", "or", "that", "the", "to",
    "was", "what", "when", "where", "which", "who", "will", "with", "you", "your",
}


def tokenize(text: str) -> list[str]:
    return [t for t in _TOKEN_RE.findall(text.lower()) if t not in _STOPWORDS]


class _BM25:
    """BM25 Okapi. Scores are unbounded and corpus-relative — the cannot-confirm
    threshold must be calibrated per corpus (see config.SCORE_THRESHOLD)."""

    def __init__(self, corpus_tokens: list[list[str]], k1: float = 1.5, b: float = 0.75):
        self.k1, self.b = k1, b
        self.docs = corpus_tokens
        self.n = len(corpus_tokens)
        self.doc_len = [len(d) for d in corpus_tokens]
        self.avgdl = (sum(self.doc_len) / self.n) if self.n else 0.0
        self.tf = [Counter(d) for d in corpus_tokens]
        df: Counter = Counter()
        for d in corpus_tokens:
            df.update(set(d))
        # BM25 idf with the +1 smoothing variant to keep idf non-negative.
        self.idf = {t: math.log(1 + (self.n - f + 0.5) / (f + 0.5)) for t, f in df.items()}

    def score(self, query_tokens: list[str], i: int) -> float:
        if self.avgdl == 0:
            return 0.0
        dl = self.doc_len[i]
        tf_i = self.tf[i]
        s = 0.0
        for t in query_tokens:
            f = tf_i.get(t, 0)
            if f == 0:
                continue
            idf = self.idf.get(t, 0.0)
            s += idf * (f * (self.k1 + 1)) / (f + self.k1 * (1 - self.b + self.b * dl / self.avgdl))
        return s


class MarkdownKBStrategy:
    def __init__(self) -> None:
        self.sections: list[Section] = []
        self._bm25: _BM25 | None = None

    def index(self, sections: list[Section]) -> int:
        self.sections = sections
        self._bm25 = _BM25([tokenize(s.text) for s in sections])
        return len(sections)

    def retrieve(self, query: str, top_k: int) -> list[tuple[Section, float]]:
        if not self._bm25 or not self.sections:
            return []
        q = tokenize(query)
        scored = [(self.sections[i], self._bm25.score(q, i)) for i in range(len(self.sections))]
        scored.sort(key=lambda x: x[1], reverse=True)
        # Drop zero-overlap matches outright — an out-of-scope query (no shared terms)
        # yields nothing, which the answerer turns into an honest cannot-confirm.
        return [(sec, sc) for sec, sc in scored[:top_k] if sc > 0.0]
