# Architecture — QR Code Generator (Rust)

C4 model diagrams for the dynamic QR-code / URL-shortener service. The boxes in the
Component diagram map 1:1 to the modules in [`../src/`](../src/).

---

## Level 1 — System Context

```mermaid
C4Context
    title System Context — QR Code Generator

    Person(creator, "Link Creator", "Creates and manages dynamic QR codes / short links")
    Person(scanner, "Scanner / End User", "Scans a QR code or opens a short link")

    System(qr, "QR Code Generator", "Issues short tokens, serves 302 redirects, renders QR images, records scan analytics")

    System_Ext(target, "Target Website", "The destination the short link redirects to")

    Rel(creator, qr, "Creates / updates / deletes links, views analytics", "HTTP/JSON")
    Rel(scanner, qr, "Requests /r/{token}", "HTTP")
    Rel(qr, scanner, "302 → target URL")
    Rel(scanner, target, "Follows redirect", "HTTPS")

    UpdateLayoutConfig($c4ShapeInRow="2", $c4BoundaryInRow="1")
```

---

## Level 2 — Containers

```mermaid
C4Container
    title Container Diagram — QR Code Generator

    Person(creator, "Link Creator", "Manages links via API")
    Person(scanner, "Scanner / End User", "Scans QR codes")
    System_Ext(target, "Target Website", "Redirect destination")

    System_Boundary(qr, "QR Code Generator") {
        Container(api, "Web / API Server", "Rust, axum, tokio", "Handles create/update/delete, redirects, QR image rendering, analytics")
        ContainerDb(cache, "Redirect Cache", "In-memory (Arc<RwLock<HashMap>>)", "token → {url, expires_at}; stand-in for Redis")
        ContainerDb(db, "SQLite Database", "sqlx + SQLite", "url_mappings + scan_events tables")
    }

    Rel(creator, api, "POST/PATCH/DELETE /api/qr/...", "HTTP/JSON")
    Rel(scanner, api, "GET /r/{token}", "HTTP")
    Rel(api, scanner, "302 redirect")
    Rel(scanner, target, "Follows redirect", "HTTPS")

    Rel(api, cache, "Reads on redirect, writes on create/update, invalidates on delete")
    Rel(api, db, "Reads/writes mappings; appends scan events", "SQL")

    UpdateLayoutConfig($c4ShapeInRow="2", $c4BoundaryInRow="1")
```

---

## Level 3 — Components (inside the Web / API Server)

```mermaid
C4Component
    title Component Diagram — Web / API Server (Rust crate)

    Person(scanner, "Scanner / Creator", "HTTP clients")
    ContainerDb(cache, "Redirect Cache", "Arc<RwLock<HashMap>>", "token → {url, expires_at}")
    ContainerDb(db, "SQLite", "sqlx", "url_mappings, scan_events")

    Container_Boundary(api, "Web / API Server") {
        Component(handlers, "Handlers", "handlers.rs", "axum routes; cache→DB→404/410 flow; builds 302 + QR PNG")
        Component(state, "AppState", "state.rs", "Shared pool + cache + token generator (Clone, injected via State extractor)")
        Component(error, "AppError", "error.rs", "Error enum + IntoResponse → 400/404/410/500")
        Component(validator, "URL Validator", "url_validator.rs", "Scheme check, blocklist, normalization")
        Component(token, "Token Generator", "token.rs", "Token newtype + TokenGenerator trait (random base62)")
        Component(repo, "Repository", "repo.rs", "All SQL; collision-retry via UNIQUE constraint")
        Component(schemas, "DTOs / Models", "schemas.rs, models.rs", "Serde request/response + FromRow rows")
    }

    Rel(scanner, handlers, "HTTP request", "axum")
    Rel(handlers, state, "Reads pool / cache / generator")
    Rel(handlers, validator, "validate_url()")
    Rel(handlers, repo, "insert / find / update / soft_delete / scans")
    Rel(handlers, error, "Returns Result<_, AppError>; ? propagation")
    Rel(handlers, schemas, "Deserializes / serializes")
    Rel(repo, token, "generate() until DB accepts")
    Rel(repo, db, "Runtime-checked queries", "SQL")
    Rel(handlers, cache, "get / put / invalidate")
    Rel(repo, schemas, "Maps rows via FromRow")

    UpdateLayoutConfig($c4ShapeInRow="3", $c4BoundaryInRow="1")
```

---

## Level 4 — Dynamics: the redirect request (`GET /r/{token}`)

The hot path. Cache hit serves a 302 without touching SQLite; on a miss the DB
decides between 302 / 404 / 410.

```mermaid
sequenceDiagram
    autonumber
    actor U as Scanner
    participant H as Handlers (redirect)
    participant C as Cache
    participant R as Repo
    participant DB as SQLite

    U->>H: GET /r/{token}

    H->>C: cache_get(token)
    alt cache hit
        C-->>H: CachedTarget{url, expires_at}
        alt expired
            H->>C: invalidate(token)
            H-->>U: 410 Gone
        else valid
            H->>R: insert_scan(token, ua, ip)
            R->>DB: INSERT scan_events
            H-->>U: 302 Location: url
        end
    else cache miss
        H->>R: find_by_token(token)
        R->>DB: SELECT * FROM url_mappings
        DB-->>R: row?
        R-->>H: Option<UrlMapping>
        alt none
            H-->>U: 404 Not Found
        else is_deleted
            H-->>U: 410 Gone
        else is_expired
            H-->>U: 410 Gone
        else valid
            H->>C: cache_put(token, url, expires_at)
            H->>R: insert_scan(token, ua, ip)
            R->>DB: INSERT scan_events
            H-->>U: 302 Location: url
        end
    end
```

---

## Level 4 — Dynamics: create a link (`POST /api/qr/create`)

Shows the token collision-retry loop driven by the DB's UNIQUE constraint.

```mermaid
sequenceDiagram
    autonumber
    actor U as Link Creator
    participant H as Handlers (create_qr)
    participant V as URL Validator
    participant R as Repo
    participant T as Token Generator
    participant DB as SQLite
    participant C as Cache

    U->>H: POST /api/qr/create {url, expires_at?}
    H->>V: validate_url(url)
    alt invalid (scheme / blocklist / length)
        V-->>H: AppError::BadRequest
        H-->>U: 400 Bad Request
    else ok
        V-->>H: normalized url
        loop until INSERT succeeds (max 10)
            H->>R: insert_mapping(...)
            R->>T: generate()
            T-->>R: Token (random base62)
            R->>DB: INSERT url_mappings (token UNIQUE)
            alt unique violation
                DB-->>R: constraint error
                Note over R: retry with a new token
            else inserted
                DB-->>R: ok
            end
        end
        R-->>H: UrlMapping
        H->>C: cache_put(token, url, expires_at)
        H-->>U: 200 {token, short_url, qr_code_url, original_url}
    end
```

---

### Notes
- Mermaid's C4 support is experimental; `UpdateLayoutConfig` tunes shapes-per-row if a
  diagram looks cramped. Sequence diagrams render cleanly everywhere.
- Renders on GitHub, in VS Code (Markdown preview), and at mermaid.live.
