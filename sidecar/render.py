"""Render an HTML string to a PNG file using Playwright Chromium.

The page is set to exactly width × height pixels (default 1080×1080 for Instagram).
"""
from __future__ import annotations

from pathlib import Path


def render_html_to_png(
    html: str,
    output_path: str,
    width: int = 1080,
    height: int = 1080,
) -> str:
    """Render *html* to *output_path* at *width* × *height* px.

    Returns the resolved output path on success.
    Raises an exception on failure.
    """
    from playwright.sync_api import sync_playwright  # lazy import

    out = Path(output_path)
    out.parent.mkdir(parents=True, exist_ok=True)

    with sync_playwright() as p:
        browser = p.chromium.launch(
            args=["--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu"]
        )
        try:
            page = browser.new_page(viewport={"width": width, "height": height})
            page.set_content(html, wait_until="domcontentloaded")
            # Clip to exact dimensions so the screenshot is always width×height
            page.screenshot(
                path=str(out),
                clip={"x": 0, "y": 0, "width": width, "height": height},
            )
        finally:
            browser.close()

    return str(out.resolve())
