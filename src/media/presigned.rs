//! Presigned upload session configuration for the Uppy upload widget.
//! Generates temporary S3 credentials and upload parameters so the client
//! can upload large files directly to storage without proxying through the server.

use serde::{Deserialize, Serialize};

/// Presigned upload session info returned to the client.
/// The client uses this to upload chunks directly to S3-compatible storage,
/// then calls the completion endpoint.
#[derive(Debug, Serialize, Deserialize)]
pub struct UploadSession {
    pub upload_id: String,
    pub film_key: String,
    pub storage_key: String,
    pub parts: Vec<PresignedPart>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PresignedPart {
    pub part_number: i32,
    pub presigned_url: String,
}

/// Client sends this after all parts are uploaded.
#[derive(Debug, Serialize, Deserialize)]
pub struct CompleteUploadRequest {
    pub upload_id: String,
    pub storage_key: String,
    pub parts: Vec<CompletedPart>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompletedPart {
    pub part_number: i32,
    pub etag: String,
}

/// Configuration for the Uppy upload widget.
#[derive(Debug, Serialize, Deserialize)]
pub struct UppyConfig {
    pub endpoint: String,
    pub film_id: String,
    pub max_file_size_mb: u64,
    pub allowed_types: Vec<String>,
}

impl UppyConfig {
    pub fn for_film(film_id: &str, base_url: &str) -> Self {
        Self {
            endpoint: format!("{base_url}/films/{film_id}/upload"),
            film_id: film_id.to_string(),
            max_file_size_mb: 50_000, // 50 GB
            allowed_types: vec![
                "video/mp4".into(),
                "video/quicktime".into(),
                "video/x-matroska".into(),
                "video/x-msvideo".into(),
                "video/webm".into(),
            ],
        }
    }

    pub fn for_poster(film_id: &str, base_url: &str) -> Self {
        Self {
            endpoint: format!("{base_url}/films/{film_id}/poster"),
            film_id: film_id.to_string(),
            max_file_size_mb: 50,
            allowed_types: vec!["image/jpeg".into(), "image/png".into(), "image/webp".into()],
        }
    }
}
