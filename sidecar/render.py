"""Render an HTML string to a PNG file using Playwright Chromium.

The page is set to exactly width × height pixels (default 1080×1080 for Instagram).
The HTML is written to a temp file with explicit UTF-8 encoding so that
accented characters are rendered correctly by Chromium.
"""
from __future__ import annotations

import os
import tempfile
from pathlib import Path


def render_html_to_png(
    html: str,
    output_path: str,
    width: int = 1080,
    height: int = 1080,
) -> str:
    """Render *html* to *output_path* at *width* × *height* px.

    Writes the HTML to a UTF-8 temp file so Chromium reads it with correct
    encoding, then screenshots it. Returns the resolved output path.
    """
    from playwright.sync_api import sync_playwright  # lazy import

    out = Path(output_path)
    out.parent.mkdir(parents=True, exist_ok=True)

    # Write HTML to a temp file with explicit UTF-8 encoding.
    # Using set_content() directly does NOT honour <meta charset>; goto(file://) does.
    tmp_fd, tmp_path = tempfile.mkstemp(suffix=".html")
    try:
        # Sanitize lone surrogates that some AI models produce (e.g. \udc90).
        # Re-encode through UTF-8 with replacement so Chromium gets clean HTML.
        safe_html = html.encode("utf-8", errors="replace").decode("utf-8")
        with os.fdopen(tmp_fd, "w", encoding="utf-8") as fh:
            fh.write(safe_html)

        # Convert to file:// URL (forward slashes required on Windows too)
        file_url = "file:///" + tmp_path.replace("\\", "/")

        with sync_playwright() as p:
            browser = p.chromium.launch(
                args=["--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu"]
            )
            try:
                page = browser.new_page(viewport={"width": width, "height": height})
                page.goto(file_url, wait_until="domcontentloaded")
                page.screenshot(
                    path=str(out),
                    clip={"x": 0, "y": 0, "width": width, "height": height},
                )
            finally:
                browser.close()
    finally:
        try:
            os.unlink(tmp_path)
        except OSError:
            pass

    return str(out.resolve())
