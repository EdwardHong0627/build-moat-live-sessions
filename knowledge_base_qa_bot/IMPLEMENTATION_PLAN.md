# Implementation Plan — KB Q&A Bot (Dual Strategy: Markdown KB + Vector RAG)

> Derived from `ARCHITECTURE.md` · Requirements in `requirements.raw.md` / `requirements.analysis.md` · 2026-05-29
> Both strategies are first-class and selectable at startup via `KB_STRATEGY=markdown_kb | vector_rag`, behind one `RetrievalStrategy` interface.
> - **Strategy A — Markdown KB:** BM25 over heading sections; `.kb/index.json`; no embeddings.
> - **Strategy B — Vector RAG:** OpenAI embeddings + FAISS; `.kb/faiss_index/` + `metadata.json`.

## 0. Guiding Rules

- Build in the order the architecture's priorities dictate: **grounding/citation correctness → OpenAI reliability → input/key security → persistence + observability**.
- Every component from `ARCHITECTURE.md` §4 maps to one module; the **load-bearing seams** (Retrieval Core interface, Strategy Selector, Citation Formatter, Embedding Service, LLM Client) are written first as contracts, then filled in.
- **Build Strategy A first** (simpler, zero-embedding, fully offline-testable), then Strategy B against the same interface and tests — proving the interface is strategy-agnostic. Everything outside the two strategy modules + Embedding Service is written **once** and shared.
- Definition of done for the core = all 9 `PROMPT.md` Verification `curl`s pass **under both strategies** (the Markdown-KB-only and Vector-RAG-only steps included), including restart-without-`/index` and the out-of-scope cannot-confirm case.

## 1. Target Layout

```text
app/
  main.py            # FastAPI app, startup index load, DI wiring
  config.py          # env/config: OPENAI_API_KEY, KB_STRATEGY, model names, threshold(s), top_k, paths
  schemas.py         # Pydantic request/response models (the /chat contract) — strategy-agnostic
  routes.py          # HTTP Router: /health /index /chat /chat/stream
  indexer.py         # Indexer: docs/*.md -> sections -> chunks  (shared by both strategies)
  embedding.py       # Embedding Service: batch (index) + single (query) — Strategy B only
  llm_client.py      # LLM Client: OpenAI wrapper (completions + embeddings), timeout/retry
  retrieval/
    base.py          # RetrievalStrategy interface: index(sections)->stats, retrieve(query)->[(chunk,score)]
    selector.py      # Strategy Selector: factory binding KB_STRATEGY -> implementation
    markdown_kb.py   # Strategy A: BM25 over sections; writes/loads .kb/index.json
    vector_rag.py    # Strategy B: FAISS ANN search; uses Embedding Service
  persistence.py     # Index Persistence: write/load .kb/ (index.json OR faiss_index/+metadata.json)
  citation.py        # Citation Formatter: filename#heading build + validate (shared)
  answerer.py        # Answer Generator: prompt assembly, threshold, cannot-confirm, streaming (shared)
  observability.py   # structured logging
tests/
  ...
.kb/                 # generated: index.json (Strategy A) OR faiss_index/{index.faiss,metadata.json} (Strategy B)
requirements.txt     # faiss-cpu + openai pulled only when Strategy B is used
```

---

## 2. Phased Build

### Phase 1 — Skeleton & contracts (no OpenAI yet)
**Goal:** the app boots, `/health` is green, and all seams exist as typed stubs.

| Task | Component | FR / NFR | Acceptance |
|---|---|---|---|
| FastAPI app + `config.py` (env-driven; key never logged; reads `KB_STRATEGY`) | main, config | CON-001, Security | App starts; `OPENAI_API_KEY` read from env only |
| `GET /health` returning `{"status":"ok"}` with no external calls | routes | FR-008 | 200 in <100ms, no OpenAI touch |
| Pydantic `ChatRequest`/`ChatResponse`/`IndexResponse` (pin the `/chat` schema) — identical for both strategies | schemas | FR-003, Usability | Missing/invalid `query` → 400; oversized capped |
| `RetrievalStrategy` interface: `index(sections)->stats`, `retrieve(query)->[(chunk,score)]` | retrieval/base | Maintainability | Both strategies implement it; routes/answerer depend only on interface |
| Strategy Selector factory binding `KB_STRATEGY` → implementation at startup | retrieval/selector | Maintainability (AD-2) | Unknown value → clear startup error; nothing else branches on strategy |
| Structured request logging middleware | observability | Observability | Each request logs endpoint, latency, status |

**Exit:** `curl /health` → 200; `curl /chat` (bad body) → 400; app boots under either `KB_STRATEGY`.

### Phase 2 — Shared indexing + Strategy A (Markdown KB)
**Goal:** `POST /index` parses docs once (shared) and builds/persists the BM25 index. Build A first — no OpenAI needed for indexing.

| Task | Component | FR / NFR | Acceptance |
|---|---|---|---|
| Markdown → heading sections `(filename, heading, body)`; slugify heading | indexer (shared) | FR-001/009 | Sections match `filename#heading` citation contract |
| Chunk long sections; carry `{filename, heading, chunk_id}` metadata | indexer (shared) | FR-015, Scalability | Deterministic chunks for same input |
| **Strategy A:** build BM25 section index; persist inspectable `.kb/index.json` | markdown_kb, persistence | FR-010/012 | `.kb/index.json` human-readable; deterministic given inputs |
| `POST /index` → `{"files_indexed":N,"sections_indexed":M}`; log counts+duration | routes, indexer | FR-002, Observability | Returns exact contract; corrupt/malformed `.md` skipped not fatal |

**Exit (Strategy A):** `curl -X POST /index` → 200 with counts; `cat .kb/index.json` is readable.

### Phase 2b — Strategy B (Vector RAG) indexing
**Goal:** the same `/index` route builds and persists a FAISS index when `KB_STRATEGY=vector_rag`.

| Task | Component | FR / NFR | Acceptance |
|---|---|---|---|
| LLM Client embeddings call (batched) + timeout/retry | llm_client, embedding | FR-015 Cost, Reliability | Index-time vectors batched; failure surfaced, not fatal per-file |
| **Strategy B:** build FAISS index from chunk embeddings; persist `index.faiss` + `metadata.json` | vector_rag, persistence | FR-010/012 | `.kb/faiss_index/metadata.json` human-readable |
| Version index against embedding model; mismatch forces re-index | persistence, config | Reliability (AD-4b) | Model change detected, not silently stale |

**Exit (Strategy B):** same `/index` contract; `cat .kb/faiss_index/metadata.json` is readable. *No route or response change vs. Phase 2 — proves the interface.*

### Phase 3 — Grounded chat (the core value, both strategies)
**Goal:** `POST /chat` returns grounded, cited answers and honest refusals — identical contract regardless of strategy.

| Task | Component | FR / NFR | Acceptance |
|---|---|---|---|
| `retrieve(query)` → top-k chunks + scores (A: BM25 search; B: embed query → FAISS ANN) | markdown_kb / vector_rag | FR-003 | Relevant chunks retrieved for sample docs under both |
| Citation Formatter: resolve `filename#heading` from chunk metadata; validate against index | citation (shared) | FR-004/005, Reliability | Only retrieved sections cited; no fabricated citations |
| Prompt assembly: labeled context + "answer only from sources" system prompt | answerer (shared) | FR-004 | Answer grounded in provided context |
| **Per-strategy score threshold** → cannot-confirm when below floor | answerer, config | FR-006/016 | Out-of-scope query → 200 honest cannot-confirm, no citation (BM25 + similarity floors tuned separately) |
| "Not indexed yet" path when `.kb/` absent | answerer, persistence | FR-007 | Chat before index → 200 clear "not indexed" message, no crash |
| OpenAI completion via LLM Client (timeout ≤30s, graceful degradation) | llm_client | Reliability, Availability | Upstream failure → graceful error, not 500 crash |

**Exit:** the three grounded/out-of-scope `curl`s pass **under both strategies**:
- "How long do refunds take?" → cites `refund_policy.md#refund-timeline`
- "Can I change my email address?" → cites `account_help.md#change-email-address`
- "Which restaurants are nearby?" → cannot-confirm

### Phase 4 — Persistence & restart (both strategies)
**Goal:** survive restart without re-indexing, for whichever index is on disk.

| Task | Component | FR / NFR | Acceptance |
|---|---|---|---|
| Load `.kb/` on startup per active strategy (`index.json` or `faiss_index/`) | main, persistence | FR-013 | After restart, `/chat` answers without `/index` under both strategies |
| Corrupt/absent index → "needs indexing" state, not crash | persistence | Availability | Unreadable `.kb/` → rebuild prompt, no exception |
| Detect strategy/artifact mismatch (e.g. `index.json` present but `KB_STRATEGY=vector_rag`) | persistence, selector | Reliability | Clear "needs (re)indexing" message, not a crash |
| Fast startup load | persistence | Performance | Sample KB loads <5s |

**Exit:** restart server under each strategy → grounded `curl` still works; both Verification blocks (Markdown-KB-only and Vector-RAG-only) pass end-to-end.

### Phase 5 — Hardening & observability
| Task | Component | FR / NFR | Acceptance |
|---|---|---|---|
| Input validation limits (length cap, content-type) | validator/schemas | Security | Malformed/oversized → 4xx |
| Key protection audit: never logged/returned | llm_client, observability | Security, CON-001 | Grep logs/responses — no key |
| Log similarity scores + refusal rate | observability | Observability | Below-threshold scores and refusals measurable |
| Concurrent index+chat behavior (last-good or 503) | routes, persistence | Availability | `/chat` during re-index doesn't error |

### Phase 6 — Stretch goals (pick per priority)
1. **`POST /chat/stream` (SSE)** — sources event first → tokens → `done`; reuse Citation Formatter + Answer Generator so citations are identical to `/chat` (FR-017).
2. **Browser UI** — tiny page rendering selected sources before the streamed answer (FR-018); key stays server-side.
3. **Score-threshold tuning + paraphrase comparison** — calibrate each strategy's floor against sample docs; with both strategies built, run the Paraphrase Comparison directly (Markdown KB vs Vector RAG): synonym misses, semantic false positives, citation quality (Risk #2).
4. **CLI / MCP** over the same retrieval core (FR-020).
5. **Multi-format import** `raw/*.txt|html → docs/*.md → /index`; sanitize HTML, preserve filename (FR-019).
6. **Wiki generation + answer filing** from index/metadata, citations preserved (FR-021/022).
7. **Conversation memory** — last-N turns informs query only; retrieved chunks still control the answer (FR-022).

---

## 3. Testing Strategy

- **Unit:** shared (heading/chunk splitting, citation slug build/validate, schema validation) tested once; threshold cutoff tested per strategy. Strategy A is fully testable with **no OpenAI mock** (BM25 is local); Strategy B mocks the LLM Client — no live OpenAI in unit tests.
- **Integration:** parametrize the suite over `KB_STRATEGY` so `/index` → `/chat`, restart-load, and cannot-confirm paths run against **both** a fixture `index.json` and a fixture FAISS index.
- **Contract/e2e:** the 9 `PROMPT.md` Verification `curl`s as a scripted smoke test, run once per strategy (the acceptance gate). The strategy-agnostic steps must produce identical response shapes.
- **Reliability:** inject OpenAI timeout/error in the LLM Client mock → assert graceful degradation, no 500 (Strategy B at index+chat; Strategy A at chat only).

## 4. Key Decisions to Confirm Before Coding

| Topic | Proposed default | Source / why |
|---|---|---|
| Default `KB_STRATEGY` | `markdown_kb` (smallest dependency surface, offline-testable) | AD-2; matches scaffold's recommended default — **(proposed)** |
| `/chat` response schema | `{"answer": str, "sources": [{"citation":"file.md#heading","score":float}], "grounded": bool}` — identical across strategies | Pin first (analysis Priority 1); not specified in source — **(proposed)** |
| Embedding model (Strategy B) | `text-embedding-3-small` | Cost/latency balance; configurable per AD-4b |
| Completion model (both) | latest cost-appropriate OpenAI chat model | Configurable in `config.py` |
| `top_k` / chunk size | k=4, ~500-token chunks w/ small overlap | **(proposed)** — tune in Phase 6 #3 |
| Score threshold | **per strategy** (BM25 floor ≠ similarity floor); calibrate against sample docs | FR-016 — score scales differ — **(proposed)** |
| Re-index concurrency | serve last-good index; 503 if none | analysis FR-002 Availability — **(proposed)** |

## 5. Risks (from analysis) & Mitigations

- **Strategy A — synonym/paraphrase misses** (BM25 lexical) → score threshold + honest cannot-confirm; switch to Strategy B when recall matters (paraphrase eval, Phase 6 #3).
- **Strategy B — semantic false positives** → score threshold + paraphrase eval (Phase 3 + 6 #3).
- **Two code paths to maintain** → keep everything except the two strategy modules + Embedding Service shared; parametrize tests over `KB_STRATEGY` so both paths are always exercised.
- **OpenAI dependency** (completions for both; embeddings for B) → single LLM Client chokepoint with timeout/retry/degradation (Phase 2b–3).
- **Embedding cost/latency** (Strategy B) → batch + persist at index time, embed only query at chat (AD-4b).
- **Strategy/artifact mismatch on restart** → detect at load, prompt re-index rather than crash (Phase 4).
- **Scale jump (10→100k)** → out of scope for core; documented evolution in `ARCHITECTURE.md` §7.
- **Stale embeddings on model change** (Strategy B) → version `.kb/` against embedding model; force re-index on mismatch.
