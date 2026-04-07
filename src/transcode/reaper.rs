use std::time::Duration;

use tokio::time::sleep;

use crate::db::Db;
use crate::models::transcode::TranscodeJob;

const REAP_INTERVAL: Duration = Duration::from_secs(60);
const STALE_THRESHOLD_SECS: i64 = 300; // 5 minutes without heartbeat

/// Periodically scan for stale claimed/processing jobs and re-queue or fail them.
/// Runs as a background task on any Pavilion node. Safe to run on multiple nodes
/// simultaneously — the UPDATE WHERE clause ensures idempotent behavior.
pub async fn run(db: Db) {
    tracing::info!("Transcode job reaper started");

    loop {
        sleep(REAP_INTERVAL).await;

        if let Err(err) = reap_stale_jobs(&db).await {
            tracing::error!(error = %err, "Error reaping stale transcode jobs");
        }
    }
}

async fn reap_stale_jobs(db: &Db) -> Result<(), surrealdb::Error> {
    // Find jobs that are claimed/processing but haven't had a heartbeat
    // in STALE_THRESHOLD_SECS seconds.
    let stale_jobs: Vec<TranscodeJob> = db
        .query(
            "SELECT * FROM transcode_job \
             WHERE status IN ['claimed', 'processing'] \
               AND claimed_at < time::now() - $threshold_duration"
        )
        .bind(("threshold_duration", format!("{}s", STALE_THRESHOLD_SECS)))
        .await?
        .take(0)?;

    for job in &stale_jobs {
        if job.retry_count + 1 >= job.max_retries {
            // Permanently fail
            db.query(
                "UPDATE $job_id SET \
                    status = 'failed', \
                    error_msg = 'Worker heartbeat timeout (max retries exceeded)', \
                    worker_id = NONE, \
                    claimed_at = NONE \
                 WHERE status IN ['claimed', 'processing']"
            )
            .bind(("job_id", job.id.clone()))
            .await?;

            tracing::warn!(job_id = ?job.id, "Stale job permanently failed (max retries)");
        } else {
            // Re-queue for retry
            db.query(
                "UPDATE $job_id SET \
                    status = 'queued', \
                    retry_count = retry_count + 1, \
                    error_msg = 'Worker heartbeat timeout (re-queued)', \
                    worker_id = NONE, \
                    claimed_at = NONE \
                 WHERE status IN ['claimed', 'processing']"
            )
            .bind(("job_id", job.id.clone()))
            .await?;

            tracing::info!(job_id = ?job.id, retry = job.retry_count + 1, "Stale job re-queued");
        }
    }

    if !stale_jobs.is_empty() {
        tracing::info!(count = stale_jobs.len(), "Reaped stale transcode jobs");
    }

    Ok(())
}
