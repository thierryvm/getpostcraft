# v0.4.x Roadmap — Plan de bataille

> Statut : **proposed**, rédigé 2026-05-11 après research compétitive + audit codebase.
> Auteur : @cc-gpc. À ratifier par @thierry.

---

## TL;DR

v0.3.x a posé les fondations (génération AI, multi-network composer, BYOK,
calendrier visuel, recovery hardening). v0.4.x ferme le gap fonctionnel sur
la concurrence (Buffer, Later, Hypefury) sans casser le moat local-first /
BYOK. Quatre features dans l'ordre — auto-publish, analytics fetch, optimal
posting time, content repurposing — plus un saupoudrage UX. 4 releases
mineures.

## Vision

Transformer Getpostcraft d'un **« composer puis publier manuellement »** en
un **assistant éditorial complet** qui :

1. **Génère** — déjà acquis (v0.3.x)
2. **Programme et publie automatiquement** — gap n°1, ferme v0.4.0
3. **Mesure ce qui a marché** — analytics post-publish, v0.4.1
4. **Suggère quand publier** — data-driven optimal time, v0.4.2
5. **Recycle le contenu cross-format** — one-click repurposing, v0.4.3

## Différenciation maintenue

Buffer / Later / Hypefury 2026 sont cloud + abonnement $15-99/mois. Tous ont
auto-publish, analytics, optimal posting time, repurposing.

Getpostcraft v0.4.x reste **local-first + BYOK** (~$0.15/mois pour un usage
typique sur Sonnet 4.6). La parité fonctionnelle ferme le gap « il manque
trop de features » tout en gardant le moat sur prix + privacy +
data-ownership.

**Aucune feature v0.4.x ne demande un backend cloud.** Tout reste pilotable
depuis la machine de l'utilisateur, en arrière-plan, avec ses propres clés
API.

---

## Top features priorisées par ROI

### #1 — Auto-publish scheduling (v0.4.0) — LE big move

**Pourquoi maintenant :** le calendrier visuel existant montre les posts
planifiés mais ne fait rien à l'heure dite. L'utilisateur doit revenir
cliquer « Publier maintenant ». C'est la friction n°1.

**Impact mesurable :**
- LinkedIn 2026 — « Golden hour » : les 60-90 premières minutes après
  publication décident de la portée. Publier précisément à l'heure
  optimale (mardi 19h00) vs « quand j'ai le temps » = 2-5× portée
  attendue (source : recherche viral 2026 dans mémoire projet).
- Instagram 2026 — même logique sur engagement initial.

**Faisabilité technique** : moyenne
- Background task `tokio::spawn` au démarrage de l'app
- Poll DB toutes les 60 s : `SELECT * FROM post_history WHERE
  scheduled_at <= datetime('now') AND status = 'draft'`
- Pour chaque post dû → appelle l'existant `publish_post` /
  `publish_linkedin_post`
- Retry policy : 3 tentatives avec backoff exponentiel (5 min / 30 min / 2 h)
- Check `token_expires_at` (déjà capturé via migration 014) avant publish
  pour éviter un 401 silencieux
- Migration 019 : ajouter `status='publishing'` + `failed_attempts INTEGER
  DEFAULT 0` + `last_attempt_at TEXT` sur `post_history`
- Surface UI : badge « Programmé pour J 19h00 » + ligne « Échec tentative
  1/3 à T+5min, prochain essai à T+30min » + notification système
  (tauri-plugin-notification) sur succès/échec final

**Edge cases à couvrir :**
- App fermée à l'heure prévue → publish au prochain launch dans une
  fenêtre ±15 min, ignore-si-trop-vieux (>24h sans publication =
  considère perdu, demande à l'utilisateur)
- Multiple posts dus en même temps → publication séquentielle, pas
  parallèle (évite rate-limit Meta/LinkedIn)
- Token expiré → status `failed` immédiat avec message « reconnecte ton
  compte dans Paramètres → Comptes »
- User publish manuellement avant l'heure → l'auto-publish voit `status
  = 'published'`, no-op

**Estimation découpée :**
- PR 1 : foundation scheduler module + migration 019 + types + tests
  unitaires (background task non encore branché)
- PR 2 : intégration dans `init_pool`/`run()`, retry policy, token check,
  notifications
- PR 3 : UI dashboard badge + history of attempts panel + Composer
  intégration

**Différenciateur** : aucun (parité avec Buffer/Later). Mais sans ça,
toute autre feature est cosmétique — c'est la **table stake**.

### #2 — Post-publish analytics fetch (v0.4.1)

**Pourquoi maintenant :** l'utilisateur poste mais ne sait pas si ça
marche. Sans feedback, impossible d'itérer la stratégie. Tous les
concurrents le font.

**Faisabilité :** moyen-haut, dépend de l'accès aux API engagement.

**APIs :**
- **Instagram Graph API** : `GET /{media_id}/insights` →
  `metric=reach,impressions,saved,likes,comments,profile_visits,shares`.
  Disponible sur Business + Creator accounts. Limite : 90 jours
  d'historique.
- **LinkedIn API** : `GET /socialActions/{urn}` →
  `numLikes, numComments, numShares`. Pas de reach/impressions sur les
  posts perso (uniquement Pages).

**Stockage :**
- Migration 020 : `CREATE TABLE post_analytics (post_id INTEGER, fetched_at
  TEXT, metrics TEXT)` (metrics = JSON pour rester forward-compat).
- Pas une table 1:1 sur `post_history.id` parce qu'on veut historiser
  l'évolution des métriques au fil du temps (engagement croît sur 48 h).

**Logique fetch :**
- Posts < 7 jours → fetch chaque heure (capture la croissance virale
  initiale).
- Posts 7-30 jours → fetch chaque jour.
- Posts > 30 jours → fetch 1× par semaine.
- Background task qui pilote l'orchestre — mêmes contraintes que
  l'auto-publish (sérialisation, rate-limit awareness).

**UI :**
- Sur le détail d'un post : 3 chiffres clés (likes / comments / saves)
  + petite courbe d'évolution si > 1 fetch
- Dashboard : nouvelle section « Top posts dernières 30 jours par
  engagement » — actionable pour décider quoi repurposer

**Hashtags 2026 Instagram** : 10 hashtags, 3 larges + 5 niche + 2
communauté (memory `reference_viral_posts_research.md`). L'analytics
permet de mesurer **lesquels** marchent → feedback loop sur la stratégie
hashtag.

**Estimation découpée :**
- PR 1 : migration 020 + module `db::analytics` + Tauri command fetch IG
- PR 2 : Tauri command fetch LinkedIn (différent endpoint)
- PR 3 : UI post detail + dashboard top-posts

### #3 — Optimal posting time AI (v0.4.2)

**Bloqué par :** analytics doit avoir accumulé ≥ 30 jours de data sur le
compte de l'utilisateur. Donc ship après v0.4.1.

**Algorithme :**
- Groupe les engagements (likes+comments+saves pondérés) par heure-de-la-
  journée × jour-de-la-semaine.
- Lisse avec moyenne mobile (atténue les outliers viraux).
- Retient le top 3 créneaux.
- Affichage Composer : « Ton meilleur créneau IG = mardi 19h00 (+38%
  d'engagement vs moyenne) ».
- Affichage Calendar : highlight le créneau optimal sur la grille
  hebdomadaire.

**Pas de cloud, pas d'API externe.** Calcul 100 % en SQLite + Rust à
partir des `post_analytics` locaux. Aucun risque GDPR.

**Estimation :** 1 PR mineure (modules + commands + UI).

### #4 — Content repurposing one-click (v0.4.3)

**Cas d'usage typique :** un post LinkedIn 1800 chars qui marche bien →
« Repurpose pour Instagram en carrousel 5 slides ». L'AI extrait la
structure, condense par slide, optimise les hooks. Sens inverse aussi
(carrousel IG → LinkedIn long-form prose).

**Sidecar nouveau prompt :** dédié, prend en input le post source + le
format cible + le réseau cible. Output = JSON formaté pour le format cible.

**UI :** bouton « Repurposer pour... » sur ContentPreview, menu dropdown
des formats accessibles (filtré selon le réseau source + cible).

**Aligné avec memory** : règles cross-platform (LinkedIn pas de liens
corps, Instagram CTA save/DM). L'AI applique les règles correspondant au
réseau cible.

**Estimation :** 1 PR.

### #5 — UX polish backlog issue #62 (saupoudrage)

10 items collectés au smoke test v0.3.9. Reproduits ici avec triage
impact/difficulté :

| # | Item | Impact | Diff | Bundle |
|---|---|---|---|---|
| 1 | Tooltip explicite sur bouton ×3 | 🟢 | trivial | v0.4.0 |
| 2 | Loading spinner full-page pendant gen | 🟡 | facile | v0.4.0 |
| 3 | Modal "voir réponse complète" sur JSON parse fail | 🟢 | moyen | v0.4.0 |
| 4 | Auto-détecter URL dans brief → propose mode URL | 🟢 | facile | v0.4.1 |
| 5 | Cost banner breakdown par réseau | 🟡 | facile | v0.4.1 |
| 6 | Navigation clavier entre tabs GroupResultPanel | 🟡 | facile | v0.4.1 |
| 7 | Empty state "Connecter @network" sur réseau coché sans compte | 🟢 | moyen | v0.4.1 |
| 8 | Bouton Cancel pendant génération en cours | 🟢 | moyen-dur | v0.4.3 |
| 9 | Restaurer la sélection multi-network du dernier composer | 🟡 | facile | v0.4.3 |
| 10 | Indicator "sidecar prêt" pour le 1er prompt | 🟡 | facile | v0.4.3 |

Tri par impact : 1, 3, 4, 7, 8 = **🟢 haute valeur** (résolvent friction
réelle observée). 2, 5, 6, 9, 10 = **🟡 nice-to-have**.

---

## Test coverage strategy (parallel work)

État au 2026-05-11 : Rust **226/226**, Frontend 105/105, Python sidecar
60/60. Gaps audit complétés :

| Module | Pub fns | Stratégie | Bundle |
|---|---|---|---|
| `commands/calendar.rs` | 5 | Tests intégration command-level (mock state) | Au prochain touch v0.4.x |
| `commands/settings.rs` | 6 | Round-trip via fake keychain (`keyring` mock) | v0.4.0 (la feature touch settings_db pour scheduler config) |
| `adapters/instagram.rs` | 4 | Wiremock pattern (déjà existant pour `publisher.rs`) | v0.4.1 (analytics fetch reuse l'adapter) |
| `commands/logs.rs` | 2 | Tests trivial — read tempfile content | PR "test sweep" v0.4.0 |
| `commands/python_deps.rs` | 2 | Integration only (process spawn) — hors scope unit | Backlog v0.5 quand on a CI Python |

**Cible 240+ tests Rust à fin v0.4.0.**

---

## Sequencing des PRs

```
v0.4.0 ─── F1 scheduler foundation (migration 019 + module + types + tests)
        ├─ F2 scheduler integration (background task + retry + notif)
        ├─ F3 scheduler UI (dashboard badge + history + Composer)
        ├─ F4 test sweep (logs + settings + 1-2 calendar tests)
        └─ F5 UX polish (items 1, 2, 3)

v0.4.1 ─── F6 analytics foundation (migration 020 + db::analytics)
        ├─ F7 analytics IG fetch (Meta Graph insights)
        ├─ F8 analytics LinkedIn fetch
        ├─ F9 analytics UI (post detail + dashboard top)
        ├─ F10 instagram adapter tests (wiremock)
        └─ F11 UX polish (items 4, 5, 6, 7)

v0.4.2 ─── F12 optimal posting time (data-driven from analytics)

v0.4.3 ─── F13 content repurposing one-click
        └─ F14 UX polish (items 8, 9, 10)
```

Estimation calendaire (pace soutenu d'1 PR/jour comme la stack v0.3.9-10) :
**~3 semaines réelles** sur v0.4.x complète, avec validation utilisateur
intercalée.

---

## Success criteria

### v0.4.0
- [ ] Schedule un post à T+5min via Calendar → publié auto à l'heure ±60s
- [ ] Schedule fail (token expired) → 3 retries avec backoff → notification
      système avec "Reconnecte ton compte"
- [ ] Schedule miss (app fermée à l'heure prévue) → publication au launch
      suivant si <15min de retard, sinon prompt utilisateur
- [ ] Test coverage : Rust **240+** (+14 vs 226)
- [ ] Smoke test desktop : publier 2 posts programmés à 5min d'intervalle
      sans intervention manuelle

### v0.4.1
- [ ] Sur un post publié il y a >1h, voir likes/comments fetchés et
      affichés dans le détail
- [ ] Dashboard "Top posts 30j par engagement" pondère likes/comments/saves
      selon les règles 2026 (saves > comments > likes pour IG)
- [ ] Test coverage : Rust **260+** (+20 vs v0.4.0)

### v0.4.2
- [ ] Composer affiche un badge "Meilleur créneau IG mardi 19h00" basé
      sur 30j+ d'analytics
- [ ] Si <30j de data → affiche "Pas encore assez de données — publie 5+
      posts pour activer"

### v0.4.3
- [ ] Bouton "Repurposer pour..." sur ContentPreview ouvre menu →
      sélection format → AI regénère → nouveau draft créé
- [ ] Préserve le lien source (`group_id` chaîne au repurposé)

---

## Hors scope v0.4.x — backlog v0.5+

- Multi-compte sur même réseau (ex: deux comptes LinkedIn)
- Stories / Reels publishing (nouvelle API IG nécessaire)
- Niveau C stat-tiles carousel (différé par décision Thierry, brief
  required)
- Multi-language UI (FR-only confirmé)
- Cloud sync optionnel pour multi-machine (contradiction avec local-first
  philosophy)
- Team / multi-user (out of philosophy)

---

## Sources externes (research mai 2026)

- [Buffer 2026 review](https://socialrails.com/blog/buffer-review)
- [Hypefury 2026 features](https://creatortrail.com/hypefury-review-2026/)
- [Best AI social media tools 2026](https://zapier.com/blog/best-ai-social-media-management/)
- [Tauri scheduling Tokio cron patterns](https://github.com/mvniekerk/tokio-cron-scheduler)
- [Tokio cron scheduler](https://lib.rs/crates/tokio-cron-scheduler)

## Sources internes (memory + repo)

- `claude-config/memory/reference_viral_posts_research.md` — règles algo
  Instagram + LinkedIn 2026 (DM shares > saves > comments > likes ; golden
  hour LinkedIn ; longueur caption optimal par réseau)
- `claude-config/memory/feedback_technical_ownership.md` — délégation
  technique totale (informe la pace d'exécution et l'autonomie sur les
  PRs)
- `docs/guides/recovery.md` — patterns de fragilité DB déjà adressés
  (heal + STARTUP_BLOCKED)
- `CHANGELOG.md` — état des features livrées jusqu'à v0.3.10
