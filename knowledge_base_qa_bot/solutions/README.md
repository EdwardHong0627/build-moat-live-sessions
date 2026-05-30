# Solutions — Two Implementations, One Contract

Two standalone implementations of the KB Q&A bot, one per retrieval strategy from
[`../ARCHITECTURE.md`](../ARCHITECTURE.md). They are **separate, runnable solutions** (not a
single app with a switch) so you can compare the strategies side by side — the
"pick one" exercise in `PROMPT.md` is left intact; here both are built.

| | [`markdown_kb/`](markdown_kb/) — Strategy A | [`vector_rag/`](vector_rag/) — Strategy B |
|---|---|---|
| Retrieval | BM25 keyword search over heading sections | OpenAI embeddings + FAISS ANN search |
| Retrieval unit | heading section | chunk (of a section) |
| Index artifact | `.kb/index.json` (human-readable) | `.kb/faiss_index/` + `metadata.json` |
| OpenAI usage (via LangChain) | final answer only (`ChatOpenAI`) | embeddings (`OpenAIEmbeddings`, `/index` + query) **and** answer |
| Extra deps | `langchain-openai` (BM25 in-repo) | `langchain-openai`, `faiss-cpu`, `numpy` |
| Best when | small, vocabulary-aligned corpus; cheap, inspectable, debuggable | synonyms/paraphrases matter; semantic recall |

## Shared contract (identical across both)

```text
GET  /health        -> {"status":"ok"}
POST /index         -> {"files_indexed":N,"sections_indexed":M}
POST /wiki          -> {"path":"wiki/index.md","files":F,"topics":T}   (FR-021; 409 if not indexed)
POST /chat          -> {"answer":str,"sources":[{"citation":"file.md#heading","score":float}],"grounded":bool}
POST /chat/stream   -> SSE: event:sources -> event:token* -> event:done   (FR-017)
```

`POST /wiki` writes a browsable `wiki/index.md` derived deterministically from the current
index (sections grouped by file, each linking back to its `filename#heading`).

Grounding rules are identical: answers cite `filename#heading`, out-of-scope queries return
an honest "I cannot confirm this from the knowledge base", and chat before `/index` says the
KB is not indexed yet.

## Shared component layout (per `ARCHITECTURE.md` §4)

```text
app/
  main.py        HTTP Router + lifespan (startup index load)
  config.py      env-driven settings (OPENAI_API_KEY server-side only)
  schemas.py     /chat contract (identical in both)
  indexer.py     docs/*.md -> sections (+ chunks in vector_rag)
  retrieval.py   the strategy (BM25  |  FAISS)         <- the only deep difference
  persistence.py write/load .kb/ (index.json | faiss_index/)
  citation.py    filename#heading formatter (shared design)
  answerer.py    prompt + threshold + cannot-confirm (shared design)
  llm_client.py  LangChain wrapper (ChatOpenAI [+ OpenAIEmbeddings]): timeout/retry/degradation; holds the key
  engine.py      orchestrator + in-memory index state
```

Only `retrieval.py` / `persistence.py` / `indexer.py` (chunking) and the deps differ; the
contract, routing, validation, citation, and grounding logic are the same — which is the
point of the `RetrievalStrategy` seam in the architecture.

## Quick start

The OpenAI key is shared — set it once in `solutions/.env`:

```bash
cp .env.example .env   # then put your OPENAI_API_KEY in solutions/.env (git-ignored)
```

Then pick a solution (both auto-load the shared key via python-dotenv):

```bash
cd markdown_kb   # or: cd vector_rag
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
uvicorn app.main:app --port 8000
```

Need to override a setting for just one solution? Drop a local `.env` in that solution's
folder — it takes precedence over the shared one.

## Status / validation

- **markdown_kb:** offline unit tests pass (parsing, slugs, BM25 ranking, out-of-scope).
  FastAPI wiring smoke-tested end-to-end without a key: `/health`, chat-before-index,
  `/index` (BM25), out-of-scope cannot-confirm, 400 on bad body, 503 when the answer model
  is unavailable. Grounded answers need `OPENAI_API_KEY`.
- **vector_rag:** offline unit tests pass (parsing, slugs, chunking). Live `/index` and
  `/chat` require `OPENAI_API_KEY` + `faiss-cpu`/`numpy` installed; not exercised here
  because no key/embeddings were available in the build environment.
