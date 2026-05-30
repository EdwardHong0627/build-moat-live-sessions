"""Indexer — turns docs/*.md into heading sections. The section is the retrieval unit
and maps 1:1 to the filename#heading citation contract (FR-001/FR-004/FR-005)."""
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


def slugify(heading: str) -> str:
    """GitHub-style heading slug, e.g. 'Refund Timeline' -> 'refund-timeline'."""
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
    """Parse every .md under docs_dir. A malformed/unreadable file is skipped, not fatal
    (Reliability NFR). Returns (sections, files_indexed)."""
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
