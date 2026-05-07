"""
AIClient — unified interface for OpenRouter, Anthropic native, and Ollama.
The API key is passed per-call from Rust; never stored beyond this call.
"""
import json
import re
from typing import Any

from openai import OpenAI
import anthropic


class AIClient:
    def __init__(
        self,
        provider: str,
        api_key: str | None,
        model: str,
        base_url: str | None = None,
    ) -> None:
        self.provider = provider
        self.api_key = api_key
        self.model = model
        self.base_url = base_url

    def generate_caption(
        self, brief: str, network: str, system_prompt: str
    ) -> dict[str, Any]:
        # LinkedIn posts target 1300-2100 chars; Instagram ~400 — adjust token budget
        max_tokens = 1200 if network == "linkedin" else 600
        if self.provider == "anthropic":
            return self._generate_anthropic(brief, system_prompt, max_tokens)
        return self._generate_openai_compat(brief, system_prompt, max_tokens)

    def generate_carousel_slides(
        self, brief: str, network: str, slide_count: int, system_prompt: str
    ) -> list[dict]:
        if self.provider == "anthropic":
            return self._carousel_anthropic(brief, slide_count, system_prompt)
        return self._carousel_openai_compat(brief, slide_count, system_prompt)

    def synthesize_product_truth(
        self, content: str, system_prompt: str
    ) -> str:
        """Synthesize a structured ProductTruth from raw scraped website content.

        Returns plain text (not JSON) — the user pastes it directly into the
        textarea, so we don't need parsing. Long output budget (1200 tokens)
        because a good ProductTruth is 200-400 words.
        """
        if self.provider == "anthropic":
            return self._synthesize_anthropic(content, system_prompt)
        return self._synthesize_openai_compat(content, system_prompt)

    def _synthesize_openai_compat(self, content: str, system_prompt: str) -> str:
        base_url = self.base_url or "https://openrouter.ai/api/v1"
        api_key = self.api_key or "ollama"
        headers: dict[str, str] = {}
        if self.provider == "openrouter":
            headers = {
                "HTTP-Referer": "https://getpostcraft.app",
                "X-Title": "Getpostcraft",
            }
        client = OpenAI(api_key=api_key, base_url=base_url, default_headers=headers)
        response = client.chat.completions.create(
            model=self.model,
            max_tokens=1200,
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": content},
            ],
        )
        return _sanitize_surrogates((response.choices[0].message.content or "").strip())

    def _synthesize_anthropic(self, content: str, system_prompt: str) -> str:
        if not self.api_key:
            raise ValueError("Anthropic requires an API key")
        client = anthropic.Anthropic(api_key=self.api_key)
        message = client.messages.create(
            model=self.model,
            max_tokens=1200,
            system=system_prompt,
            messages=[{"role": "user", "content": content}],
        )
        return _sanitize_surrogates(message.content[0].text.strip())

    def extract_visual_profile(
        self, screenshot_b64: str, system_prompt: str
    ) -> dict[str, Any]:
        """Vision-based extraction of brand identity from a website screenshot.

        Returns a structured dict: { colors, typography, mood, layout }. The
        system_prompt instructs the model to return ONLY valid JSON; we sanitize
        lone surrogates and fall back to lenient parsing on the response.

        Both OpenAI-compat (OpenRouter, OpenAI) and Anthropic native accept a
        base64-encoded screenshot — the format differs slightly per provider.
        """
        if self.provider == "anthropic":
            raw = self._extract_visual_anthropic(screenshot_b64, system_prompt)
        else:
            raw = self._extract_visual_openai_compat(screenshot_b64, system_prompt)
        return _parse_visual_profile(raw)

    def _extract_visual_openai_compat(
        self, screenshot_b64: str, system_prompt: str
    ) -> str:
        base_url = self.base_url or "https://openrouter.ai/api/v1"
        api_key = self.api_key or "ollama"
        headers: dict[str, str] = {}
        if self.provider == "openrouter":
            headers = {
                "HTTP-Referer": "https://getpostcraft.app",
                "X-Title": "Getpostcraft",
            }
        client = OpenAI(api_key=api_key, base_url=base_url, default_headers=headers)
        response = client.chat.completions.create(
            model=self.model,
            max_tokens=600,
            messages=[
                {"role": "system", "content": system_prompt},
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": f"data:image/png;base64,{screenshot_b64}"
                            },
                        },
                        {
                            "type": "text",
                            "text": "Extract the visual brand profile from this screenshot.",
                        },
                    ],
                },
            ],
        )
        return _sanitize_surrogates((response.choices[0].message.content or "").strip())

    def _extract_visual_anthropic(
        self, screenshot_b64: str, system_prompt: str
    ) -> str:
        if not self.api_key:
            raise ValueError("Anthropic requires an API key")
        client = anthropic.Anthropic(api_key=self.api_key)
        message = client.messages.create(
            model=self.model,
            max_tokens=600,
            system=system_prompt,
            messages=[
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": screenshot_b64,
                            },
                        },
                        {
                            "type": "text",
                            "text": "Extract the visual brand profile from this screenshot.",
                        },
                    ],
                }
            ],
        )
        return _sanitize_surrogates(message.content[0].text.strip())

    def _carousel_openai_compat(
        self, brief: str, slide_count: int, system_prompt: str
    ) -> list[dict]:
        base_url = self.base_url or "https://openrouter.ai/api/v1"
        api_key = self.api_key or "ollama"
        headers: dict[str, str] = {}
        if self.provider == "openrouter":
            headers = {
                "HTTP-Referer": "https://getpostcraft.app",
                "X-Title": "Getpostcraft",
            }
        client = OpenAI(api_key=api_key, base_url=base_url, default_headers=headers)
        response = client.chat.completions.create(
            model=self.model,
            max_tokens=2000,
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": brief},
            ],
        )
        raw = response.choices[0].message.content or ""
        return _parse_carousel_response(raw, slide_count)

    def _carousel_anthropic(
        self, brief: str, slide_count: int, system_prompt: str
    ) -> list[dict]:
        if not self.api_key:
            raise ValueError("Anthropic requires an API key")
        client = anthropic.Anthropic(api_key=self.api_key)
        message = client.messages.create(
            model=self.model,
            max_tokens=2000,
            system=system_prompt,
            messages=[{"role": "user", "content": brief}],
        )
        raw = message.content[0].text
        return _parse_carousel_response(raw, slide_count)

    # ── OpenRouter + Ollama (OpenAI-compatible) ────────────────────────────

    def _generate_openai_compat(self, brief: str, system_prompt: str, max_tokens: int = 600) -> dict[str, Any]:
        base_url = self.base_url or "https://openrouter.ai/api/v1"
        api_key = self.api_key or "ollama"  # Ollama ignores the key

        headers: dict[str, str] = {}
        if self.provider == "openrouter":
            headers = {
                "HTTP-Referer": "https://getpostcraft.app",
                "X-Title": "Getpostcraft",
            }

        client = OpenAI(
            api_key=api_key,
            base_url=base_url,
            default_headers=headers,
        )

        response = client.chat.completions.create(
            model=self.model,
            max_tokens=max_tokens,
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": brief},
            ],
        )

        raw = response.choices[0].message.content or ""
        return _parse_json_response(raw)

    # ── Anthropic native ───────────────────────────────────────────────────

    def _generate_anthropic(self, brief: str, system_prompt: str, max_tokens: int = 600) -> dict[str, Any]:
        if not self.api_key:
            raise ValueError("Anthropic requires an API key")

        client = anthropic.Anthropic(api_key=self.api_key)

        message = client.messages.create(
            model=self.model,
            max_tokens=max_tokens,
            system=system_prompt,
            messages=[{"role": "user", "content": brief}],
        )

        raw = message.content[0].text
        return _parse_json_response(raw)


# ── Helpers ────────────────────────────────────────────────────────────────

def _sanitize_surrogates(s: str) -> str:
    """Remove lone surrogates that some AI models produce (e.g. \\udc90).

    Lone surrogates (U+D800–U+DFFF) are valid in Python str but are rejected
    by CPython's C JSON extension and cannot be encoded as UTF-8.
    Pure char-filter avoids codec round-trip edge cases on Windows.
    """
    return "".join(ch for ch in s if not (0xD800 <= ord(ch) <= 0xDFFF))


def _parse_json_response(text: str) -> dict[str, Any]:
    """Extract JSON from model output, stripping markdown fences if present.

    Models sometimes embed literal control characters (real newlines, tabs)
    inside JSON string values, which is invalid. We sanitize those in-string
    control chars before parsing.
    """
    # Sanitize surrogates in the raw input BEFORE any string operation
    # (re.sub and json.loads in CPython can raise UnicodeEncodeError on surrogates).
    text = _sanitize_surrogates(text)
    cleaned = re.sub(r"```(?:json)?\s*|\s*```", "", text).strip()

    try:
        data = json.loads(cleaned)
    except json.JSONDecodeError:
        data = json.loads(_escape_control_chars(cleaned))

    if "caption" not in data or "hashtags" not in data:
        raise ValueError(f"Unexpected response shape: {list(data.keys())}")
    return {
        "caption": _sanitize_surrogates(str(data["caption"])),
        "hashtags": [_sanitize_surrogates(str(h)) for h in data["hashtags"]],
    }


def _parse_visual_profile(text: str) -> dict[str, Any]:
    """Extract a {colors, typography, mood, layout} JSON object from Vision output.

    Vision models occasionally wrap JSON in code fences or add a brief preamble.
    We strip fences, parse, and validate the shape — falling back to defaults
    when a key is missing so the UI always has something to render.
    """
    text = _sanitize_surrogates(text)
    cleaned = re.sub(r"```(?:json)?\s*|\s*```", "", text).strip()
    try:
        data = json.loads(cleaned)
    except json.JSONDecodeError:
        data = json.loads(_escape_control_chars(cleaned))

    if not isinstance(data, dict):
        raise ValueError(f"Expected JSON object, got {type(data).__name__}")

    # Whitelist + sanitize each field. Unknown keys are dropped.
    colors_raw = data.get("colors", [])
    colors = [
        _sanitize_surrogates(str(c))
        for c in colors_raw
        if isinstance(c, str) and c.startswith("#")
    ][:6]  # cap at 6 — UI swatch row only shows ~5

    typography = data.get("typography") or {}
    if not isinstance(typography, dict):
        typography = {}
    typography_clean = {
        "family": _sanitize_surrogates(str(typography.get("family", ""))).lower() or "sans",
        "weight": _sanitize_surrogates(str(typography.get("weight", ""))).lower() or "regular",
        "character": _sanitize_surrogates(str(typography.get("character", ""))).lower() or "neutral",
    }

    mood_raw = data.get("mood", [])
    mood = [
        _sanitize_surrogates(str(m))
        for m in mood_raw
        if isinstance(m, str) and m.strip()
    ][:5]

    layout = _sanitize_surrogates(str(data.get("layout", ""))).lower() or "unspecified"

    return {
        "colors": colors,
        "typography": typography_clean,
        "mood": mood,
        "layout": layout,
    }


def _parse_carousel_response(text: str, expected_count: int) -> list[dict]:
    """Parse a JSON array of slide objects from model output."""
    text = _sanitize_surrogates(text)
    cleaned = re.sub(r"```(?:json)?\s*|\s*```", "", text).strip()
    try:
        data = json.loads(cleaned)
    except json.JSONDecodeError:
        data = json.loads(_escape_control_chars(cleaned))

    if not isinstance(data, list):
        raise ValueError(f"Expected JSON array, got {type(data).__name__}")

    slides = []
    for i, slide in enumerate(data[:expected_count]):
        slides.append({
            "emoji": _sanitize_surrogates(str(slide.get("emoji", "💡"))),
            "title": _sanitize_surrogates(str(slide.get("title", f"Slide {i + 1}"))),
            "body": _sanitize_surrogates(str(slide.get("body", ""))),
        })
    return slides


_CTRL_ESCAPES: dict[str, str] = {
    "\n": "\\n", "\r": "\\r", "\t": "\\t",
    "\b": "\\b", "\f": "\\f",
}


def _escape_control_chars(text: str) -> str:
    """Replace literal control characters inside JSON string values only."""
    result: list[str] = []
    in_string = False
    skip_next = False
    for ch in text:
        if skip_next:
            result.append(ch)
            skip_next = False
        elif ch == "\\" and in_string:
            result.append(ch)
            skip_next = True
        elif ch == '"':
            in_string = not in_string
            result.append(ch)
        elif in_string and ord(ch) < 0x20:
            result.append(_CTRL_ESCAPES.get(ch, f"\\u{ord(ch):04x}"))
        else:
            result.append(ch)
    return "".join(result)
