# CLAUDE.md — Getpostcraft Project

> This file has **absolute priority** over the global CLAUDE.md.
> Any Claude Code session on this project starts by reading this file.

---

## Project Identity

**Name:** Getpostcraft  
**Type:** Personal desktop tool → future SaaS  
**Purpose:** AI-assisted social media content creation and publishing  
**Domain:** getpostcraft.app  
**License:** BUSL-1.1, Change Date 2030-04-11  
**Owner:** Thierry (sole developer, vibecoder, Belgium UTC+2)  
**Test account:** @terminallearning (Instagram) — dark theme `#0d1117` / `#3ddc84`, niche Linux/Terminal/DevOps

---

## Stack

### Desktop App (Tauri 2)
| Layer | Technology | Version |
|-------|-----------|---------|
| Shell | Tauri 2 | 2.x stable |
| Backend | Rust | stable toolchain |
| Frontend | React | 18.x |
| Language | TypeScript | 5.x strict:true |
| Styling | Tailwind CSS | v4 |
| Components | shadcn/ui | new-york style, dark mode (copied into project) |
| UI State | Zustand | 5.x, per-slice |
| Server State | TanStack Query | v5 |
| Routing | TanStack Router | v1, code-based |
| Forms | react-hook-form + Zod | latest |
| Charts | Recharts | 2.x |
| Dates | date-fns | 3.x |
| DB | SQLite via sqlx | 0.8.x, WAL mode, FTS5 |
| Keychain | keyring crate | 3.x |
| HTTP | reqwest + tokio | latest |
| OAuth | tauri-plugin-deep-link | 2.x, scheme: getpostcraft:// |

### Python Sidecar
| Library | Purpose |
|---------|---------|
| playwright | HTML→PNG rendering (exact network dimensions) |
| pillow | Image resize, crop, format conversion |
| anthropic | Claude API client |
| pyinstaller | Compile to standalone binary |

---

## Project Structure

```
getpostcraft/
├── CLAUDE.md                    ← This file
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs               ← Plugin + command registration
│   │   ├── commands/            ← Tauri IPC commands (one file per domain)
│   │   │   ├── ai.rs            ← generate_content, save_ai_key, test_ai_key
│   │   │   ├── oauth.rs         ← start_oauth_flow, complete_oauth_flow
│   │   │   ├── publisher.rs     ← publish_post, schedule_post
│   │   │   ├── analytics.rs     ← fetch_post_metrics, get_dashboard_data
│   │   │   └── media.rs         ← render_html_to_png, resize_image
│   │   ├── adapters/            ← One file per social network
│   │   │   ├── mod.rs
│   │   │   └── instagram.rs     ← V1 only
│   │   ├── ai_keys.rs           ← Keychain CRUD for AI API keys
│   │   ├── sidecar.rs           ← Python sidecar spawn + JSON communication
│   │   └── network_rules.rs     ← Per-network constraints (char limits, hashtags)
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/
│   ├── routes/
│   │   ├── __root.tsx           ← Layout: sidebar + <Outlet />
│   │   ├── index.tsx            ← Dashboard
│   │   ├── composer/
│   │   │   └── index.tsx        ← Composer (core MVP screen)
│   │   ├── accounts/
│   │   └── settings/
│   ├── components/
│   │   ├── ui/                  ← shadcn/ui components (copied here)
│   │   ├── composer/
│   │   ├── dashboard/
│   │   └── shared/
│   ├── stores/                  ← Zustand slices
│   │   ├── ui.store.ts          ← Sidebar state
│   │   └── composer.store.ts    ← Brief + generated content
│   ├── queries/                 ← TanStack Query hooks
│   ├── lib/
│   │   ├── tauri/               ← Typed invoke() wrappers
│   │   └── utils.ts
│   └── types/                   ← Shared TypeScript interfaces
├── sidecar/
│   ├── main.py                  ← JSON dispatcher (stdin loop)
│   ├── render.py                ← Playwright HTML→PNG
│   ├── images.py                ← Pillow resize/crop
│   ├── ai_client.py             ← Claude API calls
│   └── requirements.txt
└── docs/
    └── adr/                     ← 8 Architecture Decision Records
        ├── README.md
        ├── ADR-001-stack-desktop.md
        ├── ADR-002-frontend-stack.md
        ├── ADR-003-oauth-auth.md
        ├── ADR-004-local-storage.md
        ├── ADR-005-network-adapters.md
        ├── ADR-006-python-sidecar.md
        ├── ADR-007-ai-byok.md
        └── ADR-008-dashboard.md
```

---

## Architecture Rules

### 1. Security boundary — ZERO exceptions
- **Tokens and API keys NEVER cross IPC to renderer.** They live in OS keychain, accessed only by Rust.
- **Sidecar receives API key per-call from Rust only** — never stored in sidecar memory longer than the call.
- **OAuth tokens NEVER sent to Python sidecar** — all social network API calls are made by Rust directly.
- **No secret in SQLite in clear text** — only metadata (provider name, configured boolean, validated_at).
- **No `any` in TypeScript** — compile error, not warning.

### 2. IPC contract
- All Tauri commands are typed: Rust `#[tauri::command]` + TypeScript wrapper in `src/lib/tauri/`
- Commands return `Result<T, String>` — never panic in command handlers
- Large media payloads use **file paths**, never base64 over IPC

### 3. Data layer
- **All data is SQLite-first** — no Supabase in V1
- Each migration is a numbered SQL file in `src-tauri/src/db/migrations/`
- sqlx compile-time query verification enabled

### 4. Python sidecar
- Communication: **newline-delimited JSON over stdin/stdout**
- Actions: `render_html` | `resize_image` | `generate_content`
- Pre-warmed when Composer view opens
- Temp files in `/tmp/getpostcraft/` — cleaned by Rust after use

### 5. UI components
- **Only shadcn/ui** — no other component library
- Tailwind v4 only — no custom CSS unless documented
- Dark theme: background `#0d1117`, accent `#3ddc84`

---

## Coding Conventions

```typescript
// ✅ Correct
const result = await invoke<GeneratedContent>('generate_content', { brief, network })

// ❌ Never
const result: any = await invoke('generate_content', { brief, network })
```

- **Rust:** `cargo fmt` + `cargo clippy -- -D warnings` before commit. No `unwrap()` in prod.
- **Files:** 200 lines max — split if larger
- **Names:** variables/functions/types in English, comments in English
- **Git:** conventional commits in English (`feat/fix/refactor/test/docs/chore/security`)

---

## Session Rules (1 session = 1 objective)

**Before writing any code:**
1. State the objective in one sentence
2. List all files that will be modified/created
3. Identify security implications if any

**Before declaring done:**
- `cargo check` in `src-tauri/` → 0 errors
- `npm run typecheck` → 0 errors
- `npm run tauri dev` → app launches

---

## ADR Status

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Framework Desktop — Tauri 2 | ✅ Accepted |
| ADR-002 | Frontend Stack | ✅ Accepted |
| ADR-003 | OAuth Authentication | ✅ Accepted |
| ADR-004 | Local Storage — SQLite | ✅ Accepted |
| ADR-005 | Network Adapters | ✅ Accepted |
| ADR-006 | Python Sidecar | ✅ Accepted |
| ADR-007 | AI BYOK | ✅ Accepted |
| ADR-008 | Dashboard Architecture | ✅ Accepted |

---

## MVP Scope (V1 — Instagram only)

**In scope:**
- Instagram publishing (OAuth PKCE)
- Content generation: brief → Claude API (BYOK) → caption + hashtags
- Visual creation: HTML template → Python/Playwright → PNG 1080×1080
- Dashboard: post history, AI usage, account health (SQLite only)
- Settings: AI provider + key, account management

**Out of scope for V1:**
- LinkedIn, Twitter/X, TikTok, Facebook (V2)
- Scheduling / background publishing (V2)
- Web capture from external URLs (V2)
- Multi-user / SaaS / Supabase (V3)

---

## Core Workflow (MVP)

```
User types brief
    → invoke("generate_content", { brief, network: "instagram" })
    → Rust reads Claude API key from keychain
    → Rust sends to Python sidecar: { action: "generate_content", api_key, brief, network }
    → Python calls Claude API → returns { caption, hashtags }
    → Rust returns result to renderer
    → React displays preview (caption + hashtag badges)
    → User clicks "Publish" → invoke("publish_post", { account_id, caption, hashtags })
    → Rust calls Instagram API directly (no sidecar for publishing)
    → SQLite stores post in post_history
```

---

## Key Commands

```bash
# Development
npm run tauri dev          # Start app (run from project root)
npm run typecheck          # TypeScript check only
cd src-tauri && cargo check   # Rust compile check
cd src-tauri && cargo clippy -- -D warnings

# Python sidecar (when ready)
cd sidecar && pip install -r requirements.txt
python main.py

# Build
npm run tauri build        # Production build
```

---

## Absolute Rules

1. **Never delete a feature without Thierry's explicit confirmation**
2. **Never change the design system without validation**
3. **Always understand WHY before implementing WHAT**
4. **Security implications stated before any auth/token/key code**
5. **Project compiles and runs before declaring session complete**
6. **ALL files created or written MUST be inside `F:\PROJECTS\Apps\getpostcraft\`** — never write to `F:\`, `C:\Users\`, or any path outside the project root. Each Bash call starts with a fresh CWD; always use absolute paths.
7. **No debug/log/batch files outside the project** — temp output goes to `src-tauri/target/` or system temp; never to the drive root or parent directories.

---

## AI Models — Compatibility Matrix

Models validated for JSON output reliability (sidecar `_parse_json_response`).
Source of truth: `sidecar/tests/test_ai_client.py::TestModelOutputPatterns`.

| Model (OpenRouter ID) | JSON fiable | Notes |
|---|---|---|
| `anthropic/claude-sonnet-4.6` | ✅ | **Recommandé** — propre, pas de fence (défaut OpenRouter) |
| `anthropic/claude-haiku-latest` | ✅ | Économique, propre (alias OpenRouter) |
| `anthropic/claude-opus-4.7` | ✅ | Top qualité, propre |
| `anthropic/claude-opus-4.6-fast` | ✅ | Variante latence réduite |
| `openai/gpt-4o-mini` | ✅ | Parfois fence ```json — géré |
| `openai/gpt-4o` | ✅ | Parfois fence ```json — géré |
| `deepseek/deepseek-chat` | ✅ | Propre |
| `google/gemini-2.0-flash-001` | ✅ | Stable, remplace gemini-flash-1.5 |
| `mistralai/mistral-small-3.1-24b-instruct` | ⚠️ | Texte parasite fréquent — jsonUnreliable:true |
| `*:free` (tous) | ⚠️ | Endpoints instables (404 possible) |
| `mistralai/mistral-7b-instruct:free` | ❌ | Supprimé — endpoint mort |
| `meta-llama/llama-3.2-3b-instruct:free` | ❌ | Supprimé — endpoint mort |

**Règle** : avant d'ajouter un modèle à `settings.types.ts`, ajouter un test dans `TestModelOutputPatterns` qui documente son pattern de sortie.
