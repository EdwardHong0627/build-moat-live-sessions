# QR Code Generator — Rust (axum + sqlx/SQLite)

A Rust port of the dynamic QR-code / URL-shortener spec in `../PROMPT.md`, written to
showcase idiomatic Rust design patterns.

See [docs/architecture.md](docs/architecture.md) for C4 diagrams (Context → Container →
Component) and request-flow sequence diagrams.

## Run

```bash
cargo run        # listens on http://localhost:8000, creates ./qr_code.db
```

Then run the verification curls from `../PROMPT.md`.

## Test

```bash
cargo test        # 14 in-process integration tests (no server needed)
```

[`tests/integration.rs`](tests/integration.rs) drives the real `Router` via
`tower::oneshot` against a fresh in-memory SQLite DB. Two groups:

- `prompt_*` — every verification scenario from `../PROMPT.md` (create, 302 redirect,
  info, update, delete→410, unknown→404, PNG image, analytics) plus expiry and
  validation (400) cases.
- `sqli_*` — SQL-injection attempts (tautology token, `DROP TABLE` payload, metachars
  in the URL, injection in analytics) proving the parameterized queries in `repo.rs`
  treat attacker input as data, never SQL.

## Endpoints

| Method | Path                          | Behavior |
|--------|-------------------------------|----------|
| POST   | `/api/qr/create`              | Create a short link (+ optional `expires_at`) |
| GET    | `/r/{token}`                  | **302** redirect; **410** if deleted/expired; **404** if unknown |
| GET    | `/api/qr/{token}`             | Metadata (404 if missing/deleted) |
| PATCH  | `/api/qr/{token}`             | Update target URL and/or expiry |
| DELETE | `/api/qr/{token}`             | Soft delete |
| GET    | `/api/qr/{token}/image`       | QR PNG encoding the short URL |
| GET    | `/api/qr/{token}/analytics`   | `{ total_scans, scans_by_day }` |

## Design patterns on display

| File | Pattern |
|------|---------|
| `src/token.rs` | **Newtype** (`Token`) + **trait object** (`TokenGenerator`) for a swappable strategy |
| `src/error.rs` | **Error enum + `IntoResponse`** — handlers return `Result<_, AppError>` and use `?` |
| `src/state.rs` | **Shared state** — `AppState` is `Clone`; cache is `Arc<RwLock<..>>` |
| `src/repo.rs`  | **Repository** — all SQL lives here; collision-retry driven by the UNIQUE constraint |
| `src/handlers.rs` | Thin handlers; domain rules (status codes) here, no SQL |
| `src/models.rs` / `src/schemas.rs` | DB rows vs. API DTOs kept separate |

## Notes

- Redirects are **302** (built explicitly — axum's `Redirect::temporary` is 307).
- The in-memory cache (stand-in for Redis) stores `expires_at` alongside the URL, so a
  cache hit still returns **410** for an expired link instead of a stale 302.
- URL normalization lowercases scheme/host and strips a trailing `/`; it does **not**
  force http→https or rewrite the path (which is case-sensitive).
- QR PNGs are built from the raw module matrix, avoiding qrcode↔image version coupling.
