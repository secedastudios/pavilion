# Pavilion — Project Plan

> **One upload. Infinite screens. Your rules.**
>
> Open-source film distribution and white-label OTT platform built with Rust, Axum, SurrealDB v3, Askama, Datastar, RustFS, and semantic HTML/CSS.

## Tech Stack

| Layer | Technology | Notes |
|---|---|---|
| Language | Rust (edition 2024) | Idiomatic, no `.unwrap()` in production |
| Web framework | Axum 0.8 | SSR-first, SSE for Datastar fragments |
| Database | SurrealDB v3 (clustered) | Graph-first, SCHEMAFULL, parameterized queries |
| Templating | Askama | Server-rendered HTML, auto-escaping |
| Reactivity | Datastar | Hypermedia/SSE — zero custom JS files |
| Object storage | RustFS (S3-compatible, clustered) | Film masters, transcoded segments, posters |
| Semantic search | Qdrant | Vector similarity for curator film discovery |
| Auth | JWT + Argon2 + SlateHub OAuth | HTTP-only cookie + Bearer header |
| CSS | Semantic, single stylesheet | Custom properties, no frameworks |
| Video | HLS + DASH (CMAF) | H.264, HEVC, AV1 via FFmpeg |
| Async runtime | Tokio | Full features |

## Clustering & Deployment Architecture

```
                    ┌─────────────────┐
                    │  Load Balancer   │
                    │ (nginx/HAProxy)  │
                    └────┬───────┬────┘
                         │       │
              ┌──────────┴┐     ┌┴──────────┐
              │ Pavilion  │     │ Pavilion   │    ← Stateless Rust servers
              │ Node 1    │     │ Node N     │      (JWT auth, no sticky sessions)
              └─────┬─────┘     └─────┬──────┘
                    │                 │
        ┌───────────┼─────────────────┼───────────┐
        │           │                 │           │
  ┌─────┴─────┐ ┌───┴────┐    ┌──────┴──┐  ┌─────┴─────┐
  │ SurrealDB │ │SurrealDB│   │  RustFS  │  │  Qdrant   │
  │ Node 1    │ │Node N   │   │  Cluster │  │  Cluster  │
  │ (TiKV)    │ │(TiKV)   │   │  (RustFS) │  │           │
  └───────────┘ └─────────┘   └──────────┘  └───────────┘
```

**Clustering rules:**
- Pavilion (Axum) servers are **fully stateless** — all session state is in JWT tokens, all data in SurrealDB. Any instance can handle any request. Scale horizontally behind any load balancer.
- SurrealDB uses **TiKV** storage engine for multi-node clustering. All nodes share the same distributed data.
- RustFS runs in **distributed mode** with erasure coding for redundancy.
- Qdrant runs in **distributed mode** with sharding and replication for vector search.
- Transcode workers are separate processes that poll the SurrealDB job queue — scale independently based on queue depth.

## Data Model (Graph-First)

```
person ──[filmmaker_of]──> film
person ──[curator_of]───> platform
person ──[agreed_to]────> terms_version  (consent tracking, GDPR)
film ──[licensed_via]──> license
platform ──[carries]──> film  (through license terms)
film ──[has_asset]────> asset (transcoded renditions)
person ──[watched]────> film  (on a platform, with playhead + timestamps)
person ──[rated]──────> film  (per-platform rating, aggregatable)
film ──[screened_at]──> event
platform ──[hosts]────> event
person ──[earned]─────> payout
person ──[filed]──────> dmca_claim  (against a film)
film ──[claimed_by]───> dmca_claim
```

All tables SCHEMAFULL. Graph relations (RELATE) for every relationship. RecordId everywhere — never string manipulation for IDs. Table is `person` (not `user`).

---

## Constraints & Compliance

### Legal & Content Policy
- **Terms of Use acceptance required** before any upload or platform creation. Must explicitly agree:
  - No pornographic content
  - Uploader is the copyright holder or has written authorization
  - All content and talent releases have been obtained
  - Compliance with platform content policies
- Terms are versioned — re-acceptance required when terms change
- Consent tracked via `person -[agreed_to]-> terms_version` graph edge with timestamp

### DMCA & Copyright Protection
- Public DMCA takedown request form (no login required for copyright holders)
- DMCA claim workflow: filed → under_review → upheld/rejected → resolved
- Automated content removal on valid claim (within legally required timeframe)
- Counter-notification support
- Repeat infringer policy with account suspension
- Designated DMCA agent contact page

### GDPR Compliance (EU-Based)
- Explicit consent collection with granular options (stored, auditable)
- Right to access: person can export all their data (profile, watch history, transactions)
- Right to erasure: full account deletion with cascade (anonymize watch data, remove personal info)
- Right to portability: machine-readable data export (JSON)
- Data processing records maintained
- Cookie consent banner (only Datastar CDN is external — minimal cookies)
- Privacy policy page with clear data usage explanation
- Data retention policies (auto-purge watch session details after configurable period)
- DPO contact information

### Content Security — Zero Bypass Policy
- **No video content is ever accessible without passing through the rights enforcement layer**
- RustFS buckets are **private** — no public URLs, ever
- All segment URLs are presigned with short TTL (minutes, not hours)
- Manifest proxy verifies: platform carries film + license active + user authorized
- Referrer and origin validation on segment requests
- Rate limiting on manifest and segment endpoints
- Token-bound sessions — signed URLs tied to the requesting user's session
- Audit log for all content access attempts

---

## Phases

### Phase 0: Project Scaffolding
Status: **Complete**

- [x] **0.1** Project structure: src/ (lib.rs, main.rs, config.rs, router.rs, error.rs, sse.rs, db/mod.rs), templates/, static/css/, db/, tests/
- [x] **0.2** Cargo.toml with all dependencies (axum 0.8, surrealdb 3, askama 0.15, tokio, jsonwebtoken, argon2, qdrant-client, tracing, tower-http, etc.)
- [x] **0.3** Docker Compose: SurrealDB (SurrealKV), RustFS, Qdrant
- [x] **0.4** Makefile: dev, services, services-down, build, db-init, db-drop, db-seed, test, healthcheck
- [x] **0.5** .env-example, .gitignore
- [x] **0.6** Base Askama template (base.html) with OG tags, Datastar CDN, cookie consent
- [x] **0.7** CSS design tokens and base styles in main.css (dark mode, reduced motion, forms, reset)
- [x] **0.8** AppError type with IntoResponse (NotFound, Forbidden, Unauthorized, Validation, LicenseViolation, Database, Internal)
- [x] **0.9** Config via dotenv (.env loading with defaults)
- [x] **0.10** Logging: tracing + tracing-subscriber (pretty/JSON toggle via PRETTY_LOGS)
- [x] **0.11** GET /healthcheck (JSON: status, version, DB connectivity)
- [x] **0.12** SSE fragment + remove helpers for Datastar
- [x] **0.13** cargo build + cargo test pass (2 integration tests: healthcheck OK, 404 on unknown route)

### Phase 1: Authentication, Consent & Person Profiles
Status: **Complete**

- [x] **1.1** SurrealDB schema: `person` table (email, name, password_hash, roles, bio, avatar_url, slatehub_id, gdpr_consent, timestamps; UNIQUE indexes on email + slatehub_id; ASSERT on email format + name length)
- [x] **1.2** SurrealDB schema: `terms_version` table + `agreed_to` graph relation (FROM person TO terms_version)
- [x] **1.3** Argon2 password hashing (src/auth/password.rs)
- [x] **1.4** JWT claims with issue/verify (src/auth/claims.rs)
- [x] **1.5** Auth middleware: Claims extractor (Bearer header + cookie) + OptionalClaims (src/auth/middleware.rs)
- [x] **1.6** Auth routes: GET/POST /register, GET/POST /login, POST /logout, GET /auth/slatehub (stub), GET /auth/slatehub/callback (stub)
- [x] **1.7** Terms acceptance: 4 mandatory checkboxes on register (terms, no porn, copyright, talent); legal pages at /terms, /privacy, /content-policy with full semantic HTML
- [x] **1.8** GDPR: consent toggles on register; GET/PUT /settings/privacy; GET /settings/data-export (JSON download); POST /settings/delete-account (cascade delete + cookie clear)
- [x] **1.9** Auth templates: register.html, login.html with fieldset/legend, labels, validation errors
- [x] **1.10** Profile routes: GET /profile, GET /profile/edit (SSE fragment), PUT /profile (SSE fragment)
- [x] **1.11** Profile templates: profile.html, partials/profile_display.html, partials/profile_edit.html
- [x] **1.12** Navigation component (templates/components/nav.html) with auth state
- [x] **1.13** Cookie consent banner in base.html (Datastar dismiss)
- [x] **1.14** 18 tests passing: password hash/verify, JWT issue/verify/wrong-secret, register page/validation/success, login page/invalid-credentials, protected routes (profile, settings, data-export) return 401, legal pages return 200

### Phase 2: Film Management (Filmmaker)
Status: **Complete** (core CRUD; upload flow + Qdrant embedding deferred to Phase 3/5)

- [x] **2.1** SurrealDB schema: `film` table (title, slug, synopsis, year, duration_seconds, genres, language, country, poster_url, trailer_url, status, content_declaration, timestamps; UNIQUE slug; index on status) + `filmmaker_of` relation (FROM person TO film, with role)
- [x] **2.2** SurrealDB schema: `asset` table (asset_type, codec, resolution, bitrate, format, storage_key, size_bytes) + `has_asset` relation (FROM film TO asset)
- [x] **2.3** Film CRUD: GET /films (index, ownership-filtered), GET /films/new, POST /films (with content declaration), GET /films/:id, GET /films/:id/edit (SSE), PUT /films/:id (SSE, ownership verified), DELETE /films/:id (archive)
- [x] **2.4** Content declaration: 3 mandatory checkboxes on create (copyright, talent, no prohibited), stored per-film with timestamp
- [x] **2.5** Film templates: films_index.html, film_new.html, film_detail.html, partials/film_info.html, partials/film_edit.html
- [ ] **2.6** Film upload flow (deferred to Phase 3 — needs RustFS integration)
- [ ] **2.7** Poster/thumbnail upload (deferred to Phase 3)
- [x] **2.8** Film metadata inline edit via Datastar SSE fragments
- [x] **2.9** Status workflow: draft → published → archived → draft (with validation)
- [ ] **2.10** Qdrant embedding on publish (deferred to Phase 5 — semantic search)
- [x] **2.11** 8 tests: auth enforcement, content declaration validation, create success, index shows owned films, publish from draft, invalid transition rejected, ownership enforcement (cross-person edit blocked)

### Phase 3: Video Transcoding Pipeline (Queue-Based)
Status: **Complete** (RustFS upload/download integration pending — worker skeleton ready)

- [x] **3.1** SurrealDB schema: `transcode_job` table (film ref, status, worker_id, profile, progress_pct, error_msg, retry_count, max_retries, timestamps; compound index on status+created_at)
- [x] **3.2** Queue protocol (distributed-safe): SELECT+UPDATE two-step claim with WHERE guard, heartbeat, progress updates, complete/fail with retry logic, job listing per film
- [x] **3.3** FFmpeg module: H.264 ABR ladder (360p/480p/720p/1080p), CMAF fMP4 segments, HLS output, progress parsing from stdout
- [x] **3.4** Subtitle handling (schema ready — upload integration deferred to RustFS)
- [x] **3.5** Transcoding worker: background async task, poll→claim→transcode→upload loop, heartbeat every 30s, work directory management, cleanup
- [x] **3.6** Stale job reaper: 60s scan interval, 5min heartbeat threshold, re-queue or permanently fail based on retry_count
- [x] **3.7** Manifest generation: HLS master .m3u8 + DASH MPD, referencing CMAF segments per rendition
- [x] **3.8** Transcode progress UI: SSE endpoint polling job status every 2s, Datastar fragment updates with progress bar, auto-stops on terminal state
- [x] **3.9** 11 tests: enqueue, claim, no-jobs, double-claim prevention, progress update, complete, fail+requeue, fail permanently after max retries, jobs-for-film, HLS manifest, DASH MPD

### Phase 4: Licensing Engine
Status: **Complete**

- [x] **4.1** SurrealDB schema: `license` table (license_type, territories, window_start/end, approval_required, active, plus type-specific pricing fields for TVOD/SVOD/AVOD/event/educational/CC) + `licensed_via` graph relation (FROM film TO license)
- [x] **4.2** All 7 license types with type-specific validation: TVOD (rental/purchase with duration), SVOD (flat fee or revenue share), AVOD (revenue share required), Hybrid, Event (flat fee or ticket split), Educational (institution types, pricing tier), Creative Commons (CC type required). Revenue share and ticket split validated 0-100%.
- [x] **4.3** License management controllers: GET /films/:id/licenses (index), GET /films/:id/licenses/new (form), POST /films/:id/licenses (create with validation + graph relation), GET/PUT /films/:film_id/licenses/:license_id (edit/update via SSE), POST /films/:film_id/licenses/:license_id/deactivate. All ownership-verified.
- [x] **4.4** License form templates: license_new.html (all type sections with Datastar type selector), licenses_index.html, partials/license_detail.html, partials/license_edit.html
- [x] **4.5** Validation at model level (validate_license) + DB ASSERT constraints on license_type enum
- [x] **4.6** Rights resolution engine (src/licensing/rights.rs): resolve_available_films (territory + window + active filter), film_is_licensed_for (specific film + territory), film_has_any_license, licenses_for_film. Territory-aware, window-aware, uses graph traversal.
- [x] **4.7** 11 tests: 7 validation unit tests (TVOD requires price, rental needs duration, SVOD needs fee/share, AVOD needs share, CC needs type, share 0-100%), 4 integration tests (auth required, create TVOD, validation rejection, deactivate)

### Phase 5: Semantic Search & Catalog Discovery
Status: **Complete** (Qdrant semantic search deferred — DB-based text search and filtering operational)

- [ ] **5.1** Qdrant integration (deferred — requires embedding model API decision; DB search covers MVP)
- [ ] **5.2** Semantic search endpoint (deferred — current text search + filters covers MVP)
- [x] **5.3** Catalog browse: GET /catalog with text search (title/synopsis), genre/year/language filters, published-only + active-license-only filtering, HashMap-based dynamic query bindings
- [x] **5.4** Catalog templates: catalog.html (search bar, filter inputs, responsive film card grid with synopsis truncation), catalog_film.html (full detail with available licenses, acquire buttons, JSON-LD Movie schema, breadcrumb nav)
- [x] **5.5** License acquisition: POST /catalog/:id/acquire — auto-approve if approval_required=false, pending otherwise. Duplicate detection. Acquisition result page with status-specific messaging.
- [x] **5.6** Filmmaker approval workflow: GET /films/:id/requests (list all requests with status), POST approve/reject with resolved_at + resolved_by tracking
- [x] **5.7** Catalog filtering: published status, active licenses (graph traversal ->licensed_via->license), text search, genre/year/language. DMCA filtering ready for Phase 10.
- [x] **5.8** 7 tests: catalog loads, published films with licenses appear, draft films hidden, detail shows licenses, acquire without approval (auto-approve), acquire requires auth, filmmaker sees requests

### Phase 6: White-Label Platform Engine & Site Builder
Status: **Complete** (visual wizard builder and branded player builder deferred to UX refinement)

- [x] **6.1** SurrealDB schema: `platform` table (name, slug, domain, description, logo_url, theme object with CSS properties, monetization_model, subscription_price_cents, status; UNIQUE slug + domain) + `curator_of` relation (FROM person TO platform with role) + `carries` relation (FROM platform TO film with position, featured, acquisition ref)
- [x] **6.2** Platform CRUD: GET /platforms (index, ownership-filtered), GET /platforms/new, POST /platforms (create + curator_of relation), GET /platforms/:id (dashboard with carried films), GET/PUT /platforms/:id/edit (SSE), POST /platforms/:id/activate
- [ ] **6.3** Visual site builder wizard (deferred — current form covers all settings; step-by-step wizard is UX polish)
- [ ] **6.4** Branded video player builder (deferred to Phase 7 integration)
- [x] **6.5** Platform theme engine: PlatformTheme struct → `to_css_overrides()` generates `:root` CSS custom property block, injected into public templates via `<style>` tag. Color pickers, border radius selection, dark mode toggle.
- [x] **6.6** Content management: POST /platforms/:id/content (add film with featured flag), POST /platforms/:id/content/:film_id/remove. Dashboard shows carried films with remove buttons.
- [x] **6.7** Public rendering: GET /p/:slug (platform home, themed, shows carried published films), GET /p/:slug/:film_slug (film detail, themed, carries verification, JSON-LD Movie schema). Only active platforms render publicly.
- [x] **6.8** 7 tests: auth required, create success, index shows owned, activate platform, public 404 when setup, public renders when active (with theme CSS injection), ownership enforcement (other curator blocked)

### Phase 7: Video Player & Secure Delivery (Zero-Bypass)
Status: **Complete** (RustFS streaming pending integration; analytics aggregation deferred to Phase 10)

- [x] **7.1** Rights-aware manifest proxy: GET /watch/:platform_slug/:film_slug/manifest.m3u8 (HLS) and .mpd (DASH). Full enforcement chain: (1) platform active, (2) film published, (3) platform carries film, (4) active license exists. Manifest rewritten with signed segment URLs. Audit logged. DMCA + entitlement checks stubbed for Phase 9/11.
- [x] **7.2** Segment proxy: GET /segments/:token — HMAC-SHA256 signed tokens with base64 encoding. Token contains person_id, film_id, platform_id, segment_path, expires_at (5min TTL). Verifies signature, expiry, and person match. RustFS streaming pending integration.
- [x] **7.3** HTML5 video player: `<video>` element with HLS.js for non-Safari, native HLS for Safari. Platform-themed via CSS custom properties. Resume from last position. Accessible (aria-label, keyboard controls).
- [x] **7.4** Playhead tracking: `watch_session` table (person, film, platform, progress_seconds, duration_seconds, completed). Heartbeat POST every 15s from player JS. UPSERT per person+film+platform. Auto-completes at 90% duration. Resume position loaded on player open.
- [ ] **7.5** Per-film analytics aggregation (deferred to Phase 10 — raw data collected via watch_session)
- [x] **7.6** Content protection: HMAC-signed user-bound expiring tokens, manifest no-store cache, audit logging (stream_audit table with person, film, platform, action, result, reason). Rate limiting and concurrent stream limits deferred to Phase 15.
- [x] **7.7** 10 tests: token sign/verify, wrong secret rejected, tampered token rejected, person mismatch detected, player requires auth, nonexistent platform 404, full chain player loads, HLS manifest has signed URLs only, uncarried film denied, invalid segment token rejected

### Phase 8: Ratings System
Status: **Complete**

- [x] **8.1** SurrealDB schema: `rating` table (person, film, platform, score 1-5, review_text, hidden; UNIQUE index on person+film+platform for one-rating-per-person-per-film-per-platform)
- [x] **8.2** Rating controllers: POST /p/:slug/films/:id/rate (submit or upsert), DELETE /p/:slug/films/:id/rate (remove own), GET /p/:slug/films/:id/ratings (list via SSE fragment), POST /p/:slug/ratings/:id/hide (curator moderation)
- [x] **8.3** Rating aggregation: per-platform average and count computed from query results. Cross-platform aggregate function available for catalog/filmmaker dashboard.
- [x] **8.4** Rating display: SSE fragment template showing average, count, rating list, submit/update form, delete button
- [x] **8.5** Moderation: curator can hide ratings on their platform (hidden flag, excluded from display and aggregation)
- [x] **8.6** 5 tests: auth required, submit and retrieve, score validation (1-5), update existing rating, per-platform isolation (ratings on one platform don't appear on another)

### Phase 9: Viewer Payments & Entitlements
Status: **Complete** (viewer subscription management UI deferred)

Architecture: `PaymentProvider` trait with Stripe impl + `NoopProvider`. All config via env vars — leave `STRIPE_SECRET_KEY` empty to disable payments entirely. Self-hosters run without Stripe, can implement their own provider.

- [x] **9.1** SurrealDB schema: `viewer_subscription` (person, platform, provider, external_id, status; UNIQUE person+platform), `entitlement` (person, film, platform, type, expires_at; indexed lookup), `payment_account` (platform, provider, external_account_id, onboarding_complete; UNIQUE platform)
- [x] **9.2** Entitlement checking: `check_entitlement()` — AVOD/CC/free auto-grant, SVOD checks active subscription, TVOD checks entitlement+expiry. `grant_entitlement()`, `grant_subscription_entitlements()`, `revoke_subscription_entitlements()`.
- [x] **9.3** `PaymentProvider` trait: `create_connect_account()`, `create_checkout_session()`, `verify_webhook()`. Stripe impl via direct reqwest API calls. `NoopProvider` for disabled instances.
- [x] **9.4** Viewer checkout: POST /p/:slug/checkout — Stripe Checkout with connected account, configurable application fee (FACILITATION_FEE_PCT), metadata for webhook fulfillment.
- [x] **9.5** Entitlement checking wired into player (stubbed in enforcement chain, ready to activate)
- [x] **9.6** Curator Stripe Connect: GET /platforms/:id/payments, POST connect, GET callback
- [ ] **9.7** Viewer subscription management UI (deferred — backend fully operational)
- [x] **9.8** 8 tests: AVOD/CC auto-grant, TVOD requires entitlement, purchase grants access, rental with future expiry, disabled payment pages, webhook signature rejection

### Phase 10: Revenue Tracking & Payouts
Status: **Complete** (per-film/per-platform breakdowns and payout system deferred)

- [x] **10.1** SurrealDB schema: `transaction` table (type, amount_cents, currency, film, platform, person, external_id, status; indexed on platform, film, created_at) + `revenue_split` table (transaction, recipient, role, amount_cents; indexed on recipient, transaction)
- [x] **10.2** Revenue recording: `record_transaction()` creates transaction + auto-calculates 3-way split (platform fee % → filmmaker share % → curator remainder). Configurable facilitation fee (0% for self-hosted).
- [x] **10.3** Filmmaker revenue dashboard: GET /revenue — total earned, transaction count, formatted dollar display. Empty state messaging.
- [x] **10.4** Revenue split calculation: fee → filmmaker → curator split with configurable FACILITATION_FEE_PCT. Curator found via graph traversal (curator_of relation). Splits stored as individual revenue_split records.
- [ ] **10.5** Payout system (deferred — Stripe Connect handles automatic payouts to curators; filmmaker payout aggregation TBD)
- [x] **10.6** Curator analytics dashboard: GET /platforms/:id/analytics — total revenue, curator share, transaction count, active subscribers, total views. Stat grid layout.
- [x] **10.7** 6 tests: transaction with splits, zero-fee transaction, filmmaker starts at zero, platform starts at zero, dashboard requires auth, dashboard loads with $0.00

### Phase 11: DMCA & Copyright Tools
Status: **Complete** (bulk claim tools and API endpoint deferred)

- [x] **11.1** SurrealDB schema: `dmca_claim` table (claimant info, film ref, description, evidence_url, status workflow, good_faith + perjury declarations, counter_reason, admin_notes, timestamps; indexed on film + status)
- [x] **11.2** Public DMCA form: GET /dmca (no login required), POST /dmca (validates name, email, description, good faith + perjury checkboxes). Success confirmation page.
- [x] **11.3** DMCA workflow: Admin review via POST /admin/dmca/:id (uphold/reject/resolve with notes). Filmmaker counter-notification via POST /films/:id/claims/:id/counter. Status transitions: filed → under_review → upheld/rejected/counter_filed → resolved.
- [x] **11.4** Automated enforcement: `film_has_active_claim()` check wired into manifest proxy enforcement chain (Phase 7). Films with filed/under_review/upheld claims are blocked from streaming. `upheld_claims_for_filmmaker()` for repeat infringer detection.
- [x] **11.5** DMCA designated agent page: GET /dmca/agent with legally required contact information and filing instructions.
- [ ] **11.6** Copyright holder tools (deferred — bulk CSV upload and API endpoint for automated monitoring)
- [x] **11.7** 7 tests: form loads without auth, agent page loads, submit requires declarations, submit success, film has no claims initially, filed claim blocks film, filmmaker sees claims

### Phase 12: Events & Screenings
Status: **Complete** (live chat/Q&A and notifications deferred)

- [x] **12.1** SurrealDB schema: `event` table (title, description, event_type, film, platform, start/end time, max_attendees, ticket_price_cents, status; indexed on platform, status, start_time) + `attending` relation (FROM person TO event with ticket_id)
- [x] **12.2** Event CRUD: GET /platforms/:id/events (curator list), GET /platforms/:id/events/new, POST /platforms/:id/events (create). GET /events/:id (public detail with auth-aware ticket/register buttons).
- [x] **12.3** Ticketing: POST /events/:id/tickets — attendee cap enforcement, duplicate registration prevention, UUID ticket IDs. Paid ticket flow deferred to Stripe integration.
- [ ] **12.4** Live event page with chat/Q&A (deferred — uses same Phase 7 player, chat is SSE enhancement)
- [ ] **12.5** Event notifications (deferred — scheduling reminders and "starting now" push)
- [x] **12.6** Status workflow: upcoming → live → ended, upcoming → canceled. Curator-only status changes. Invalid transitions rejected. Public detail shows "Join screening" link when live, "Register" button when upcoming.
- [x] **12.7** 5 tests: curator access enforcement, create event success, ticket purchase + sold out cap, status transitions (valid + invalid), public event detail without auth

### Phase 13: Storage Costs & Billing
Status: **Complete**

- [x] **13.1** Storage metering: `storage_usage` table (per-person total_bytes, master_bytes, rendition_bytes, asset_count, film_count; UNIQUE person). Functions: `get_usage`, `record_upload`, `record_deletion`, `increment_film_count`, `format_bytes` (human-readable).
- [x] **13.2** Pricing tiers: `pricing_tier` table (name, max_storage, max_films, price; UNIQUE name). 4 default tiers: Free (10GB/1 film/$0), Starter (50GB/5/$9.99), Pro (250GB/25/$29.99), Studio (1TB/100/$99.99). `recommended_tier`, `exceeds_tier`, `estimate_monthly_cost`.
- [x] **13.3** Billing dashboard: GET /billing — storage breakdown (total, master, renditions), film/asset counts, current tier, estimated monthly cost, credit balance. Stat grid layout.
- [x] **13.4** Curator credits: `credit_balance` (per-person, UNIQUE) + `credit_transaction` (purchase/deduction/refund with description). `get_balance`, `add_credits`, `deduct_credits` (with insufficient balance check), `transaction_history`.
- [x] **13.5** 9 tests: storage starts at zero, upload increases totals, format_bytes, default tiers exist, credit balance starts at zero, add+deduct credits, insufficient balance fails, billing requires auth, dashboard loads

### Phase 14: Admin & Operations
Status: **Complete**

- [x] **14.1** Admin dashboard: GET /admin — system overview with live counts (persons, films, platforms, pending DMCA, active streams, queued transcode jobs)
- [x] **14.2** Person management: GET /admin/persons — list all persons with roles, POST update roles, GET GDPR data export, POST GDPR account deletion (cascades all relations)
- [x] **14.3** DMCA admin: GET /admin/dmca — list all claims with uphold/reject/resolve actions
- [x] **14.4** GDPR admin tools: per-person data export (JSON download), full erasure (deletes person + all graph relations, watch sessions, entitlements, subscriptions, ratings, credits, storage usage)
- [x] **14.5** Admin-only role check middleware (`require_admin` function)
- [x] **14.6** 4 tests: admin requires auth, admin requires admin role (non-admin gets 403), showcase loads without auth, security headers present on responses

### Phase 15: Production Hardening
Status: **Complete** (rate limiting, CORS, load testing, and backup strategy deferred to deployment)

- [x] **15.1** Dockerfile: multi-stage build (rust:1.85-bookworm builder → gcr.io/distroless/cc-debian12 runtime). Dependency caching, static files + templates + db schema copied.
- [x] **15.2** Security headers middleware: X-Content-Type-Options: nosniff, X-Frame-Options: DENY, X-XSS-Protection, Referrer-Policy, Content-Security-Policy (allows Datastar CDN + Stripe)
- [x] **15.3** Response compression: gzip via tower-http CompressionLayer
- [x] **15.4** Error pages: branded 404 and 500 templates with semantic HTML and navigation links
- [ ] **15.5** Rate limiting (deferred to deployment — use nginx/Caddy reverse proxy)
- [ ] **15.6** CORS, load testing, backup strategy (deployment configuration)

### Phase 16: Example Streaming Site (Reference Implementation)
Status: **Complete** (Stripe payment integration and documentation deferred)

- [x] **16.1** "Pavilion Showcase" at GET /showcase — fully working curated streaming site showing all published, licensed films
- [x] **16.2** Public pages: hero section, film grid (reuses catalog-grid CSS), CTA to create platform
- [ ] **16.3** Payment integration in showcase (deferred — Stripe flow is built in Phase 9, showcase links to catalog for acquisition)
- [ ] **16.4** Documentation: "Build your own streaming site" guide (deferred)

---

## Key Design Decisions

### Why Graph-First (SurrealDB RELATE)
Every relationship in Pavilion carries data: a filmmaker's *role* on a film, a license's *terms* between film and platform, a viewer's *watch progress* and *playhead position*. Graph edges with properties model this naturally without junction tables or JSON blobs.

### Why SSR + Datastar (No SPA)
Film catalog pages, dashboards, and settings must be SEO-friendly and fast. SSR with Askama gives instant first paint. Datastar adds interactivity (inline editing, live progress, real-time revenue, playhead tracking) via SSE without a JS framework or client-side state management.

### Why RustFS (Clustered)
Video files are large (masters can be 50GB+). S3-compatible object storage separates binary assets from the application database. RustFS is self-hostable and supports distributed mode with erasure coding — keeping the "no vendor lock-in" promise while enabling redundancy.

### Why Qdrant for Search
Curators need to find films by *concept*, not just keywords. "Dark European comedies about dysfunctional families" requires semantic understanding. Qdrant provides fast vector similarity search that can be filtered by metadata (territory, license type, genre) and clustered for scale.

### Why CMAF
CMAF (Common Media Application Format) uses fMP4 segments that work with both HLS and DASH manifests. One set of transcoded segments serves all players — no duplicate storage.

### Why `person` Not `user`
Aligns with SlateHub's data model where profiles represent people (actors, crew, filmmakers), not just application accounts. Also avoids conflicts with SurrealDB's reserved `user` concept in its auth system.

### Ownership Verification Pattern
**Every controller that accesses a resource MUST verify ownership via graph traversal**, not just by knowing the ID. Example: to edit a film, verify `person -[filmmaker_of]-> film` exists. To manage a platform's content, verify `person -[curator_of]-> platform`. This is enforced at the application level AND via SurrealDB PERMISSIONS where possible.

### Content Security — Zero Trust
RustFS is never exposed to clients. Every video byte passes through Pavilion's rights enforcement layer. Signed, user-bound, time-limited tokens on every segment request. No shortcut, no direct URL, no bypass. If the license check fails, the content doesn't flow.

---

## Current Step
**All phases complete. RustFS integrated. TMDB/IMDB enrichment operational.** Remaining work: Qdrant semantic search, viewer subscription management UI, rate limiting, CORS, load testing, backup strategy, native uploader (Tauri), and documentation.
