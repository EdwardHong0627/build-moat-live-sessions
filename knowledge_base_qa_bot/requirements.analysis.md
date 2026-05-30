# Requirements — Analysis (NFRs)

> Source: `PROMPT.md` · Derived from `requirements.raw.md` · 2026-05-29

## Overview
This is a retrieval-augmented Q&A service whose dominant quality concerns are correctness/grounding (answers must be traceable to source Markdown), reliability of the external LLM dependency, and the latency/cost profile of OpenAI calls during chat. Because the system handles user-supplied queries and writes derived artifacts to disk, input validation, observability, and clear failure UX also matter. Most NFR targets are not given in the source and are proposed below.

## NFRs by Functional Requirement

### FR-001 / FR-009 / FR-011 — Index documents (build pipeline)
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Performance | Indexing of the small sample KB completes quickly | Full `/index` over the sample `docs/` finishes < 30s (proposed) | proposed |
| Scalability | Indexing scales toward larger corpora | Design Question 8 (10 → 100k files) addressed in design notes; chunk/section count returned (`files_indexed`/`sections_indexed`) | from source |
| Maintainability | Parsing logic is testable and isolated per strategy | Deterministic index given same inputs; unit tests on heading/chunk splitting (proposed) | proposed |
| Reliability | Partial/corrupt Markdown does not abort the whole build | Malformed file is skipped or reported, not fatal (proposed) | proposed |

### FR-002 — `POST /index` endpoint
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Reliability | Returns the exact result contract | 200 with `{"files_indexed": N, "sections_indexed": M}` | from source |
| Observability | Index runs are logged with counts and duration | Each `/index` emits a log line with file/section counts and elapsed time (proposed) | proposed |
| Availability | Concurrent index + chat is safe | `/chat` during re-index returns last-good index or a clear 503/busy signal (proposed) | proposed |

### FR-003 / FR-014 — `POST /chat` + LLM answer generation
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Performance | Bounded response latency despite LLM call | p95 end-to-end `/chat` < 5s (proposed) | proposed |
| Security | Validate query input | Reject non-JSON / missing `query` with 400; cap query length (proposed) | proposed |
| Reliability | Tolerate OpenAI errors/timeouts | On upstream failure return graceful 200 fallback or 502 with retry; timeout ≤ 30s (proposed) | proposed |
| Compliance & Privacy | User queries are sent to OpenAI | Disclose third-party processing; no PII retention beyond request unless memory enabled (proposed) | proposed |
| Usability | Clear, grounded answers | Answer text plus citation returned in a stable JSON shape (proposed) | proposed |

### FR-004 / FR-005 — Grounding and `filename#heading` citation
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Usability | Citations are inspectable and resolve to real sections | Every grounded answer includes ≥1 `filename#heading` that exists in the index | from source |
| Maintainability | Grounding/citation logic shared across `/chat` and `/chat/stream` | Single citation formatter used by all interfaces (proposed) | proposed |
| Reliability | No fabricated citations | Citations only emitted for sections actually retrieved (proposed) | proposed |

### FR-006 / FR-016 — Cannot-confirm fallback & score threshold
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Usability | Honest refusal instead of forced citation | Out-of-scope query returns 200 with explicit cannot-confirm message | from source |
| Reliability | Threshold is configurable and tested | Threshold value externalized; tests cover above/below cutoff (proposed) | proposed |
| Observability | Refusals are measurable | Refusal rate and below-threshold scores logged (proposed) | proposed |

### FR-007 — Chat before indexing
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Usability | Distinct, clear "not indexed" message | 200 response clearly states KB not yet indexed | from source |
| Reliability | No crash when no index present | No exception/500 when `.kb/` is absent (proposed) | proposed |

### FR-008 — `GET /health`
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Availability | Cheap liveness probe | Returns 200 `{"status":"ok"}` without touching OpenAI | from source |
| Observability | Suitable for monitoring/orchestration | Responds in < 100ms, no external dependency (proposed) | proposed |

### FR-010 / FR-012 / FR-013 — Index persistence & startup load
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Reliability | Survives restart without re-indexing | After restart, `/chat` answers correctly without `/index` | from source |
| Performance | Fast startup load | Index loads from `.kb/` in < 5s for sample KB (proposed) | proposed |
| Maintainability | Index artifact is inspectable | `.kb/index.json` / `metadata.json` human-readable | from source |
| Availability | Corrupt persisted index degrades gracefully | Unreadable `.kb/` triggers rebuild prompt, not a crash (proposed) | proposed |

### FR-015 — Embeddings at index & query time (Strategy B)
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Performance | Embedding latency bounded per query | Query embedding adds < 1s to `/chat` p95 (proposed) | proposed |
| Cost / Maintainability | Embedding calls are minimized | Index-time embeddings cached/persisted; only query embedded at chat time (proposed) | proposed |

### FR-017 — Streaming `/chat/stream` (SSE)
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Performance | Low time-to-first-byte | Sources event sent before first token; first token < 2s (proposed) | proposed |
| Reliability | Well-formed stream lifecycle | Sources first, then tokens, then a `done` event | from source |
| Usability | Same grounding/citation as `/chat` | Streamed answer carries identical citations to non-streamed path | from source |

### FR-018 / FR-020 — Browser UI & alternative interfaces (CLI/MCP/Web)
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Usability & Accessibility | Sources shown before answer; readable UI | UI renders selected sources prior to answer text | from source |
| Maintainability | One shared retrieval core across interfaces | CLI/MCP/UI call the same retrieval/answer functions, not forks | from source |
| Security | UI/CLI do not expose API keys to clients | `OPENAI_API_KEY` stays server-side only (proposed) | proposed |

### FR-019 — Multi-format import (txt/html → md)
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Reliability | Index rebuilt after import | Import flow ends with a fresh retrieval index | from source |
| Maintainability | Source filename preserved through conversion | Converted `docs/*.md` retains original source name | from source |
| Security | Sanitize untrusted HTML/text input | Strip scripts/active content during conversion (proposed) | proposed |

### FR-021 / FR-022 — Wiki generation, answer filing, conversation memory
| NFR category | Implied requirement | Acceptance criterion | Origin |
|---|---|---|---|
| Maintainability | Wiki derived deterministically from index | `wiki/index.md` regenerated from `.kb/index.json` | from source |
| Usability | Filed answers keep citations | Q&A written to `wiki/` retains `filename#heading` links | from source |
| Reliability | Memory must not override grounding | Final answer still controlled by retrieved sources | from source |
| Compliance & Privacy | Conversation memory retention bounded | Memory limited to last N turns / cleared per session (proposed) | proposed |

## Cross-Cutting NFRs
NFRs that apply system-wide rather than to one FR.

| NFR category | Requirement | Acceptance criterion | Rationale |
|---|---|---|---|
| Security | Protect the OpenAI key and config | `OPENAI_API_KEY` read from env, never logged or returned (proposed) | CON-001 makes the key the most sensitive secret |
| Security | Input validation on all request bodies | Malformed/oversized payloads rejected with 4xx (proposed) | `/chat` and `/index` accept external input |
| Observability | Structured logging across endpoints | Each request logs endpoint, latency, status, retrieval scores (proposed) | Needed to debug grounding/cost issues |
| Availability | Graceful degradation when OpenAI is down | `/health` stays green; `/chat` returns clear error, not a crash (proposed) | Hard external dependency on OpenAI |
| Maintainability | Strategy A and B behind a common retrieval interface | Swapping strategy needs no `/chat` contract change (proposed) | Two interchangeable strategies are first-class in the design |
| Performance & Cost | LLM/embedding call budget per request | Bounded token usage; configurable model (proposed) | OpenAI usage drives latency and cost |
| Compliance & Privacy | Disclose third-party data processing | Document that queries/contexts are sent to OpenAI (proposed) | Queries leave the local boundary |

## Risks & Trade-offs
- Grounding strictness vs. helpfulness: an aggressive score threshold (FR-016) reduces hallucinated citations but increases cannot-confirm refusals.
- Markdown KB (BM25) vs. Vector RAG: BM25 is inspectable, cheap, and zero-embedding-cost but misses synonyms/paraphrases; vector RAG handles semantics but adds embedding latency, cost, and semantic false positives (Design Questions 5–8, Paraphrase Comparison stretch).
- External LLM dependency vs. availability: every `/chat` depends on OpenAI, coupling uptime, latency, and cost to a third party (CON-001).
- Streaming TTFB vs. complexity: SSE improves perceived latency but duplicates grounding/citation logic unless the retrieval core is shared.
- Scale jump (10 → 100k files): BM25/JSON index and a single FAISS file may not hold; sharding, an external vector DB, and incremental indexing become necessary.

## Recommended Priorities
1. Grounding + citation correctness (FR-004/FR-005) with the cannot-confirm fallback (FR-006) — the core value and trust property of the bot; pin down the `/chat` response schema first.
2. Reliability of the OpenAI dependency (timeouts, error handling, graceful degradation) — every chat answer hinges on it.
3. Input validation and key protection (cross-cutting Security) — cheap, high-value hardening for an open local API.
4. Index persistence and clean startup load (FR-013) plus observability of retrieval scores — enables debugging and the score-threshold stretch goal.
