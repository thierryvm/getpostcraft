use crate::sidecar::{silent_std_command, silent_tokio_command};
/// Install / verify the Python packages the sidecar imports.
///
/// ## Why this exists
///
/// We ship the sidecar as raw `.py` files (per `tauri.conf.json::bundle.resources`),
/// not as a PyInstaller binary. The .py files import `openai`, `anthropic`,
/// `playwright`, `pillow` — packages that live in the user's *system Python*
/// install. A user running v0.3.4 with a fresh Python install hits
/// `ModuleNotFoundError: No module named 'openai'` on every AI call until
/// they manually `pip install -r requirements.txt`.
///
/// PR-Q4 (v0.3.5) ships an in-app button that runs the install for them.
/// V0.4 will replace the whole picture with a PyInstaller-bundled sidecar
/// binary, but until then this is the bridge that unblocks fresh users
/// without making them open a terminal.
///
/// ## Security
///
/// `pip install` reads `requirements.txt` from the bundled resource dir —
/// we resolve the path through the same `sidecar_script` candidate list
/// the sidecar itself uses, so no user input flows into the install
/// command. The `--no-warn-script-location` flag silences the
/// "this script is not on PATH" warning that confuses users.
use std::process::Stdio;
use tokio::time::{timeout, Duration};

/// Locate the `requirements.txt` shipped alongside the sidecar `.py` files.
/// Mirrors the candidate-list strategy from `sidecar_script` so the path
/// resolves correctly across NSIS / MSI / .app / dev-mode layouts without
/// duplicating the logic.
fn requirements_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidates: Vec<std::path::PathBuf> = vec![
        // NSIS layout (Windows .exe installer).
        dir.join("_up_").join("sidecar").join("requirements.txt"),
        dir.join("sidecar").join("requirements.txt"),
        // MSI / Linux installers.
        dir.join("resources")
            .join("_up_")
            .join("sidecar")
            .join("requirements.txt"),
        dir.join("resources")
            .join("sidecar")
            .join("requirements.txt"),
        // macOS .app bundles.
        dir.join("..")
            .join("Resources")
            .join("_up_")
            .join("sidecar")
            .join("requirements.txt"),
        dir.join("..")
            .join("Resources")
            .join("sidecar")
            .join("requirements.txt"),
        // Dev-mode fallback.
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sidecar/requirements.txt"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

/// Same probe the sidecar uses — keeps the `python3 → python → py` order
/// so we hit the same interpreter the sidecar will. Duplicating the
/// helper here (rather than depending on `crate::sidecar::python_executable`)
/// because that function is private; both could be unified in V0.4.
fn python_executable() -> &'static str {
    use std::sync::OnceLock;
    static CACHED: OnceLock<&'static str> = OnceLock::new();
    CACHED.get_or_init(|| {
        for candidate in ["python3", "python", "py"] {
            let ok = silent_std_command(candidate)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                return candidate;
            }
        }
        "python3"
    })
}

/// Quick check: are `openai`, `anthropic`, and `playwright` importable?
/// Returns the missing module names — empty Vec means everything's there.
/// The renderer uses this to show / hide the install banner in Settings.
#[tauri::command]
pub async fn check_python_deps() -> Result<Vec<String>, String> {
    let py = python_executable();
    let probe = "import sys, importlib\n\
                 missing = []\n\
                 for m in ['openai', 'anthropic', 'playwright']:\n\
                 \timport importlib.util as u\n\
                 \tif u.find_spec(m) is None:\n\
                 \t\tmissing.append(m)\n\
                 print(','.join(missing))\n";

    let output = silent_tokio_command(py)
        .arg("-c")
        .arg(probe)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Spawn Python probe: {e}"))?;

    if !output.status.success() {
        // Python itself is missing or broken — surface that distinctly so
        // the UI can prompt to install Python rather than asking the user
        // to install packages that have no host.
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "Python introuvable ou cassé : {stderr} — installe Python ≥ 3.11 depuis python.org"
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        Ok(Vec::new())
    } else {
        Ok(stdout.split(',').map(str::to_string).collect())
    }
}

/// Run `python -m pip install -r requirements.txt` against the bundled
/// `requirements.txt`. Long-running (30 s on a fast connection, several
/// minutes on a slow one for Playwright). The renderer shows a busy
/// indicator and disables the button while this runs.
///
/// Returns an Ok / Err with the captured stderr on failure so the user
/// can see *why* (no internet, pip out of date, permission denied, …).
#[tauri::command]
pub async fn install_python_deps() -> Result<String, String> {
    let req = requirements_path()
        .ok_or_else(|| "requirements.txt introuvable dans le bundle".to_string())?;
    let py = python_executable();

    log::info!(
        "python_deps: running `{py} -m pip install -r {}`",
        req.display()
    );

    // Cap at 10 minutes — Playwright's wheels are large but not THAT
    // large, even on a slow connection.
    let result = timeout(
        Duration::from_secs(600),
        silent_tokio_command(py)
            .arg("-m")
            .arg("pip")
            .arg("install")
            .arg("--user")
            .arg("--no-warn-script-location")
            .arg("-r")
            .arg(&req)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| "pip install a dépassé 10 minutes — vérifie ta connexion".to_string())?
    .map_err(|e| format!("Spawn pip: {e}"))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
        log::warn!("python_deps: pip install failed — {stderr}");
        return Err(format!("pip install a échoué : {stderr}"));
    }

    log::info!("python_deps: install succeeded");

    // Last step: also pull the Chromium browser Playwright needs to render
    // HTML → PNG. Without it the render flow fails on the first call with
    // an opaque "Browser is not installed" error. This is fast (browser
    // already cached if anything Playwright-related ran before) and idempotent.
    let _ = silent_tokio_command(py)
        .arg("-m")
        .arg("playwright")
        .arg("install")
        .arg("chromium")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    Ok("Dépendances Python installées avec succès.".to_string())
}
