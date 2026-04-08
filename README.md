<p align="center">
  <img src="static/images/banner.svg" alt="Pavilion" width="800">
</p>

<p align="center">
  <strong>Open-source film distribution and white-label streaming platform</strong><br>
  Upload once. Set your terms. Reach every screen.
</p>

<p align="center">
  <a href="#quick-start">Quick start</a> •
  <a href="#features">Features</a> •
  <a href="#what-it-costs">Costs</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#modules">Modules</a> •
  <a href="#deploy-to-production">Deploy</a> •
  <a href="LICENSE">License</a>
</p>

---

We built Pavilion because the options for indie filmmakers are bad. Aggregators take 30% and give you a quarterly PDF. White-label platforms charge per subscriber and cap your bandwidth. Building on AWS means your CloudFront bill eats your ticket sales.

Pavilion is self-hosted. You run it on a dedicated server with flat-rate bandwidth, keep 100% of your revenue, and see every transaction as it happens. Curators get white-label streaming sites they can launch in hours. The whole thing is Apache 2.0.

---

## What does it do?

**Filmmakers** upload a film, set licensing terms (rental, subscription, ad-supported, event, educational, Creative Commons — mix and match by territory and time window), and Pavilion handles transcoding up to 4K, storage, delivery, and revenue splits.

**Curators** browse a catalog that only shows films they can actually license, pick what fits their brand, and get a streaming platform with their own domain, colors, player, and monetization. Stripe Connect handles the money.

**Self-hosters** run the whole stack on their own hardware. Set the platform fee to zero. Swap in a different payment processor. Fork it.

---

## What it costs

50 films, 5,000 monthly viewers, ~25 TB bandwidth:

| | OTT service | AWS | Pavilion (self-hosted) |
|---|---|---|---|
| Monthly | $500 – $1,500+ | ~$2,200 | **~$75 – $150** |
| Per subscriber | $0.50 – $1.00 | — | **None** |
| Bandwidth | Metered / capped | $0.085/GB | **Included** |
| Transcoding | Limited | $0.024/min | **Your CPU** |
| Revenue cut | 5 – 20% | — | **0%** |
| Own the code | No | Some | **Yes** |

A screening for 1,000 viewers costs about $425 on CloudFront just in bandwidth. On a Hetzner box running Pavilion, that's $0 — bandwidth is part of the monthly rate.

The math is simple: AWS charges per gigabyte, one 1080p viewer burns 5 GB, and it adds up fast. Dedicated servers from [Hetzner](https://www.hetzner.com) (from ~€44/month) or [OVH](https://www.ovhcloud.com) (~$70/month) include 20 TB+ at full speed.

---

## Features

### For filmmakers

- Transcoding from 360p to 4K (H.264, CMAF, HLS + DASH)
- 7 licensing models: TVOD, SVOD, AVOD, hybrid, event, educational, Creative Commons
- Per-territory and time-window licensing
- Revenue dashboard with per-film, per-platform breakdowns and automatic 3-way splits
- TMDB metadata import — cast, crew, posters, synopses in one click
- Poster auto-resize to 4 variants
- DMCA counter-notification tools
- GDPR data export, consent management, account deletion

### For curators

- Branded streaming sites (custom domain, colors, fonts, dark mode)
- Rights-aware catalog — you only see what you can license
- Stripe Connect for subscriptions, pay-per-view, and ad-supported models
- Platform analytics: subscribers, views, revenue, popular films
- Event screenings with ticketing and attendee caps
- Per-platform ratings with moderation

### For admins

- System dashboard, person management, DMCA review
- Per-filmmaker storage metering with pricing tiers
- 121 integration tests across auth, films, licensing, catalog, platforms, player, payments, ratings, revenue, DMCA, events, billing, and admin

### Video security

Every segment request goes through the full chain before anything is served:

1. Platform is active
2. Film is published
3. Platform carries the film via a license
4. License is within its time window
5. No active DMCA claims
6. Viewer has a valid entitlement (for paid content)
7. Segment URL is HMAC-signed, tied to that user, and expires in 5 minutes

Storage is never public. No direct URLs. No way around it.

---

## Designed for Performance & Scalability?

Streaming video is one of the hardest things to do efficiently on a web server. Every concurrent viewer is sustained bandwidth, CPU work for manifest rewriting, and I/O for segment delivery. Language choice matters here more than most places.

| | Rust | Node.js |
|---|---|---|
| Memory (idle) | ~15–30 MB | ~80–200 MB |
| Concurrency | Async I/O, no GC | Event loop, GC pauses |
| Tail latency | Predictable | GC spikes |
| CPU throughput | Near C | 3–10x slower |
| Safety | Compile-time | Runtime exceptions |
| Deploy size | ~15 MB binary | ~200 MB with node_modules |

When you're serving thousands of concurrent viewers and every millisecond of segment delivery matters, the difference between 15 MB and 200 MB of memory per process isn't academic.

Pavilion runs on [Axum](https://github.com/tokio-rs/axum) / [Tokio](https://tokio.rs) for async HTTP, [Askama](https://github.com/djc/askama) for compiled templates, and [Datastar](https://data-star.dev) for server-driven reactivity. No React. No Next.js. No client-side framework.

---

## Architecture

```
                    ┌─────────────────┐
                    │  Load Balancer   │
                    └────┬───────┬────┘
                         │       │
              ┌──────────┴┐     ┌┴──────────┐
              │ Pavilion  │     │ Pavilion   │   Stateless Rust servers
              │ Node 1    │     │ Node N     │   (JWT, no sticky sessions)
              └─────┬─────┘     └─────┬──────┘
                    │                 │
        ┌───────────┼─────────────────┼───────────┐
        │           │                 │           │
  ┌─────┴─────┐ ┌───┴────┐    ┌──────┴──┐  ┌─────┴─────┐
  │ SurrealDB │ │SurrealDB│   │  RustFS  │  │  Qdrant   │
  │ (TiKV)    │ │(TiKV)   │   │  Cluster │  │  Cluster  │
  └───────────┘ └─────────┘   └──────────┘  └───────────┘
```

Pavilion servers are stateless. Session state lives in JWT tokens, data lives in SurrealDB. Put as many instances behind a load balancer as you need.

SurrealDB clusters via TiKV. RustFS (S3-compatible) runs in distributed mode with erasure coding. Qdrant handles vector search for semantic film discovery (planned). Transcode workers poll the job queue independently and scale with demand.

---

## Modules

The project is a Cargo workspace with two crates. We split the video infrastructure from the business logic on purpose — so you can build a streaming backend without the licensing layer if that's what you need.

### `pavilion` — the application

The full distribution platform: auth, licensing, payments, ratings, events, admin, everything above.

### `pavilion-media` — video library (standalone)

Lives at `crates/pavilion-media/`. Handles storage (any S3-compatible service), FFmpeg transcoding (360p–4K, CMAF), HLS/DASH manifest generation, and HMAC-signed segment tokens.

The API uses generic names (`subject/resource/scope`) instead of Pavilion-specific ones, so it works for other things:

| Use case | subject | resource | scope |
|---|---|---|---|
| Film distribution | person ID | film ID | platform ID |
| Acting reels | `"public"` | reel ID | `"default"` |
| Course platform | student ID | lesson ID | course ID |
| Internal video | user ID | video ID | org ID |

Add it as a dependency, wire your own auth, done. [Crate docs are here.](crates/pavilion-media/src/lib.rs)

---

## Quick start

You'll need [Rust](https://rustup.rs/) 1.85+, [Docker](https://docs.docker.com/get-docker/), the [SurrealDB CLI](https://surrealdb.com/install), and [FFmpeg](https://ffmpeg.org/).

```sh
git clone https://github.com/secedastudios/pavilion.git
cd pavilion
cp .env-example .env
make services      # SurrealDB, RustFS, Qdrant
make db-init       # apply schema
make dev           # http://localhost:3000
```

| Command | What it does |
|---|---|
| `make dev` | Run the app |
| `make services` | Start backing services |
| `make services-down` | Stop them |
| `make build` | Release binary |
| `make db-init` | Apply schema |
| `make db-drop` | Drop database |
| `make db-seed` | Load seed data |
| `make test` | Run all 121 tests |
| `make docs` | Build rustdocs to `docs/` |
| `make healthcheck` | Ping the running instance |

---

## Deploy to production

```sh
cp .env-example .env
# fill in production credentials, your domain, Stripe keys if you want payments

docker compose -f docker-compose.prod.yml up -d
```

That starts Pavilion, SurrealDB, RustFS, and Qdrant. Data persists in Docker volumes. The Pavilion image is a distroless container.

To scale, put more instances behind nginx/Caddy/HAProxy. SurrealDB clusters via TiKV, RustFS distributes, transcode workers scale independently.

---

## Configuration

Everything is in environment variables, loaded from `.env`. Full list in [`.env-example`](.env-example).

| Variable | What it does |
|---|---|
| `STRIPE_SECRET_KEY` | Enables payments. Leave empty to run without Stripe. |
| `FACILITATION_FEE_PCT` | Platform fee. Set to `0` for self-hosted instances. |
| `TMDB_API_KEY` | Enables metadata, cast, crew, and poster import from TMDB. |
| `BASE_URL` | Your public domain. Used for OAuth redirects and cookie settings. |

---

## Tests

```sh
make test
```

121 tests, 14 files, in-memory SurrealDB. No running services needed.

---

## Tech stack

| | Technology | |
|---|---|---|
| Language | Rust 2024 | Memory safe, fast, predictable |
| HTTP | Axum | Async, composable, tower ecosystem |
| Database | SurrealDB v3 | Graph-first, SCHEMAFULL, clusterable |
| Templates | Askama | Compiled, type-safe, auto-escaping |
| Reactivity | Datastar | Server-sent events, no JS framework |
| Storage | RustFS | S3-compatible, Apache 2.0 |
| Transcoding | FFmpeg | H.264, HEVC, AV1 |
| Payments | Stripe Connect | Pluggable via trait |
| Video | pavilion-media | Standalone reusable crate |

---

## Contributing

We're actively working on this. If you're interested — filmmaker, curator, or developer — open an issue or send a PR.

The [build plan](PROJECT.md) covers all 16 phases. Run `make docs` for API documentation.

---

## License

[Apache 2.0](LICENSE)

---

## Who's behind this

**[Seceda Studios](https://secedastudios.com)** builds open-source tools for film and creative industries. We think filmmakers should own their infrastructure, control their platforms, and actually be able to verify their revenue. Pavilion is how we're trying to make that real.

If you work in film, TV, or content creation, take a look at **[SlateHub](https://slatehub.com)** too. It's a free networking platform where actors, crew, and filmmakers manage their profiles, credits, and reels in one place. We built that as well.

---

<p align="center">
  <sub>Made with Rust. For filmmakers, by filmmakers.</sub>
</p>
