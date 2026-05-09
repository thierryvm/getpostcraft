use crate::commands::ai::CarouselSlide;
use base64::Engine as _;
use std::path::PathBuf;

// ── Branding defaults ─────────────────────────────────────────────────────────

const DEFAULT_HANDLE: &str = "yourbrand";
const DEFAULT_BRAND_COLOR: &str = "#3ddc84";

/// Resolved branding values used by every template.
/// `handle` is rendered in titlebars/branding labels (no leading "@" — added in HTML).
/// `brand_color` paints prompts, cursors, tags, accents, dot indicators.
struct Brand {
    handle: String,
    brand_color: String,
}

impl Brand {
    fn resolve(handle: Option<&str>, brand_color: Option<&str>) -> Self {
        // Filter AFTER stripping the leading '@' so a bare "@" (or "  @  ")
        // resolves to the default handle instead of an empty string.
        let handle = handle
            .map(str::trim)
            .map(|s| s.trim_start_matches('@').trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| DEFAULT_HANDLE.to_string());
        let brand_color = brand_color
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| DEFAULT_BRAND_COLOR.to_string());
        Self {
            handle,
            brand_color,
        }
    }
}

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

fn build_post_html(
    caption: &str,
    hashtags: &[String],
    width: u32,
    height: u32,
    brand: &Brand,
) -> String {
    let caption_escaped = html_escape(caption).replace('\n', "<br>");
    let hashtag_html: String = hashtags
        .iter()
        .map(|t| format!("<span class=\"tag\">#{}</span>", html_escape(t)))
        .collect::<Vec<_>>()
        .join(" ");

    // Scale font size down for longer captions so the content stays within the frame.
    let caption_len = caption.chars().count();
    let caption_font = if caption_len <= 160 {
        32
    } else if caption_len <= 320 {
        26
    } else if caption_len <= 600 {
        22
    } else {
        18
    };
    let tag_font = (caption_font as f32 * 0.60) as u32;
    let content_padding = if caption_len <= 320 { 52 } else { 36 };
    let handle_escaped = html_escape(&brand.handle);
    let brand_color = &brand.brand_color;

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
    font-family: "SF Mono", "Fira Code", "Cascadia Code", Consolas, "Courier New", monospace;
    display: flex; flex-direction: column;
    justify-content: center; align-items: center;
    padding: 56px;
  }}
  .terminal {{
    width: 100%;
    max-height: calc({height}px - 112px);
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 14px;
    overflow: hidden;
    box-shadow: 0 24px 64px rgba(0,0,0,0.7), 0 0 0 1px {brand_color}14;
  }}
  .titlebar {{
    background: #21262d;
    padding: 16px 24px;
    display: flex;
    align-items: center;
    gap: 10px;
    border-bottom: 1px solid #30363d;
    flex-shrink: 0;
  }}
  .dot {{ width: 14px; height: 14px; border-radius: 50%; flex-shrink: 0; }}
  .dot-r {{ background: #ff5f57; }}
  .dot-y {{ background: #ffbd2e; }}
  .dot-g {{ background: #28c840; }}
  .wintitle {{
    flex: 1; text-align: center;
    font-size: 18px; color: #8b949e;
    letter-spacing: 0.03em; font-weight: 500;
  }}
  .content {{
    padding: {content_padding}px;
    display: flex; flex-direction: column; gap: 0;
    overflow: hidden;
  }}
  .prompt-row {{
    display: flex; align-items: flex-start; gap: 16px;
  }}
  .prompt {{ color: {brand_color}; font-size: {caption_font}px; line-height: 1.6; flex-shrink: 0; font-weight: 600; }}
  .caption-text {{
    color: #e6edf3; font-size: {caption_font}px; line-height: 1.6;
    word-break: break-word; overflow: hidden;
  }}
  .cursor {{
    display: inline-block; width: 3px; height: {caption_font}px;
    background: {brand_color}; margin-left: 6px;
    vertical-align: middle; opacity: 0.9;
  }}
  .sep {{
    margin: 28px 0 22px;
    height: 1px; background: linear-gradient(to right, {brand_color}30, #30363d, transparent);
    flex-shrink: 0;
  }}
  .tags {{
    display: flex; flex-wrap: wrap; gap: 10px; flex-shrink: 0;
  }}
  .tag {{ font-size: {tag_font}px; color: {brand_color}; opacity: 0.75; }}
</style>
</head>
<body>
  <div class="terminal">
    <div class="titlebar">
      <div class="dot dot-r"></div>
      <div class="dot dot-y"></div>
      <div class="dot dot-g"></div>
      <span class="wintitle">@{handle_escaped} — zsh</span>
    </div>
    <div class="content">
      <div class="prompt-row">
        <span class="prompt">$</span>
        <span class="caption-text">{caption}<span class="cursor"></span></span>
      </div>
      <div class="sep"></div>
      <div class="tags">{hashtags}</div>
    </div>
  </div>
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
    brand: &Brand,
) -> String {
    let code_escaped = html_escape(code);
    let file_label = html_escape(filename.unwrap_or(language));
    let lang_label = html_escape(language);
    let line_count = code.lines().count().max(1);
    let line_numbers = (1..=line_count)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("<br>");
    let handle_escaped = html_escape(&brand.handle);
    let brand_color = &brand.brand_color;

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
  .lang {{ font-size: 13px; color: {brand_color}; background: {brand_color}1f; padding: 4px 12px; border-radius: 20px; font-family: monospace; font-weight: 600; }}
  .code-wrap {{ padding: 36px 36px 36px 28px; display: flex; gap: 24px; overflow: hidden; }}
  .ln {{ color: #3d444d; font-family: "Consolas", "Courier New", monospace; font-size: 22px; line-height: 1.75; text-align: right; min-width: 36px; user-select: none; }}
  pre {{
    flex: 1; font-family: "Consolas", "Courier New", monospace;
    font-size: 22px; line-height: 1.75; color: #e6edf3;
    white-space: pre; overflow: hidden;
  }}
  .branding {{
    position: absolute; bottom: 36px; right: 52px;
    font-size: 20px; color: {brand_color}; opacity: 0.6;
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
  <div class="branding">@{handle_escaped}</div>
</body>
</html>"#,
        file_label = file_label,
        lang_label = lang_label,
        line_numbers = line_numbers,
        code_escaped = code_escaped,
    )
}

fn build_terminal_html(
    command: &str,
    output: Option<&str>,
    width: u32,
    height: u32,
    brand: &Brand,
) -> String {
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
    let handle_escaped = html_escape(&brand.handle);
    let brand_color = &brand.brand_color;

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
  .prompt {{ color: {brand_color}; font-size: 34px; font-weight: bold; white-space: nowrap; }}
  .cmd {{ color: #e6edf3; font-size: 34px; }}
  .cursor {{
    display: inline-block; width: 20px; height: 36px;
    background: {brand_color}; margin-left: 6px;
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
    font-size: 20px; color: {brand_color}; opacity: 0.6;
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
      <div class="title">bash — @{handle_escaped}</div>
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
  <div class="branding">@{handle_escaped}</div>
</body>
</html>"#,
        command = cmd_escaped,
        cursor = cursor,
        output = output_html,
    )
}

/// Build the HTML for a single carousel slide.
///
/// Visual grammar (v0.3.6, inspired by hand-crafted reference posts):
/// - Subtle 60×60px grid background (#161b22 stroke @ ~6% opacity) provides
///   a tech-niche texture without competing with the content.
/// - Counter top-right in monospace dim grey (`01 / 07` style).
/// - Brand stamp bottom-right `>_ @handle` in monospace, branded color.
/// - Content is left-aligned within the upper 2/3 of the canvas: title in
///   bold sans-serif, accent rule, then body. Emoji becomes an optional
///   pill in the top-left so it never has to fight the title for visual
///   weight.
///
/// All sizes are derived from `width` so the same template renders cleanly
/// at 1080×1080 (square) and 1080×1350 (4:5 portrait, the new IG default).
fn build_carousel_slide_html(
    slide: &CarouselSlide,
    width: u32,
    height: u32,
    brand: &Brand,
) -> String {
    let handle_escaped = html_escape(&brand.handle);
    let brand_color = &brand.brand_color;

    // Typography scale derived from canvas width — keeps proportions on
    // both 1080² and 1080×1350. Hero titles are intentionally large so
    // mobile-feed scrolls are stopped without resorting to all-caps.
    let w = width as f32;
    let pad = (w * 0.072).round() as u32; // 78px @ 1080
    let title_size = (w * 0.082).round() as u32; // 88px @ 1080
    let body_size = (w * 0.027).round() as u32; // 30px @ 1080
    let chrome_size = (w * 0.02).round() as u32; // 22px @ 1080
    let badge_size = (w * 0.02).round() as u32; // 22px @ 1080
    let accent_height = (w * 0.005).round().max(4.0) as u32;
    let accent_width = (w * 0.06).round() as u32;

    // Resolve the role-driven badge first; fall back to the index-derived
    // label if the AI didn't tag the slide. The badge color overrides the
    // brand color *only inside the badge* — the rest of the chrome
    // (counter, stamp, accent rule) stays branded.
    let role_meta = role_meta_for(slide.role.as_deref());
    let badge_label = role_meta
        .label
        .map(str::to_string)
        .unwrap_or_else(|| slide_label(slide.index, slide.total).to_string());
    let badge_color = role_meta.color.unwrap_or(brand_color.as_str());
    let badge_mark = if slide.emoji.trim().is_empty() {
        role_meta.mark.unwrap_or("✦").to_string()
    } else {
        slide.emoji.clone()
    };
    let badge_html = format!(
        r#"<div class="badge" style="border-color:{badge_color}55;background:{badge_color}1f;color:{badge_color}"><span class="badge-mark">{mark}</span><span class="badge-label">{label}</span></div>"#,
        mark = html_escape(&badge_mark),
        label = html_escape(&badge_label),
    );

    // Encoded as a data URI so Chromium renders it without an extra fetch.
    // One <pattern> repeats across the full body. Stroke colour stays cool
    // grey so the grid doesn't compete with the brand accent.
    let grid_svg = "data:image/svg+xml;utf8,\
        %3Csvg%20xmlns%3D%27http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%27%20width%3D%2760%27%20height%3D%2760%27%3E\
        %3Cpath%20d%3D%27M%2060%200%20L%200%200%200%2060%27%20fill%3D%27none%27%20stroke%3D%27%23161b22%27%20stroke-width%3D%271%27%2F%3E%3C%2Fsvg%3E";

    let css = format!(
        "*{{margin:0;padding:0;box-sizing:border-box}}\
         body{{width:{width}px;height:{height}px;background:#0d1117;\
         background-image:url(\"{grid_svg}\");background-size:60px 60px;\
         font-family:'Inter','Segoe UI',system-ui,-apple-system,sans-serif;\
         color:#e6edf3;position:relative;overflow:hidden;padding:{pad}px;\
         display:flex;flex-direction:column;justify-content:flex-start}}\
         .badge{{display:inline-flex;align-items:center;gap:8px;\
         padding:8px 16px;border-radius:999px;border:1px solid;\
         font-family:'JetBrains Mono','SF Mono','Fira Code',Consolas,monospace;\
         font-size:{badge_size}px;font-weight:600;letter-spacing:.04em;\
         margin-bottom:{pad}px;width:max-content;max-width:80%}}\
         .badge-mark{{font-size:1em}}\
         .counter{{position:absolute;top:{pad}px;right:{pad}px;\
         font-family:'JetBrains Mono','SF Mono','Fira Code',Consolas,monospace;\
         font-size:{chrome_size}px;color:#8b949e;letter-spacing:.12em}}\
         .stamp{{position:absolute;bottom:{pad}px;right:{pad}px;\
         font-family:'JetBrains Mono','SF Mono','Fira Code',Consolas,monospace;\
         font-size:{chrome_size}px;color:{brand_color};opacity:.85;letter-spacing:.06em}}\
         .stamp-prompt{{color:{brand_color};opacity:.7;margin-right:6px}}\
         .title{{font-size:{title_size}px;font-weight:800;color:#fff;\
         line-height:1.08;letter-spacing:-0.02em;max-width:90%}}\
         .accent{{width:{accent_width}px;height:{accent_height}px;\
         background:{brand_color};border-radius:3px;margin:32px 0 28px}}\
         .body{{font-size:{body_size}px;color:#c9d1d9;line-height:1.55;\
         max-width:88%}}",
    );

    let mut html = String::with_capacity(3800);
    html.push_str(r#"<!DOCTYPE html><html lang="fr"><head><meta charset="UTF-8"><style>"#);
    html.push_str(&css);
    html.push_str("</style></head><body>");
    html.push_str(&format!(
        r#"<div class="counter">{:02} / {:02}</div>"#,
        slide.index, slide.total
    ));
    html.push_str(&badge_html);
    html.push_str(&format!(
        r#"<div class="title">{}</div>"#,
        html_escape(&slide.title)
    ));
    html.push_str(r#"<div class="accent"></div>"#);
    html.push_str(&format!(
        r#"<div class="body">{}</div>"#,
        html_escape(&slide.body).replace('\n', "<br>"),
    ));
    html.push_str(&format!(
        r#"<div class="stamp"><span class="stamp-prompt">&gt;_</span>@{handle_escaped}</div>"#
    ));
    html.push_str("</body></html>");
    html
}

/// Tiny helper that maps the slide's position to a section label used in the
/// top-left badge when the AI didn't tag the slide with a role.
fn slide_label(index: u8, total: u8) -> &'static str {
    if index == 1 {
        "intro"
    } else if index == total {
        "à toi"
    } else {
        "lis-moi"
    }
}

/// Visual metadata for a role-tagged slide. Color is the badge accent,
/// label the human-readable chip text, mark the unicode prefix used when
/// the AI doesn't supply an emoji.
#[derive(Default, Debug, Clone, Copy)]
struct RoleMeta {
    color: Option<&'static str>,
    label: Option<&'static str>,
    mark: Option<&'static str>,
}

/// Map a normalised role string to its visual metadata. Unknown / missing
/// roles return an empty `RoleMeta`, which the caller treats as "fall back
/// to the index-derived label and brand color".
fn role_meta_for(role: Option<&str>) -> RoleMeta {
    match role.unwrap_or("") {
        "hero" => RoleMeta {
            color: None, // brand color
            label: Some("hello"),
            mark: Some(">_"),
        },
        "problem" => RoleMeta {
            color: Some("#ff6b6b"),
            label: Some("le problème"),
            mark: Some("◆"),
        },
        "approach" => RoleMeta {
            color: None, // brand color — solution stays on-brand
            label: Some("notre approche"),
            mark: Some("✦"),
        },
        "tech" => RoleMeta {
            color: Some("#60a5fa"),
            label: Some("sous le capot"),
            mark: Some("⌗"),
        },
        "change" => RoleMeta {
            color: Some("#fbbf24"),
            label: Some("ce qui change"),
            mark: Some("◇"),
        },
        "moment" => RoleMeta {
            color: Some("#c084fc"),
            label: Some("un exemple"),
            mark: Some("◈"),
        },
        "cta" => RoleMeta {
            color: None, // brand color — final CTA on-brand
            label: Some("à toi"),
            mark: Some("→"),
        },
        _ => RoleMeta::default(),
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Render caption + hashtags to PNG at given dimensions. Returns base64 data URL.
#[tauri::command]
pub async fn render_post_image(
    caption: String,
    hashtags: Vec<String>,
    width: Option<u32>,
    height: Option<u32>,
    handle: Option<String>,
    brand_color: Option<String>,
) -> Result<String, String> {
    let w = width.unwrap_or(1080);
    let h = height.unwrap_or(1080);
    let brand = Brand::resolve(handle.as_deref(), brand_color.as_deref());
    render_to_base64(&build_post_html(&caption, &hashtags, w, h, &brand), w, h).await
}

/// Render a code snippet card to PNG at given dimensions. Returns base64 data URL.
#[tauri::command]
pub async fn render_code_image(
    code: String,
    language: String,
    filename: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    handle: Option<String>,
    brand_color: Option<String>,
) -> Result<String, String> {
    let w = width.unwrap_or(1080);
    let h = height.unwrap_or(1080);
    let brand = Brand::resolve(handle.as_deref(), brand_color.as_deref());
    render_to_base64(
        &build_code_html(&code, &language, filename.as_deref(), w, h, &brand),
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
    handle: Option<String>,
    brand_color: Option<String>,
) -> Result<String, String> {
    let w = width.unwrap_or(1080);
    let h = height.unwrap_or(1080);
    let brand = Brand::resolve(handle.as_deref(), brand_color.as_deref());
    render_to_base64(
        &build_terminal_html(&command, output.as_deref(), w, h, &brand),
        w,
        h,
    )
    .await
}

/// Render each carousel slide to PNG. Returns Vec of base64 data URLs (same order as input).
///
/// Accepts an explicit canvas size so the renderer follows the format the
/// user picks in the Composer. Defaulting to the IG 4:5 portrait (1080×1350)
/// gives more vertical real-estate for the typography-heavy templates.
#[tauri::command]
pub async fn render_carousel_slides(
    slides: Vec<CarouselSlide>,
    handle: Option<String>,
    brand_color: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<Vec<String>, String> {
    let brand = Brand::resolve(handle.as_deref(), brand_color.as_deref());
    let w = width.unwrap_or(1080);
    let h = height.unwrap_or(1350);
    let mut images = Vec::with_capacity(slides.len());
    for slide in &slides {
        let data_url =
            render_to_base64(&build_carousel_slide_html(slide, w, h, &brand), w, h).await?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brand_resolve_uses_defaults_when_inputs_empty() {
        let b = Brand::resolve(None, None);
        assert_eq!(b.handle, DEFAULT_HANDLE);
        assert_eq!(b.brand_color, DEFAULT_BRAND_COLOR);

        let b = Brand::resolve(Some(""), Some(""));
        assert_eq!(b.handle, DEFAULT_HANDLE);
        assert_eq!(b.brand_color, DEFAULT_BRAND_COLOR);

        let b = Brand::resolve(Some("   "), Some("   "));
        assert_eq!(b.handle, DEFAULT_HANDLE);
        assert_eq!(b.brand_color, DEFAULT_BRAND_COLOR);
    }

    #[test]
    fn brand_resolve_strips_leading_at_sign() {
        let b = Brand::resolve(Some("@myhandle"), None);
        assert_eq!(b.handle, "myhandle");
    }

    #[test]
    fn brand_resolve_falls_back_to_default_for_bare_at_sign() {
        // A handle of just "@" (or "@   ") must NOT produce an empty handle —
        // that would render the templates as "@ — zsh" with nothing after the @.
        for handle in ["@", "@   ", "  @  ", "@@"] {
            let b = Brand::resolve(Some(handle), None);
            assert_eq!(
                b.handle, DEFAULT_HANDLE,
                "input {handle:?} must fall back to default"
            );
        }
    }

    #[test]
    fn brand_resolve_passes_custom_color_through() {
        let b = Brand::resolve(None, Some("#ff00aa"));
        assert_eq!(b.brand_color, "#ff00aa");
    }

    #[test]
    fn post_html_uses_provided_handle_and_color() {
        let brand = Brand::resolve(Some("ankora"), Some("#0d9488"));
        let html = build_post_html("hello", &[], 1080, 1080, &brand);
        assert!(html.contains("@ankora"), "must render the provided handle");
        assert!(
            html.contains("#0d9488"),
            "must render the provided brand color"
        );
        assert!(
            !html.contains("@yourbrand"),
            "must not leak the default handle when a custom one is provided"
        );
    }

    /// Test helper — builds a CarouselSlide with sane defaults so the
    /// growing list of fields doesn't make every test case noisy.
    fn make_slide(index: u8, total: u8) -> CarouselSlide {
        CarouselSlide {
            index,
            total,
            emoji: "x".into(),
            title: "Test".into(),
            body: "Body".into(),
            role: None,
        }
    }

    #[test]
    fn carousel_html_uses_provided_handle_and_color() {
        let brand = Brand::resolve(Some("ankora"), Some("#0d9488"));
        let slide = CarouselSlide {
            emoji: "🚀".into(),
            title: "Title".into(),
            body: "Body".into(),
            ..make_slide(1, 3)
        };
        let html = build_carousel_slide_html(&slide, 1080, 1350, &brand);
        assert!(html.contains("@ankora"));
        assert!(html.contains("#0d9488"));
    }

    #[test]
    fn carousel_html_renders_monospace_chrome_and_grid() {
        let brand = Brand::resolve(Some("ankora"), Some("#3ddc84"));
        let slide = CarouselSlide {
            emoji: "✦".into(),
            title: "Hello".into(),
            body: "World".into(),
            ..make_slide(3, 7)
        };
        let html = build_carousel_slide_html(&slide, 1080, 1350, &brand);
        assert!(
            html.contains("03 / 07"),
            "counter must be zero-padded XX / YY"
        );
        assert!(
            html.contains("&gt;_"),
            "stamp must include the >_ shell prompt prefix"
        );
        assert!(
            html.contains("data:image/svg+xml"),
            "background grid must be inlined as SVG"
        );
        assert!(
            html.contains("JetBrains Mono"),
            "monospace stack must lead with JetBrains Mono"
        );
    }

    #[test]
    fn carousel_html_scales_with_canvas_dimensions() {
        let brand = Brand::resolve(Some("ankora"), Some("#3ddc84"));
        let slide = make_slide(1, 5);
        let portrait = build_carousel_slide_html(&slide, 1080, 1350, &brand);
        let square = build_carousel_slide_html(&slide, 1080, 1080, &brand);
        assert!(portrait.contains("width:1080px"));
        assert!(portrait.contains("height:1350px"));
        assert!(square.contains("width:1080px"));
        assert!(square.contains("height:1080px"));
    }

    #[test]
    fn slide_label_marks_intro_and_outro_distinctly() {
        assert_eq!(slide_label(1, 5), "intro");
        assert_eq!(slide_label(5, 5), "à toi");
        assert_eq!(slide_label(3, 5), "lis-moi");
        // Single-slide carousel: opening trumps closing — keep "intro" so
        // a degenerate 1-slide post still reads as a hook.
        assert_eq!(slide_label(1, 1), "intro");
    }

    #[test]
    fn role_meta_for_known_roles_returns_distinct_colors() {
        let problem = role_meta_for(Some("problem"));
        let tech = role_meta_for(Some("tech"));
        let change = role_meta_for(Some("change"));
        let moment = role_meta_for(Some("moment"));
        assert!(
            problem.color.is_some()
                && tech.color.is_some()
                && change.color.is_some()
                && moment.color.is_some(),
            "tagged roles must define a badge color"
        );
        assert_ne!(problem.color, tech.color, "each role gets its own accent");
        assert_ne!(tech.color, change.color);
        assert_ne!(change.color, moment.color);
    }

    #[test]
    fn role_meta_for_brand_aligned_roles_keeps_brand_color() {
        // hero / approach / cta intentionally inherit the brand color so the
        // post opens, climaxes, and closes on the brand's signature accent.
        for role in ["hero", "approach", "cta"] {
            let meta = role_meta_for(Some(role));
            assert!(
                meta.color.is_none(),
                "role {role} must inherit brand color (color = None)"
            );
        }
    }

    #[test]
    fn role_meta_for_unknown_role_falls_back_to_default() {
        let meta = role_meta_for(Some("does-not-exist"));
        assert!(meta.color.is_none() && meta.label.is_none() && meta.mark.is_none());
    }

    #[test]
    fn carousel_html_uses_role_label_when_provided() {
        let brand = Brand::resolve(Some("ankora"), Some("#3ddc84"));
        let slide = CarouselSlide {
            role: Some("problem".into()),
            ..make_slide(2, 5)
        };
        let html = build_carousel_slide_html(&slide, 1080, 1350, &brand);
        assert!(
            html.contains("le problème"),
            "role=problem must surface its human label in the badge"
        );
        assert!(
            html.contains("#ff6b6b"),
            "role=problem must paint its red accent"
        );
    }

    #[test]
    fn carousel_html_falls_back_to_index_label_without_role() {
        let brand = Brand::resolve(Some("ankora"), Some("#3ddc84"));
        let slide = make_slide(1, 5);
        let html = build_carousel_slide_html(&slide, 1080, 1350, &brand);
        assert!(
            html.contains("intro"),
            "untagged slide 1 must use the index-derived 'intro' label"
        );
    }

    #[test]
    fn templates_are_persona_agnostic() {
        let brand = Brand::resolve(Some("ankora"), Some("#0d9488"));
        let post = build_post_html("x", &[], 1080, 1080, &brand);
        let code = build_code_html("x", "rust", None, 1080, 1080, &brand);
        let term = build_terminal_html("ls", None, 1080, 1080, &brand);
        let slide = make_slide(1, 1);
        let car = build_carousel_slide_html(&slide, 1080, 1350, &brand);
        for html in [&post, &code, &term, &car] {
            assert!(
                !html.contains("@terminallearning"),
                "no template may carry the legacy hardcoded handle"
            );
        }
    }
}
