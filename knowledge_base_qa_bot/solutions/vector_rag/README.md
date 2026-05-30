# Solution B — Vector RAG (FAISS + OpenAI embeddings)

Implements **Strategy B** from `ARCHITECTURE.md`: chunks → embeddings → FAISS vector
search → retrieved context → LLM answer. Via LangChain (`OpenAIEmbeddings` +
`ChatOpenAI`), OpenAI is used for embeddings (at `/index` and per query) **and** the
final answer.

```text
Markdown files -> sections -> chunks -> embeddings -> FAISS (.kb/faiss_index/) -> LLM answer
```

## Run

```bash
cd solutions/vector_rag
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
# set OPENAI_API_KEY once in the shared solutions/.env (see ../.env.example)
uvicorn app.main:app --port 8000
```

## Verify

```bash
curl http://localhost:8000/health
# -> {"status":"ok"}

curl -X POST http://localhost:8000/index
# -> {"files_indexed":3,"sections_indexed":N}   (N = chunk count)
cat .kb/faiss_index/metadata.json     # inspectable vector->citation mapping

curl -X POST http://localhost:8000/wiki
# -> {"path":"wiki/index.md","files":3,"topics":N}; browse wiki/index.md (FR-021)

curl -X POST http://localhost:8000/chat -H 'Content-Type: application/json' \
  -d '{"query":"How long do refunds take?"}'
# -> cites refund_policy.md#refund-timeline

curl -X POST http://localhost:8000/chat -H 'Content-Type: application/json' \
  -d '{"query":"Which restaurants are nearby?"}'
# -> "I cannot confirm this from the knowledge base."
```

Restart the server and ask again **without** `/index` — it loads `.kb/faiss_index/` on startup.

## Offline tests (no API key)

```bash
pip install pytest && pytest tests/   # parsing + chunking only; retrieval needs embeddings
```

## Config (env)

| Var | Default | Purpose |
|---|---|---|
| `OPENAI_API_KEY` | — | required for `/index` and `/chat` |
| `CHAT_MODEL` | `gpt-4o-mini` | answer model |
| `EMBED_MODEL` | `text-embedding-3-small` | embedding model (recorded in metadata.json) |
| `DOCS_DIR` | `docs` | source Markdown |
| `KB_DIR` | `.kb` | index location (`faiss_index/` inside) |
| `WIKI_DIR` | `wiki` | generated `index.md` location (FR-021) |
| `TOP_K` | `4` | chunks fed to the LLM |
| `SCORE_THRESHOLD` | `0.25` | cosine cannot-confirm floor |
| `CHUNK_MAX_CHARS` / `CHUNK_OVERLAP` | `1000` / `150` | chunking |

## Notes

- Cosine similarity via `IndexFlatIP` over L2-normalised vectors; scores in `[-1, 1]`, so
  the threshold differs from the BM25 solution's.
- Index-time embeddings are persisted in FAISS; only the query is embedded at chat time.
- `metadata.json` records `embed_model`; a mismatch on startup logs a re-index warning.
- Unlike Strategy A, `/index` requires the API key because it embeds the corpus.
