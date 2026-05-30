"""Offline tests — no OpenAI needed. Covers parsing, slugs, and BM25 ranking,
which is the whole reason Strategy A is the easy-to-debug default."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

from app.indexer import parse_markdown, slugify  # noqa: E402
from app.retrieval import MarkdownKBStrategy  # noqa: E402

SAMPLE = """# Refund Policy

## Refund Timeline

Approved refunds are processed within 5-7 business days.

## Non-Refundable Items

Gift cards are not refundable.
"""


def test_slugify():
    assert slugify("Refund Timeline") == "refund-timeline"
    assert slugify("Change Email Address") == "change-email-address"


def test_parse_sections_and_citations():
    sections = parse_markdown(SAMPLE, "refund_policy.md")
    citations = [s.citation for s in sections]
    assert "refund_policy.md#refund-timeline" in citations
    assert "refund_policy.md#non-refundable-items" in citations
    # The top-level '# Refund Policy' is also a heading section.
    assert "refund_policy.md#refund-policy" in citations


def test_bm25_ranks_relevant_section_first():
    sections = parse_markdown(SAMPLE, "refund_policy.md")
    strat = MarkdownKBStrategy()
    strat.index(sections)
    results = strat.retrieve("how long do refunds take", top_k=3)
    assert results, "expected at least one match"
    assert results[0][0].citation == "refund_policy.md#refund-timeline"


def test_out_of_scope_returns_nothing():
    sections = parse_markdown(SAMPLE, "refund_policy.md")
    strat = MarkdownKBStrategy()
    strat.index(sections)
    assert strat.retrieve("which restaurants are nearby", top_k=3) == []
