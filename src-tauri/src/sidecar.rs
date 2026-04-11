use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::time::{timeout, Duration};

#[cfg(windows)]
const PYTHON: &str = "python";
#[cfg(not(windows))]
const PYTHON: &str = "python3";

fn sidecar_script() -> std::path::PathBuf {
    let raw = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../sidecar/main.py");
    raw.canonicalize().unwrap_or(raw)
}

// ── AI generation request / response ─────────────────────────────────────────

#[derive(Serialize)]
pub struct SidecarRequest {
    pub action: String,
    pub provider: String,
    /// SECURITY: passed per-call, never stored or logged
    pub api_key: Option<String>,
    pub model: String,
    pub base_url: Option<String>,
    pub brief: String,
    pub network: String,
    pub system_prompt: String,
}

#[derive(Deserialize)]
pub struct SidecarData {
    pub caption: String,
    pub hashtags: Vec<String>,
}

#[derive(Deserialize)]
struct SidecarResponse {
    ok: bool,
    data: Option<SidecarData>,
    error: Option<String>,
}

// ── Render request / response ─────────────────────────────────────────────────

#[derive(Serialize)]
struct RenderRequest<'a> {
    action: &'static str,
    html: &'a str,
    output_path: &'a str,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
struct RenderData {
    path: String,
}

#[derive(Deserialize)]
struct RenderResponse {
    ok: bool,
    data: Option<RenderData>,
    error: Option<String>,
}

// ── Internal runner ───────────────────────────────────────────────────────────

async fn run_sidecar(json_input: String, timeout_secs: u64) -> Result<String, String> {
    let script = sidecar_script();

    timeout(Duration::from_secs(timeout_secs), async move {
        let mut child = tokio::process::Command::new(PYTHON)
            .arg(&script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Spawn Python sidecar: {e}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(format!("{json_input}\n").as_bytes())
                .await
                .map_err(|e| format!("Write to sidecar: {e}"))?;
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| format!("Sidecar wait: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Sidecar exited with error: {}", stderr.trim()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    })
    .await
    .map_err(|_| {
        format!(
            "Sidecar timeout ({timeout_secs}s) — is Python installed and in PATH?"
        )
    })?
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Low-level: send arbitrary JSON to the sidecar and get raw stdout back.
/// Used by warmup and other fire-and-forget callers.
pub async fn run_sidecar_raw(json_input: String, timeout_secs: u64) -> Result<String, String> {
    run_sidecar(json_input, timeout_secs).await
}

pub async fn call_sidecar(request: SidecarRequest) -> Result<SidecarData, String> {
    let json_input = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    let stdout = run_sidecar(json_input, 30).await?;

    let resp: SidecarResponse = serde_json::from_str(&stdout)
        .map_err(|e| format!("Parse sidecar output: {e} (got: {stdout})"))?;

    if resp.ok {
        resp.data.ok_or_else(|| "Sidecar ok but no data".to_string())
    } else {
        Err(resp.error.unwrap_or_else(|| "Unknown sidecar error".to_string()))
    }
}

pub async fn call_render_sidecar(
    html: &str,
    output_path: &str,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let req = RenderRequest {
        action: "render_html",
        html,
        output_path,
        width,
        height,
    };
    let json_input = serde_json::to_string(&req).map_err(|e| e.to_string())?;
    // Playwright browser launch can be slow — give 60 s
    let stdout = run_sidecar(json_input, 60).await?;

    let resp: RenderResponse = serde_json::from_str(&stdout)
        .map_err(|e| format!("Parse render sidecar output: {e} (got: {stdout})"))?;

    if resp.ok {
        resp.data
            .map(|d| d.path)
            .ok_or_else(|| "Render sidecar ok but no path".to_string())
    } else {
        Err(resp.error.unwrap_or_else(|| "Unknown render sidecar error".to_string()))
    }
}
