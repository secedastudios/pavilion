//! FFmpeg transcoding job queue, worker, and reaper.
//! Re-exports core transcode and manifest functionality from `pavilion-media`
//! and provides the job scheduling layer for background video processing.

pub mod queue;
pub mod reaper;
pub mod worker;

// Re-export from pavilion-media
pub use pavilion_media::transcode as ffmpeg;
pub use pavilion_media::manifest;
