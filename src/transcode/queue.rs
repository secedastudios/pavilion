use surrealdb::types::RecordId;

use crate::db::Db;
use crate::models::transcode::{CreateTranscodeJob, TranscodeJob, TranscodeProfile};

/// Enqueue a new transcode job for a film.
pub async fn enqueue(
    db: &Db,
    film_id: RecordId,
    profile: TranscodeProfile,
) -> Result<TranscodeJob, surrealdb::Error> {
    let job: Option<TranscodeJob> = db
        .create("transcode_job")
        .content(CreateTranscodeJob {
            film: film_id,
            status: "queued".to_string(),
            profile,
            max_retries: 3,
        })
        .await?;

    job.ok_or_else(|| surrealdb::Error::thrown("Failed to create transcode job".into()))
}

/// Atomically claim the oldest queued job for a worker.
/// Returns None if no jobs available.
///
/// Uses a two-step approach: SELECT the oldest queued job, then UPDATE it
/// with a WHERE guard to prevent double-claims.
pub async fn claim(
    db: &Db,
    worker_id: &str,
) -> Result<Option<TranscodeJob>, surrealdb::Error> {
    // Find the oldest queued job
    let candidates: Vec<TranscodeJob> = db
        .query("SELECT * FROM transcode_job WHERE status = 'queued' ORDER BY created_at ASC LIMIT 1")
        .await?
        .take(0)?;

    let candidate = match candidates.into_iter().next() {
        Some(j) => j,
        None => return Ok(None),
    };

    // Atomically claim it — the WHERE status = 'queued' guard prevents
    // double-claims if another worker claimed it between our SELECT and UPDATE.
    let claimed: Vec<TranscodeJob> = db
        .query(
            "UPDATE $job_id SET \
                status = 'claimed', \
                worker_id = $worker_id, \
                claimed_at = time::now() \
             WHERE status = 'queued' \
             RETURN AFTER"
        )
        .bind(("job_id", candidate.id))
        .bind(("worker_id", worker_id.to_string()))
        .await?
        .take(0)?;

    Ok(claimed.into_iter().next())
}

/// Update the heartbeat timestamp for a claimed job.
pub async fn heartbeat(
    db: &Db,
    job_id: &RecordId,
    worker_id: &str,
) -> Result<(), surrealdb::Error> {
    db.query(
        "UPDATE $job_id SET claimed_at = time::now() \
         WHERE worker_id = $worker_id AND status IN ['claimed', 'processing']"
    )
    .bind(("job_id", job_id.clone()))
    .bind(("worker_id", worker_id.to_string()))
    .await?;
    Ok(())
}

/// Update job progress percentage.
pub async fn update_progress(
    db: &Db,
    job_id: &RecordId,
    progress_pct: i64,
) -> Result<(), surrealdb::Error> {
    db.query(
        "UPDATE $job_id SET progress_pct = $progress_pct, status = 'processing', claimed_at = time::now()"
    )
    .bind(("job_id", job_id.clone()))
    .bind(("progress_pct", progress_pct))
    .await?;
    Ok(())
}

/// Mark a job as complete.
pub async fn complete(
    db: &Db,
    job_id: &RecordId,
) -> Result<(), surrealdb::Error> {
    db.query(
        "UPDATE $job_id SET status = 'complete', progress_pct = 100, completed_at = time::now()"
    )
    .bind(("job_id", job_id.clone()))
    .await?;
    Ok(())
}

/// Mark a job as failed. Re-queues if retries remain, otherwise permanently fails.
pub async fn fail(
    db: &Db,
    job_id: &RecordId,
    error_msg: &str,
) -> Result<(), surrealdb::Error> {
    db.query(
        "UPDATE $job_id SET \
            retry_count = retry_count + 1, \
            error_msg = $error_msg, \
            status = IF retry_count + 1 >= max_retries THEN 'failed' ELSE 'queued' END, \
            worker_id = NONE, \
            claimed_at = NONE"
    )
    .bind(("job_id", job_id.clone()))
    .bind(("error_msg", error_msg.to_string()))
    .await?;
    Ok(())
}

/// Get a job by ID.
pub async fn get_job(
    db: &Db,
    job_id: &RecordId,
) -> Result<Option<TranscodeJob>, surrealdb::Error> {
    db.select(job_id.clone()).await
}

/// List jobs for a film, ordered by most recent first.
pub async fn jobs_for_film(
    db: &Db,
    film_id: &RecordId,
) -> Result<Vec<TranscodeJob>, surrealdb::Error> {
    let jobs: Vec<TranscodeJob> = db
        .query("SELECT * FROM transcode_job WHERE film = $film_id ORDER BY created_at DESC")
        .bind(("film_id", film_id.clone()))
        .await?
        .take(0)?;
    Ok(jobs)
}
