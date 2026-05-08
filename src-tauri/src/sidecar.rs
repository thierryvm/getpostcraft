use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::io::AsyncWriteExt;
use tokio::time::{timeout, Duration};

/// Find a working Python interpreter once and cache it for the rest of
/// the process lifetime. Tries `python3`, then `python`, then `py` (the
/// Windows launcher). The order matters on Windows 11: `python` without
/// any install resolves to the Microsoft Store *stub*, which prints a
/// "open Store" message and exits non-zero — every sidecar call would
/// fail silently with no clue why. `python3` and `py` are not stub-shadowed.
fn python_executable() -> &'static str {
    static CACHED: OnceLock<&'static str> = OnceLock::new();
    CACHED.get_or_init(|| {
        for candidate in ["python3", "python", "py"] {
            // `--version` is a cheap, side-effect-free check supported by
            // every Python ≥ 2. The MS Store stub returns 9009 (or just
            // prints the install prompt to stderr) — `success()` is false.
            let ok = std::process::Command::new(candidate)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                log::info!("sidecar: using Python interpreter `{candidate}`");
                return candidate;
            }
        }
        log::warn!("sidecar: no working Python found in PATH (python3, python, py all failed)");
        // Fall back to "python3" so the eventual error message names a
        // reasonable command for diagnostic purposes. The user-facing
        // error path already mentions "is Python installed and in PATH?".
        "python3"
    })
}

/// Windows CreateProcess flag that suppresses the console window when a GUI app
/// spawns a console subprocess. Without it, every sidecar call flashes a black
/// terminal for ~50ms — visually disruptive for the end user.
/// Source: <https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags>
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Locate `sidecar/main.py` at runtime.
///
/// `env!("CARGO_MANIFEST_DIR")` resolves at *compile* time. In dev mode
/// that's the project root and works fine. In CI-built binaries it
/// expands to the GitHub Actions runner path (e.g.
/// `D:\a\getpostcraft\getpostcraft\src-tauri`) which obviously doesn't
/// exist on the user's machine — we shipped this bug in v0.3.x and the
/// owner hit it on `analyze_url_visual` (2026-05-08).
///
/// Strategy: try a list of candidates and return the first that exists.
/// The dev path stays first so `npm run tauri dev` keeps working without
/// re-bundling resources every change.
///
/// Production layout (Tauri 2 puts `bundle.resources` under
/// `{install}/resources/`, prefixing relative `..` paths with `_up_`):
///   - Windows MSI/NSIS:  `{install}/resources/_up_/sidecar/main.py`
///   - macOS .app:        `{App}/Contents/Resources/_up_/sidecar/main.py`
///   - Linux AppImage:    `{install}/resources/_up_/sidecar/main.py`
fn sidecar_script() -> std::path::PathBuf {
    use std::path::PathBuf;

    let mut candidates: Vec<PathBuf> = Vec::new();

    // Production paths first — if we're running from an installed binary
    // these are the shapes the Tauri bundle produces. Putting them before
    // the dev path means the CI-baked CARGO_MANIFEST_DIR (which points to
    // the runner's filesystem and never exists on a user machine) is the
    // ABSOLUTE last resort, not the silent default. The earlier order had
    // a subtle hole: if every prod candidate failed `.exists()` we fell
    // back to candidates[0] which was the broken dev path — same class
    // of bug as v0.3.x. Flipping the order means a misconfigured bundle
    // surfaces as a clear "main.py not found at <prod path>" error
    // instead of a confusing CI-runner-path message.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Per-component `.join()` calls instead of a single string —
            // PathBuf::join with embedded `/` would still work on Windows
            // (Rust converts) but joining one segment at a time is more
            // idiomatic and removes any "is the separator right?" doubt.

            // NSIS .exe installer (Tauri 2 default on Windows): files
            // land DIRECTLY under the install root with the `_up_`
            // mangling. Confirmed on a fresh v0.3.2 install at
            // `C:\Program Files\getpostcraft\_up_\sidecar\main.py`. This
            // candidate was missing in v0.3.2 — every AI command failed
            // because we only looked under `resources/` like the .msi.
            // Gated on Windows: the `_up_` directly-under-exe layout
            // only happens with NSIS, never on other platforms.
            #[cfg(target_os = "windows")]
            {
                candidates.push(dir.join("_up_").join("sidecar").join("main.py"));
                candidates.push(dir.join("sidecar").join("main.py"));
            }

            // MSI installer + Linux AppImage / .deb / .rpm: resources
            // live under `resources/`, again with the `_up_` prefix for
            // `..`-rooted source paths.
            candidates.push(
                dir.join("resources")
                    .join("_up_")
                    .join("sidecar")
                    .join("main.py"),
            );
            candidates.push(dir.join("resources").join("sidecar").join("main.py"));

            // macOS .app bundles: binary in Contents/MacOS/, resources
            // in Contents/Resources/. Gated to macOS so we don't bother
            // checking these on other platforms.
            #[cfg(target_os = "macos")]
            {
                candidates.push(
                    dir.join("..")
                        .join("Resources")
                        .join("_up_")
                        .join("sidecar")
                        .join("main.py"),
                );
                candidates.push(
                    dir.join("..")
                        .join("Resources")
                        .join("sidecar")
                        .join("main.py"),
                );
            }
        }
    }

    // Dev mode last — alongside the source tree, only used when running
    // via `npm run tauri dev` or `cargo run` from the project root.
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sidecar/main.py"));

    // First existing wins. If absolutely none exist (broken install),
    // return the FIRST production candidate so the eventual "No such
    // file" error mentions a path the user might recognise as install-
    // shaped rather than a CI runner path.
    candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone())
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

#[derive(Deserialize, Default, Clone, Copy)]
pub struct TokenUsage {
    #[serde(default)]
    pub input_tokens: i64,
    #[serde(default)]
    pub output_tokens: i64,
}

#[derive(Deserialize)]
pub struct SidecarData {
    pub caption: String,
    pub hashtags: Vec<String>,
    /// Populated by the sidecar's `generate_content` action so the Rust
    /// side can persist token counts into `ai_usage` (PR cost-tracker).
    /// None for older sidecar builds that don't yet emit usage.
    #[serde(default)]
    pub usage: Option<TokenUsage>,
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
        let mut command = tokio::process::Command::new(python_executable());
        command
            .arg(&script)
            // Force UTF-8 I/O on all platforms; 'replace' keeps us alive
            // even if something slips through our sanitisation layer.
            .env("PYTHONIOENCODING", "utf-8:replace")
            .env("PYTHONUTF8", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Suppress the flashing terminal window on Windows when the Tauri GUI
        // spawns a console subprocess. No-op on Unix — there is no console to
        // hide when the parent has none.
        // tokio::process::Command exposes `creation_flags` directly on Windows.
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);

        let mut child = command
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
    .map_err(|_| format!("Sidecar timeout ({timeout_secs}s) — is Python installed and in PATH?"))?
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
        resp.data
            .ok_or_else(|| "Sidecar ok but no data".to_string())
    } else {
        Err(resp
            .error
            .unwrap_or_else(|| "Unknown sidecar error".to_string()))
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
        Err(resp
            .error
            .unwrap_or_else(|| "Unknown render sidecar error".to_string()))
    }
}
