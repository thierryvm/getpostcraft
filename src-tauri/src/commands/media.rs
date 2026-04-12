use crate::commands::ai::CarouselSlide;
use base64::Engine as _;
use std::path::PathBuf;

// ── Helpers ───────────────────────────────────────────────────────────────────

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

/// Render HTML → PNG → base64 data URL. Cleans up the temp file after reading.
async fn render_to_base64(html: &str, width: u32, height: u32) -> Result<String, String> {
    let dir = renders_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create renders dir: {e}"))?;

    let filename = format!("{}.png", chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f"));
    let output_path = dir.join(filename);
    let output_str = output_path.to_string_lossy().to_string();

    crate::sidecar::call_render_sidecar(html, &output_str, width, height).await?;

    let bytes = std::fs::read(&output_path).map_err(|e| format!("Read rendered PNG: {e}"))?;
    let _ = std::fs::remove_file(&output_path);

    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:image/png;base64,{b64}"))
}

// ── Template builders ─────────────────────────────────────────────────────────

fn build_post_html(caption: &str, hashtags: &[String], width: u32, height: u32) -> String {
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
    width: {width}px; height: {height}px; overflow: hidden;
    background: #0d1117;
    font-family: -apple-system, "Segoe UI", "Helvetica Neue", Arial, sans-serif;
    display: flex; flex-direction: column;
    justify-content: center; align-items: center;
    padding: 72px; color: #e6edf3;
  }}
  .card {{
    width: 100%; background: #161b22;
    border: 1px solid #21262d; border-radius: 20px;
    padding: 64px; display: flex; flex-direction: column; gap: 36px;
    box-shadow: 0 8px 32px rgba(0,0,0,0.5);
  }}
  .caption {{ font-size: 38px; line-height: 1.55; color: #e6edf3; font-weight: 400; }}
  .divider {{ height: 1px; background: #21262d; }}
  .tags {{ display: flex; flex-wrap: wrap; gap: 14px; }}
  .tag {{ font-size: 26px; color: #3ddc84; font-weight: 500; }}
  .branding {{
    position: absolute; bottom: 44px; right: 64px;
    font-size: 22px; color: #3ddc84; opacity: 0.75;
    font-weight: 600; letter-spacing: 0.04em;
  }}
</style>
</head>
<body>
  <div class="card">
    <div class="caption">{caption}</div>
    <div class="divider"></div>
    <div class="tags">{hashtags}</div>
  </div>
  <div class="branding">@terminallearning</div>
</body>
</html>"#,
        caption = caption_escaped,
        hashtags = hashtag_html,
    )
}

fn build_code_html(
    code: &str,
    language: &str,
    filename: Option<&str>,
    width: u32,
    height: u32,
) -> String {
    let code_escaped = html_escape(code);
    let file_label = html_escape(filename.unwrap_or(language));
    let lang_label = html_escape(language);
    let line_count = code.lines().count().max(1);
    let line_numbers = (1..=line_count)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("<br>");

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{
    width: {width}px; height: {height}px; overflow: hidden;
    background: #0d1117;
    display: flex; align-items: center; justify-content: center;
    padding: 64px;
  }}
  .window {{
    width: 100%; background: #161b22;
    border-radius: 14px; border: 1px solid #21262d;
    box-shadow: 0 24px 64px rgba(0,0,0,0.7); overflow: hidden;
  }}
  .titlebar {{
    background: #1c2128; padding: 16px 24px;
    display: flex; align-items: center; gap: 14px;
    border-bottom: 1px solid #21262d;
  }}
  .dots {{ display: flex; gap: 8px; }}
  .dot {{ width: 14px; height: 14px; border-radius: 50%; }}
  .dot-r {{ background: #ff5f57; }} .dot-y {{ background: #ffbd2e; }} .dot-g {{ background: #28ca41; }}
  .file {{ flex: 1; text-align: center; font-size: 15px; color: #8b949e; font-family: "Consolas", monospace; margin-right: 60px; }}
  .lang {{ font-size: 13px; color: #3ddc84; background: rgba(61,220,132,0.12); padding: 4px 12px; border-radius: 20px; font-family: monospace; font-weight: 600; }}
  .code-wrap {{ padding: 36px 36px 36px 28px; display: flex; gap: 24px; overflow: hidden; }}
  .ln {{ color: #3d444d; font-family: "Consolas", "Courier New", monospace; font-size: 22px; line-height: 1.75; text-align: right; min-width: 36px; user-select: none; }}
  pre {{
    flex: 1; font-family: "Consolas", "Courier New", monospace;
    font-size: 22px; line-height: 1.75; color: #e6edf3;
    white-space: pre; overflow: hidden;
  }}
  .branding {{
    position: absolute; bottom: 36px; right: 52px;
    font-size: 20px; color: #3ddc84; opacity: 0.6;
    font-weight: 600; font-family: monospace; letter-spacing: 0.04em;
  }}
</style>
</head>
<body>
  <div class="window">
    <div class="titlebar">
      <div class="dots">
        <div class="dot dot-r"></div>
        <div class="dot dot-y"></div>
        <div class="dot dot-g"></div>
      </div>
      <div class="file">{file_label}</div>
      <div class="lang">{lang_label}</div>
    </div>
    <div class="code-wrap">
      <div class="ln">{line_numbers}</div>
      <pre>{code_escaped}</pre>
    </div>
  </div>
  <div class="branding">@terminallearning</div>
</body>
</html>"#,
        file_label = file_label,
        lang_label = lang_label,
        line_numbers = line_numbers,
        code_escaped = code_escaped,
    )
}

fn build_terminal_html(command: &str, output: Option<&str>, width: u32, height: u32) -> String {
    let cmd_escaped = html_escape(command);
    let output_html = output
        .filter(|o| !o.trim().is_empty())
        .map(|o| {
            format!(
                "<div class=\"output\">{}</div>",
                html_escape(o).replace('\n', "<br>")
            )
        })
        .unwrap_or_default();
    let cursor = if output.map(|o| !o.trim().is_empty()).unwrap_or(false) {
        ""
    } else {
        "<span class=\"cursor\"></span>"
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{
    width: {width}px; height: {height}px; overflow: hidden;
    background: #0d1117;
    display: flex; align-items: center; justify-content: center;
    padding: 64px; font-family: "Consolas", "Courier New", monospace;
  }}
  .terminal {{
    width: 100%; background: #0d1117;
    border-radius: 14px; border: 1px solid #21262d;
    box-shadow: 0 24px 64px rgba(0,0,0,0.9); overflow: hidden;
  }}
  .titlebar {{
    background: #1c2128; padding: 16px 24px;
    display: flex; align-items: center; gap: 14px;
    border-bottom: 1px solid #21262d;
  }}
  .dots {{ display: flex; gap: 8px; }}
  .dot {{ width: 14px; height: 14px; border-radius: 50%; }}
  .dot-r {{ background: #ff5f57; }} .dot-y {{ background: #ffbd2e; }} .dot-g {{ background: #28ca41; }}
  .title {{ flex: 1; text-align: center; font-size: 14px; color: #8b949e; margin-right: 60px; }}
  .body {{ padding: 48px; }}
  .prompt-line {{ display: flex; align-items: baseline; gap: 14px; }}
  .prompt {{ color: #3ddc84; font-size: 34px; font-weight: bold; white-space: nowrap; }}
  .cmd {{ color: #e6edf3; font-size: 34px; }}
  .cursor {{
    display: inline-block; width: 20px; height: 36px;
    background: #3ddc84; margin-left: 6px;
    vertical-align: middle; opacity: 0.85;
    animation: none;
  }}
  .output {{
    color: #8b949e; font-size: 26px; line-height: 1.7;
    margin-top: 28px; padding-left: 12px;
    border-left: 3px solid #21262d;
  }}
  .branding {{
    position: absolute; bottom: 36px; right: 52px;
    font-size: 20px; color: #3ddc84; opacity: 0.6;
    font-weight: 600; letter-spacing: 0.04em;
  }}
</style>
</head>
<body>
  <div class="terminal">
    <div class="titlebar">
      <div class="dots">
        <div class="dot dot-r"></div>
        <div class="dot dot-y"></div>
        <div class="dot dot-g"></div>
      </div>
      <div class="title">bash — @terminallearning</div>
    </div>
    <div class="body">
      <div class="prompt-line">
        <span class="prompt">$</span>
        <span class="cmd">{command}</span>
        {cursor}
      </div>
      {output}
    </div>
  </div>
  <div class="branding">@terminallearning</div>
</body>
</html>"#,
        command = cmd_escaped,
        cursor = cursor,
        output = output_html,
    )
}

fn build_carousel_slide_html(slide: &CarouselSlide) -> String {
    let dots: String = (1..=slide.total)
        .map(|i| {
            if i == slide.index {
                r#"<div class="dot active"></div>"#.to_string()
            } else {
                r#"<div class="dot"></div>"#.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("");

    let css = concat!(
        "*{margin:0;padding:0;box-sizing:border-box}",
        "body{width:1080px;height:1080px;background:#0d1117;",
        "font-family:'Segoe UI',system-ui,-apple-system,sans-serif;",
        "display:flex;flex-direction:column;align-items:center;",
        "justify-content:center;padding:80px;position:relative}",
        ".brand{position:absolute;top:40px;right:48px;font-size:22px;",
        "color:#3ddc84;font-weight:700;letter-spacing:.04em}",
        ".counter{position:absolute;top:40px;left:48px;font-size:22px;",
        "color:#8b949e;font-weight:500}",
        ".content{display:flex;flex-direction:column;align-items:center;",
        "text-align:center;max-width:900px}",
        ".emoji{font-size:104px;line-height:1;margin-bottom:48px}",
        ".title{font-size:58px;font-weight:800;color:#fff;",
        "line-height:1.15;margin-bottom:28px}",
        ".accent{width:64px;height:5px;background:#3ddc84;",
        "border-radius:3px;margin-bottom:36px}",
        ".body{font-size:30px;color:#c9d1d9;line-height:1.6}",
        ".dots{position:absolute;bottom:44px;left:50%;",
        "transform:translateX(-50%);display:flex;gap:10px;align-items:center}",
        ".dot{width:10px;height:10px;border-radius:50%;background:#30363d}",
        ".dot.active{width:28px;height:10px;border-radius:5px;background:#3ddc84}",
    );

    let mut html = String::with_capacity(3500);
    html.push_str(r#"<!DOCTYPE html><html><head><meta charset="UTF-8"><style>"#);
    html.push_str(css);
    html.push_str("</style></head><body>");
    html.push_str(r#"<div class="brand">getpostcraft</div>"#);
    html.push_str(&format!(
        r#"<div class="counter">{}/{}</div>"#,
        slide.index, slide.total
    ));
    html.push_str(r#"<div class="content">"#);
    html.push_str(&format!(
        r#"<div class="emoji">{}</div>"#,
        html_escape(&slide.emoji)
    ));
    html.push_str(&format!(
        r#"<div class="title">{}</div>"#,
        html_escape(&slide.title)
    ));
    html.push_str(r#"<div class="accent"></div>"#);
    html.push_str(&format!(
        r#"<div class="body">{}</div>"#,
        html_escape(&slide.body).replace('\n', "<br>"),
    ));
    html.push_str("</div>");
    html.push_str(&format!(r#"<div class="dots">{}</div>"#, dots));
    html.push_str("</body></html>");
    html
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Render caption + hashtags to PNG at given dimensions. Returns base64 data URL.
#[tauri::command]
pub async fn render_post_image(
    caption: String,
    hashtags: Vec<String>,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<String, String> {
    let w = width.unwrap_or(1080);
    let h = height.unwrap_or(1080);
    render_to_base64(&build_post_html(&caption, &hashtags, w, h), w, h).await
}

/// Render a code snippet card to PNG at given dimensions. Returns base64 data URL.
#[tauri::command]
pub async fn render_code_image(
    code: String,
    language: String,
    filename: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<String, String> {
    let w = width.unwrap_or(1080);
    let h = height.unwrap_or(1080);
    render_to_base64(
        &build_code_html(&code, &language, filename.as_deref(), w, h),
        w,
        h,
    )
    .await
}

/// Render a terminal mockup to PNG at given dimensions. Returns base64 data URL.
#[tauri::command]
pub async fn render_terminal_image(
    command: String,
    output: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<String, String> {
    let w = width.unwrap_or(1080);
    let h = height.unwrap_or(1080);
    render_to_base64(
        &build_terminal_html(&command, output.as_deref(), w, h),
        w,
        h,
    )
    .await
}

/// Render each carousel slide to PNG. Returns Vec of base64 data URLs (same order as input).
#[tauri::command]
pub async fn render_carousel_slides(slides: Vec<CarouselSlide>) -> Result<Vec<String>, String> {
    let mut images = Vec::with_capacity(slides.len());
    for slide in &slides {
        let data_url = render_to_base64(&build_carousel_slide_html(slide), 1080, 1080).await?;
        images.push(data_url);
    }
    Ok(images)
}

/// Pack base64 data-URL images into a ZIP and save it to the Downloads folder.
/// Returns the absolute path to the created ZIP file.
#[tauri::command]
pub async fn export_carousel_zip(images: Vec<String>) -> Result<String, String> {
    use std::io::Write as _;
    use zip::write::SimpleFileOptions;

    let downloads = dirs::download_dir()
        .ok_or_else(|| "Impossible de trouver le dossier Téléchargements".to_string())?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let zip_path = downloads.join(format!("carousel_{timestamp}.zip"));

    let file = std::fs::File::create(&zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);

    for (i, data_url) in images.iter().enumerate() {
        let b64 = data_url
            .strip_prefix("data:image/png;base64,")
            .ok_or_else(|| format!("Invalid data URL at index {i}"))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| format!("Base64 decode error at index {i}: {e}"))?;

        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored); // PNGs are already compressed

        zip.start_file(format!("slide_{:02}.png", i + 1), options)
            .map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }

    zip.finish().map_err(|e| e.to_string())?;
    Ok(zip_path.to_string_lossy().to_string())
}
