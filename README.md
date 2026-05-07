# Getpostcraft

> AI-assisted social media content creation — Tauri 2 desktop app

[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-orange)](https://tauri.app)
[![TypeScript](https://img.shields.io/badge/TypeScript-strict-blue)](https://www.typescriptlang.org)
[![CI](https://github.com/thierryvm/getpostcraft/actions/workflows/ci.yml/badge.svg)](https://github.com/thierryvm/getpostcraft/actions/workflows/ci.yml)

Generate Instagram captions, hashtags, and 1080×1080 post visuals from a brief using AI.  
Built for the [@terminallearning](https://instagram.com/terminallearning) account — Linux/Terminal/DevOps niche.

---

## Features

| Feature | Status |
|---------|--------|
| Caption + hashtag generation (French, plain text) | ✅ |
| BYOK key management — OpenRouter / Anthropic / Ollama | ✅ |
| Editable hashtags + copy to clipboard | ✅ |
| Visual post creation — HTML → PNG 1080×1080 via Playwright | ✅ |
| Post history dashboard (SQLite) | ✅ |
| Instagram OAuth publishing | 🔜 V1 |
| Multi-network — LinkedIn, Twitter/X, TikTok | 🔜 V2 |
| Scheduling / background publishing | 🔜 V2 |
| SaaS / multi-user (Supabase) | 🔜 V3 |

---

## Stack

| Layer | Technology |
|-------|-----------|
| Shell | Tauri 2 |
| Frontend | React 18 + TypeScript strict |
| Styling | Tailwind CSS v4 + shadcn/ui (new-york, dark) |
| State | Zustand 5 + TanStack Router v1 |
| Backend | Rust (Tauri commands) |
| Database | SQLite via sqlx 0.8 (WAL mode) |
| AI | Python sidecar — OpenRouter / Anthropic / Ollama |
| Image render | Playwright Chromium (HTML → PNG) |

---

## Getting started

### Prerequisites

- [Node.js](https://nodejs.org) ≥ 20
- [Rust](https://rustup.rs) stable toolchain
- [Python](https://python.org) ≥ 3.11
- Tauri prerequisites for your OS — see [tauri.app/start/prerequisites](https://tauri.app/start/prerequisites/)

### Install

```bash
# Clone
git clone https://github.com/thierryvm/getpostcraft.git
cd getpostcraft

# Frontend dependencies
npm install

# Python sidecar dependencies
pip install -r sidecar/requirements.txt
python -m playwright install chromium
```

### Run in development

```bash
npm run tauri dev
```

### Build for production

```bash
npm run tauri build
```

Bundles produits dans `src-tauri/target/release/bundle/` :
- Windows : `.msi` + `.exe` (NSIS) — installer prêt à distribuer
- macOS : `.dmg` + `.app` (build local impossible depuis Windows/Linux)
- Linux : `.deb` + `.AppImage` + `.rpm`

### Releases multi-plateforme + auto-update

Voir [`docs/RELEASING.md`](docs/RELEASING.md) — pipeline GitHub Actions matrix
windows/macos/linux + signature Ed25519 + manifest auto-update, **0€ d'infra**.

```bash
# Bumper la version puis tagger pour déclencher la release :
npm run check:versions    # vérifie alignement package.json / Cargo.toml / tauri.conf.json
git tag v0.2.0 && git push --tags
```

---

## Configuration

Open **Settings → Intelligence Artificielle** and add an API key:

| Provider | Key required | Notes |
|----------|-------------|-------|
| [OpenRouter](https://openrouter.ai) | Yes | Recommended — access to all major models |
| Anthropic | Yes | Direct Claude API |
| Ollama | No | Local models, no internet required |

Keys are stored in `%APPDATA%\getpostcraft\api_keys.json` and never leave the machine.

---

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for branch naming, commit conventions, and PR process.

### Key commands

```bash
npm run typecheck                          # TypeScript check
cd src-tauri && cargo check               # Rust compile check
cd src-tauri && cargo clippy -- -D warnings  # Lint
cd src-tauri && cargo fmt                 # Format
```

### Project structure

```
src/                  React + TypeScript frontend
src-tauri/src/        Rust backend (Tauri commands)
  commands/           IPC handlers (ai, media, settings)
  db/                 SQLite pool + migrations
sidecar/              Python AI + image render process
docs/adr/             Architecture Decision Records
```

---

## License

[BUSL-1.1](LICENSE) — personal use only until **2030-04-11**, then MIT.  
See [LICENSE](LICENSE) for the full terms.
