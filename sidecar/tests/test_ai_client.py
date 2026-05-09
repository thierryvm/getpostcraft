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
    _parse_visual_profile,
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

    def test_role_passes_through_when_in_whitelist(self):
        slides = [
            {"emoji": "✦", "title": "T", "body": "B", "role": "problem"},
            {"emoji": "✦", "title": "T", "body": "B", "role": "approach"},
            {"emoji": "✦", "title": "T", "body": "B", "role": "cta"},
        ]
        raw = json.dumps(slides)
        result = _parse_carousel_response(raw, 3)
        assert result[0]["role"] == "problem"
        assert result[1]["role"] == "approach"
        assert result[2]["role"] == "cta"

    def test_role_normalises_case_and_whitespace(self):
        slides = [{"emoji": "✦", "title": "T", "body": "B", "role": "  PROBLEM  "}]
        raw = json.dumps(slides)
        result = _parse_carousel_response(raw, 1)
        assert result[0]["role"] == "problem"

    def test_unknown_role_becomes_none(self):
        slides = [{"emoji": "✦", "title": "T", "body": "B", "role": "fluffy-cat"}]
        raw = json.dumps(slides)
        result = _parse_carousel_response(raw, 1)
        assert result[0]["role"] is None

    def test_missing_role_becomes_none(self):
        slides = [{"emoji": "✦", "title": "T", "body": "B"}]
        raw = json.dumps(slides)
        result = _parse_carousel_response(raw, 1)
        assert result[0]["role"] is None


# ── Sequence sanity check ────────────────────────────────────────────────────
#
# These tests assert the editorial-coherence guard added on top of the
# per-slide whitelist. They feed the parser whole carousels (not just one
# slide) and check that degenerate sequences get all roles stripped to None.

class TestSlideRoleSequence:
    def _carousel(self, roles: list[str | None]) -> str:
        slides = [
            {
                "emoji": "✦",
                "title": f"S{i+1}",
                "body": "B",
                **({"role": r} if r is not None else {}),
            }
            for i, r in enumerate(roles)
        ]
        return json.dumps(slides)

    def test_healthy_sequence_keeps_roles(self):
        # hero → problem → approach → tech → cta is the canonical arc.
        raw = self._carousel(["hero", "problem", "approach", "tech", "cta"])
        result = _parse_carousel_response(raw, 5)
        assert [s["role"] for s in result] == [
            "hero",
            "problem",
            "approach",
            "tech",
            "cta",
        ]

    def test_no_problem_before_approach_strips_all(self):
        # Approach without any prior problem is the "fix without pain" anti-pattern.
        raw = self._carousel(["hero", "approach", "tech", "tech", "cta"])
        result = _parse_carousel_response(raw, 5)
        assert all(s["role"] is None for s in result)

    def test_majority_same_role_strips_all(self):
        # 4 of 5 middle slots tagged "tech" → 80% > 60% threshold.
        # (Middle = slides 2..6 in a 7-slide post, so we need 7 total here.)
        raw = self._carousel(
            ["hero", "tech", "tech", "tech", "tech", "problem", "cta"]
        )
        result = _parse_carousel_response(raw, 7)
        assert all(s["role"] is None for s in result)

    def test_first_slide_not_hero_warns_but_keeps(self):
        # First slot mistagged is cosmetic — renderer's index fallback
        # carries the user-visible "intro" label, so we keep the rest.
        raw = self._carousel(["tech", "problem", "approach", "tech", "cta"])
        result = _parse_carousel_response(raw, 5)
        # All roles preserved (no degenerate-strip) — sequence is otherwise fine.
        assert [s["role"] for s in result] == [
            "tech",
            "problem",
            "approach",
            "tech",
            "cta",
        ]

    def test_no_roles_at_all_is_valid(self):
        # Legitimate "AI didn't tag anything" — sequence check must not fire.
        raw = self._carousel([None, None, None, None, None])
        result = _parse_carousel_response(raw, 5)
        assert all(s["role"] is None for s in result)


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


# ── Matrice de compatibilité modèles ─────────────────────────────────────────
# Ces tests documentent les patterns de sortie réels observés par famille.
# Ils ne font AUCUN appel API — ils reproduisent les outputs connus.
#
# Modèles validés (compatible JSON-only) :
#   ✅ anthropic/claude-* (propre, pas de fence)
#   ✅ openai/gpt-4o, gpt-4o-mini (parfois fence ```json)
#   ✅ openai/gpt-3.5-turbo (parfois fence ```json)
#   ⚠️  mistralai/mistral-large, mistral-medium (fence ```json fréquente)
#   ⚠️  mistralai/mistral-small-* (texte parasite avant/après JSON)
#   ⚠️  meta-llama/* (contrôle chars dans les valeurs, surrogates)
#   ⚠️  qwen/* (surrogates Unicode fréquents)
#   ❌  mistralai/mistral-7b-instruct (rarement JSON valide sans préambule)

class TestModelOutputPatterns:
    """Patterns de sortie réels observés — tests de non-régression."""

    # ── Claude (Anthropic) ────────────────────────────────────────────────

    def test_claude_clean_json_no_fence(self):
        """Claude retourne du JSON propre, sans fence ni préambule."""
        output = '{"caption": "Tu perds 40 min/semaine à retaper les mêmes commandes. J\'ai mis 3 min à régler ça.", "hashtags": ["linux", "bash", "terminal", "sysadmin", "devops"]}'
        result = _parse_json_response(output)
        assert len(result["hashtags"]) == 5
        assert "caption" in result

    # ── GPT-4o / GPT-4o-mini (OpenAI) ────────────────────────────────────

    def test_gpt4o_json_in_markdown_fence(self):
        """GPT-4o wrapping fréquent dans ```json."""
        output = '```json\n{"caption": "Astuce grep.", "hashtags": ["linux", "bash"]}\n```'
        result = _parse_json_response(output)
        assert result["caption"] == "Astuce grep."

    def test_gpt4o_mini_trailing_newline(self):
        """GPT-4o-mini ajoute parfois un \\n final."""
        output = '{"caption": "test", "hashtags": ["tag1"]}\n'
        result = _parse_json_response(output)
        assert result["caption"] == "test"

    # ── Mistral Large / Medium ────────────────────────────────────────────

    def test_mistral_large_json_fence(self):
        """Mistral Large utilise systématiquement ```json."""
        output = "```json\n{\"caption\": \"Commande find oubliée.\", \"hashtags\": [\"linux\", \"sysadmin\"]}\n```"
        result = _parse_json_response(output)
        assert "find" in result["caption"]

    # ── Mistral Small — comportement problématique documenté ─────────────

    def test_mistral_small_preamble_text(self):
        """
        Mistral Small ajoute souvent du texte AVANT le JSON.
        Comportement actuel : lève ValueError (attendu).
        Quand on améliore le parser avec extraction regex, ce test devra passer.
        """
        json_part = '{"caption": "Astuce bash.", "hashtags": ["bash"]}'
        output = f"Voici la caption que j'ai générée pour toi :\n{json_part}"
        try:
            result = _parse_json_response(output)
            # Si le parser s'améliore et extrait le JSON, les clés doivent être là
            assert "caption" in result
        except (ValueError, json.JSONDecodeError):
            # Comportement actuel attendu — pas une régression
            pass

    def test_mistral_small_suffix_text(self):
        """Mistral Small ajoute parfois du texte APRÈS le JSON."""
        json_part = '{"caption": "Astuce bash.", "hashtags": ["bash"]}'
        output = f"{json_part}\n\nJ'espère que cette caption vous plaira !"
        try:
            result = _parse_json_response(output)
            assert "caption" in result
        except (ValueError, json.JSONDecodeError):
            pass  # Comportement actuel attendu

    # ── LLaMA / Meta ──────────────────────────────────────────────────────

    def test_llama_literal_newline_in_value(self):
        """LLaMA insère parfois des \\n réels dans les valeurs de string."""
        output = '{"caption": "ligne1\nligne2", "hashtags": ["linux"]}'
        result = _parse_json_response(output)
        assert "ligne1" in result["caption"]
        assert "ligne2" in result["caption"]

    def test_llama_tab_in_value(self):
        """LLaMA insère parfois des tabs dans les valeurs."""
        output = '{"caption": "col1\tcol2", "hashtags": ["linux"]}'
        result = _parse_json_response(output)
        assert "col1" in result["caption"]

    # ── Qwen ──────────────────────────────────────────────────────────────

    def test_qwen_surrogate_in_caption(self):
        """Certains Qwen produisent des surrogates dans leur sortie."""
        caption = "caption avec surrogate\udc90ici"
        raw = json.dumps({"caption": caption, "hashtags": ["linux"]}, ensure_ascii=True)
        result = _parse_json_response(raw)
        assert "\udc90" not in result["caption"]
        assert "caption avec surrogate" in result["caption"]

    # ── Edge cases généraux ───────────────────────────────────────────────

    def test_extra_whitespace_around_json(self):
        output = "\n\n  " + '{"caption": "ok", "hashtags": ["a"]}' + "  \n\n"
        result = _parse_json_response(output)
        assert result["caption"] == "ok"

    def test_unicode_accents_in_caption(self):
        """Accents français dans la caption — fréquent pour notre niche."""
        output = '{"caption": "Arrête d\'utiliser cat. Voici pourquoi.", "hashtags": ["linux"]}'
        result = _parse_json_response(output)
        assert "Arrête" in result["caption"]


# ── _parse_visual_profile ────────────────────────────────────────────────────

class TestParseVisualProfile:
    """Vision-based brand extraction returns JSON with colors / typography /
    mood / layout. Models occasionally wrap in code fences or add preamble —
    these tests pin our normalization + sanitization behavior so a regression
    on the parser doesn't silently produce bad ProductTruth blocks."""

    def test_full_clean_response(self):
        raw = (
            '{"colors": ["#0d1117", "#3ddc84"], '
            '"typography": {"family": "mono", "weight": "bold", "character": "technical"}, '
            '"mood": ["minimalist", "developer-focused"], '
            '"layout": "minimal-dense"}'
        )
        result = _parse_visual_profile(raw)
        assert result["colors"] == ["#0d1117", "#3ddc84"]
        assert result["typography"]["family"] == "mono"
        assert result["typography"]["weight"] == "bold"
        assert result["typography"]["character"] == "technical"
        assert result["mood"] == ["minimalist", "developer-focused"]
        assert result["layout"] == "minimal-dense"

    def test_strips_markdown_fences(self):
        raw = '```json\n{"colors": ["#fff"], "typography": {}, "mood": [], "layout": "x"}\n```'
        result = _parse_visual_profile(raw)
        assert result["colors"] == ["#fff"]

    def test_drops_non_hex_colors(self):
        # Models sometimes return color names instead of hex — drop them silently.
        raw = '{"colors": ["#fff", "red", "#ababab", "blue"], "typography": {}, "mood": [], "layout": "x"}'
        result = _parse_visual_profile(raw)
        assert result["colors"] == ["#fff", "#ababab"]

    def test_caps_colors_at_six(self):
        raw = (
            '{"colors": ["#aaa", "#bbb", "#ccc", "#ddd", "#eee", "#fff", "#012", "#345"], '
            '"typography": {}, "mood": [], "layout": "x"}'
        )
        result = _parse_visual_profile(raw)
        assert len(result["colors"]) == 6

    def test_typography_defaults_when_missing_keys(self):
        raw = '{"colors": [], "typography": {}, "mood": [], "layout": "x"}'
        result = _parse_visual_profile(raw)
        assert result["typography"]["family"] == "sans"
        assert result["typography"]["weight"] == "regular"
        assert result["typography"]["character"] == "neutral"

    def test_typography_lowercases_values(self):
        raw = '{"colors": [], "typography": {"family": "MONO", "weight": "BOLD", "character": "Technical"}, "mood": [], "layout": "X"}'
        result = _parse_visual_profile(raw)
        assert result["typography"]["family"] == "mono"
        assert result["typography"]["weight"] == "bold"
        assert result["typography"]["character"] == "technical"
        assert result["layout"] == "x"

    def test_mood_filters_empty_strings_and_caps_at_five(self):
        raw = (
            '{"colors": [], "typography": {}, '
            '"mood": ["a", "", "b", "  ", "c", "d", "e", "f", "g"], "layout": ""}'
        )
        result = _parse_visual_profile(raw)
        assert result["mood"] == ["a", "b", "c", "d", "e"]

    def test_layout_defaults_to_unspecified(self):
        raw = '{"colors": [], "typography": {}, "mood": [], "layout": ""}'
        result = _parse_visual_profile(raw)
        assert result["layout"] == "unspecified"

    def test_typography_non_dict_falls_back_to_empty(self):
        # Defensive: model returned a string instead of an object — don't crash.
        raw = '{"colors": [], "typography": "sans-serif bold", "mood": [], "layout": "x"}'
        result = _parse_visual_profile(raw)
        assert result["typography"]["family"] == "sans"

    def test_rejects_non_object_root(self):
        # Defensive: model returned an array.
        raw = '["#fff", "#000"]'
        try:
            _parse_visual_profile(raw)
            raised = False
        except ValueError:
            raised = True
        assert raised, "must reject non-object roots"
