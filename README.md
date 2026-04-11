# Getpostcraft

> AI-assisted social media content creation — Tauri 2 desktop app

[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-orange)](https://tauri.app)
[![TypeScript](https://img.shields.io/badge/TypeScript-strict-blue)](https://www.typescriptlang.org)

## What it does

Generate Instagram captions and hashtags from a brief using AI (OpenRouter, Anthropic, or local Ollama).
Built for the [@terminallearning](https://instagram.com/terminallearning) account — Linux/Terminal/DevOps niche.

## Stack

| Layer | Technology |
|-------|-----------|
| Shell | Tauri 2 |
| Frontend | React 18 + TypeScript strict |
| Styling | Tailwind CSS v4 + shadcn/ui (new-york, dark) |
| State | Zustand 5 + TanStack Router v1 |
| Backend | Rust (Tauri commands) |
| AI | Python sidecar (OpenAI SDK + Anthropic SDK) |

## Getting started

```bash
npm install
pip install -r sidecar/requirements.txt
npm run tauri dev
```

## Configuration

Go to **Settings → Intelligence Artificielle** and add an API key:
- [OpenRouter](https://openrouter.ai) — recommended
- Anthropic direct
- Ollama (local, no key needed)

Keys are stored in `%APPDATA%\getpostcraft\api_keys.json` — never leave the machine.

## V1 scope (Instagram)

- [x] Caption + hashtag generation (French, plain text)
- [x] BYOK key management — OpenRouter / Anthropic / Ollama
- [x] Editable hashtags, copy to clipboard
- [ ] Instagram OAuth publishing
- [ ] Visual post creation (HTML to PNG)
- [ ] Post history dashboard

## License

[BUSL-1.1](LICENSE) — personal use only until 2030-04-11, then MIT.
