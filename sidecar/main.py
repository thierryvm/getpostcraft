"""
Getpostcraft Python sidecar — newline-delimited JSON dispatcher.
Reads one JSON request from stdin, writes one JSON response to stdout, exits.
"""
import sys
import json

from ai_client import AIClient, _sanitize_surrogates


def _deep_sanitize(obj: object) -> object:
    """Recursively replace lone surrogates in all string values of a dict/list."""
    if isinstance(obj, str):
        return _sanitize_surrogates(obj)
    if isinstance(obj, dict):
        return {k: _deep_sanitize(v) for k, v in obj.items()}
    if isinstance(obj, list):
        return [_deep_sanitize(item) for item in obj]
    return obj


def _write_line(payload: str) -> None:
    """Write a JSON line to stdout using raw bytes when available.

    In production stdout is a TextIOWrapper that exposes .buffer — writing
    raw ASCII bytes bypasses the text-mode encoding layer entirely, preventing
    any UnicodeEncodeError regardless of Windows code page.
    In tests stdout is a StringIO (no .buffer) — fall back to a plain write.
    """
    if hasattr(sys.stdout, "buffer"):
        sys.stdout.buffer.write((payload + "\n").encode("ascii"))
        sys.stdout.buffer.flush()
    else:
        sys.stdout.write(payload + "\n")
        sys.stdout.flush()


def _respond_ok(data: object) -> None:
    payload = json.dumps({"ok": True, "data": _deep_sanitize(data)}, ensure_ascii=True)
    _write_line(payload)


def main() -> None:
    line = sys.stdin.readline()
    if not line.strip():
        _respond_error("Empty input")
        return

    try:
        req = json.loads(line)
    except json.JSONDecodeError as exc:
        _respond_error(f"Invalid JSON input: {exc}")
        return

    action = req.get("action")

    if action == "generate_content":
        try:
            client = AIClient(
                provider=req["provider"],
                api_key=req.get("api_key"),
                model=req["model"],
                base_url=req.get("base_url"),
            )
            data = client.generate_caption(
                brief=req["brief"],
                network=req["network"],
                system_prompt=req["system_prompt"],
            )
            _respond_ok(data)
        except Exception as exc:  # noqa: BLE001
            _respond_error(str(exc))

    elif action == "render_html":
        try:
            from render import render_html_to_png

            path = render_html_to_png(
                html=req["html"],
                output_path=req["output_path"],
                width=req.get("width", 1080),
                height=req.get("height", 1080),
            )
            _respond_ok({"path": path})
        except Exception as exc:  # noqa: BLE001
            _respond_error(str(exc))

    elif action == "generate_carousel":
        try:
            client = AIClient(
                provider=req["provider"],
                api_key=req.get("api_key"),
                model=req["model"],
                base_url=req.get("base_url"),
            )
            slides = client.generate_carousel_slides(
                brief=req["brief"],
                network=req["network"],
                slide_count=int(req.get("slide_count", 5)),
                system_prompt=req["system_prompt"],
            )
            _respond_ok({"slides": slides})
        except Exception as exc:  # noqa: BLE001
            _respond_error(str(exc))

    elif action == "scrape_url":
        try:
            from scraper import scrape_url

            text = scrape_url(req["url"], max_chars=req.get("max_chars", 3000))
            _respond_ok({"text": text})
        except Exception as exc:  # noqa: BLE001
            _respond_error(str(exc))

    elif action == "scrape_url_rendered":
        try:
            from scraper import scrape_url_rendered

            text = scrape_url_rendered(req["url"], max_chars=req.get("max_chars", 8000))
            _respond_ok({"text": text})
        except Exception as exc:  # noqa: BLE001
            _respond_error(str(exc))

    elif action == "scrape_url_rendered_with_screenshot":
        try:
            from scraper import scrape_url_rendered_with_screenshot

            text, screenshot_b64 = scrape_url_rendered_with_screenshot(
                req["url"],
                max_chars=req.get("max_chars", 8000),
                capture_screenshot=req.get("capture_screenshot", True),
            )
            _respond_ok({"text": text, "screenshot": screenshot_b64})
        except Exception as exc:  # noqa: BLE001
            _respond_error(str(exc))

    elif action == "extract_visual_profile":
        try:
            client = AIClient(
                provider=req["provider"],
                api_key=req.get("api_key"),
                model=req["model"],
                base_url=req.get("base_url"),
            )
            profile = client.extract_visual_profile(
                screenshot_b64=req["screenshot"],
                system_prompt=req["system_prompt"],
            )
            _respond_ok(profile)
        except Exception as exc:  # noqa: BLE001
            _respond_error(str(exc))

    elif action == "synthesize_product_truth":
        try:
            client = AIClient(
                provider=req["provider"],
                api_key=req.get("api_key"),
                model=req["model"],
                base_url=req.get("base_url"),
            )
            text = client.synthesize_product_truth(
                content=req["content"],
                system_prompt=req["system_prompt"],
            )
            _respond_ok({"product_truth": text})
        except Exception as exc:  # noqa: BLE001
            _respond_error(str(exc))

    elif action == "warmup":
        # Verify required modules are importable; used for pre-warming on Composer open.
        missing = []
        for mod in ("openai", "anthropic"):
            try:
                __import__(mod)
            except ImportError:
                missing.append(mod)
        if missing:
            _respond_error(f"Missing Python dependencies: {', '.join(missing)}")
        else:
            _respond_ok({"status": "ready"})

    else:
        _respond_error(f"Unknown action: {action}")


def _respond_error(msg: str) -> None:
    safe_msg = "".join(ch for ch in msg if not (0xD800 <= ord(ch) <= 0xDFFF))
    payload = json.dumps({"ok": False, "error": safe_msg}, ensure_ascii=True)
    _write_line(payload)


if __name__ == "__main__":
    main()
