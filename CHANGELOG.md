# Changelog

All notable changes to Getpostcraft are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [Unreleased]

## [0.3.9] — 2026-05-11

### Added — multi-network composer (v0.3.9 stack)
- **Migration 018 + `db::groups`** (PR #56) — sibling-row model that
  lets one Composer pass produce N drafts (one per network) bound by
  a shared `group_id`. NULL on legacy rows, no backfill needed.
- **`generate_and_save_group` Tauri command** (PR #57) — fans out N
  sidecar calls in parallel via `tokio::task::spawn`, persists
  successes as a transactional `post_groups` parent + N sibling
  drafts. Best-effort: a single failing network does NOT abort the
  whole flow.
- **Composer UI multi-network mode** — checkbox grid replaces the
  v0.3.8 single-network dropdown. 1 ticked → existing single flow
  (zero behaviour change for solo users). 2-3 ticked → new group
  flow. Per-network account selectors cascade-reveal under each
  checked network so the IG and LinkedIn picks stay independent.
  Hard-cap of 3 networks per group is enforced both in the UI
  (4th checkbox disabled) and in the Tauri command (rejected
  before any AI call fires).
- **Cost estimate banner** in the Composer — surfaces an
  upper-bound USD estimate for the upcoming generation
  (≈ $0.0028 for IG+LinkedIn on Sonnet 4.6) sourced from the
  OpenRouter pricing snapshot already used by the AI usage panel,
  so a model change in Settings reflects automatically. Hidden in
  dev mode (no Tauri runtime → no pricing data).
- **Group result panel** replaces the rich ContentPreview when a
  multi-network generation completes. Per-member tile shows
  status (ok / error), caption preview, hashtag preview, and a
  "Continuer sur {network}" button that loads that sibling as the
  active draft (single-network mode) for the rich edit/publish
  flow. Failed members get an inline retry that re-runs just that
  network and merges the result back without losing the other
  siblings.

### Changed
- `composer.store` carries `selectedNetworks: Set<Network>`,
  `accountIds: Partial<Record<Network, number | null>>`, and
  `groupResult` alongside the legacy `network` / `accountId` /
  `result`. The legacy fields stay in sync with the new ones so
  unmodified callers (single-network code paths) keep working.
- The ×3 variants button is mono-network only — running 3 tones × 2
  networks would be 6 parallel AI calls, which both blows the cost
  banner and turns the preview UI into a 6-tab grid that doesn't fit
  on a laptop screen. The button is disabled with a tooltip
  explaining the constraint when 2+ networks are ticked.

### Tests
- Frontend tests updated to match the multi-network store shape
  (`selectedNetworks`, `accountIds`, `groupResult`). 105/105 still
  green, no behavioural regression on the legacy single-network
  flow that the existing tests exercise.
- `PostRecord` test fixtures pick up the new `group_id: null`
  field (legacy single-network mock posts).

### Added — group visibility surfaces
- `PostRecord.group_id` is now exposed by every Rust query
  (`get_by_id`, `list_recent`, `list_in_range`) and the matching
  TypeScript type. The dashboard list, the dashboard detail sheet,
  and the calendar detail modal each render a compact "Groupe #N"
  badge when the post belongs to a multi-network group, so the
  user can see at a glance which drafts were generated together.
- New `GroupBadge` component (`src/components/shared/`) with two
  visual weights (inline pill for list rows, chip for detail
  headers). NULL group_id renders nothing — legacy mono-network
  rows are untouched.

### Fixed — post-smoke hotfixes
- **Cryptic JSON parse errors** (PR #60) — `_parse_json_response` in
  the sidecar now surfaces the raw model response (first 200 chars)
  when the model returns empty or malformed JSON, plus a French
  user-facing explanation listing likely causes (URL-fetch demand,
  content-policy refusal, rate limit). Replaces the bare
  `Expecting value: line 1 column 1 (char 0)` traceback that gave
  no actionable info.
- **Select trigger displayed "✓ Product Truth" inside the dropdown
  label** (PR #60) — Radix's `<SelectValue />` was rendering the
  full `<SelectItem />` children inside the trigger. Split:
  `SelectItem` = `@handle` only, Product Truth status is a separate
  line below the Select (orange warning for missing-truth, primary
  green for loaded).
- **Race on account auto-select** (PR #61) — a user clicking
  Générer in the same frame as their last network toggle could
  outrun the useEffect that auto-picks the single-match account.
  Defensive submit-time resolver re-reads the canonical answer
  (one matching account = pre-select it) synchronously, forwards
  to the Tauri command + back to the store. Networks with 0 or
  2+ matches keep their existing pick untouched.
- **Composer responsive breakpoint dropped from `lg:` (1024px) to
  `md:` (768px)** (PR #61) — narrow Tauri windows now show the
  brief sidebar + preview pane side-by-side instead of stacking
  vertically and pushing the preview off-screen.

### Security notes
- API key resolved once from the keychain and passed by reference
  into each parallel sidecar call. Same discipline as
  `generate_content` — never logged, never returned to the renderer,
  never persisted in cleartext on disk.
- Hard caps on the new command: brief ≥ 10 chars, 1–3 networks per
  group, no duplicate networks. Each is enforced before any AI call
  fires so a malformed payload can't waste tokens.

## [0.3.8] — 2026-05-10

### Fixed
- **Editorial calendar reflects what actually shipped** — a draft
  scheduled for May 9 and published on May 10 used to stay glued on
  May 9 in the calendar grid forever. The frontend `getPostDate` and
  the backend `list_in_range` SQL query both now follow the same
  most-concrete-event-wins precedence (`published_at > scheduled_at >
  created_at`). Posts move to their actual publish day after the
  publish completes.

### Added
- **Status pill on every calendar tile** — `Brouillon` / `Planifié` /
  `Publié` / `Échec` is now visible at a glance without opening the
  detail modal. Same colour palette as the dashboard / detail view so
  the visual language stays consistent across surfaces.

### Tests
- **+3 Rust** integration tests on `db::history::list_in_range` (fresh
  in-memory pool): published-day bucketing, full precedence ladder,
  no-duplicate when all three date columns match.
- **+2 frontend** calendar timezone tests covering `published_at`
  precedence (with and without a prior `scheduled_at`).
- Frontend **105 / 105** (was 103/103), Rust **187 / 187** (was 184/184).

## [0.3.7] — 2026-05-10

### Added
- **Live OpenRouter pricing** — replaces the hardcoded `pricing_map()` with
  a fetch from `https://openrouter.ai/api/v1/models`. Refreshed once 5 s
  after startup (fire-and-forget) and on demand from a new "Rafraîchir"
  button in Settings → Intelligence Artificielle → Coût d'utilisation.
  When OpenRouter is unreachable the static fallback table answers
  correctly — live pricing is purely a freshness boost. Anthropic native
  and Ollama models stay on the static table since they don't go through
  OpenRouter.
- **Pricing freshness banner** in the AI usage panel: shows model count,
  last-refresh relative time ("il y a 3 minutes"), and any error from
  the most recent fetch.
- **Instagram + LinkedIn permalink** stored at publish time so the
  "Voir sur {network}" button deep-links to the actual post instead of
  the account profile feed (Instagram) or a rebuilt URN (LinkedIn).
  Migration 017 adds the nullable `published_url` column on
  `post_history`; legacy rows keep their previous behaviour.
- **Rotated pre-migration snapshots (N=3)** — `init_pool` now keeps the
  three most recent pre-migration DB copies, named
  `<stem>.db.pre-migrate-{UTC-TIMESTAMP}.bak`, instead of a single
  overwritten file. A user who relaunches the app once or twice after a
  bad migration still has the pre-failure copy on disk. The legacy
  unrotated `<stem>.db.pre-migrate.bak` is removed on first v0.3.7
  launch once a timestamped snapshot exists. Disk cost is bounded —
  three SQLite files for the typical drafts-only DB add up to
  single-digit MB. See [docs/guides/recovery.md](docs/guides/recovery.md)
  for the rollback procedure.
- **Dev-mode IPC fallback** — every `invoke()` runs through
  `src/lib/tauri/invoke.ts`, which detects a missing
  `window.__TAURI_INTERNALS__` and rejects with a typed
  `TauriRuntimeUnavailableError` instead of the cryptic
  `Cannot read properties of undefined`. A `DevModeBanner` shows once
  at the app root in Vite-only mode (gated on both
  `import.meta.env.DEV` AND the runtime probe so it can never appear
  in a production bundle); the AI usage panel renders a muted hint
  instead of a destructive red block when the runtime is absent.

### Changed
- `network_rules::price_for_with_live_cache` joins the live cache lookup
  with the existing static fallback. Historical token counts re-price
  automatically when the cache refreshes — no migration needed.
- `db::ai_usage::summarise` now takes a `&PricingCache` so cost
  computation uses live rates when available.

### Fixed
- **Native time/date picker icons** — Chrome's default
  `::-webkit-calendar-picker-indicator` was nearly invisible on the
  dark theme. CSS now inverts and dims them for contrast.

### Tests
- **+5 Rust** tests on the pricing cache (new_cache empty, populated lookup,
  is_stale never-filled / freshly-refreshed / older-than-max-age).
- **+5 Rust** snapshot rotation tests (timestamped creation + legacy
  cleanup, first-launch no-op, N=3 retention across many runs, unrelated
  `.bak` files untouched, non-default DB filename keeps legacy compat).
- Test setup injects a stub `__TAURI_INTERNALS__` so the existing
  `vi.mock("@tauri-apps/api/core")` keeps driving unit tests after the
  IPC wrapper migration. Frontend stays at 103/103 green.

## [0.3.6] — 2026-05-09

### Fixed
- **Instagram publish failed via Litterbox URL** — Meta's CDN rejects fetches
  from `litter.catbox.moe` with error code 9004 / subcode 2207052 ("Only photo
  or video can be accepted as media type"). Litterbox is removed from the free
  upload fallback chain. New chain: **Catbox → 0x0.st → tmpfiles.org**. If all
  free hosts fail, configure an imgbb key in Settings → Publication.

### Added — Drafts go from dead-ends to one-click ship
- **Actionable drafts everywhere** — every saved post (Dashboard list,
  Calendar cells, detail modals) now shows its generated thumbnail plus
  inline buttons to **Ouvrir** in the Composer, **Publier maintenant**, or
  **Supprimer**. No more dead drafts.
- **Carousel-aware previews** — multi-slide drafts show all slides in a
  scrollable strip with a slide count badge. Single posts get a tall
  preview.
- **Calendar planning UX** — every day cell has a discreet `+` button on
  hover that opens a picker of unscheduled drafts; pick a draft + time
  and it's on the calendar in one click. Per-cell mini-thumbnails and
  network-color dots make the editorial view scannable at a glance. A
  network filter row hides what you're not planning right now. The
  reschedule modal now lets you set both date and time directly,
  without needing to drag-and-drop.
- **Network filter on Calendar** — multi-network users can mute one
  channel while planning another.

### Added — Visual renderer Niveau A + B
- **4:5 portrait by default for IG carousels** (1080×1350) — square stays
  selectable. The portrait gives the typography-heavy templates more room
  and matches the IG mobile feed's optimal ratio.
- **Subtle 60×60 grid background** drawn from an inline SVG data URI —
  zero extra HTTP, just a tech-niche texture under the content.
- **Monospace counter top-right** (`01 / 07`), **brand stamp bottom-right**
  (`>_ @handle`) — anchors the post visually like a terminal session.
- **Left-aligned hero typography** at ~88px — replaces the centered
  emoji-and-bar layout that read as a slide deck.
- **Slide roles drive badge color and label** — the AI now tags each
  slide with one of `hero`, `problem`, `approach`, `tech`, `change`,
  `moment`, `cta`. Brand-aligned roles (hero, approach, cta) inherit
  the brand color so the post opens, climaxes, and closes on the brand
  signature; problem is red, tech is blue, change is amber, moment is
  violet — a 7-slide carousel reads like a colour-coded narrative arc.

### Changed
- `render_carousel_slides` now accepts optional `width` / `height` and
  defaults to 1080×1350. The Composer forwards the user-picked
  `imageFormat` so the rendered slides follow the chosen ratio.
- The carousel JSON contract gained a `role` field. Sidecar parser
  whitelists known roles + normalises case; unknown values become
  `null` so the Rust renderer falls back to its index-derived label.

### Hardened — DB resilience
- **Migration checksum healing** — fixes the `migration N was
  previously applied but has been modified` startup panic that hit
  users upgrading from v0.3.5 to v0.3.6. `init_pool` now reconciles
  the bytes embedded in the binary against `_sqlx_migrations.checksum`
  for already-applied rows before letting `migrate!().run()` enforce
  its mismatch check. The schema is unchanged; only the recorded
  hash is updated. Brand-new installs are detected and bypass the
  heal entirely.
- **Pre-migration snapshot** — every successful startup copies the
  live `app.db` to `app.db.pre-migrate.bak` next to it BEFORE
  `migrate!()` touches the schema. Best-effort (logged warn on I/O
  failure rather than aborting startup), single rotated copy
  (overwritten each launch). The daily auto-backup in
  `~/Documents/Getpostcraft/backups/` continues to cover archival
  recovery — this snapshot closes the 5-minute window between
  upgrade install and the first daily backup. See
  [docs/guides/recovery.md](docs/guides/recovery.md) for the
  rollback procedure.

### Hardened — visual renderer
- **Brand color validation** — `Brand::resolve` now rejects anything
  that isn't a strict `#RRGGBB` hex (no `rgb()`, no 8-digit, no named
  colors) and falls back to the default with a warn log. Prevents
  silent CSS corruption when `{brand_color}55` is concatenated.
- **Role sequence sanity** — the sidecar's carousel parser refuses
  degenerate outputs ("approach" before any "problem", or one role
  flooding > 60 % of the middle slides) and strips all roles to
  `None` on failure. Graceful degradation: index-derived labels keep
  rendering. Logged to stderr so the user can re-generate if they
  want a tagged version.

### Tests
- **+21 Rust** tests (visual chrome, role mapping, canvas-scaling,
  brand hex validation across 7 invalid/valid inputs, +6 DB
  resilience: heal checksums repairs drift / noops on fresh DB /
  noops on in-sync DB, snapshot copies+overwrites+noops) → **180 /
  180**.
- **+9 Python** tests (role whitelist + sequence sanity check) →
  **60 / 60** green.
- **Frontend** tests retargeted at the new Dashboard / PostActions
  flow (single-button delete toggle, network-routed publish,
  inline edit) → **99 / 99** green.

## [0.2.0] — 2026-05-07

### Added
- **Carousel publishing fix (PR #11)** — Instagram CAROUSEL flow (3+ children → parent → publish) + LinkedIn multi-image gallery (N register/upload + media array). Auparavant seule la 1re slide était publiée silencieusement.
- **Reload draft from history (PR #12)** — bouton "Ouvrir" sur les drafts du calendrier → recharge dans le Composer avec bouton "Publier" visible. Résout le cas "drafts en historique impossibles à publier".
- **Wiremock integration tests (PR #13)** — 12 tests qui mockent les API Meta + LinkedIn et assertent l'ordre/body de chaque appel. Total 81 tests Rust (vs 41). Aurait attrapé le bug carousel avant prod.
- **Auto-updater Tauri (PR #6)** — UI "Mises à jour" dans Settings → À propos avec status badge, progress bar, console log inline. Endpoint manifest sur GitHub Releases.
- **Release pipeline multi-OS (PR #6)** — workflow GitHub Actions matrix windows/macos/linux, version-check fail-fast, generate-update-manifest, signature Ed25519. 0€ d'infra.
- **Website analyzer ProductTruth (PR #10)** — bouton "Analyser depuis URL" dans Settings → Comptes. Playwright render le SPA + Sonnet 4.6 synthétise un Product Truth structuré (chiffres, modules, voix). Preview éditable avant apply.
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

[Unreleased]: https://github.com/thierryvm/getpostcraft/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/thierryvm/getpostcraft/releases/tag/v0.2.0
[0.1.0]: https://github.com/thierryvm/getpostcraft/releases/tag/v0.1.0
