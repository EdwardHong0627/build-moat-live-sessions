"""Citation Formatter — the single place retrieved chunks become the API source shape.
Citations are only emitted for chunks actually retrieved (no fabricated citations, FR-005).
Multiple chunks from the same section collapse to one citation."""
from typing import Iterable

from .indexer import Chunk


def dedupe_sources(retrieved: Iterable[tuple[Chunk, float]]) -> list[dict]:
    seen: set[str] = set()
    out: list[dict] = []
    for chunk, score in retrieved:
        if chunk.citation in seen:
            continue
        seen.add(chunk.citation)
        out.append({"citation": chunk.citation, "score": round(float(score), 4)})
    return out
