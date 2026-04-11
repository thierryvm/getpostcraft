use base64::Engine as _;
use std::path::PathBuf;

/// Returns a temp path for rendered images.
fn renders_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("getpostcraft")
        .join("renders")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn build_post_html(caption: &str, hashtags: &[String]) -> String {
    let caption_escaped = html_escape(caption).replace('\n', "<br>");
    let hashtag_html: String = hashtags
        .iter()
        .map(|t| format!("<span class=\"tag\">#{}</span>", html_escape(t)))
        .collect::<Vec<_>>()
        .join("\n        ");

    format!(
        r#"<!DOCTYPE html>
<html lang="fr">
<head>
<meta charset="UTF-8">
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{
    width: 1080px; height: 1080px; overflow: hidden;
    background: #0d1117;
    font-family: -apple-system, "Segoe UI", "Helvetica Neue", Arial, sans-serif;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    padding: 72px;
    color: #e6edf3;
  }}
  .card {{
    width: 100%;
    background: #161b22;
    border: 1px solid #21262d;
    border-radius: 20px;
    padding: 64px;
    display: flex;
    flex-direction: column;
    gap: 36px;
    box-shadow: 0 8px 32px rgba(0,0,0,0.5);
  }}
  .caption {{
    font-size: 38px;
    line-height: 1.55;
    color: #e6edf3;
    font-weight: 400;
    letter-spacing: -0.01em;
  }}
  .divider {{
    height: 1px;
    background: #21262d;
  }}
  .tags {{
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
  }}
  .tag {{
    font-size: 26px;
    color: #3ddc84;
    font-weight: 500;
    letter-spacing: 0.01em;
  }}
  .branding {{
    position: absolute;
    bottom: 44px;
    right: 64px;
    font-size: 22px;
    color: #3ddc84;
    opacity: 0.75;
    font-weight: 600;
    letter-spacing: 0.04em;
  }}
</style>
</head>
<body>
  <div class="card">
    <div class="caption">{caption}</div>
    <div class="divider"></div>
    <div class="tags">
        {hashtags}
    </div>
  </div>
  <div class="branding">@terminallearning</div>
</body>
</html>"#,
        caption = caption_escaped,
        hashtags = hashtag_html,
    )
}

/// Render the post to a 1080×1080 PNG.
/// Returns a `data:image/png;base64,...` string ready for use in an <img> src.
#[tauri::command]
pub async fn render_post_image(
    caption: String,
    hashtags: Vec<String>,
) -> Result<String, String> {
    let dir = renders_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create renders dir: {e}"))?;

    let filename = format!("{}.png", chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f"));
    let output_path = dir.join(filename);
    let output_str = output_path.to_string_lossy().to_string();

    let html = build_post_html(&caption, &hashtags);
    crate::sidecar::call_render_sidecar(&html, &output_str, 1080, 1080).await?;

    // Read bytes and encode as base64 data URL — avoids asset protocol path issues
    let bytes =
        std::fs::read(&output_path).map_err(|e| format!("Read rendered PNG: {e}"))?;
    let _ = std::fs::remove_file(&output_path); // clean up after reading

    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/png;base64,{b64}"))
}
