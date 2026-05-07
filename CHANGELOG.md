# Changelog

All notable changes to Getpostcraft are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [Unreleased]

### Added
- **Persona-agnostic prompts + image templates** — handle, niche et hashtags spécifiques à un compte ne sont plus en dur ; tout passe par `ProductTruth` + nouvelles colonnes `brand_color` / `accent_color` par compte. Le compte actif sélectionné dans le Composer fournit automatiquement son handle et sa couleur de marque aux 4 templates HTML→PNG (post, code, terminal, carrousel). Fallback `@yourbrand` + `#3ddc84` si aucun compte sélectionné. Débloque l'utilisation multi-projet (Terminal Learning, Ankora, etc.) sans réécrire le code.
- Migration `009_account_branding` — colonnes `brand_color TEXT` et `accent_color TEXT` dans `accounts` (nullable).
- Commande Tauri `update_account_branding` + composant `BrandColorsEditor` (color picker + hex input) dans `InstagramSection` / `LinkedInSection`.
- **Product Truth par compte** — champ texte libre par compte connecté (Instagram, LinkedIn), injecté dans le system prompt IA lors de chaque génération. Contraint la génération aux produits/services réels du compte. Éditable depuis Paramètres → Comptes. Indicateur `✓ Product Truth` dans le sélecteur de compte du Composer.
- **Sélecteur de compte dans le Composer** — dropdown permettant de choisir le compte cible avant génération ; auto-sélection si un seul compte connecté pour le réseau choisi.
- Migration `007_product_truth` — colonne `product_truth TEXT` dans la table `accounts` (nullable, préservée lors des re-connexions OAuth).
- **Journal système (Logs)** — onglet "Logs" dans Paramètres avec `tauri-plugin-log` : fichier de log rotatif (5 Mo × 3), filtre par niveau, copie, ouverture du fichier directement
- `get_app_logs` / `get_log_file_path` — commandes Tauri pour lire les logs depuis l'UI
- `log::info/warn/error` sur les points clés : génération IA (provider, modèle, résultat), exchange token OAuth Instagram
- Script `scripts/free-port.mjs` — libère automatiquement le port 1420 avant le démarrage de Vite (plus d'orphelins de session précédente)
- Migration `006_update_default_model` — met à jour les anciennes entrées DB vers l'ID de modèle correct au démarrage
- Tests unitaires sidecar `TestModelOutputPatterns` — matrice de compatibilité JSON par famille de modèle (Claude, GPT-4o, Mistral, LLaMA, Qwen) — 41 tests au total
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

### Changed
- Modèles Anthropic mis à jour vers la famille 4.5/4.6 : `claude-haiku-4-5-20251001` (défaut), `claude-sonnet-4-6`, `claude-opus-4-6`
- Modèles OpenRouter nettoyés : `gemini-flash-1.5` → `gemini-2.0-flash-001`, `mistral-small` → `mistral-small-3.1-24b-instruct` (marqué `jsonUnreliable`)
- Nouveau champ `ModelOption.jsonUnreliable` pour signaler les modèles qui ne suivent pas les instructions JSON-only de façon fiable
- Script npm `tauri` : `tauri` → `npx tauri` pour résoudre la résolution du binaire sur Windows
- `log crate (0.4)` ajouté aux dépendances Rust

### Fixed
- Exchange token long-lived Instagram : `unwrap_or` silencieux remplacé par `match` explicite avec `log::info/warn`
- LinkedIn fold 140 → 210 caractères (valeur réelle du feed mobile)
- Paramètres IA : changement de modèle sans ressaisir la clé API existante
- UI : suppression du wrapper `max-h-48` autour de la caption (espace mort)
- UI : indicateur de fold en amber + texte après fold en pleine opacité
- UI : image rendue remplit la largeur du conteneur (`w-full h-auto`)
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
