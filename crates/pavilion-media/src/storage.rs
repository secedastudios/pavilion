use std::path::Path;

use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::Region;

use crate::config::StorageConfig;
use crate::error::MediaError;

/// S3-compatible storage client for video files.
///
/// Works with RustFS, MinIO, AWS S3, or any S3-compatible service.
#[derive(Clone)]
pub struct StorageClient {
    bucket: Box<Bucket>,
}

impl StorageClient {
    /// Create a new storage client from config.
    pub fn new(config: &StorageConfig) -> Result<Self, MediaError> {
        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )
        .map_err(|e| MediaError::Storage(format!("Invalid credentials: {e}")))?;

        let region = Region::Custom {
            region: config.region.clone(),
            endpoint: config.endpoint.clone(),
        };

        let mut bucket = Bucket::new(&config.bucket, region, credentials)
            .map_err(|e| MediaError::Storage(format!("Bucket error: {e}")))?;

        if config.path_style {
            bucket = bucket.with_path_style();
        }

        Ok(Self { bucket })
    }

    /// Upload a file to storage.
    pub async fn put_file(
        &self,
        key: &str,
        file_path: &Path,
    ) -> Result<(), MediaError> {
        let content = tokio::fs::read(file_path)
            .await
            .map_err(|e| MediaError::Storage(format!("Read file error: {e}")))?;

        self.bucket
            .put_object(key, &content)
            .await
            .map_err(|e| MediaError::Storage(format!("Upload error: {e}")))?;

        tracing::info!(key = %key, size = content.len(), "Uploaded to storage");
        Ok(())
    }

    /// Upload bytes to storage.
    pub async fn put_bytes(
        &self,
        key: &str,
        content: &[u8],
        content_type: Option<&str>,
    ) -> Result<(), MediaError> {
        if let Some(ct) = content_type {
            self.bucket
                .put_object_with_content_type(key, content, ct)
                .await
                .map_err(|e| MediaError::Storage(format!("Upload error: {e}")))?;
        } else {
            self.bucket
                .put_object(key, content)
                .await
                .map_err(|e| MediaError::Storage(format!("Upload error: {e}")))?;
        }

        Ok(())
    }

    /// Download a file from storage to a local path.
    pub async fn get_file(
        &self,
        key: &str,
        dest_path: &Path,
    ) -> Result<(), MediaError> {
        let response = self
            .bucket
            .get_object(key)
            .await
            .map_err(|e| MediaError::Storage(format!("Download error: {e}")))?;

        if response.status_code() == 404 {
            return Err(MediaError::NotFound(format!("Object not found: {key}")));
        }

        tokio::fs::write(dest_path, response.bytes())
            .await
            .map_err(|e| MediaError::Storage(format!("Write file error: {e}")))?;

        tracing::info!(key = %key, dest = ?dest_path, "Downloaded from storage");
        Ok(())
    }

    /// Download bytes from storage.
    pub async fn get_bytes(&self, key: &str) -> Result<Vec<u8>, MediaError> {
        let response = self
            .bucket
            .get_object(key)
            .await
            .map_err(|e| MediaError::Storage(format!("Download error: {e}")))?;

        if response.status_code() == 404 {
            return Err(MediaError::NotFound(format!("Object not found: {key}")));
        }

        Ok(response.bytes().to_vec())
    }

    /// Stream bytes from storage (returns the raw bytes — caller handles streaming).
    pub async fn get_stream(&self, key: &str) -> Result<(Vec<u8>, String), MediaError> {
        let response = self
            .bucket
            .get_object(key)
            .await
            .map_err(|e| MediaError::Storage(format!("Download error: {e}")))?;

        if response.status_code() == 404 {
            return Err(MediaError::NotFound(format!("Object not found: {key}")));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .cloned()
            .unwrap_or_else(|| "application/octet-stream".to_string());

        Ok((response.bytes().to_vec(), content_type))
    }

    /// Delete an object from storage.
    pub async fn delete(&self, key: &str) -> Result<(), MediaError> {
        self.bucket
            .delete_object(key)
            .await
            .map_err(|e| MediaError::Storage(format!("Delete error: {e}")))?;

        tracing::info!(key = %key, "Deleted from storage");
        Ok(())
    }

    /// Check if an object exists.
    pub async fn exists(&self, key: &str) -> Result<bool, MediaError> {
        match self.bucket.head_object(key).await {
            Ok((_, code)) => Ok(code == 200),
            Err(_) => Ok(false),
        }
    }

    /// Upload all files in a directory tree to storage under a key prefix.
    pub async fn upload_directory(
        &self,
        local_dir: &Path,
        key_prefix: &str,
    ) -> Result<usize, MediaError> {
        let mut count = 0;
        let mut entries = tokio::fs::read_dir(local_dir)
            .await
            .map_err(|e| MediaError::Storage(format!("Read dir error: {e}")))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| MediaError::Storage(format!("Dir entry error: {e}")))?
        {
            let path = entry.path();
            if path.is_dir() {
                let sub_prefix = format!(
                    "{}/{}",
                    key_prefix,
                    entry.file_name().to_string_lossy()
                );
                count += Box::pin(self.upload_directory(&path, &sub_prefix)).await?;
            } else {
                let key = format!(
                    "{}/{}",
                    key_prefix,
                    entry.file_name().to_string_lossy()
                );
                self.put_file(&key, &path).await?;
                count += 1;
            }
        }

        Ok(count)
    }
}

impl std::fmt::Debug for StorageClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StorageClient")
            .field("bucket", &self.bucket.name())
            .finish()
    }
}
