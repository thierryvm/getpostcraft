"""
Unit tests for ai_client.py helpers.

These tests cover the parsing and sanitization logic without making real API
calls. All tests use only the module-level helper functions that are pure
(no network I/O, no AI provider).
"""
import json
import sys
import io
from pathlib import Path

import pytest

# Add sidecar/ to sys.path so we can import ai_client directly
sys.path.insert(0, str(Path(__file__).parent.parent))

from ai_client import (
    _sanitize_surrogates,
    _parse_json_response,
    _parse_carousel_response,
    _escape_control_chars,
)


# ── _sanitize_surrogates ─────────────────────────────────────────────────────

class TestSanitizeSurrogates:
    def test_clean_string_unchanged(self):
        s = "Hello, Linux enthusiasts! 🐧"
        assert _sanitize_surrogates(s) == s

    def test_lone_surrogate_replaced(self):
        # \udc90 is a lone low surrogate — not valid UTF-8
        s = "text\udc90more"
        result = _sanitize_surrogates(s)
        assert "\udc90" not in result
        # The replacement character or empty substitution is acceptable
        assert "text" in result
        assert "more" in result

    def test_valid_surrogate_pair_preserved(self):
        # \U0001F600 = emoji 😀 — represented as surrogate pair in UTF-16
        # but it's a valid codepoint and should survive
        s = "smile \U0001F600 end"
        assert _sanitize_surrogates(s) == s

    def test_empty_string(self):
        assert _sanitize_surrogates("") == ""

    def test_multiple_surrogates(self):
        s = "\udc90\udc91\udc92"
        result = _sanitize_surrogates(s)
        # Must not raise; lone surrogates gone
        assert "\udc90" not in result
        assert "\udc91" not in result

    def test_result_is_json_serializable(self):
        s = "caption with surrogate \udc90 embedded"
        safe = _sanitize_surrogates(s)
        # Must not raise json.dumps
        serialized = json.dumps(safe)
        assert isinstance(serialized, str)


# ── _parse_json_response ─────────────────────────────────────────────────────

class TestParseJsonResponse:
    def test_clean_json_object(self):
        raw = '{"caption": "Hello world", "hashtags": ["linux", "devops"]}'
        result = _parse_json_response(raw)
        assert result["caption"] == "Hello world"
        assert result["hashtags"] == ["linux", "devops"]

    def test_strips_markdown_json_fence(self):
        raw = '```json\n{"caption": "Test", "hashtags": []}\n```'
        result = _parse_json_response(raw)
        assert result["caption"] == "Test"

    def test_strips_plain_code_fence(self):
        raw = '```\n{"caption": "Fenced", "hashtags": ["tag"]}\n```'
        result = _parse_json_response(raw)
        assert result["caption"] == "Fenced"

    def test_sanitizes_surrogates_in_caption(self):
        # Build raw JSON with a surrogate in the caption using ensure_ascii=False
        # We encode the surrogate manually since json.dumps would refuse it
        caption_with_surrogate = "caption\udc90end"
        raw = json.dumps(
            {"caption": caption_with_surrogate, "hashtags": []},
            ensure_ascii=True,  # surrogates serialized as \udc90
        )
        result = _parse_json_response(raw)
        assert "\udc90" not in result["caption"]

    def test_sanitizes_surrogates_in_hashtags(self):
        tag_with_surrogate = "linux\udc90"
        raw = json.dumps(
            {"caption": "ok", "hashtags": [tag_with_surrogate]},
            ensure_ascii=True,
        )
        result = _parse_json_response(raw)
        assert all("\udc90" not in h for h in result["hashtags"])

    def test_raises_on_missing_caption_key(self):
        raw = '{"text": "wrong key", "hashtags": []}'
        with pytest.raises(ValueError, match="Unexpected response shape"):
            _parse_json_response(raw)

    def test_raises_on_missing_hashtags_key(self):
        raw = '{"caption": "ok"}'
        with pytest.raises(ValueError, match="Unexpected response shape"):
            _parse_json_response(raw)

    def test_literal_newline_in_value_recovered(self):
        # A literal newline inside a JSON string value is invalid JSON
        # but should be recovered via _escape_control_chars
        raw = '{"caption": "line1\nline2", "hashtags": []}'
        result = _parse_json_response(raw)
        # After escaping the \n becomes \\n in JSON → parsed as newline char
        assert "line1" in result["caption"]
        assert "line2" in result["caption"]

    def test_empty_hashtags_list(self):
        raw = '{"caption": "Solo caption", "hashtags": []}'
        result = _parse_json_response(raw)
        assert result["hashtags"] == []

    def test_returns_string_types(self):
        raw = '{"caption": "ok", "hashtags": ["a", "b"]}'
        result = _parse_json_response(raw)
        assert isinstance(result["caption"], str)
        assert all(isinstance(h, str) for h in result["hashtags"])


# ── _parse_carousel_response ─────────────────────────────────────────────────

class TestParseCarouselResponse:
    def _make_slides(self, n: int) -> str:
        slides = [
            {"emoji": "💡", "title": f"Slide {i+1}", "body": f"Body {i+1}"}
            for i in range(n)
        ]
        return json.dumps(slides)

    def test_parses_valid_slides(self):
        raw = self._make_slides(3)
        result = _parse_carousel_response(raw, 3)
        assert len(result) == 3
        assert result[0]["title"] == "Slide 1"
        assert result[2]["body"] == "Body 3"

    def test_truncates_to_expected_count(self):
        raw = self._make_slides(5)
        result = _parse_carousel_response(raw, 3)
        assert len(result) == 3

    def test_fewer_slides_than_expected(self):
        raw = self._make_slides(2)
        result = _parse_carousel_response(raw, 5)
        assert len(result) == 2

    def test_sanitizes_surrogates_in_title(self):
        slides = [{"emoji": "🔥", "title": "title\udc90", "body": "body"}]
        raw = json.dumps(slides, ensure_ascii=True)
        result = _parse_carousel_response(raw, 1)
        assert "\udc90" not in result[0]["title"]

    def test_sanitizes_surrogates_in_body(self):
        slides = [{"emoji": "💡", "title": "ok", "body": "body\udc90here"}]
        raw = json.dumps(slides, ensure_ascii=True)
        result = _parse_carousel_response(raw, 1)
        assert "\udc90" not in result[0]["body"]

    def test_raises_on_non_array(self):
        raw = '{"caption": "wrong", "hashtags": []}'
        with pytest.raises(ValueError, match="Expected JSON array"):
            _parse_carousel_response(raw, 3)

    def test_default_emoji_when_missing(self):
        raw = json.dumps([{"title": "T", "body": "B"}])
        result = _parse_carousel_response(raw, 1)
        assert result[0]["emoji"] == "💡"

    def test_default_title_when_missing(self):
        raw = json.dumps([{"emoji": "🔥", "body": "B"}])
        result = _parse_carousel_response(raw, 1)
        assert "1" in result[0]["title"]  # "Slide 1"

    def test_strips_markdown_fence(self):
        raw = "```json\n" + self._make_slides(2) + "\n```"
        result = _parse_carousel_response(raw, 2)
        assert len(result) == 2

    def test_result_is_json_serializable(self):
        raw = self._make_slides(3)
        result = _parse_carousel_response(raw, 3)
        # Must not raise
        assert json.dumps(result)


# ── _respond_error (via main module) ─────────────────────────────────────────

class TestRespondError:
    """Test that _respond_error never crashes even with surrogate-containing messages."""

    def _call_respond_error(self, msg: str) -> dict:
        """Import and call _respond_error, capture stdout."""
        from main import _respond_error  # noqa: PLC0415

        buf = io.StringIO()
        sys.stdout = buf
        try:
            _respond_error(msg)
        finally:
            sys.stdout = sys.__stdout__

        output = buf.getvalue().strip()
        return json.loads(output)

    def test_plain_error_message(self):
        result = self._call_respond_error("Database connection failed")
        assert result["ok"] is False
        assert result["error"] == "Database connection failed"

    def test_surrogate_in_error_message_does_not_crash(self):
        msg_with_surrogate = "Error at position 430: \udc90 invalid char"
        # Must not raise — if it does, the test fails
        result = self._call_respond_error(msg_with_surrogate)
        assert result["ok"] is False
        assert "\udc90" not in result["error"]

    def test_output_is_valid_json(self):
        result = self._call_respond_error("some error")
        # Already parsed above — just verify shape
        assert "ok" in result
        assert "error" in result

    def test_error_field_is_string(self):
        result = self._call_respond_error("test")
        assert isinstance(result["error"], str)
