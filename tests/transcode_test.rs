mod common;

use surrealdb::types::RecordId;

use pavilion::models::transcode::TranscodeProfile;
use pavilion::transcode::{manifest, queue};

// ── Queue lifecycle ────────────────────────────────────────

#[tokio::test]
async fn enqueue_creates_queued_job() {
    let db = common::setup_test_db().await;
    let film_id = RecordId::new("film", "test-film");
    let profile = TranscodeProfile::h264_default();

    let job = queue::enqueue(&db, film_id.clone(), profile).await.unwrap();

    assert_eq!(job.status, "queued");
    assert_eq!(job.film, film_id);
    assert_eq!(job.progress_pct, 0);
    assert_eq!(job.retry_count, 0);
    assert_eq!(job.max_retries, 3);
    assert!(job.worker_id.is_none());
    assert!(job.claimed_at.is_none());
}

#[tokio::test]
async fn claim_returns_queued_job() {
    let db = common::setup_test_db().await;
    let film_id = RecordId::new("film", "test-film");
    let profile = TranscodeProfile::h264_default();

    queue::enqueue(&db, film_id, profile).await.unwrap();

    let claimed = queue::claim(&db, "worker-1").await.unwrap();
    assert!(claimed.is_some());

    let job = claimed.unwrap();
    assert_eq!(job.status, "claimed");
    assert_eq!(job.worker_id, Some("worker-1".to_string()));
    assert!(job.claimed_at.is_some());
}

#[tokio::test]
async fn claim_returns_none_when_no_jobs() {
    let db = common::setup_test_db().await;

    let claimed = queue::claim(&db, "worker-1").await.unwrap();
    assert!(claimed.is_none());
}

#[tokio::test]
async fn double_claim_only_gives_one_job() {
    let db = common::setup_test_db().await;
    let film_id = RecordId::new("film", "test-film");
    let profile = TranscodeProfile::h264_default();

    queue::enqueue(&db, film_id, profile).await.unwrap();

    let claim1 = queue::claim(&db, "worker-1").await.unwrap();
    let claim2 = queue::claim(&db, "worker-2").await.unwrap();

    assert!(claim1.is_some());
    assert!(claim2.is_none());
}

#[tokio::test]
async fn update_progress_sets_processing() {
    let db = common::setup_test_db().await;
    let film_id = RecordId::new("film", "test-film");
    let profile = TranscodeProfile::h264_default();

    let job = queue::enqueue(&db, film_id, profile).await.unwrap();
    let _ = queue::claim(&db, "worker-1").await.unwrap();

    queue::update_progress(&db, &job.id, 42).await.unwrap();

    let updated = queue::get_job(&db, &job.id).await.unwrap().unwrap();
    assert_eq!(updated.status, "processing");
    assert_eq!(updated.progress_pct, 42);
}

#[tokio::test]
async fn complete_sets_100_percent() {
    let db = common::setup_test_db().await;
    let film_id = RecordId::new("film", "test-film");
    let profile = TranscodeProfile::h264_default();

    let job = queue::enqueue(&db, film_id, profile).await.unwrap();
    let _ = queue::claim(&db, "worker-1").await.unwrap();

    queue::complete(&db, &job.id).await.unwrap();

    let completed = queue::get_job(&db, &job.id).await.unwrap().unwrap();
    assert_eq!(completed.status, "complete");
    assert_eq!(completed.progress_pct, 100);
    assert!(completed.completed_at.is_some());
}

#[tokio::test]
async fn fail_requeues_if_retries_remain() {
    let db = common::setup_test_db().await;
    let film_id = RecordId::new("film", "test-film");
    let profile = TranscodeProfile::h264_default();

    let job = queue::enqueue(&db, film_id, profile).await.unwrap();
    let _ = queue::claim(&db, "worker-1").await.unwrap();

    queue::fail(&db, &job.id, "FFmpeg crashed").await.unwrap();

    let failed = queue::get_job(&db, &job.id).await.unwrap().unwrap();
    assert_eq!(failed.status, "queued"); // Re-queued for retry
    assert_eq!(failed.retry_count, 1);
    assert_eq!(failed.error_msg, Some("FFmpeg crashed".to_string()));
    assert!(failed.worker_id.is_none());
}

#[tokio::test]
async fn fail_permanently_after_max_retries() {
    let db = common::setup_test_db().await;
    let film_id = RecordId::new("film", "test-film");
    let profile = TranscodeProfile::h264_default();

    let job = queue::enqueue(&db, film_id, profile).await.unwrap();

    // Exhaust all retries
    for i in 0..3 {
        let _ = queue::claim(&db, &format!("worker-{i}")).await.unwrap();
        queue::fail(&db, &job.id, "Keeps failing").await.unwrap();
    }

    let final_job = queue::get_job(&db, &job.id).await.unwrap().unwrap();
    assert_eq!(final_job.status, "failed"); // Permanently failed
    assert_eq!(final_job.retry_count, 3);
}

#[tokio::test]
async fn jobs_for_film_returns_all_jobs() {
    let db = common::setup_test_db().await;
    let film_id = RecordId::new("film", "test-film");
    let profile = TranscodeProfile::h264_default();

    queue::enqueue(&db, film_id.clone(), profile.clone()).await.unwrap();
    queue::enqueue(&db, film_id.clone(), profile).await.unwrap();

    let jobs = queue::jobs_for_film(&db, &film_id).await.unwrap();
    assert_eq!(jobs.len(), 2);
}

// ── Manifest generation ────────────────────────────────────

#[test]
fn hls_master_playlist_has_all_renditions() {
    use pavilion::transcode::manifest::RenditionInfo;

    let renditions = vec![
        RenditionInfo {
            name: "360p".to_string(),
            width: 640,
            height: 360,
            bandwidth: 800_000,
            playlist_file: "360p.m3u8".to_string(),
        },
        RenditionInfo {
            name: "720p".to_string(),
            width: 1280,
            height: 720,
            bandwidth: 2_800_000,
            playlist_file: "720p.m3u8".to_string(),
        },
    ];

    let m3u8 = manifest::generate_hls_master(&renditions, "/segments/my-film");

    assert!(m3u8.contains("#EXTM3U"));
    assert!(m3u8.contains("#EXT-X-VERSION:7"));
    assert!(m3u8.contains("RESOLUTION=640x360"));
    assert!(m3u8.contains("RESOLUTION=1280x720"));
    assert!(m3u8.contains("/segments/my-film/360p/360p.m3u8"));
    assert!(m3u8.contains("/segments/my-film/720p/720p.m3u8"));
}

#[test]
fn dash_mpd_has_all_representations() {
    use pavilion::transcode::manifest::RenditionInfo;

    let renditions = vec![
        RenditionInfo {
            name: "480p".to_string(),
            width: 854,
            height: 480,
            bandwidth: 1_400_000,
            playlist_file: "480p.m3u8".to_string(),
        },
        RenditionInfo {
            name: "1080p".to_string(),
            width: 1920,
            height: 1080,
            bandwidth: 5_000_000,
            playlist_file: "1080p.m3u8".to_string(),
        },
    ];

    let mpd = manifest::generate_dash_mpd(&renditions, "/segments/my-film", 3600);

    assert!(mpd.contains("PT3600S"));
    assert!(mpd.contains("width=\"854\""));
    assert!(mpd.contains("width=\"1920\""));
    assert!(mpd.contains("height=\"480\""));
    assert!(mpd.contains("height=\"1080\""));
}
