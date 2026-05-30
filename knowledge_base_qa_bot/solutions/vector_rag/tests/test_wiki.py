"""Offline tests for wiki index generation from chunks — no OpenAI needed."""
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

from app import wiki  # noqa: E402
from app.indexer import chunk_sections, parse_markdown  # noqa: E402

SAMPLE = "# Refund Policy\n\n## Refund Timeline\n\nApproved refunds take 5-7 business days.\n"


def test_wiki_from_chunks_groups_by_file():
    sections = parse_markdown(SAMPLE, "refund_policy.md")
    chunks = chunk_sections(sections, max_chars=1000, overlap=150)
    with tempfile.TemporaryDirectory() as d:
        path, files, topics = wiki.generate_index(chunks, d, ".kb/faiss_index/metadata.json")
        content = open(path, encoding="utf-8").read()
    assert "## refund_policy.md" in content
    assert "refund_policy.md#refund-timeline" in content
    # Short sections -> one chunk each -> one topic per section.
    assert files == 1 and topics == len(sections)


def test_wiki_collapses_multichunk_sections():
    # A long section splits into several chunks that share one citation -> one topic.
    long_md = "# Big\n\n## Section\n\n" + ("word " * 600)
    chunks = chunk_sections(parse_markdown(long_md, "big.md"), max_chars=500, overlap=50)
    assert len(chunks) > 2  # multiple chunks exist
    with tempfile.TemporaryDirectory() as d:
        _, _, topics = wiki.generate_index(chunks, d, "x")
    assert topics == 2  # "Big" + "Section", not one-per-chunk
