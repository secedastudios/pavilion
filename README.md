# Pavilion

**One upload. Infinite screens. Your rules.**

Open-source film distribution and white-label OTT platform built with Rust. See [PRODUCT.md](PRODUCT.md) for the full product vision.

## Tech Stack

- **Rust** (edition 2024) with **Axum** — server-side rendered web application
- **SurrealDB v3** — graph-first multi-model database (clusterable via TiKV)
- **Askama** — compiled Jinja-like templates
- **Datastar** — hypermedia reactivity via SSE (zero custom JavaScript)
- **RustFS** — S3-compatible object storage for video files (clusterable)
- **Qdrant** — vector search for semantic film discovery
- **FFmpeg** — adaptive bitrate transcoding (HLS + DASH via CMAF, up to 4K)
- **Stripe Connect** — marketplace payments with configurable revenue splits
- **pavilion-media** — reusable video infrastructure crate (storage, transcode, signed tokens)

## What's Built

Pavilion is a complete, working application with 121 passing tests:

- **Auth** — Registration with terms acceptance, login, JWT (cookie + Bearer), SlateHub OAuth stubs, GDPR consent management, data export, account deletion
- **Films** — CRUD with content declarations, status workflow (draft → published → archived), ownership via graph relations, file upload to RustFS, poster processing (4 sizes), TMDB/IMDB metadata enrichment with cast/crew import
- **Transcoding** — Queue-based FFmpeg pipeline (SurrealDB job queue, distributed-safe atomic claim, heartbeat, stale reaper). H.264 ABR ladder from 360p to 4K. CMAF output for both HLS and DASH
- **Licensing** — 7 license types (TVOD, SVOD, AVOD, hybrid, event, educational, Creative Commons) with territory and time window support, per-type validation
- **Catalog** — Public browsing with text search, genre/year/language filters. Only shows published films with active licenses
- **Acquisitions** — Curators request licenses, auto-approve or filmmaker approval workflow
- **Platforms** — White-label streaming sites with theme engine (CSS custom property overrides, color pickers, dark mode). Public rendering at `/p/:slug`
- **Secure Delivery** — HMAC-SHA256 signed, user-bound, time-limited segment tokens. Full enforcement chain: platform active → film published → platform carries → license active → DMCA clear → entitlement valid. Every request audit-logged
- **Video Player** — HLS.js + native HLS, platform-themed, resume playback, 15-second heartbeat tracking
- **Ratings** — Per-platform with cross-platform aggregation, curator moderation
- **Payments** — Pluggable `PaymentProvider` trait with Stripe implementation and `NoopProvider` for self-hosted. Entitlements for SVOD/TVOD, webhook handling
- **Revenue** — Transaction recording with 3-way splits (filmmaker + curator + platform fee), dashboards for both filmmakers and curators
- **DMCA** — Public takedown form, admin review workflow, counter-notifications, automated manifest blocking
- **Events** — Screenings, premieres, Q&A with ticketing and attendee caps
- **Billing** — Storage metering, 4 pricing tiers, curator credit system
- **Admin** — Dashboard with live system counts, person management, role updates, GDPR erasure
- **Landing Page** — Full marketing copy with conversion-focused sections
- **Showcase** — Reference streaming site at `/showcase`
- **Security** — CSP, X-Frame-Options, nosniff headers, gzip compression, Dockerfile (distroless runtime)

## Workspace

Pavilion is a Cargo workspace with two crates:

```
pavilion/              # The application
crates/pavilion-media/ # Reusable video infrastructure library
```

**pavilion-media** handles storage, transcoding, manifest generation, and signed tokens. It's generic (uses `subject/resource/scope` instead of Pavilion-specific names) so it can be used for any video streaming project. [Read the docs](crates/pavilion-media/src/lib.rs).

## Prerequisites

- [Rust](https://rustup.rs/) (stable, 1.85+)
- [Docker](https://docs.docker.com/get-docker/) and Docker Compose
- [SurrealDB CLI](https://surrealdb.com/install) (for schema management)
- [FFmpeg](https://ffmpeg.org/) (for transcoding)

## Quick Start

### 1. Clone and configure

```sh
git clone https://github.com/secedastudios/pavilion.git
cd pavilion
cp .env-example .env
```

Edit `.env` if you need to change any defaults (database credentials, ports, etc.).

### 2. Start services

```sh
make services
```

This starts SurrealDB, RustFS, and Qdrant via Docker Compose (on non-default ports to avoid conflicts).

### 3. Initialize the database

```sh
make db-init
```

### 4. Run the application

```sh
make dev
```

Pavilion will be running at [http://localhost:3000](http://localhost:3000).

### 5. Verify

```sh
make healthcheck
```

## Available Make Targets

| Target | Description |
|---|---|
| `make dev` | Run the application |
| `make services` | Start SurrealDB, RustFS, and Qdrant |
| `make services-down` | Stop all services |
| `make build` | Build a release binary |
| `make db-init` | Apply schema to SurrealDB |
| `make db-drop` | Drop the database |
| `make db-seed` | Load seed data |
| `make test` | Run all tests (workspace-wide) |
| `make healthcheck` | Check the running application |

## Running Tests

```sh
make test
```

Tests use an in-memory SurrealDB instance — no running services required. Currently 121 tests across 14 test files covering auth, films, licensing, catalog, platforms, player security, payments, ratings, revenue, DMCA, events, billing, and admin.

## Project Structure

```
pavilion/
├── Cargo.toml                 # Workspace root
├── crates/
│   └── pavilion-media/        # Reusable video infrastructure crate
│       └── src/
│           ├── lib.rs          # Storage, transcode, tokens, manifests
│           ├── storage.rs      # S3-compatible upload/download/stream
│           ├── transcode.rs    # FFmpeg ABR ladder (360p–4K)
│           ├── manifest.rs     # HLS/DASH generation and rewriting
│           ├── token.rs        # HMAC-SHA256 signed segment tokens
│           └── config.rs       # Storage, transcode, token config
├── src/
│   ├── main.rs                # Entry point
│   ├── lib.rs                 # Module docs and re-exports
│   ├── config.rs              # Environment configuration (dotenv)
│   ├── error.rs               # AppError with HTTP status mapping
│   ├── router.rs              # All routes + AppState
│   ├── util.rs                # RecordIdExt trait, relation checks, slugify
│   ├── sse.rs                 # Datastar SSE fragment helpers
│   ├── middleware.rs          # Security headers (CSP, X-Frame-Options)
│   ├── auth/                  # JWT, Argon2, request extractors
│   ├── controllers/           # 22 route handler modules
│   ├── models/                # 9 SurrealDB record/view model modules
│   ├── delivery/              # Signed tokens, manifest rewrite, audit log
│   ├── licensing/             # Rights resolution engine
│   ├── payments/              # Stripe Connect, entitlements, provider trait
│   ├── revenue/               # Transactions, splits, dashboards
│   ├── billing/               # Storage metering, tiers, credits
│   ├── media/                 # Image processing, TMDB enrichment
│   └── transcode/             # Job queue, worker, reaper
├── templates/                 # Askama HTML templates
│   ├── base.html              # Base layout with OG tags, Datastar, cookie consent
│   ├── pages/                 # 30+ full page templates
│   ├── partials/              # SSE fragment templates
│   └── components/            # Reusable includes (nav)
├── static/css/main.css        # Single stylesheet, CSS custom properties, dark mode
├── db/
│   ├── schema.surql           # Complete SurrealDB schema (IF NOT EXISTS, idempotent)
│   ├── migrations/            # Versioned migrations
│   └── seed.surql             # Seed data
├── tests/                     # 14 integration test files + shared helpers
├── docker-compose.yml         # Dev services (non-default ports)
├── Dockerfile                 # Multi-stage build (distroless runtime)
├── Makefile
├── PROJECT.md                 # Detailed build plan with phase status
└── PRODUCT.md                 # Product vision and features
```

## Configuration

All configuration is via environment variables (loaded from `.env` via dotenv):

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | `ws://localhost:8001` | SurrealDB connection URL |
| `DATABASE_NS` | `pavilion` | SurrealDB namespace |
| `DATABASE_DB` | `pavilion` | SurrealDB database |
| `DATABASE_USER` | `root` | SurrealDB username |
| `DATABASE_PASS` | `root` | SurrealDB password |
| `JWT_SECRET` | `change-me-in-production` | Secret for signing JWT tokens |
| `RUSTFS_ENDPOINT` | `http://localhost:9002` | RustFS S3 API endpoint |
| `RUSTFS_ACCESS_KEY` | `rustfsadmin` | RustFS access key |
| `RUSTFS_SECRET_KEY` | `rustfsadmin` | RustFS secret key |
| `RUSTFS_BUCKET` | `pavilion` | Storage bucket name |
| `QDRANT_ENDPOINT` | `http://localhost:6336` | Qdrant gRPC endpoint |
| `HOST` | `0.0.0.0` | Bind address |
| `PORT` | `3000` | Bind port |
| `BASE_URL` | `http://localhost:3000` | Public URL (for OAuth redirects, emails) |
| `PRETTY_LOGS` | `true` | Pretty log output (false for JSON) |
| `RUST_LOG` | `pavilion=debug,tower_http=debug` | Log level filter |
| `STRIPE_SECRET_KEY` | *(empty)* | Stripe secret key (leave empty to disable payments) |
| `STRIPE_PUBLISHABLE_KEY` | *(empty)* | Stripe publishable key |
| `STRIPE_WEBHOOK_SECRET` | *(empty)* | Stripe webhook signing secret |
| `FACILITATION_FEE_PCT` | `5.0` | Platform fee percentage (0 for self-hosted) |
| `TMDB_API_KEY` | *(empty)* | TMDB API key for metadata enrichment |

## Self-Hosting

Pavilion is designed to run on your own infrastructure with no vendor lock-in:

- **Payments**: Leave `STRIPE_SECRET_KEY` empty to disable. Implement the `PaymentProvider` trait for your own payment processor.
- **Platform fee**: Set `FACILITATION_FEE_PCT=0` to remove the platform fee entirely.
- **Storage**: RustFS, MinIO, or any S3-compatible service.
- **Database**: SurrealDB clusters via TiKV for horizontal scaling.
- **Deployment**: Multi-stage Dockerfile with distroless runtime. Stateless servers behind any load balancer.

## License

Open source under the SlateHub umbrella. License TBD.
