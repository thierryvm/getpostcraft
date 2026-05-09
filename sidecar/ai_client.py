"""
AIClient — unified interface for OpenRouter, Anthropic native, and Ollama.
The API key is passed per-call from Rust; never stored beyond this call.
"""
import json
import re
import sys
from typing import Any

from openai import OpenAI
import anthropic


def _log_warn(msg: str) -> None:
    """Emit a warning to stderr (Rust side captures it via the sidecar pipe).

    Stdout is reserved for the JSON response — anything written there would
    poison the parser. stderr is forwarded to the Tauri log panel and is the
    right channel for soft failures that don't break the contract.
    """
    sys.stderr.write(f"[sidecar:warn] {msg}\n")
    sys.stderr.flush()


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
        # LinkedIn posts target 1300-2500 chars per LINKEDIN_PROMPT.
        # Budget calculation:
        #   2500 chars French (verbose: accents, longer conjunctions) ≈
        #   ~700-800 caption tokens. Plus the JSON wrapper and the model's
        #   internal self-check pass costs another ~200-400. 1200 was
        #   the v0.3.x ceiling — too tight, posts at the upper bound were
        #   silently truncated mid-sentence. 1800 leaves comfortable margin
        #   without paying for tokens we won't use.
        # Instagram targets 250-400 chars → 600 stays generous.
        max_tokens = 1800 if network == "linkedin" else 600
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
        parsed = _parse_json_response(raw)
        # Cost tracker: surface the token counts the SDK already collected
        # so the Rust side can persist them. Both OpenRouter and OpenAI
        # populate `response.usage`; Ollama does too in recent builds.
        parsed["usage"] = _openai_compat_usage(response)
        return parsed

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
        parsed = _parse_json_response(raw)
        parsed["usage"] = _anthropic_usage(message)
        return parsed


# ── Helpers ────────────────────────────────────────────────────────────────

def _openai_compat_usage(response: Any) -> dict[str, int]:
    """Extract `{input_tokens, output_tokens}` from an OpenAI-SDK response.

    The SDK exposes `response.usage` as a `CompletionUsage` object with
    `prompt_tokens` and `completion_tokens`. OpenRouter's proxy and most
    OpenAI-compatible servers (Ollama, LM Studio) follow the same shape.
    Defensive against the rare provider that omits usage entirely — a
    {0, 0} record still anchors the call in the ledger so the user sees
    that an attempt happened.
    """
    usage = getattr(response, "usage", None)
    if usage is None:
        return {"input_tokens": 0, "output_tokens": 0}
    return {
        "input_tokens": int(getattr(usage, "prompt_tokens", 0) or 0),
        "output_tokens": int(getattr(usage, "completion_tokens", 0) or 0),
    }


def _anthropic_usage(message: Any) -> dict[str, int]:
    """Extract `{input_tokens, output_tokens}` from an Anthropic Messages
    SDK response. Field names are already the canonical ones."""
    usage = getattr(message, "usage", None)
    if usage is None:
        return {"input_tokens": 0, "output_tokens": 0}
    return {
        "input_tokens": int(getattr(usage, "input_tokens", 0) or 0),
        "output_tokens": int(getattr(usage, "output_tokens", 0) or 0),
    }


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

    # Allowed slide-role tags. Anything else (including missing) becomes None
    # so the Rust renderer falls back to its index-derived label. This mirrors
    # the Rust-side `role_meta_for` whitelist.
    allowed_roles = {"hero", "problem", "approach", "tech", "change", "moment", "cta"}

    slides = []
    for i, slide in enumerate(data[:expected_count]):
        raw_role = str(slide.get("role", "")).strip().lower()
        role = raw_role if raw_role in allowed_roles else None
        slides.append({
            "emoji": _sanitize_surrogates(str(slide.get("emoji", "💡"))),
            "title": _sanitize_surrogates(str(slide.get("title", f"Slide {i + 1}"))),
            "body": _sanitize_surrogates(str(slide.get("body", ""))),
            "role": role,
        })

    # Sequence sanity — guards against degenerate LLM outputs that pass the
    # per-slide whitelist but produce an editorially broken carousel
    # (e.g. all 7 slides tagged "approach", or "approach" before any
    # "problem"). On failure we strip every role to None — graceful
    # degradation, since the Rust renderer falls back to index-derived
    # labels + brand color, which is the pre-roles behavior.
    if not _slide_role_sequence_is_healthy(slides):
        roles_seen = [s["role"] for s in slides]
        _log_warn(f"slide_role_sequence_degenerate roles={roles_seen}")
        for s in slides:
            s["role"] = None

    return slides


def _slide_role_sequence_is_healthy(slides: list[dict]) -> bool:
    """Return True if the role sequence is editorially coherent.

    Checks (skipped if every role is None — that's the legitimate
    "AI didn't tag anything" case, not a degenerate one):
      1. At least one `problem` slide must appear before any `approach`.
         The narrative arc Cowork's reference posts use is "pain → fix",
         not "fix → pain".
      2. No more than 60% of MIDDLE slides may share the same role.
         Stops outputs like "5 of 5 middles tagged approach" that flatten
         the carousel back to a single-color wall.

    The first/last slot rules (hero / cta) are advisory in the prompt
    but not hard-enforced here — the renderer's index-derived fallback
    already carries the user-visible labels for slot 1 and slot N, so a
    misuse there is cosmetic rather than narrative.
    """
    roles = [s["role"] for s in slides]

    if all(r is None for r in roles):
        return True

    # Rule 1 — problem must appear before approach.
    first_problem = next((i for i, r in enumerate(roles) if r == "problem"), None)
    first_approach = next((i for i, r in enumerate(roles) if r == "approach"), None)
    if first_approach is not None and (first_problem is None or first_problem > first_approach):
        return False

    # Rule 2 — max 60% of middle slides share the same role.
    # "Middle" = everything except first and last. The 60% ratio is only
    # meaningful with ≥3 middle slots; for shorter carousels (3-4 slides)
    # the middle has 1-2 slots where the math degenerates to 100% by
    # construction, so we skip the check.
    middle = roles[1:-1]
    if len(middle) >= 3:
        non_none = [r for r in middle if r is not None]
        if non_none:
            counts: dict[str, int] = {}
            for r in non_none:
                counts[r] = counts.get(r, 0) + 1
            top_count = max(counts.values())
            if top_count / len(middle) > 0.6:
                return False

    return True


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
