"""Scrape a URL and extract meaningful text for use as a post brief.

Two modes:
- `scrape_url` (default, fast) — urllib + HTML parser. Works for static pages.
  Fails on SPAs that render content client-side (returns just <title>).
- `scrape_url_rendered` — Playwright Chromium. Renders JS so SPAs come through.
  ~3-5 seconds per call (browser launch + page load), used by the website
  analyzer flow when richer content is needed.
"""
from __future__ import annotations

import re
from urllib.request import Request, urlopen
from html.parser import HTMLParser


# Tags whose text content we skip entirely
_SKIP_TAGS = {
    "script", "style", "noscript", "nav", "footer", "header",
    "aside", "svg", "meta", "link", "head",
}

# Tags that add a line break when encountered
_BLOCK_TAGS = {
    "p", "div", "section", "article", "h1", "h2", "h3",
    "h4", "h5", "h6", "li", "br", "tr", "blockquote", "pre",
}


class _TextExtractor(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self._skip_depth = 0
        self._parts: list[str] = []

    def handle_starttag(self, tag: str, attrs: list) -> None:
        if tag in _SKIP_TAGS:
            self._skip_depth += 1
        if tag in _BLOCK_TAGS and self._skip_depth == 0:
            self._parts.append("\n")

    def handle_endtag(self, tag: str) -> None:
        if tag in _SKIP_TAGS:
            self._skip_depth = max(0, self._skip_depth - 1)
        if tag in _BLOCK_TAGS and self._skip_depth == 0:
            self._parts.append("\n")

    def handle_data(self, data: str) -> None:
        if self._skip_depth == 0:
            self._parts.append(data)

    def get_text(self) -> str:
        raw = "".join(self._parts)
        # Collapse whitespace runs, keeping single newlines as paragraph separators
        lines = [re.sub(r"[ \t]+", " ", ln).strip() for ln in raw.splitlines()]
        # Remove empty line runs
        result: list[str] = []
        prev_blank = False
        for ln in lines:
            if ln:
                result.append(ln)
                prev_blank = False
            elif not prev_blank:
                result.append("")
                prev_blank = True
        return "\n".join(result).strip()


def scrape_url(url: str, max_chars: int = 3000) -> str:
    """Fetch *url* and return extracted text truncated to *max_chars*.

    Uses only the Python standard library — no external dependencies.
    Raises on HTTP errors or non-HTML content.

    Limitation: client-rendered SPAs only expose <title> and shell markup.
    For those, callers should use `scrape_url_rendered` instead.
    """
    req = Request(
        url,
        headers={
            "User-Agent": (
                "Mozilla/5.0 (compatible; Getpostcraft/0.1; "
                "+https://getpostcraft.app)"
            )
        },
    )
    with urlopen(req, timeout=15) as resp:
        content_type = resp.headers.get("Content-Type", "")
        if "html" not in content_type and "text" not in content_type:
            raise ValueError(
                f"Content-Type '{content_type}' is not HTML — cannot extract text."
            )
        raw_bytes = resp.read(512_000)  # cap at 500 KB

    # Detect encoding
    charset = "utf-8"
    if "charset=" in content_type:
        charset = content_type.split("charset=")[-1].split(";")[0].strip()

    html = raw_bytes.decode(charset, errors="replace")

    parser = _TextExtractor()
    parser.feed(html)
    text = parser.get_text()

    return _truncate(text, max_chars)


def scrape_url_rendered(url: str, max_chars: int = 8000) -> str:
    """Fetch *url* via a real browser (Playwright Chromium) and return its text.

    Required for SPAs (React/Vue/Svelte/Next-without-SSR) where ``scrape_url``
    only finds the empty shell. Slower (~3-5 s per call) so this is reserved
    for the explicit "Analyser depuis URL" flow, not every brief extract.

    Returns the rendered visible text, truncated to *max_chars*. The default
    is generous (8000) because the AI synthesis step that consumes this output
    benefits from a wider context window than a brief field.
    """
    from playwright.sync_api import sync_playwright  # lazy import

    with sync_playwright() as p:
        browser = p.chromium.launch(
            args=["--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu"]
        )
        try:
            page = browser.new_page(
                user_agent=(
                    "Mozilla/5.0 (compatible; Getpostcraft/0.1; "
                    "+https://getpostcraft.app)"
                )
            )
            # `networkidle` waits for SPAs to finish their initial fetches.
            # Cap at 30 s so a stuck site doesn't hang the sidecar.
            page.goto(url, wait_until="networkidle", timeout=30_000)

            # innerText skips hidden elements and follows display:none. Better
            # signal-to-noise than textContent for analysis.
            text = page.evaluate("() => document.body.innerText || ''")
        finally:
            browser.close()

    if not isinstance(text, str):
        raise ValueError("Renderer returned non-string content")

    # Re-collapse whitespace; the browser preserves it more aggressively than our parser.
    text = re.sub(r"\n{3,}", "\n\n", text)
    text = re.sub(r"[ \t]+", " ", text)
    return _truncate(text.strip(), max_chars)


def _truncate(text: str, max_chars: int) -> str:
    """Cut at last sentence boundary if we are well over budget; otherwise hard cap."""
    if len(text) <= max_chars:
        return text
    cut = text[:max_chars].rfind(". ")
    truncated = text[: cut + 1] if cut > max_chars // 2 else text[:max_chars]
    return truncated + "\n[…tronqué]"
