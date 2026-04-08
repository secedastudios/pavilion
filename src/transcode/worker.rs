use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::time::sleep;

use pavilion_media::storage::StorageClient;

use crate::db::Db;
use crate::transcode::{ffmpeg, manifest, queue};

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// Start the transcoding worker loop.
pub async fn run(db: Arc<Db>, storage: Arc<StorageClient>, worker_id: String, work_dir: PathBuf) {
    tracing::info!(worker_id = %worker_id, "Transcode worker started");

    loop {
        match queue::claim(&db, &worker_id).await {
            Ok(Some(job)) => {
                tracing::info!(
                    job_id = ?job.id,
                    worker_id = %worker_id,
                    "Claimed transcode job"
                );
                process_job(&db, &storage, &job.id, &worker_id, &work_dir).await;
            }
            Ok(None) => {
                sleep(POLL_INTERVAL).await;
            }
            Err(err) => {
                tracing::error!(error = %err, "Error polling transcode queue");
                sleep(POLL_INTERVAL).await;
            }
        }
    }
}

async fn process_job(
    db: &Db,
    storage: &StorageClient,
    job_id: &surrealdb::types::RecordId,
    worker_id: &str,
    work_dir: &Path,
) {
    let job = match queue::get_job(db, job_id).await {
        Ok(Some(j)) => j,
        Ok(None) => {
            tracing::error!(job_id = ?job_id, "Job not found after claim");
            return;
        }
        Err(err) => {
            tracing::error!(error = %err, "Failed to fetch claimed job");
            return;
        }
    };

    let job_key = crate::util::record_id_key_string(&job.id.key);
    let film_key = crate::util::record_id_key_string(&job.film.key);
    let job_dir = work_dir.join(&job_key);

    if let Err(err) = tokio::fs::create_dir_all(&job_dir).await {
        tracing::error!(error = %err, "Failed to create job work directory");
        let _ = queue::fail(db, job_id, &format!("Work dir error: {err}")).await;
        return;
    }

    let _ = queue::update_progress(db, job_id, 0).await;

    // Heartbeat
    let hb_db = db.clone();
    let hb_job_id = job_id.clone();
    let hb_worker_id = worker_id.to_string();
    let heartbeat_handle = tokio::spawn(async move {
        loop {
            sleep(HEARTBEAT_INTERVAL).await;
            if queue::heartbeat(&hb_db, &hb_job_id, &hb_worker_id)
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Download master from RustFS
    let master_storage_key = format!("masters/{film_key}.mp4");
    let master_path = job_dir.join("master.mp4");

    if let Err(err) = storage.get_file(&master_storage_key, &master_path).await {
        tracing::error!(error = %err, film = %film_key, "Failed to download master from storage");
        let _ = queue::fail(db, job_id, &format!("Master download error: {err}")).await;
        heartbeat_handle.abort();
        let _ = tokio::fs::remove_dir_all(&job_dir).await;
        return;
    }

    let _ = queue::update_progress(db, job_id, 10).await;
    tracing::info!(film = %film_key, "Downloaded master file");

    // Transcode all renditions
    let output_dir = job_dir.join("output");
    match ffmpeg::transcode_all_renditions(&master_path, &output_dir).await {
        Ok(results) => {
            let _ = queue::update_progress(db, job_id, 80).await;

            // Generate master manifests
            let output_prefix = format!("videos/{film_key}");
            let renditions: Vec<_> = results
                .iter()
                .filter_map(|r| r.to_rendition_info())
                .collect();
            let hls_master = manifest::generate_hls_master(&renditions, &output_prefix);
            let dash_mpd = manifest::generate_dash_mpd(&renditions, &output_prefix, 0);

            // Write manifests locally
            let _ = tokio::fs::write(output_dir.join("master.m3u8"), &hls_master).await;
            let _ = tokio::fs::write(output_dir.join("manifest.mpd"), &dash_mpd).await;

            // Upload everything to RustFS
            let _ = queue::update_progress(db, job_id, 90).await;
            match storage.upload_directory(&output_dir, &output_prefix).await {
                Ok(count) => {
                    tracing::info!(film = %film_key, files = count, "Uploaded transcoded files to storage");

                    // Create asset records in SurrealDB
                    for result in &results {
                        let storage_key = format!("{output_prefix}/{}/", result.resolution);
                        let _ = db.query(
                            "CREATE asset SET \
                                asset_type = 'rendition', \
                                codec = 'h264', \
                                resolution = $resolution, \
                                format = 'fmp4', \
                                storage_key = $storage_key; \
                             LET $asset = (SELECT * FROM asset WHERE storage_key = $storage_key LIMIT 1); \
                             IF array::len($asset) > 0 { \
                                 RELATE $film->has_asset->$asset[0].id; \
                             };"
                        )
                        .bind(("resolution", result.resolution.clone()))
                        .bind(("storage_key", storage_key))
                        .bind(("film", job.film.clone()))
                        .await;
                    }

                    let _ = queue::complete(db, job_id).await;
                    tracing::info!(job_id = ?job_id, film = %film_key, "Transcode complete");
                }
                Err(err) => {
                    tracing::error!(error = %err, "Failed to upload transcoded files");
                    let _ = queue::fail(db, job_id, &format!("Upload error: {err}")).await;
                }
            }
        }
        Err(err) => {
            tracing::error!(error = %err, job_id = ?job_id, "Transcode failed");
            let _ = queue::fail(db, job_id, &err.to_string()).await;
        }
    }

    heartbeat_handle.abort();
    let _ = tokio::fs::remove_dir_all(&job_dir).await;
}
