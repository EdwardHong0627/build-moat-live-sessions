"""Index persistence — writes an inspectable .kb/index.json and loads it on startup so
the server survives a restart without re-indexing (FR-010/FR-012/FR-013)."""
import json
import os

from .indexer import Section


def save_sections(index_path: str, sections: list[Section]) -> None:
    os.makedirs(os.path.dirname(index_path) or ".", exist_ok=True)
    data = {
        "version": 1,
        "strategy": "markdown_kb",
        "sections": [
            {"filename": s.filename, "heading": s.heading, "slug": s.slug, "body": s.body}
            for s in sections
        ],
    }
    with open(index_path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)


def load_sections(index_path: str) -> list[Section] | None:
    """Return persisted sections, or None if absent/corrupt (degrade to 'needs indexing'
    rather than crashing — Availability NFR)."""
    if not os.path.exists(index_path):
        return None
    try:
        with open(index_path, encoding="utf-8") as f:
            data = json.load(f)
        return [Section(d["filename"], d["heading"], d["slug"], d["body"]) for d in data["sections"]]
    except Exception:
        return None
