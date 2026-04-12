# Roadmap de migration Desktop → SaaS

> Dernière vérification : avril 2026  
> Audience : développeur senior, décision d'architecture future

Ce document décrit les changements techniques nécessaires pour faire évoluer Getpostcraft d'un outil desktop personnel (V1) vers un SaaS multi-utilisateurs (V3). Les étapes V2 (autres réseaux sociaux, scheduling) sont listées pour situer la migration dans la roadmap globale.

---

## Roadmap globale

```
V1 (actuel)        V2 (prochaine)         V3 (SaaS)
───────────────    ──────────────────     ───────────────────
Desktop Tauri 2    Desktop +              Web SaaS
Instagram seul     LinkedIn, Twitter,     Multi-réseau
OAuth local        TikTok, Facebook       Multi-utilisateurs
SQLite             Scheduling/Queue       PostgreSQL/Supabase
imgbb (BYOK)       Scheduling UI          CDN propre
Claude BYOK        Claude BYOK            Claude côté serveur
```

---

## 1. Ce qui change côté authentification

### V1 — OAuth desktop (actuel)

- Callback sur `https://localhost:7891/callback` (serveur Rust éphémère)
- `client_secret` stocké dans `api_keys.json` sur la machine utilisateur
- Token stocké dans `oauth_tokens.json` sur la machine utilisateur
- 1 utilisateur = 1 installation desktop

### SaaS — OAuth serveur-side

```
Utilisateur (browser)    Next.js API Route           Instagram Meta
       |                      |                            |
       |--GET /api/oauth/start->|                          |
       |                      |--redirect authorize URL--->|
       |<--redirect to instagram.com---------------------|
       |   [login + authorize]                             |
       |<--callback?code=...&state=...-------------------|
       |--GET /api/oauth/callback------------------------->|
       |                      |--POST /access_token------->|
       |                      |<--{ access_token }---------|
       |                      |--store encrypted token (Supabase)
       |<--redirect /dashboard-|                          |
```

**Changements clés :**
- Le callback devient une route Next.js (`/api/oauth/callback`) sur un domaine HTTPS réel
- Le `client_secret` est une variable d'environnement serveur (Vercel env vars), jamais exposé au client
- Le `state` CSRF est stocké en session serveur (cookie signé HttpOnly) plutôt qu'en mémoire Rust
- PKCE reste une bonne pratique mais n'est plus le seul mécanisme de sécurité

**URI de callback à enregistrer dans Meta (SaaS) :**
```
https://app.getpostcraft.app/api/oauth/callback
```

Plus de `localhost` — l'app devra passer la Meta App Review pour utiliser des domaines publics.

### Gestion multi-tenants

```sql
-- Supabase schema
CREATE TABLE instagram_accounts (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id     uuid REFERENCES auth.users(id) ON DELETE CASCADE,
  ig_user_id  text NOT NULL,
  username    text NOT NULL,
  token_enc   bytea NOT NULL,  -- token chiffré avec pgcrypto
  token_iv    bytea NOT NULL,
  expires_at  timestamptz,
  created_at  timestamptz DEFAULT now()
);

ALTER TABLE instagram_accounts ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can only see their own accounts"
  ON instagram_accounts FOR ALL
  USING (auth.uid() = user_id);
```

---

## 2. Ce qui change côté stockage

### V1 — SQLite local

- Fichier unique : `{data_dir}/getpostcraft/getpostcraft.db`
- Accès exclusif depuis l'app Tauri
- Pas de migration en ligne, pas de sauvegarde cloud

### SaaS — PostgreSQL via Supabase

**Migration de schéma :**

| Table SQLite V1 | Table PostgreSQL SaaS | Changements |
|----------------|----------------------|-------------|
| `accounts` | `instagram_accounts` | Ajout `user_id` (FK auth.users), token chiffré |
| `posts` | `posts` | Ajout `user_id`, suppression `image_path` local |
| `settings` | `user_settings` | Ajout `user_id`, certains secrets migrent côté serveur |
| `ai_usage` | `ai_usage` | Ajout `user_id`, facturation possible |

**Stack de migration suggérée :**
- Supabase (PostgreSQL + Auth + RLS + Storage)
- `drizzle-orm` ou `prisma` pour les requêtes type-safe côté Next.js
- Supabase Edge Functions pour les webhooks et jobs légers

---

## 3. Ce qui change côté secrets

### V1 — Secrets locaux

| Secret | Stockage V1 |
|--------|------------|
| `client_secret` Instagram | `api_keys.json` (machine utilisateur) |
| Tokens OAuth | `oauth_tokens.json` (machine utilisateur) |
| Clé Claude | `api_keys.json` (machine utilisateur) |
| Clé imgbb | SQLite `settings` (machine utilisateur) |

### SaaS — Secrets côté serveur

| Secret | Stockage SaaS |
|--------|--------------|
| `client_secret` Instagram | Vercel Environment Variable (`INSTAGRAM_CLIENT_SECRET`) |
| Tokens OAuth utilisateurs | PostgreSQL chiffré (`pgcrypto`, clé de chiffrement = Vercel env var) |
| Clé Claude (BYOK) | Supabase Vault ou colonne chiffrée par utilisateur |
| Clé Claude (plateforme) | Vercel Environment Variable pour le modèle SaaS avec abonnement |
| Clé imgbb / CDN | Vercel Environment Variable ou Supabase Storage (clé serveur unique) |

**Gestion des clés de chiffrement :**
- Utiliser [Supabase Vault](https://supabase.com/docs/guides/database/vault) pour les secrets par utilisateur
- Rotation automatique des tokens via `expires_at` + job cron (Supabase pg_cron)

---

## 4. Ce qui change côté hébergement des images

### V1 — imgbb (clé utilisateur)

Chaque utilisateur fournit sa propre clé imgbb. L'image est uploadée depuis le Rust local.

### SaaS — Option 1 : Supabase Storage

```typescript
// Upload via Supabase Storage
const { data, error } = await supabase.storage
  .from('post-images')
  .upload(`${userId}/${postId}.jpg`, imageBuffer, {
    contentType: 'image/jpeg',
    cacheControl: '3600',
    upsert: false
  })

// URL publique pour Instagram
const { data: { publicUrl } } = supabase.storage
  .from('post-images')
  .getPublicUrl(`${userId}/${postId}.jpg`)
```

- Bucket public avec policy de lecture anonyme
- Rétention : supprimer l'image 48h après publication via pg_cron
- Free tier : 1 GB stockage, 2 GB bandwidth

### SaaS — Option 2 : AWS S3 + CloudFront

- Scalabilité maximale, CDN mondial
- Coût faible (< $1/mois pour le volume d'un petit SaaS)
- Setup plus complexe

**Recommandation :** démarrer avec Supabase Storage (0 friction), migrer vers S3 si le volume dépasse les limites du free tier.

---

## 5. Ce qui change côté application Meta

### V1 — Mode Development

- App en mode "Development"
- Seuls les testeurs déclarés peuvent s'authentifier
- Pas de Meta App Review nécessaire
- Redirect URI : `https://localhost:7891/callback`

### SaaS — Mode Live

1. **Redirect URI** : `https://app.getpostcraft.app/api/oauth/callback` (domaine réel)
2. **Meta App Review** obligatoire pour :
   - Scopes avancés (`instagram_business_content_publish`) en mode Live
   - Délai : 5–14 jours ouvrés
   - Soumettre une vidéo de démonstration du flux OAuth et de la fonctionnalité de publication
3. **Type d'app Meta** : rester en "Business" (déjà correct)
4. **Politique de confidentialité** : URL obligatoire (ex: `https://getpostcraft.app/privacy`)
5. **CGU** : URL obligatoire (ex: `https://getpostcraft.app/terms`)

> **Attention légale :** Les Conditions d'utilisation de l'API Instagram interdisent :
> - Le scraping de contenu sans consentement explicite
> - La revente ou redistribution de données Instagram
> - Les applications automatisées sans action humaine initiée (bots)
>
> Getpostcraft publie du contenu à la demande explicite de l'utilisateur — conforme.

---

## 6. Stack suggérée pour le SaaS

Cohérente avec le profil tech global (Next.js 15/16, Supabase, Vercel) :

```
Frontend          : Next.js 16 App Router + TypeScript strict
UI                : shadcn/ui + Tailwind CSS v4 (réutiliser les composants Tauri)
Auth              : Supabase Auth (email + OAuth providers)
DB                : Supabase PostgreSQL + RLS
Storage           : Supabase Storage (images temporaires) → S3 si volume
Backend           : Next.js API Routes (léger) + Supabase Edge Functions (jobs)
Queue/Scheduling  : pg_notify + Supabase pg_cron (scheduling simple)
                    ou Trigger.dev (scheduling avancé, fiable)
IA                : Anthropic SDK côté serveur (BYOK utilisateur ou abonnement)
Déploiement       : Vercel (frontend + API) + Supabase (DB + storage)
Monitoring        : Sentry (erreurs) + Vercel Analytics
```

**Estimation migration V1 → SaaS :**
- OAuth + Auth multi-users : 2–3 semaines
- Migration schéma DB : 1 semaine
- Publication API (réutilisation du code Rust → portage TypeScript) : 1 semaine
- UI web (réutilisation des composants Tauri/React) : 2–3 semaines
- App Review Meta : 1–2 semaines (parallèle)
- Total : ~8–10 semaines à temps plein

---

## 7. Points de vigilance pour la migration

### Sécurité

- **Jamais stocker les tokens OAuth en clair** — chiffrer avec pgcrypto ou Supabase Vault
- **Rate limiting** sur les endpoints OAuth et publication (par user + par IP)
- **CSRF protection** : Supabase Auth gère nativement le state OAuth
- **RLS strict** : politique `user_id = auth.uid()` sur toutes les tables sensibles

### Meta / Instagram

- La Meta App Review peut refuser si la fonctionnalité n'est pas clairement démontrée
- En mode Live, les erreurs d'API Instagram sont plus restrictives (quota enforcement)
- Surveiller les dépréciations d'API : Meta annonce les changements avec 1 an de préavis minimum
- Les tokens long-lived (60 jours) nécessitent un refresh proactif (alerter l'utilisateur à J-7)

### RGPD / Données personnelles

- Les tokens OAuth sont des données personnelles : consentement explicite, droit à l'oubli
- `DELETE user` doit supprimer tokens + posts + métadonnées Instagram (cascade)
- Ne pas stocker de contenu Instagram au-delà de ce qui est strictement nécessaire au service
