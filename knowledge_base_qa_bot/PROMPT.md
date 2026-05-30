# Design a Knowledge Base Q&A Bot

## System Requirements

Build a Q&A bot over a small Markdown knowledge base:

- The repo provides sample `.md` documents in `docs/`
- The system builds an index from those documents
- The Markdown KB strategy should write an inspectable `.kb/index.json`
- The Vector RAG strategy should persist its FAISS index in `.kb/faiss_index/`
- Users ask questions through an API
- Answers must be grounded in the indexed documents
- Answers must cite sources using `filename#heading`
- If the knowledge base does not contain the answer, the system should say it cannot confirm

## Choose a Retrieval Strategy

You can solve this with either strategy:

### Strategy A: Markdown KB

```text
Markdown files -> heading sections -> section index -> BM25 keyword search -> raw Markdown context -> LLM answer
```

This is inspired by the Karpathy-style LLM knowledge base pattern: plain Markdown files, explicit indexes, and LLM-readable context instead of embeddings.

### Strategy B: Vector RAG

```text
Markdown files -> chunks -> embeddings -> vector search -> retrieved context -> LLM answer
```

This is the traditional RAG path: semantic retrieval with embeddings and a vector store.

## Design Questions

Answer these before you start coding:

1. Which retrieval strategy did you choose, and why?

   **Strategy A: Markdown KB (BM25 over heading sections).** At this scale (a handful
   of `.md` files), it is the right default: smallest dependency surface, no embedding
   API calls on `/index` or per `/chat` query (so lower cost and latency), and a fully
   inspectable `.kb/index.json`. The corpus is short, factual FAQ-style content where
   the user's vocabulary closely matches the docs ("refund", "email", "shipping"), which
   is exactly where keyword search shines. Crucially, it keeps the system debuggable: when
   an answer is wrong I can read the index and the matched section directly instead of
   reasoning about an opaque embedding space. Vector RAG is kept as the documented
   migration path for when semantic matching becomes necessary (see Q6).

2. What is the retrieval unit in your design: file, section, or chunk?

   **The heading section.** Each document is split on Markdown headings into
   `(filename, heading, body)` records, and the section is both the unit that gets indexed
   and the unit that gets retrieved. This matches the citation contract (`filename#heading`)
   one-to-one, so every retrieved unit can be cited without further bookkeeping. A section
   is large enough to be self-contained and answerable, but small enough to keep irrelevant
   text out of the prompt. Files are too coarse (they mix unrelated topics — e.g.
   `refund_policy.md` covers cancellation, timeline, and non-refundable items); arbitrary
   fixed-size chunks are unnecessary here because the docs are already short and the headings
   provide natural, human-meaningful boundaries.

3. How do you decide what goes into the prompt?

   Rank all sections by BM25 against the query, take the top-k (small, e.g. k=3–5), and
   apply a score floor (see Q5). The selected sections' raw Markdown — each prefixed with
   its `filename#heading` source label — is concatenated into a bounded context block. The
   system prompt instructs the model to answer **only** from the provided sections, to cite
   the sections it used, and to say it cannot confirm if the context does not contain the
   answer. Raw Markdown is passed through unmodified (no summarization) so the model and a
   human auditor see exactly the same text. The query is held constant; only the retrieved
   context varies, which keeps behavior reproducible.

4. How do you cite sources so users can inspect the original Markdown?

   Each indexed section carries its origin as `filename#heading`, where the heading is
   slugified to match the doc (e.g. `refund_policy.md#refund-timeline`,
   `account_help.md#change-email-address`). That label is attached to the section in the
   context block, the model is required to cite using it, and the API returns the selected
   sources alongside the answer. Because the citation is just the file plus the heading slug,
   a user can open the original `.md` and jump straight to the cited heading — the citation
   is a verifiable pointer into the canonical source, not a paraphrase.

5. What should happen when retrieval finds weak or irrelevant results?

   Apply a retrieval **score threshold**. If the best section's score is below the floor (or
   no section clears it), the system does **not** force a citation — it returns an honest
   "I cannot confirm that from the knowledge base" answer with no fabricated source. This is
   the same path used for genuinely out-of-scope questions (e.g. "Which restaurants are
   nearby?"). Grounding takes priority over helpfulness: a confident wrong answer with a
   bogus citation is worse than admitting the KB does not cover the question. The threshold
   is tunable and should be calibrated against the sample docs so in-scope questions clear it
   and off-topic ones do not.

6. When would you switch from Markdown KB to Vector RAG?

   When **lexical matching starts missing answers that are actually in the KB** — i.e. when
   users phrase questions with different vocabulary than the docs (synonyms, paraphrases,
   conceptual queries like "how do I get my money back?" vs. a doc titled "Refund Timeline").
   Concretely: rising "cannot confirm" rates on questions the KB does cover, or a paraphrase
   comparison showing BM25 synonym misses. Other triggers: the corpus grows large and diverse
   enough that keyword overlap is no longer a reliable relevance signal, or content shifts
   from short factual FAQs toward longer prose where semantic similarity beats term frequency.

7. When would you switch from Vector RAG back to a Markdown index?

   When the cost, latency, and opacity of embeddings stop paying for themselves. Specifically:
   the corpus is small and vocabulary-aligned (keyword search already retrieves correctly);
   embedding API cost or per-query latency is unacceptable; the team needs an inspectable,
   diff-able, version-controllable index (`.kb/index.json`) for debugging and auditing; or
   answers must be exactly traceable to a heading and semantic false positives (confidently
   retrieving the wrong-but-similar section) are hurting trust. In short, switch back when
   the data is small and exact, and explainability matters more than semantic recall.

8. If the knowledge base grows from 10 files to 100,000 files, what changes?

   - **Index storage & loading:** a single in-memory `.kb/index.json` no longer scales —
     move to a persisted, queryable store (a real search engine for BM25, or a vector
     database for embeddings) instead of loading the whole index on startup.
   - **Retrieval algorithm:** linear scoring over every section becomes too slow; need
     inverted indexes / ANN (e.g. FAISS) so query time grows sub-linearly with corpus size.
   - **Strategy:** at this scale semantic retrieval (Strategy B), or a **hybrid** of BM25 +
     vector search with a re-ranking stage, becomes worthwhile — lexical-only recall degrades
     across a large, diverse corpus.
   - **Indexing pipeline:** full re-index on every `/index` is infeasible; move to
     **incremental/streaming indexing** (only changed files), with chunking for long docs and
     batched embedding calls.
   - **Operational concerns:** index build cost and time, embedding spend, memory footprint,
     concurrency during re-index, and freshness/invalidation all become first-class design
     problems rather than afterthoughts. Citation granularity (`filename#heading`) still holds,
     but heading slugs must be guaranteed unique across 100k files.

## Verification

Before running the server, set your OpenAI API key:

```bash
export OPENAI_API_KEY="sk-..."
```

Both strategies use OpenAI for final answer generation. Vector RAG also uses OpenAI embeddings during `/index` and for each `/chat` query.

Your prototype should pass all of these:

```bash
# Health check
curl http://localhost:8000/health
# -> 200, {"status": "ok"}

# Chat before indexing
curl -X POST http://localhost:8000/chat \
  -H "Content-Type: application/json" \
  -d '{"query": "How long do refunds take?"}'
# -> 200, should indicate the knowledge base has not been indexed yet

# Build the index from docs/*.md
curl -X POST http://localhost:8000/index
# -> 200, returns {"files_indexed": N, "sections_indexed": M}

# Markdown KB only: inspect the generated section index
cat .kb/index.json

# Markdown KB only: restart the server, then ask again without POST /index
# -> should load .kb/index.json on startup

# Vector RAG only: inspect the persisted FAISS index metadata
cat .kb/faiss_index/metadata.json

# Vector RAG only: restart the server, then ask again without POST /index
# -> should load .kb/faiss_index/ on startup

# Ask a question answered by the docs
curl -X POST http://localhost:8000/chat \
  -H "Content-Type: application/json" \
  -d '{"query": "How long do refunds take?"}'
# -> 200, answer cites refund_policy.md#refund-timeline

# Ask another grounded question
curl -X POST http://localhost:8000/chat \
  -H "Content-Type: application/json" \
  -d '{"query": "Can I change my email address?"}'
# -> 200, answer cites account_help.md#change-email-address

# Ask an out-of-scope question
curl -X POST http://localhost:8000/chat \
  -H "Content-Type: application/json" \
  -d '{"query": "Which restaurants are nearby?"}'
# -> 200, answer should say it cannot confirm from the knowledge base
```

## Suggested Tech Stack

Python + FastAPI is recommended, but Challenge Track students may use any language or framework.

## Stretch Goals

Pick one or more after the core `/index` and `/chat` flow works.

### Score Threshold and Fallback

Add a retrieval score threshold. If the best sections or chunks are too weak, return an honest cannot-confirm answer instead of forcing a citation.

### Streaming Interface

After `/chat` works, add:

```text
POST /chat/stream
```

Use SSE to stream the answer token by token. A good streaming response should:

- Return selected sources first, so users can see what context the bot is using
- Stream answer tokens as they arrive
- End with a clear `done` event
- Preserve the same grounding and citation rules as `/chat`

Optional UI challenge: build a tiny HTML page that calls `/chat/stream` and renders the answer incrementally.

### Browser UI

Build a tiny browser UI over `/chat` or `/chat/stream`. Show selected sources before the answer so users can inspect grounding.

### Multi-Format Import

Add a small normalization pipeline before indexing:

```text
raw/*.txt or raw/*.html -> docs/*.md -> POST /index -> retrieval index
```

Requirements:

- Keep Markdown as the canonical knowledge format
- Preserve the original source filename
- Convert headings into Markdown headings
- Rebuild the retrieval index after import

Start with `.txt` or `.html`. More complex formats such as PDFs, spreadsheets, and transcripts can be discussed as production extensions.

### Alternative Interfaces

Expose the same retrieval core through another interface:

```text
CLI: kb index / kb ask
MCP: expose index, search, and chat as agent tools
Web UI: simple chat screen over /chat or /chat/stream
```

The goal is to compare interface tradeoffs, not to change the retrieval design.

### Wiki Index Generation

Generate `wiki/index.md` from `.kb/index.json` so humans and agents can browse the available topics.

### Answer Filing

Write useful Q&A results back into `wiki/` after review. Preserve citations back to the source Markdown sections.

### Conversation Memory

Add short conversation memory for follow-up questions. Memory can help interpret the query, but retrieved sources must still control the final answer.

### Paraphrase Comparison

Create paraphrased queries and compare Markdown KB vs Vector RAG. Look for synonym misses, semantic false positives, and citation quality.
