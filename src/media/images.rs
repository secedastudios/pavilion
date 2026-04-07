//! Poster image resizing to four standard size variants.
//! Takes an original poster image and produces thumb, small, medium, and
//! large variants, then uploads each to S3-compatible storage.

use std::io::Cursor;

use image::imageops::FilterType;
use image::ImageFormat;

use pavilion_media::storage::StorageClient;

/// Poster size variants.
pub struct PosterSizes {
    pub thumb: Vec<u8>,   // 92x138
    pub small: Vec<u8>,   // 185x278
    pub medium: Vec<u8>,  // 370x556
    pub large: Vec<u8>,   // 780x1170
}

/// Process a poster image: validate, resize to multiple sizes, return all variants.
pub fn process_poster(data: &[u8]) -> anyhow::Result<PosterSizes> {
    let img = image::load_from_memory(data)?;

    Ok(PosterSizes {
        thumb: resize_to_jpeg(&img, 92, 138)?,
        small: resize_to_jpeg(&img, 185, 278)?,
        medium: resize_to_jpeg(&img, 370, 556)?,
        large: resize_to_jpeg(&img, 780, 1170)?,
    })
}

/// Generate a video thumbnail placeholder (solid color with film title).
/// In production this would be extracted from a video frame via FFmpeg.
pub fn generate_video_thumbnail(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    // For now, if we receive image data, just resize it
    let img = image::load_from_memory(data)?;
    resize_to_jpeg(&img, 320, 180)
}

fn resize_to_jpeg(img: &image::DynamicImage, width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
    let resized = img.resize_to_fill(width, height, FilterType::Lanczos3);
    let mut buf = Cursor::new(Vec::new());
    resized.write_to(&mut buf, ImageFormat::Jpeg)?;
    Ok(buf.into_inner())
}

/// Upload all poster variants to storage and return the keys.
pub async fn upload_poster_variants(
    storage: &StorageClient,
    film_key: &str,
    data: &[u8],
) -> anyhow::Result<PosterKeys> {
    let sizes = process_poster(data)?;

    let base = format!("posters/{film_key}");
    let thumb_key = format!("{base}/thumb.jpg");
    let small_key = format!("{base}/small.jpg");
    let medium_key = format!("{base}/medium.jpg");
    let large_key = format!("{base}/large.jpg");

    storage.put_bytes(&thumb_key, &sizes.thumb, Some("image/jpeg")).await
        .map_err(|e| anyhow::anyhow!("Upload thumb: {e}"))?;
    storage.put_bytes(&small_key, &sizes.small, Some("image/jpeg")).await
        .map_err(|e| anyhow::anyhow!("Upload small: {e}"))?;
    storage.put_bytes(&medium_key, &sizes.medium, Some("image/jpeg")).await
        .map_err(|e| anyhow::anyhow!("Upload medium: {e}"))?;
    storage.put_bytes(&large_key, &sizes.large, Some("image/jpeg")).await
        .map_err(|e| anyhow::anyhow!("Upload large: {e}"))?;

    tracing::info!(film = %film_key, "Uploaded 4 poster variants");

    Ok(PosterKeys {
        thumb: thumb_key,
        small: small_key,
        medium: medium_key,
        large: large_key,
    })
}

pub struct PosterKeys {
    pub thumb: String,
    pub small: String,
    pub medium: String,
    pub large: String,
}
