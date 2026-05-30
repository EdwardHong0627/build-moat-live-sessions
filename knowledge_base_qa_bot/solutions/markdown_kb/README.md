# Solution A — Markdown KB (BM25)

Implements **Strategy A** from `ARCHITECTURE.md`: heading sections → BM25 keyword search →
raw Markdown context → LLM answer. No embeddings; the LLM (via LangChain `ChatOpenAI`) is
used only for the final answer.

```text
Markdown files -> heading sections -> BM25 index (.kb/index.json) -> LLM answer
```

## Run

```bash
cd solutions/markdown_kb
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
# set OPENAI_API_KEY once in the shared solutions/.env (see ../.env.example)
uvicorn app.main:app --port 8000
```

## Verify

```bash
curl http://localhost:8000/health
# -> {"status":"ok"}

curl -X POST http://localhost:8000/chat -H 'Content-Type: application/json' \
  -d '{"query":"How long do refunds take?"}'
# before indexing -> grounded:false, "not been indexed yet"

curl -X POST http://localhost:8000/index
# -> {"files_indexed":3,"sections_indexed":N}
cat .kb/index.json            # inspectable section index

curl -X POST http://localhost:8000/wiki
# -> {"path":"wiki/index.md","files":3,"topics":N}; browse wiki/index.md (FR-021)

curl -X POST http://localhost:8000/chat -H 'Content-Type: application/json' \
  -d '{"query":"How long do refunds take?"}'
# -> cites refund_policy.md#refund-timeline

curl -X POST http://localhost:8000/chat -H 'Content-Type: application/json' \
  -d '{"query":"Which restaurants are nearby?"}'
# -> "I cannot confirm this from the knowledge base."
```

Restart the server and ask again **without** `/index` — it loads `.kb/index.json` on startup.

## Offline tests (no API key)

```bash
pip install pytest && pytest tests/
```

## Config (env)

| Var | Default | Purpose |
|---|---|---|
| `OPENAI_API_KEY` | — | required for `/chat` |
| `CHAT_MODEL` | `gpt-4o-mini` | answer model |
| `DOCS_DIR` | `docs` | source Markdown |
| `KB_DIR` | `.kb` | index location |
| `WIKI_DIR` | `wiki` | generated `index.md` location (FR-021) |
| `TOP_K` | `4` | sections fed to the LLM |
| `SCORE_THRESHOLD` | `0.0` | BM25 cannot-confirm floor (raise to be stricter) |

## Notes

- The section (`filename#heading`) is the retrieval unit and the citation unit — one-to-one.
- BM25 scores are unbounded and corpus-relative, so the threshold is tuned separately from
  the Vector RAG solution. Out-of-scope queries share no terms → empty retrieval → cannot-confirm.
