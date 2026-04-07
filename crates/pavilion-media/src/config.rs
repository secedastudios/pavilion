/// Configuration for S3-compatible object storage.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: String,
    pub path_style: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:9002".into(),
            access_key: "rustfsadmin".into(),
            secret_key: "rustfsadmin".into(),
            bucket: "pavilion".into(),
            region: "us-east-1".into(),
            path_style: true,
        }
    }
}

/// Configuration for the transcoding pipeline.
#[derive(Debug, Clone)]
pub struct TranscodeConfig {
    /// Path to the ffmpeg binary.
    pub ffmpeg_path: String,
    /// Local directory for transcode working files.
    pub work_dir: String,
    /// Maximum concurrent transcode jobs.
    pub max_concurrent: usize,
}

impl Default for TranscodeConfig {
    fn default() -> Self {
        Self {
            ffmpeg_path: "ffmpeg".into(),
            work_dir: "/tmp/pavilion-media".into(),
            max_concurrent: 2,
        }
    }
}

/// Configuration for signed token generation.
#[derive(Debug, Clone)]
pub struct TokenConfig {
    /// HMAC secret for signing tokens.
    pub secret: String,
    /// Token time-to-live in seconds.
    pub ttl_secs: u64,
    /// Base path prefix for segment URLs (e.g., "/segments/").
    pub segment_url_prefix: String,
}

impl Default for TokenConfig {
    fn default() -> Self {
        Self {
            secret: "change-me-in-production".into(),
            ttl_secs: 300,
            segment_url_prefix: "/segments/".into(),
        }
    }
}
