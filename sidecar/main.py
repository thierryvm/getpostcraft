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
    else:
        _respond_error(f"Unknown action: {action}")


def _respond_error(msg: str) -> None:
    print(json.dumps({"ok": False, "error": msg}), flush=True)


if __name__ == "__main__":
    main()
