use std::sync::Arc;

use axum::extract::{Multipart, Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use surrealdb::types::RecordId;

use crate::auth::claims::Claims;
use crate::billing::metering;
use crate::error::AppError;
use crate::media::images;
use crate::models::transcode::TranscodeProfile;
use crate::router::AppState;
use crate::transcode::queue;

/// Handle multipart film file upload.
///
/// Flow: receive file → upload to RustFS as masters/{film_key}.mp4 →
/// record storage usage → enqueue transcode job → redirect to film page.
pub async fn upload_film(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    // Read the file from multipart
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name = String::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("Upload error: {e}")))?
    {
        if field.name() == Some("file") {
            file_name = field.file_name().unwrap_or("upload.mp4").to_string();

            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::Validation(format!("Read error: {e}")))?;

            if bytes.is_empty() {
                return Err(AppError::Validation("File is empty.".into()));
            }

            file_data = Some(bytes.to_vec());
        }
    }

    let data = file_data.ok_or_else(|| AppError::Validation("No file provided.".into()))?;
    let size_bytes = data.len() as i64;

    // Determine content type from extension
    let content_type = if file_name.ends_with(".mp4") {
        "video/mp4"
    } else if file_name.ends_with(".mov") {
        "video/quicktime"
    } else if file_name.ends_with(".mkv") {
        "video/x-matroska"
    } else if file_name.ends_with(".avi") {
        "video/x-msvideo"
    } else {
        "application/octet-stream"
    };

    // Upload to RustFS
    let film_key = crate::util::record_id_key_string(&film.id.key);
    let storage_key = format!("masters/{film_key}.mp4");

    state
        .storage
        .put_bytes(&storage_key, &data, Some(content_type))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Storage upload failed: {e}")))?;

    tracing::info!(
        film = %film_key,
        size = size_bytes,
        filename = %file_name,
        "Master file uploaded to storage"
    );

    // Record storage usage
    let person_id = claims.person_id();
    // Ensure storage_usage record exists
    let _ = metering::get_usage(&state.db, &person_id).await;
    let _ = metering::record_upload(&state.db, &person_id, size_bytes, true).await;

    // Create master asset record
    state
        .db
        .query(
            "LET $asset = (CREATE asset SET \
                asset_type = 'master', \
                codec = 'unknown', \
                storage_key = $storage_key, \
                size_bytes = $size_bytes \
             RETURN AFTER); \
             RELATE $film->has_asset->$asset[0].id;",
        )
        .bind(("storage_key", storage_key))
        .bind(("size_bytes", size_bytes))
        .bind(("film", film.id.clone()))
        .await?;

    // Enqueue transcode job
    let film_record = RecordId::new("film", film_id.as_str());
    queue::enqueue(&state.db, film_record, TranscodeProfile::h264_default()).await?;

    tracing::info!(film = %film_key, "Transcode job enqueued");

    Ok(Redirect::to(&format!("/films/{film_id}")).into_response())
}

/// Handle poster image upload. Processes into multiple sizes and stores all variants.
pub async fn upload_poster(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let mut file_data: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("Upload error: {e}")))?
    {
        if field.name() == Some("poster") {
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::Validation(format!("Read error: {e}")))?;
            if !bytes.is_empty() {
                file_data = Some(bytes.to_vec());
            }
        }
    }

    let data = file_data.ok_or_else(|| AppError::Validation("No poster file provided.".into()))?;
    let film_key = crate::util::record_id_key_string(&film.id.key);

    // Process and upload all poster sizes
    let keys = images::upload_poster_variants(&state.storage, &film_key, &data)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Poster processing failed: {e}")))?;

    // Update film with poster URLs
    let fid = surrealdb::types::RecordId::new("film", film_id.as_str());
    state
        .db
        .query(
            "UPDATE $fid SET \
            poster_url = $medium, \
            poster_thumb = $thumb, \
            poster_small = $small, \
            poster_large = $large",
        )
        .bind(("fid", fid))
        .bind(("medium", keys.medium))
        .bind(("thumb", keys.thumb))
        .bind(("small", keys.small))
        .bind(("large", keys.large))
        .await?;

    // Track storage
    let person_id = claims.person_id();
    let _ = metering::get_usage(&state.db, &person_id).await;
    let _ = metering::record_upload(&state.db, &person_id, data.len() as i64, false).await;

    tracing::info!(film = %film_key, "Poster uploaded and processed into 4 sizes");
    Ok(Redirect::to(&format!("/films/{film_id}")).into_response())
}
