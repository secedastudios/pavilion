use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::manifest::RenditionInfo;
use crate::storage::StorageClient;

/// Resolution profiles for the H.264 adaptive bitrate ladder.
pub struct ResolutionProfile {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub bitrate: &'static str,
    pub maxrate: &'static str,
    pub bufsize: &'static str,
}

pub const H264_LADDER: &[ResolutionProfile] = &[
    ResolutionProfile { name: "360p",  width: 640,  height: 360,  bitrate: "800k",   maxrate: "856k",   bufsize: "1200k"  },
    ResolutionProfile { name: "480p",  width: 854,  height: 480,  bitrate: "1400k",  maxrate: "1498k",  bufsize: "2100k"  },
    ResolutionProfile { name: "720p",  width: 1280, height: 720,  bitrate: "2800k",  maxrate: "2996k",  bufsize: "4200k"  },
    ResolutionProfile { name: "1080p", width: 1920, height: 1080, bitrate: "5000k",  maxrate: "5350k",  bufsize: "7500k"  },
    ResolutionProfile { name: "1440p", width: 2560, height: 1440, bitrate: "10000k", maxrate: "10700k", bufsize: "15000k" },
    ResolutionProfile { name: "2160p", width: 3840, height: 2160, bitrate: "16000k", maxrate: "17120k", bufsize: "24000k" },
];

/// Result of a single rendition transcode.
pub struct TranscodeResult {
    pub resolution: String,
    pub output_dir: PathBuf,
    pub segment_pattern: String,
    pub playlist_file: String,
}

impl TranscodeResult {
    /// Convert to a RenditionInfo for manifest generation.
    pub fn to_rendition_info(&self) -> Option<RenditionInfo> {
        let profile = H264_LADDER.iter().find(|p| p.name == self.resolution)?;
        Some(RenditionInfo {
            name: self.resolution.clone(),
            width: profile.width,
            height: profile.height,
            bandwidth: bandwidth_from_bitrate(profile.bitrate),
            playlist_file: self.playlist_file.clone(),
        })
    }
}

/// Transcode a source video into CMAF HLS segments for one resolution.
pub async fn transcode_rendition<F>(
    input: &Path,
    output_dir: &Path,
    profile: &ResolutionProfile,
    on_progress: F,
) -> anyhow::Result<TranscodeResult>
where
    F: Fn(u8) + Send + 'static,
{
    let rendition_dir = output_dir.join(profile.name);
    tokio::fs::create_dir_all(&rendition_dir).await?;

    let playlist = format!("{}.m3u8", profile.name);
    let segment_pattern = format!("{}_%04d.m4s", profile.name);

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-i", input.to_str().unwrap_or_default(),
        "-c:v", "libx264",
        "-preset", "medium",
        "-profile:v", "main",
        "-b:v", profile.bitrate,
        "-maxrate", profile.maxrate,
        "-bufsize", profile.bufsize,
        "-vf", &format!("scale={}:{}", profile.width, profile.height),
        "-c:a", "aac",
        "-b:a", "128k",
        "-ac", "2",
        "-f", "hls",
        "-hls_time", "6",
        "-hls_playlist_type", "vod",
        "-hls_segment_type", "fmp4",
        "-hls_fmp4_init_filename", "init.mp4",
        "-hls_segment_filename", rendition_dir.join(&segment_pattern).to_str().unwrap_or_default(),
        "-progress", "pipe:1",
        "-y", rendition_dir.join(&playlist).to_str().unwrap_or_default(),
    ]);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(time_str) = line.strip_prefix("out_time_us=") {
                    if let Ok(us) = time_str.parse::<u64>() {
                        let seconds = us / 1_000_000;
                        on_progress(seconds.min(99) as u8);
                    }
                }
            }
        });
    }

    let output = child.wait().await?;
    if !output.success() {
        anyhow::bail!("FFmpeg failed for {} with exit code: {:?}", profile.name, output.code());
    }

    Ok(TranscodeResult {
        resolution: profile.name.to_string(),
        output_dir: rendition_dir,
        segment_pattern,
        playlist_file: playlist,
    })
}

/// Transcode all renditions in the H.264 ladder.
pub async fn transcode_all_renditions(
    input: &Path,
    output_dir: &Path,
) -> anyhow::Result<Vec<TranscodeResult>> {
    let mut results = Vec::new();
    for (i, profile) in H264_LADDER.iter().enumerate() {
        tracing::info!(resolution = profile.name, "Transcoding rendition {}/{}", i + 1, H264_LADDER.len());
        let result = transcode_rendition(input, output_dir, profile, |_| {}).await?;
        results.push(result);
    }
    Ok(results)
}

/// Full pipeline: download master from storage, transcode, upload results, return manifest.
///
/// - `storage_key`: key of the master file in S3 (e.g., "masters/abc123.mp4")
/// - `output_prefix`: key prefix for transcoded output (e.g., "videos/abc123")
/// - `work_dir`: local temp directory for processing
pub async fn transcode_and_upload(
    storage: &StorageClient,
    storage_key: &str,
    output_prefix: &str,
    work_dir: &Path,
) -> Result<Vec<TranscodeResult>, crate::error::MediaError> {
    tokio::fs::create_dir_all(work_dir).await
        .map_err(|e| crate::error::MediaError::Transcode(format!("Work dir error: {e}")))?;

    // Download master from storage
    let master_path = work_dir.join("master.mp4");
    storage.get_file(storage_key, &master_path).await?;
    tracing::info!(key = %storage_key, "Downloaded master file");

    // Transcode
    let output_dir = work_dir.join("output");
    let results = transcode_all_renditions(&master_path, &output_dir)
        .await
        .map_err(|e| crate::error::MediaError::Transcode(e.to_string()))?;

    // Generate and write master manifests
    let rendition_infos: Vec<RenditionInfo> = results
        .iter()
        .filter_map(|r| r.to_rendition_info())
        .collect();

    let hls_master = crate::manifest::generate_hls_master(&rendition_infos, output_prefix);
    let dash_mpd = crate::manifest::generate_dash_mpd(&rendition_infos, output_prefix, 0);

    tokio::fs::write(output_dir.join("master.m3u8"), &hls_master).await
        .map_err(|e| crate::error::MediaError::Transcode(format!("Write manifest: {e}")))?;
    tokio::fs::write(output_dir.join("manifest.mpd"), &dash_mpd).await
        .map_err(|e| crate::error::MediaError::Transcode(format!("Write manifest: {e}")))?;

    // Upload everything to storage
    let count = storage.upload_directory(&output_dir, output_prefix).await?;
    tracing::info!(prefix = %output_prefix, files = count, "Uploaded transcoded files");

    // Clean up
    let _ = tokio::fs::remove_dir_all(work_dir).await;

    Ok(results)
}

fn bandwidth_from_bitrate(bitrate: &str) -> u64 {
    let trimmed = bitrate.trim_end_matches('k').trim_end_matches('K');
    trimmed.parse::<u64>().unwrap_or(1000) * 1000
}
