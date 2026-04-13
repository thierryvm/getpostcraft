# Changelog

All notable changes to Getpostcraft are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [Unreleased]

### Planned (V2 ideas)
- **Product Truth per account** — a "context / product truth" field per connected account, injected into AI prompts to constrain generation to what actually exists. Prevents publishing content about features that aren't live yet. Inspired by real-world editorial discipline: only show what exists, clearly mark what's in progress or planned. Source: Terminal Learning Instagram strategy (April 2026).

### Added
- `render_post_image` Tauri command — renders caption + hashtags to a 1080×1080 PNG via Python/Playwright sidecar
- `warmup_sidecar` command — pre-loads Python interpreter when Composer opens to reduce first-generation latency
- "Générer l'image" button in ContentPreview with spinner and inline preview (max-h-72)
- `CONTRIBUTING.md` — branch naming, commit conventions, PR and issue process
- Issue templates: feature request, chore/tech-debt, improved bug report
- PR template with related-issue linkage, test plan, and quality checklist
- Instagram OAuth now exchanges short-lived token for long-lived token (~60 days) immediately after code exchange — prevents "Session has expired" errors on publish
- Image upload fallback chain for Instagram publishing: Catbox → Litterbox → tmpfiles.org → 0x0.st — publish no longer fails if one free host is down
- OpenRouter model list updated with pricing ($/1M tokens input/output) and `⚠ instable` warning for free-tier endpoints that may go offline
- Settings AI page now loads the saved provider + model from SQLite on mount — selection persists across app restarts
- Migration `005_reset_dead_models` — auto-resets deprecated free model IDs to `claude-3-5-haiku` on startup

### Fixed
- Playwright rendered accented French characters as garbled Latin-1 — fixed by writing HTML to a UTF-8 temp file and using `page.goto(file://)` instead of `page.set_content()`
- Image preview used `w-full aspect-square` making it full-screen — now `max-h-72 object-contain`
- Asset protocol path resolution failure on Windows — switched to `data:image/png;base64` data URL returned directly from Rust
- Migration `005_reset_dead_models` referenced `app_settings` instead of `settings` — caused startup panic on fresh installs
- Free OpenRouter models returning 404 (`mistralai/mistral-7b-instruct:free`, `deepseek/deepseek-r1:free`) removed from model list

---

## [0.1.0] — 2026-04-11

### Added
- Tauri 2 desktop application scaffold (React 18 + TypeScript strict + Tailwind v4 + shadcn/ui new-york dark)
- TanStack Router v1 with three routes: Dashboard `/`, Composer `/composer`, Settings `/settings`
- Collapsible sidebar with navigation and state persisted via Zustand
- **Composer** — brief input → AI generation → caption + editable hashtags + copy buttons
- AI generation via Python sidecar (OpenAI-compatible + Anthropic native): OpenRouter, Anthropic, Ollama
- BYOK key management — stored in `%APPDATA%\getpostcraft\api_keys.json` (keyring v3 replaced due to Windows bug)
- In-memory key cache pre-loaded at startup — no keychain re-reads per generation call
- Active provider + model persisted to SQLite (`settings` table)
- Post history saved to SQLite (`post_history` table) with `draft` / `published` / `failed` status
- **Dashboard** — stat cards (total / published / drafts) + recent history list from SQLite
- Settings → **Intelligence Artificielle** tab — add / test / delete API key, switch provider/model
- Settings → **Comptes** tab — Instagram connection placeholder (OAuth V2)
- Settings → **À propos** tab — version badge, stack info, BUSL-1.1 license
- Publish button redirects to Accounts tab when no account is connected
- SQLite WAL mode with automatic migrations (`src-tauri/src/db/migrations/`)
- Python sidecar: JSON control-character sanitisation for malformed model output
- Network rules: French-only Instagram system prompt, plain-text enforced
- Resizable window (1100×700 default, 640×480 minimum)
- GitHub Actions CI: TypeScript typecheck, Rust check + clippy + fmt, Python import check
- BUSL-1.1 licence — Change Date 2030-04-11, Change Licence MIT
- 8 Architecture Decision Records (`docs/adr/`)

[Unreleased]: https://github.com/thierryvm/getpostcraft/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/thierryvm/getpostcraft/releases/tag/v0.1.0
