"""Indexer — parses docs/*.md into heading sections (shared logic with Strategy A),
then chunks sections into embedding-sized units. The chunk carries its parent
filename#heading so citations resolve to a real section (FR-004/FR-005)."""
import re
from dataclasses import dataclass
from pathlib import Path

_HEADING_RE = re.compile(r"^(#{1,6})\s+(.*)$")


@dataclass
class Section:
    filename: str
    heading: str
    slug: str
    body: str

    @property
    def citation(self) -> str:
        return f"{self.filename}#{self.slug}"

    @property
    def text(self) -> str:
        return f"{self.heading}\n\n{self.body}".strip()


@dataclass
class Chunk:
    filename: str
    heading: str
    slug: str
    text: str
    chunk_id: int

    @property
    def citation(self) -> str:
        return f"{self.filename}#{self.slug}"


def slugify(heading: str) -> str:
    s = heading.strip().lower()
    s = re.sub(r"[^\w\s-]", "", s)
    s = re.sub(r"[\s_]+", "-", s)
    return s.strip("-")


def parse_markdown(text: str, filename: str) -> list[Section]:
    sections: list[Section] = []
    heading: str | None = None
    body_lines: list[str] = []

    def flush() -> None:
        if heading is not None:
            sections.append(Section(filename, heading, slugify(heading), "\n".join(body_lines).strip()))

    for line in text.splitlines():
        m = _HEADING_RE.match(line)
        if m:
            flush()
            heading = m.group(2).strip()
            body_lines = []
        elif heading is not None:
            body_lines.append(line)
    flush()
    return sections


def read_sections(docs_dir: str) -> tuple[list[Section], int]:
    sections: list[Section] = []
    files_indexed = 0
    for path in sorted(Path(docs_dir).glob("*.md")):
        try:
            text = path.read_text(encoding="utf-8")
        except Exception:
            continue
        secs = parse_markdown(text, path.name)
        if secs:
            files_indexed += 1
            sections.extend(secs)
    return sections, files_indexed


def _split(text: str, max_chars: int, overlap: int) -> list[str]:
    if len(text) <= max_chars:
        return [text]
    parts: list[str] = []
    start = 0
    step = max(1, max_chars - overlap)
    while start < len(text):
        parts.append(text[start : start + max_chars])
        start += step
    return parts


def chunk_sections(sections: list[Section], max_chars: int, overlap: int) -> list[Chunk]:
    """Most sample sections fit in one chunk; long ones are split with overlap.
    Production would split on token/sentence boundaries — char windows are fine here."""
    chunks: list[Chunk] = []
    for s in sections:
        for i, piece in enumerate(_split(s.text, max_chars, overlap)):
            chunks.append(Chunk(s.filename, s.heading, s.slug, piece, i))
    return chunks
