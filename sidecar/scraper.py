"""Scrape a URL and extract meaningful text for use as a post brief."""
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

    if len(text) > max_chars:
        # Truncate at last sentence boundary within limit
        cut = text[:max_chars].rfind(". ")
        text = text[: cut + 1] if cut > max_chars // 2 else text[:max_chars]
        text += "\n[…tronqué]"

    return text
