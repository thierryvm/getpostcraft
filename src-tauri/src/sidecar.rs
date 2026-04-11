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

// ── Request / Response ────────────────────────────────────────────────────────

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

// ── Public API ────────────────────────────────────────────────────────────────

pub async fn call_sidecar(request: SidecarRequest) -> Result<SidecarData, String> {
    let json_input = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    let script = sidecar_script();

    timeout(Duration::from_secs(30), async move {
        let mut child = tokio::process::Command::new(PYTHON)
            .arg(&script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Spawn Python sidecar: {e}"))?;

        // Write request then close stdin so Python's readline() returns
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

        let stdout = String::from_utf8_lossy(&output.stdout);
        let resp: SidecarResponse = serde_json::from_str(stdout.trim())
            .map_err(|e| format!("Parse sidecar output: {e} (got: {})", stdout.trim()))?;

        if resp.ok {
            resp.data.ok_or_else(|| "Sidecar ok but no data".to_string())
        } else {
            Err(resp.error.unwrap_or_else(|| "Unknown sidecar error".to_string()))
        }
    })
    .await
    .map_err(|_| "Sidecar timeout (30s) — is Python installed and in PATH?".to_string())?
}
