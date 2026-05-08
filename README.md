<div align="center">

# Getpostcraft

**Local-first AI content studio for Instagram & LinkedIn.**
Generate captions, carousels, and post visuals from a brief — your keys stay on your machine.

[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-1f6feb?style=flat-square)](LICENSE)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.x-FFC131?style=flat-square&logo=tauri&logoColor=black)](https://tauri.app)
[![TypeScript strict](https://img.shields.io/badge/TypeScript-strict-3178c6?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org)
[![Rust stable](https://img.shields.io/badge/Rust-stable-orange?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![CI](https://img.shields.io/github/actions/workflow/status/thierryvm/getpostcraft/ci.yml?branch=main&label=CI&style=flat-square)](https://github.com/thierryvm/getpostcraft/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/thierryvm/getpostcraft?style=flat-square&label=release&color=3ddc84)](https://github.com/thierryvm/getpostcraft/releases/latest)

</div>

---

## What it is

A desktop app that turns a one-line brief into a publish-ready Instagram or
LinkedIn post — caption, hashtags, and a 1080-pixel rendered visual — in
under a minute. Bring your own AI key (OpenRouter, Anthropic, or Ollama).
Everything runs locally: keys live in the OS keychain, posts and accounts
in a local SQLite database. No cloud, no telemetry, no lock-in.

## Why it exists

Existing AI content tools either lock you into their cloud, charge a SaaS
markup on top of API costs, or generate generic outputs that read as
machine-written. Getpostcraft inverts those defaults:

- **You own your keys, data, and history.** Export anytime to JSON or a
  ready-to-load Postgres schema for migration.
- **Algorithm-aware prompts.** Hooks formulas, AIDA structure, anti-AI-tell
  filters, and self-check rules tuned to the 2026 Instagram and LinkedIn
  rankers.
- **Honest per-account context.** A "ProductTruth" block forces the model
  to stay grounded in what your account *actually* publishes — no more
  hallucinated lesson counts or invented features.

---

## Features

### Shipping today

| Capability | Detail |
|---|---|
| Caption + hashtag generation | Network-aware (IG vs LinkedIn), per-account ProductTruth, 4 tone variants |
| Carousel generation | Multi-slide IG, full publish flow (not just slide 1) |
| Visual rendering | HTML → PNG via Playwright Chromium, terminal/code/post layouts |
| Vision-based brand extraction | Pull colours and typography from a website to keep posts on-brand |
| Instagram + LinkedIn OAuth publishing | PKCE on Instagram, full ugcPosts on LinkedIn |
| BYOK provider support | OpenRouter (recommended), Anthropic native, Ollama (local, free) |
| OS-native secret storage | DPAPI / Keychain / libsecret — never plaintext on disk |
| AI usage and cost tracker | Per-model breakdown, month-to-date estimate |
| Token expiry warnings | Badge before the 60-day OAuth window closes |
| Data portability | `.gpcbak` SQLite snapshot or portable JSON + Postgres schema |
| Strict CSP and trufflehog CI | Renderer is locked down, every PR scanned for accidental secrets |
| Auto-update channel | Signed Ed25519 release manifest |

### On the roadmap

| | |
|---|---|
| **V2** | Twitter/X & TikTok adapters, scheduling, A/B variant promotion |
| **V3** | Optional cloud sync (Supabase), team workspaces |

---

## Tech stack

```
┌─ Renderer ─────────────────────────────────────────────────┐
│  React 18 · TanStack Router · Zustand · TanStack Query     │
│  Tailwind v4 · shadcn/ui (new-york dark)                   │
└────────────────────────────────────────────────────────────┘
                            │ Tauri IPC (typed)
┌─ Backend (Rust) ───────────────────────────────────────────┐
│  Tauri 2 · sqlx 0.8 (SQLite WAL) · keyring 3 · reqwest    │
│  rustls · rcgen (localhost OAuth callback)                 │
└────────────────────────────────────────────────────────────┘
                            │ stdin/stdout JSON
┌─ Sidecar (Python) ─────────────────────────────────────────┐
│  Playwright · Pillow · OpenAI / Anthropic SDKs             │
└────────────────────────────────────────────────────────────┘
```

Cross-platform: Windows · macOS · Linux. Single codebase, ~22 MB installer.

---

## Quick start

### Install (end users)

Grab the latest signed installer for your OS from
**[Releases](https://github.com/thierryvm/getpostcraft/releases/latest)**:

| Platform | Asset |
|---|---|
| Windows | `getpostcraft_*_x64_en-US.msi` |
| macOS  | `getpostcraft_*_universal.dmg` |
| Linux | `getpostcraft_*_amd64.AppImage` (also `.deb`, `.rpm`) |

> V1 alpha: SmartScreen / Gatekeeper warnings expected (no OS code-signing
> yet). The app is fully Ed25519-signed at the auto-updater layer.

### Run from source

```bash
git clone https://github.com/thierryvm/getpostcraft.git
cd getpostcraft

npm install
pip install -r sidecar/requirements.txt
python -m playwright install chromium

npm run tauri dev
```

Prereqs: Node 20+, Rust stable, Python 3.11+, plus the
[Tauri OS prerequisites](https://tauri.app/start/prerequisites/).

### First-launch checklist

1. **Settings → Intelligence Artificielle** → paste an OpenRouter or
   Anthropic key. Stored in the OS keychain only.
2. **Settings → Comptes** → connect Instagram or LinkedIn via OAuth.
3. **Composer** → write a brief, generate, render, publish. ~30 seconds.

---

## Security boundary

Getpostcraft handles three classes of secret. Each lives where it belongs:

| Secret | Where it lives | Why |
|---|---|---|
| AI provider key (BYOK) | OS keychain | DPAPI / Keychain / libsecret encrypt at rest |
| OAuth access token | OS keychain | Same. Never crosses IPC to the renderer. |
| Sidecar per-call keys | Process memory only | Wiped after the call returns |

The renderer never receives a secret. The Rust backend reads keys, calls
the upstream API, and returns scrubbed responses.

What we ship to harden the rest:

- **Strict CSP** (`script-src 'self'`, no `'unsafe-eval'`)
- **Log redaction** for upstream OAuth bodies (defense in depth)
- **OAuth PKCE + CSRF state** on every connect flow
- **VACUUM INTO**-based snapshots for the export feature (no WAL races)
- **Trufflehog secret scanning** on every push
- **Migration regression tests** to catch schema breakage before release

Found something? Email **thierry@getpostcraft.app** with details — please
do not file a public issue for vulnerabilities.

---

## Data portability

Settings → Données ships two complementary export formats so the app can
never hold your data hostage:

- **`.gpcbak`** — a ZIP wrapping a transactionally-consistent SQLite
  snapshot. Open it with `sqlite3 app.db` directly. Best for moving to
  another GPC install.
- **Portable `.zip`** — flat JSON tables (`accounts.json`, `posts.json`,
  `settings.json`), media decoded to actual PNG files, plus a
  Supabase-ready `schema.sql`. Best for migrating off GPC entirely.

Both formats deliberately exclude credentials — keys re-bind on a fresh
install via the OS keychain.

---

## Development

```bash
npm run typecheck                                      # TS
cargo check  --manifest-path src-tauri/Cargo.toml      # Rust
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo fmt    --manifest-path src-tauri/Cargo.toml
npm test                                               # vitest
cargo test   --manifest-path src-tauri/Cargo.toml --lib
pytest sidecar/tests/                                  # Python
```

**Quality gates before merge** (enforced in CI):
- TypeScript strict, zero `any`
- Clippy clean with `-D warnings`
- Trufflehog: no verified secrets in the diff
- Migration regression tests
- Vitest, cargo test, pytest all green

### Project layout

```
src/                    React frontend
  components/           UI building blocks (shadcn-based)
  routes/               TanStack Router pages
  lib/tauri/            Typed IPC wrappers
src-tauri/src/          Rust backend
  commands/             Tauri command handlers (one file per domain)
  adapters/             Per-network publishing (Instagram, LinkedIn)
  db/                   SQLite pool, migrations, accounts, history
  ai_keys.rs            OS keychain wrapper for BYOK + client secrets
  token_store.rs        OS keychain wrapper for OAuth tokens
  log_redact.rs         Sanitize upstream bodies before logging
sidecar/                Python AI + Playwright render process
docs/adr/               Architecture Decision Records (1-9)
```

See **[CONTRIBUTING.md](CONTRIBUTING.md)** for branch / commit / PR
conventions.

---

## License

**Business Source License 1.1** with an MIT change date of **2030-04-11**.

In plain English:
- Read, fork, modify, run for **personal** use → free.
- Distribute, host, or run as a **service** → contact for a commercial
  license.
- After 2030-04-11, the entire codebase converts to **MIT** automatically.

Full terms: [`LICENSE`](LICENSE) — commercial inquiries: thierry@getpostcraft.app.

---

<div align="center">

Built solo by [Thierry Vanmeeteren](https://github.com/thierryvm) ·
Belgium · 2026.

If Getpostcraft helps you ship better posts, a star here means a lot.

</div>
