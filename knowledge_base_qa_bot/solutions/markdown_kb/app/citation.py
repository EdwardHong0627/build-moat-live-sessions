"""Citation Formatter — the single place sources are turned into the API shape.
Citations are only emitted for sections actually retrieved (no fabricated citations,
FR-005). Shared by /chat and /chat/stream so both paths cite identically."""
from typing import Iterable

from .indexer import Section


def dedupe_sources(retrieved: Iterable[tuple[Section, float]]) -> list[dict]:
    seen: set[str] = set()
    out: list[dict] = []
    for section, score in retrieved:
        if section.citation in seen:
            continue
        seen.add(section.citation)
        out.append({"citation": section.citation, "score": round(float(score), 4)})
    return out
