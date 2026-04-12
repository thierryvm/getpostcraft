# Architecture Decision Records — Getpostcraft

Ce répertoire contient les ADR (Architecture Decision Records) du projet Getpostcraft.

Un ADR documente une décision architecturale significative : le contexte qui l'a motivée, les options considérées, la décision prise et ses conséquences.

Format inspiré de [Michael Nygard's ADR template](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions).

---

## Index des ADRs

| ADR | Titre | Statut | Date |
|-----|-------|--------|------|
| [ADR-001](./ADR-001-stack-desktop.md) | Framework Desktop — Tauri 2 | ✅ Accepted | 2024 |
| [ADR-002](./ADR-002-frontend-stack.md) | Frontend Stack (React + TypeScript + Tailwind) | ✅ Accepted | 2024 |
| [ADR-003](./ADR-003-oauth-auth.md) | OAuth Authentication — PKCE + TLS local | ✅ Accepted | 2024 |
| [ADR-004](./ADR-004-local-storage.md) | Local Storage — SQLite via sqlx | ✅ Accepted | 2024 |
| [ADR-005](./ADR-005-network-adapters.md) | Network Adapters — un fichier par réseau | ✅ Accepted | 2024 |
| [ADR-006](./ADR-006-python-sidecar.md) | Python Sidecar — rendu HTML→PNG | ✅ Accepted | 2024 |
| [ADR-007](./ADR-007-ai-byok.md) | AI BYOK — Bring Your Own Key | ✅ Accepted | 2024 |
| [ADR-008](./ADR-008-dashboard.md) | Dashboard Architecture — SQLite-first | ✅ Accepted | 2024 |

---

## Documentation technique

Les guides et références techniques détaillés sont dans les répertoires suivants :

### Setup

| Fichier | Description |
|---------|-------------|
| [setup/META_APP_SETUP.md](../setup/META_APP_SETUP.md) | Guide pas-à-pas pour créer et configurer l'application Meta Developer (App ID, App Secret, scopes, redirect URI, testeurs) |
| [setup/GETPOSTCRAFT_SETUP.md](../setup/GETPOSTCRAFT_SETUP.md) | Installation de Getpostcraft, configuration initiale (App ID, App Secret, imgbb, IA) et première connexion Instagram |

### Architecture

| Fichier | Description |
|---------|-------------|
| [architecture/OAUTH_FLOW.md](../architecture/OAUTH_FLOW.md) | Documentation technique complète du flow OAuth 2.0 PKCE : diagramme séquentiel, explication de chaque étape, sécurité, évolution vers le SaaS |
| [architecture/PUBLISHING_FLOW.md](../architecture/PUBLISHING_FLOW.md) | Pipeline de publication Instagram : diagramme séquentiel, rôle d'imgbb, limites API Instagram, alternatives pour le SaaS |

### Guides

| Fichier | Description |
|---------|-------------|
| [guides/SAAS_MIGRATION.md](../guides/SAAS_MIGRATION.md) | Roadmap technique de migration de l'app desktop vers un SaaS multi-utilisateurs : auth, stockage, secrets, CDN, App Review Meta, stack suggérée |

---

## Décisions notables hors ADR

Ces choix ont été faits en cours de développement sans ADR formel, mais méritent d'être tracés :

| Décision | Contexte | Date |
|----------|---------|------|
| `client_secret` **requis avec PKCE** | Meta impose les deux simultanément — non-standard RFC 7636. Le secret est stocké dans `api_keys.json` (répertoire data user, non versionné) | Avril 2026 |
| **TLS auto-signé** (`rcgen`) pour callback OAuth | Instagram exige HTTPS même pour `localhost`. Certificat éphémère généré à chaque flow via `rcgen` + `tokio-rustls` | Avril 2026 |
| **Port fixe 7891** pour le callback OAuth | Meta n'accepte pas les wildcards de port — l'URI doit être enregistrée exactement | Avril 2026 |
| **imgbb** pour hébergement temporaire des images | Instagram Graph API n'accepte pas l'upload direct — exige une URL publique HTTPS | Avril 2026 |
| **Token short-lived uniquement** en V1 | Simplification intentionnelle — le refresh vers long-lived token (60 jours) est prévu en V1.1 | Avril 2026 |

---

## Comment créer un nouvel ADR

1. Copier le template ci-dessous dans un nouveau fichier `ADR-00X-titre-court.md`
2. Renseigner tous les champs
3. Ajouter l'entrée dans le tableau Index ci-dessus

```markdown
# ADR-00X — Titre de la décision

**Date :** JJ mois AAAA  
**Statut :** Proposed | Accepted | Deprecated | Superseded by ADR-00Y  
**Décideurs :** Thierry

## Contexte

[Quel problème cherche-t-on à résoudre ? Quelles contraintes ?]

## Options considérées

1. **Option A** — [description]
2. **Option B** — [description]
3. **Option C** — [description]

## Décision

[Quelle option a été choisie et pourquoi ?]

## Conséquences

**Positives :**
- [...]

**Négatives / Trade-offs :**
- [...]

**Neutres :**
- [...]
```
