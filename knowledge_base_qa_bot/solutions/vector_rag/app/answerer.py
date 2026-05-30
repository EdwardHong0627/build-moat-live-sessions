"""Answer Generator — grounding rules, independent of retrieval strategy. Identical to
the Markdown KB solution's: assemble prompt from retrieved chunks, answer only from
sources, own the cannot-confirm contract (FR-004/FR-006/FR-007)."""
from typing import Iterable

from .indexer import Chunk

CANNOT_CONFIRM = "I cannot confirm this from the knowledge base."
NOT_INDEXED = "The knowledge base has not been indexed yet. Run POST /index first."

SYSTEM_PROMPT = (
    "You are a knowledge base assistant. Answer the user's question using ONLY the "
    "provided sources. Each source is labelled with its citation in the form "
    "[filename#heading].\n"
    "- If the sources contain the answer, respond concisely and cite the source(s) you "
    "used inline, e.g. [refund_policy.md#refund-timeline].\n"
    "- If the sources do NOT contain the answer, reply exactly: "
    f"'{CANNOT_CONFIRM}'\n"
    "Never use outside knowledge and never invent citations."
)


def build_context(retrieved: Iterable[tuple[Chunk, float]]) -> str:
    return "\n\n---\n\n".join(f"[{chunk.citation}]\n{chunk.text}" for chunk, _ in retrieved)


def build_user_prompt(query: str, context: str) -> str:
    return f"Sources:\n\n{context}\n\nQuestion: {query}"


def is_refusal(answer: str) -> bool:
    return "cannot confirm" in answer.lower()
