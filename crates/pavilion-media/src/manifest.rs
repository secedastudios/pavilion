use crate::token::sign_segment_url;

/// Rewrite an HLS manifest, replacing relative segment URLs with signed proxy URLs.
pub fn rewrite_hls_manifest(
    manifest: &str,
    subject: &str,
    resource: &str,
    scope: &str,
    secret: &str,
    ttl_secs: u64,
    url_prefix: &str,
) -> String {
    manifest
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                line.to_string()
            } else {
                sign_segment_url(subject, resource, scope, trimmed, secret, ttl_secs, url_prefix)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Rewrite a DASH manifest, replacing BaseURL and sourceURL references with signed URLs.
pub fn rewrite_dash_manifest(
    manifest: &str,
    subject: &str,
    resource: &str,
    scope: &str,
    secret: &str,
    ttl_secs: u64,
    url_prefix: &str,
) -> String {
    let mut result = manifest.to_string();

    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<BaseURL>") && trimmed.ends_with("</BaseURL>") {
            let path = trimmed
                .strip_prefix("<BaseURL>")
                .and_then(|s| s.strip_suffix("</BaseURL>"))
                .unwrap_or("");
            if !path.is_empty() {
                let signed =
                    sign_segment_url(subject, resource, scope, path, secret, ttl_secs, url_prefix);
                result = result.replace(trimmed, &format!("<BaseURL>{signed}</BaseURL>"));
            }
        }
        if trimmed.contains("sourceURL=\"") {
            if let Some(start) = trimmed.find("sourceURL=\"") {
                let after = &trimmed[start + 11..];
                if let Some(end) = after.find('"') {
                    let path = &after[..end];
                    let signed = sign_segment_url(
                        subject, resource, scope, path, secret, ttl_secs, url_prefix,
                    );
                    result = result.replace(
                        &format!("sourceURL=\"{path}\""),
                        &format!("sourceURL=\"{signed}\""),
                    );
                }
            }
        }
    }

    result
}

/// Generate an HLS master playlist (.m3u8) that references rendition playlists.
pub fn generate_hls_master(renditions: &[RenditionInfo], base_url: &str) -> String {
    let mut lines = vec![
        "#EXTM3U".to_string(),
        "#EXT-X-VERSION:7".to_string(),
    ];

    for r in renditions {
        lines.push(format!(
            "#EXT-X-STREAM-INF:BANDWIDTH={},RESOLUTION={}x{},CODECS=\"avc1.4d401f,mp4a.40.2\"",
            r.bandwidth, r.width, r.height,
        ));
        lines.push(format!("{base_url}/{}/{}", r.name, r.playlist_file));
    }

    lines.join("\n") + "\n"
}

/// Generate a DASH MPD manifest.
pub fn generate_dash_mpd(
    renditions: &[RenditionInfo],
    base_url: &str,
    duration_seconds: u64,
) -> String {
    let duration_iso = format!("PT{duration_seconds}S");
    let mut adaptations = String::new();

    for r in renditions {
        adaptations.push_str(&format!(
            r#"      <Representation id="{name}" mimeType="video/mp4" codecs="avc1.4d401f" width="{w}" height="{h}" bandwidth="{bw}">
        <BaseURL>{base}/{name}/</BaseURL>
        <SegmentList>
          <Initialization sourceURL="init.mp4"/>
        </SegmentList>
      </Representation>
"#,
            name = r.name,
            w = r.width,
            h = r.height,
            bw = r.bandwidth,
            base = base_url,
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" type="static" mediaPresentationDuration="{duration}" minBufferTime="PT2S" profiles="urn:mpeg:dash:profile:isoff-on-demand:2011">
  <Period>
    <AdaptationSet contentType="video" segmentAlignment="true" bitstreamSwitching="true">
{adaptations}    </AdaptationSet>
  </Period>
</MPD>
"#,
        duration = duration_iso,
        adaptations = adaptations,
    )
}

/// Describes a single rendition for manifest generation.
#[derive(Debug, Clone)]
pub struct RenditionInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub bandwidth: u64,
    pub playlist_file: String,
}
