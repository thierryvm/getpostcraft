"""
Getpostcraft Python sidecar — newline-delimited JSON dispatcher.
Reads one JSON request from stdin, writes one JSON response to stdout, exits.
"""
import sys
import json

from ai_client import AIClient


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
            print(json.dumps({"ok": True, "data": data}), flush=True)
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
            print(json.dumps({"ok": True, "data": {"path": path}}), flush=True)
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
            print(json.dumps({"ok": True, "data": {"slides": slides}}), flush=True)
        except Exception as exc:  # noqa: BLE001
            _respond_error(str(exc))

    elif action == "scrape_url":
        try:
            from scraper import scrape_url

            text = scrape_url(req["url"], max_chars=req.get("max_chars", 3000))
            print(json.dumps({"ok": True, "data": {"text": text}}), flush=True)
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
            print(json.dumps({"ok": True, "data": {"status": "ready"}}), flush=True)

    else:
        _respond_error(f"Unknown action: {action}")


def _respond_error(msg: str) -> None:
    print(json.dumps({"ok": False, "error": msg}), flush=True)


if __name__ == "__main__":
    main()
