"""Offline tests — no OpenAI/FAISS needed. Covers parsing, slugs, and chunking.
Retrieval itself needs embeddings, so it is exercised via the live API, not here."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

from app.indexer import chunk_sections, parse_markdown, slugify  # noqa: E402

SAMPLE = """# Refund Policy

## Refund Timeline

Approved refunds are processed within 5-7 business days.
"""


def test_slugify():
    assert slugify("Refund Timeline") == "refund-timeline"


def test_parse_sections_and_citations():
    sections = parse_markdown(SAMPLE, "refund_policy.md")
    citations = [s.citation for s in sections]
    assert "refund_policy.md#refund-timeline" in citations


def test_short_section_is_single_chunk():
    sections = parse_markdown(SAMPLE, "refund_policy.md")
    chunks = chunk_sections(sections, max_chars=1000, overlap=150)
    # Sample sections are short, so each yields exactly one chunk.
    assert len(chunks) == len(sections)
    assert chunks[0].citation == sections[0].citation


def test_long_section_splits_with_overlap():
    long_section = parse_markdown("# Big\n\n" + ("word " * 600), "big.md")
    chunks = chunk_sections(long_section, max_chars=1000, overlap=150)
    assert len(chunks) > 1
    assert all(c.citation == "big.md#big" for c in chunks)
