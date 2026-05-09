use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::io::AsyncWriteExt;
use tokio::time::{timeout, Duration};

/// Find a working Python interpreter once and cache it for the rest of
/// the process lifetime.
///
/// Two-pass probe to handle the Windows MS Store stub trap. On Windows 11
/// a user can have `python3` AND `python` both on PATH, with one
/// resolving to the Store stub (fake Python — runs `--version` but has
/// no real install or site-packages) and the other to a real install
/// with the sidecar packages.
///
/// Pass 1 — pick the interpreter that can `import openai, anthropic,
/// playwright`. That's necessarily the one with packages we need.
/// Order doesn't matter here: only one will satisfy.
///
/// Pass 2 — fallback when no interpreter has the packages yet (fresh
/// install). Falls back to `--version` and picks the first that
/// responds. The in-app `install_python_deps` command (Settings → IA)
/// then bootstraps the packages into that interpreter.
///
/// 2026-05-08 incident: v0.3.4 cached `python3` from `--version` even
/// though it was the Store stub on the owner's machine, then sidecar
/// imports failed with `ModuleNotFoundError: No module named 'openai'`
/// even after the user pip-installed against `python` (real install).
/// Pass 1 catches the right interpreter the FIRST time.
fn python_executable() -> &'static str {
    static CACHED: OnceLock<&'static str> = OnceLock::new();
    CACHED.get_or_init(|| {
        let candidates = ["python", "python3", "py"];

        // Pass 1 — interpreter with the sidecar packages already installed.
        for candidate in candidates {
            let ok = silent_std_command(candidate)
                .arg("-c")
                .arg("import openai, anthropic, playwright")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                log::info!("sidecar: using Python `{candidate}` (packages OK)");
                return candidate;
            }
        }

        // Pass 2 — packages not yet installed. Pick any working Python so
        // the in-app installer has a target. MS Store stub fails `--version`
        // too on most Windows configs, so we still skip it.
        for candidate in candidates {
            let ok = silent_std_command(candidate)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                log::warn!(
                    "sidecar: using Python `{candidate}` (packages MISSING — run \
                     install_python_deps from Settings → IA)"
                );
                return candidate;
            }
        }

        log::error!("sidecar: no working Python found in PATH (python, python3, py all failed)");
        "python3"
    })
}

/// Windows CreateProcess flag that suppresses the console window when a GUI app
/// spawns a console subprocess. Without it, every sidecar call flashes a black
/// terminal for ~50ms — visually disruptive for the end user.
/// Source: <https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags>
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a `std::process::Command` that won't flash a terminal window on
/// Windows. No-op on Unix where there's no console to hide. Used by every
/// Python probe and pip helper across the codebase — the helper exists so
/// adding a new spawn point doesn't accidentally regress the no-flash UX.
pub(crate) fn silent_std_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// `tokio::process::Command` variant of [`silent_std_command`]. Tokio's
/// Windows-only `creation_flags` method does not need the `CommandExt` import.
pub(crate) fn silent_tokio_command(
    program: impl AsRef<std::ffi::OsStr>,
) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Tauri 2 mangles `bundle.resources` paths starting with `..` by
/// replacing the `..` segment with this literal in the deployed bundle.
/// Centralised so the convention is documented in one place and the
/// repeated string doesn't drift between platform-specific helpers.
const TAURI_UP_DIR_MARKER: &str = "_up_";

/// Build a candidate path of the form `{root}/[segments...]/sidecar/main.py`.
/// Per-component `.join()` chain so the path separator is always OS-native
/// regardless of how the segments were declared in source.
fn make_candidate(root: &std::path::Path, segments: &[&str]) -> std::path::PathBuf {
    let mut path = root.to_path_buf();
    for s in segments {
        path = path.join(s);
    }
    path.join("sidecar").join("main.py")
}

/// NSIS .exe installer layout (Windows-only). Files land DIRECTLY under
/// the install root with the `_up_` mangling — confirmed on a fresh
/// v0.3.2 install at `C:\Program Files\getpostcraft\_up_\sidecar\main.py`.
/// The Windows MSI uses a different layout (`resources/_up_/...`) handled
/// by `bundled_resource_candidates`.
#[cfg(target_os = "windows")]
fn nsis_candidates(exe_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    vec![
        make_candidate(exe_dir, &[TAURI_UP_DIR_MARKER]),
        make_candidate(exe_dir, &[]),
    ]
}

/// MSI (Windows) and Linux installer layout: resources live under
/// `resources/` with the `_up_` prefix for `..`-rooted source paths.
/// Always checked — both Windows MSI and every Linux installer use it.
fn bundled_resource_candidates(exe_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    vec![
        make_candidate(exe_dir, &["resources", TAURI_UP_DIR_MARKER]),
        make_candidate(exe_dir, &["resources"]),
    ]
}

/// macOS .app bundle layout. The binary sits in `Contents/MacOS/` and
/// resources in `Contents/Resources/`, so we walk up from the executable.
#[cfg(target_os = "macos")]
fn macos_app_candidates(exe_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    vec![
        make_candidate(exe_dir, &["..", "Resources", TAURI_UP_DIR_MARKER]),
        make_candidate(exe_dir, &["..", "Resources"]),
    ]
}

/// Dev mode: `cargo run` and `npm run tauri dev` resolve
/// `CARGO_MANIFEST_DIR` to the actual project root. Last-resort
/// candidate so a CI-built binary running on a user machine, where this
/// path points at the GitHub Actions runner filesystem, never silently
/// becomes the answer.
fn dev_mode_candidate() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sidecar/main.py")
}

/// Locate `sidecar/main.py` at runtime.
///
/// `env!("CARGO_MANIFEST_DIR")` resolves at *compile* time. In dev mode
/// that's the project root and works fine. In CI-built binaries it
/// expands to the GitHub Actions runner path (e.g.
/// `D:\a\getpostcraft\getpostcraft\src-tauri`) which obviously doesn't
/// exist on the user's machine — we shipped this bug in v0.3.x and the
/// owner hit it on `analyze_url_visual` (2026-05-08).
///
/// Strategy: try a list of candidates and return the first that exists,
/// production layouts first. If every check misses we fall back to the
/// FIRST production candidate so the resulting "No such file" error
/// mentions an install-shaped path rather than a CI-runner one.
///
/// Production layouts (per-platform helpers above):
///   - Windows NSIS .exe: `{install}/_up_/sidecar/main.py`
///   - Windows MSI:       `{install}/resources/_up_/sidecar/main.py`
///   - Linux installers:  `{install}/resources/_up_/sidecar/main.py`
///   - macOS .app:        `{App}/Contents/Resources/_up_/sidecar/main.py`
fn sidecar_script() -> std::path::PathBuf {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            #[cfg(target_os = "windows")]
            candidates.extend(nsis_candidates(dir));

            candidates.extend(bundled_resource_candidates(dir));

            #[cfg(target_os = "macos")]
            candidates.extend(macos_app_candidates(dir));
        }
    }

    candidates.push(dev_mode_candidate());

    // First existing wins. The fallback uses `.first()` instead of indexing
    // so an unexpected empty `candidates` (theoretically possible if both
    // `current_exe()` fails AND `dev_mode_candidate()` somehow panics
    // before push) doesn't add a startup panic on top of whatever already
    // went wrong. The empty case can never happen today — `dev_mode_candidate`
    // is unconditionally pushed — but defending the unwrap costs zero runtime
    // and removes one source of crashes-with-no-log.
    candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .or_else(|| candidates.first().cloned())
        .unwrap_or_else(|| std::path::PathBuf::from("sidecar/main.py"))
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
        let mut command = silent_tokio_command(python_executable());
        command
            .arg(&script)
            // Force UTF-8 I/O on all platforms; 'replace' keeps us alive
            // even if something slips through our sanitisation layer.
            .env("PYTHONIOENCODING", "utf-8:replace")
            .env("PYTHONUTF8", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

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
