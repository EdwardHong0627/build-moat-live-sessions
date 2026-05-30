"""Offline tests for wiki index generation — deterministic, no OpenAI needed."""
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

from app import wiki  # noqa: E402
from app.indexer import parse_markdown  # noqa: E402

SAMPLE = "# Refund Policy\n\n## Refund Timeline\n\nApproved refunds take 5-7 business days.\n"


def test_wiki_generates_grouped_index():
    sections = parse_markdown(SAMPLE, "refund_policy.md")
    with tempfile.TemporaryDirectory() as d:
        path, files, topics = wiki.generate_index(sections, d, ".kb/index.json")
        content = open(path, encoding="utf-8").read()
    assert "# Knowledge Base Index" in content
    assert "## refund_policy.md" in content
    assert "refund_policy.md#refund-timeline" in content
    assert "Approved refunds take 5-7 business days." in content
    assert files == 1 and topics == len(sections)


def test_wiki_dedupes_citations():
    sections = parse_markdown(SAMPLE, "refund_policy.md")
    with tempfile.TemporaryDirectory() as d:
        _, _, topics = wiki.generate_index(sections + sections, d, "x")
    assert topics == len(sections)


def test_wiki_is_deterministic():
    sections = parse_markdown(SAMPLE, "refund_policy.md")
    with tempfile.TemporaryDirectory() as d:
        p1, *_ = wiki.generate_index(sections, d, "x")
        first = open(p1, encoding="utf-8").read()
        p2, *_ = wiki.generate_index(sections, d, "x")
        second = open(p2, encoding="utf-8").read()
    assert first == second
