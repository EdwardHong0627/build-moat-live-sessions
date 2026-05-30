"""Wiki Index Generation (FR-021) — derives a browsable wiki/index.md from the indexed
sections so humans and agents can see the available topics. Deterministic: same index in,
same wiki out. Citations (filename#heading) link back to the source Markdown."""
import os
from typing import Iterable, Protocol


class _Topic(Protocol):
    filename: str
    heading: str
    slug: str
    @property
    def citation(self) -> str: ...
    @property
    def text(self) -> str: ...


def _description(text: str, heading: str, limit: int = 140) -> str:
    """First line of the section body (heading stripped), truncated."""
    body = text[len(heading):] if text.startswith(heading) else text
    lines = [ln.strip() for ln in body.strip().splitlines() if ln.strip()]
    first = lines[0] if lines else ""
    return (first[:limit].rstrip() + "…") if len(first) > limit else first


def generate_index(
    topics: Iterable[_Topic], wiki_dir: str, source_label: str, docs_rel: str = "../docs"
) -> tuple[str, int, int]:
    """Write wiki/index.md grouped by file. Returns (path, files, topics).
    Duplicate citations (e.g. multiple chunks of one section) collapse to one entry."""
    seen: set[str] = set()
    groups: dict[str, list[tuple[str, str, str, str]]] = {}
    total = 0
    for t in topics:
        if t.citation in seen:
            continue
        seen.add(t.citation)
        groups.setdefault(t.filename, []).append(
            (t.heading, t.slug, t.citation, _description(t.text, t.heading))
        )
        total += 1

    lines = [
        "# Knowledge Base Index",
        "",
        f"> Generated from `{source_label}` — {total} topics across {len(groups)} files.",
        "",
    ]
    for filename in sorted(groups):
        lines.append(f"## {filename}")
        lines.append("")
        for heading, slug, citation, desc in groups[filename]:
            entry = f"- [{heading}]({docs_rel}/{filename}#{slug}) — `{citation}`"
            if desc:
                entry += f"  \n  {desc}"
            lines.append(entry)
        lines.append("")

    os.makedirs(wiki_dir, exist_ok=True)
    path = os.path.join(wiki_dir, "index.md")
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines).rstrip() + "\n")
    return path, len(groups), total
