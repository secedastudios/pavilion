//! # pavilion-media
//!
//! Everything you need to store, transcode, and securely stream video from a
//! Rust application. Think of it as the video infrastructure layer that sits
//! between your app and your users' eyeballs.
//!
//! pavilion-media handles the hard parts:
//!
//! - **Upload** video files to any S3-compatible storage (RustFS, MinIO, AWS S3)
//! - **Transcode** them into adaptive bitrate HLS/DASH using FFmpeg, up to 4K
//! - **Generate** HLS (.m3u8) and DASH (.mpd) manifests
//! - **Protect** every segment with signed, time-limited, user-bound tokens
//!
//! Your app stays in control of who can watch what. pavilion-media just makes
//! sure the bytes flow securely.
//!
//! # Quick Start
//!
//! Here's the simplest thing that works: upload a video, transcode it, and
//! generate a signed manifest.
//!
//! ```rust,no_run
//! use pavilion_media::config::StorageConfig;
//! use pavilion_media::storage::StorageClient;
//! use pavilion_media::transcode;
//! use pavilion_media::manifest;
//! use pavilion_media::token;
//! use std::path::Path;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Connect to your S3-compatible storage
//! let storage = StorageClient::new(&StorageConfig {
//!     endpoint: "http://localhost:9002".into(),
//!     access_key: "rustfsadmin".into(),
//!     secret_key: "rustfsadmin".into(),
//!     bucket: "videos".into(),
//!     region: "us-east-1".into(),
//!     path_style: true,
//! })?;
//!
//! // 2. Upload a master video file
//! storage.put_file("masters/my-film.mp4", Path::new("/path/to/film.mp4")).await?;
//!
//! // 3. Transcode it (downloads from storage, transcodes, uploads results back)
//! let results = transcode::transcode_and_upload(
//!     &storage,
//!     "masters/my-film.mp4",    // where the master lives in storage
//!     "videos/my-film",          // prefix for transcoded output
//!     Path::new("/tmp/work"),    // local temp directory
//! ).await?;
//!
//! // 4. Generate a master HLS playlist
//! let renditions: Vec<_> = results.iter().filter_map(|r| r.to_rendition_info()).collect();
//! let m3u8 = manifest::generate_hls_master(&renditions, "/videos/my-film");
//! // => #EXTM3U
//! // => #EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360,...
//! // => /videos/my-film/360p/360p.m3u8
//! // => ...up to 2160p (4K)
//!
//! // 5. When a viewer requests the manifest, rewrite it with signed URLs
//! let signed = manifest::rewrite_hls_manifest(
//!     &m3u8,
//!     "user-123",           // subject (who's watching)
//!     "film-abc",           // resource (what they're watching)
//!     "platform-xyz",       // scope (where they're watching)
//!     "your-hmac-secret",   // signing secret
//!     300,                  // token TTL in seconds (5 minutes)
//!     "/segments/",         // URL prefix for your segment proxy
//! );
//! // Every segment URL is now a signed token like:
//! // /segments/dG9rZW4tcGF5bG9hZC5zaWduYXR1cmU...
//! # Ok(())
//! # }
//! ```
//!
//! # How the Security Model Works
//!
//! The key insight: **your storage is never exposed to the internet**. Every
//! video byte passes through your application, which decides who gets access.
//!
//! Here's the flow when a viewer hits play:
//!
//! ```text
//! Viewer → Your App (auth check) → Manifest Proxy → Rewrite with signed URLs
//!                                                          ↓
//! Viewer ← Signed .m3u8 ←──────────────────────────────────┘
//!   │
//!   │  (player requests each segment)
//!   ↓
//! Viewer → /segments/<signed_token> → Your App validates token
//!                                          ↓
//!                                     Token OK? → Fetch from Storage → Stream to viewer
//!                                     Token bad? → 401 Unauthorized
//! ```
//!
//! Each signed token contains:
//! - **subject**: who's watching (user ID, "public", whatever you want)
//! - **resource**: what they're watching (video ID)
//! - **scope**: context (platform ID, org ID, "default")
//! - **segment_path**: which segment file to serve
//! - **expires_at**: when the token dies (default 5 minutes)
//!
//! Tokens are HMAC-SHA256 signed, base64 encoded, and verified in constant time.
//! Even if someone copies a segment URL, it's bound to their identity and expires
//! in minutes.
//!
//! # Use Cases
//!
//! pavilion-media is designed to be generic. The `subject/resource/scope` naming
//! in tokens is intentionally abstract so you can use it for anything:
//!
//! | Use case | subject | resource | scope |
//! |----------|---------|----------|-------|
//! | Film distribution (Pavilion) | person ID | film ID | platform ID |
//! | Public acting reels (SlateHub) | "public" | reel ID | "default" |
//! | Org-internal videos | user ID | video ID | org ID |
//! | Course platform | student ID | lesson ID | course ID |
//! | Free public streaming | "anonymous" | video ID | "public" |
//!
//! For public videos, just use a fixed subject like `"public"` and skip the
//! identity check on your segment proxy.
//!
//! # Modules
//!
//! | Module | What it does |
//! |--------|-------------|
//! | [`storage`] | Upload, download, and manage files on S3-compatible storage |
//! | [`transcode`] | FFmpeg-based adaptive bitrate transcoding (360p to 4K) |
//! | [`manifest`] | Generate and rewrite HLS/DASH manifests |
//! | [`token`] | Sign and verify time-limited segment access tokens |
//! | [`config`] | Configuration structs for storage, transcoding, and tokens |
//! | [`error`] | Error types for the whole crate |
//!
//! # Transcoding Details
//!
//! The default H.264 ladder produces six renditions:
//!
//! | Resolution | Bitrate | Use case |
//! |-----------|---------|----------|
//! | 360p | 800 kbps | Mobile on slow connections |
//! | 480p | 1.4 Mbps | Mobile on decent connections |
//! | 720p | 2.8 Mbps | Tablets, small screens |
//! | 1080p | 5 Mbps | Laptops, desktop |
//! | 1440p | 10 Mbps | Large monitors |
//! | 2160p | 16 Mbps | 4K displays |
//!
//! Output format is CMAF (Common Media Application Format) with fMP4 segments.
//! A single set of segments works with both HLS and DASH players, so you don't
//! store anything twice.
//!
//! Transcoding requires FFmpeg installed on the system. The crate calls it as
//! a subprocess, so you get full FFmpeg codec support without linking to
//! C libraries.
//!
//! # Storage Compatibility
//!
//! Tested with:
//! - [RustFS](https://rustfs.com) (recommended, Apache 2.0 licensed)
//! - MinIO
//! - AWS S3
//! - Any S3-compatible service
//!
//! Uses the `rust-s3` crate under the hood. Set `path_style: true` in
//! [`StorageConfig`](config::StorageConfig) for RustFS/MinIO (most self-hosted
//! setups need this).

pub mod config;
pub mod error;
pub mod manifest;
pub mod storage;
pub mod token;
pub mod transcode;
