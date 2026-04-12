# Pipeline de publication — Documentation technique

> Dernière vérification : avril 2026  
> Fichiers source : `src-tauri/src/commands/publisher.rs`  
> API : [Instagram Content Publishing](https://developers.facebook.com/docs/instagram-platform/content-publishing/), [imgbb API](https://api.imgbb.com/)

---

## Vue d'ensemble

La publication d'un post Instagram via l'API Graph suit obligatoirement un pipeline en deux étapes distinctes :
1. **Création d'un container média** — Instagram prépare le post
2. **Publication du container** — le post devient visible

De plus, Instagram exige que les images soient accessibles via une URL HTTPS publique. Getpostcraft utilise imgbb comme service d'hébergement temporaire d'images.

---

## Diagramme séquentiel

```
Renderer (React)        Rust (Tauri)             imgbb API           Instagram Graph API
      |                     |                        |                       |
      |--invoke("publish_post", {post_id})----------->|                       |
      |                     |                        |                       |
      |                 [1] Load post from SQLite (caption, hashtags, image_path)
      |                 [2] Load Instagram account (user_id, token_key)      |
      |                 [3] get_token("instagram:{user_id}")                 |
      |                     |  → read oauth_tokens.json (never IPC)         |
      |                 [4] get setting "imgbb_api_key" from SQLite           |
      |                     |                        |                       |
      |                 [5] Read image file from disk                        |
      |                 [6] Base64-encode image                              |
      |                     |--POST /1/upload?key=...--->|                   |
      |                     |  body: image={base64}      |                   |
      |                     |<--{ success: true, data: { url: "https://i.ibb.co/..." } }
      |                     |                        |                       |
      |                 [7] Build full caption (caption + \n\n + #hashtags)  |
      |                     |--POST /v21.0/{ig_user_id}/media--------------->|
      |                     |  image_url={imgbb_url}                         |
      |                     |  caption={full_caption}                        |
      |                     |  access_token={token}                          |
      |                     |<--{ id: "{container_id}" }-------------------|
      |                     |                        |                       |
      |                     |--POST /v21.0/{ig_user_id}/media_publish------->|
      |                     |  creation_id={container_id}                   |
      |                     |  access_token={token}                          |
      |                     |<--{ id: "{ig_media_id}" }---------------------|
      |                     |                        |                       |
      |                 [8] UPDATE posts SET status='published',             |
      |                     ig_media_id=..., published_at=... WHERE id={post_id}
      |<--Ok({ post_id, ig_media_id, published_at })------------------------|
```

---

## Détail de chaque étape

### [1-4] Préparation

```rust
// Chargement du post depuis SQLite
let post = crate::db::history::get_by_id(&state.db, post_id).await?;

// Garde contre double-publication
if post.status == "published" {
    return Err("This post is already published");
}

// Token récupéré depuis oauth_tokens.json — ne passe jamais en IPC
let access_token = crate::token_store::get_token(&account.token_key)?;

// Clé imgbb depuis SQLite settings
let imgbb_key = crate::db::settings_db::get(&state.db, "imgbb_api_key").await?;
```

### [5-6] Upload imgbb

L'image locale (générée par le Python sidecar, format PNG 1080×1080) est lue depuis le disque, encodée en base64 et envoyée à imgbb.

```
POST https://api.imgbb.com/1/upload?key={API_KEY}
Content-Type: application/x-www-form-urlencoded

image={BASE64_ENCODED_IMAGE}
```

Réponse attendue :
```json
{
  "success": true,
  "data": {
    "url": "https://i.ibb.co/XXXX/filename.png"
  }
}
```

imgbb retourne une URL publique HTTPS persistante. Cette URL est ensuite passée à Instagram.

### [7] Création du container média Instagram

```
POST https://graph.instagram.com/v21.0/{ig_user_id}/media
Content-Type: application/x-www-form-urlencoded

image_url={IMGBB_PUBLIC_URL}
&caption={CAPTION_WITH_HASHTAGS}
&access_token={ACCESS_TOKEN}
```

Le `caption` inclut la caption générée par Claude + les hashtags séparés par des espaces, précédés d'une double ligne vide :

```
{caption}

#{hashtag1} #{hashtag2} #{hashtag3}
```

Réponse : `{ "id": "17854360229135492" }` — c'est le `container_id`.

### Publication du container

```
POST https://graph.instagram.com/v21.0/{ig_user_id}/media_publish
Content-Type: application/x-www-form-urlencoded

creation_id={CONTAINER_ID}
&access_token={ACCESS_TOKEN}
```

Réponse : `{ "id": "17920238022017421" }` — c'est l'`ig_media_id`, l'identifiant permanent du post Instagram.

### [8] Mise à jour SQLite

```sql
UPDATE posts SET
  status = 'published',
  published_at = '{ISO8601}',
  ig_media_id = '{IG_MEDIA_ID}'
WHERE id = {POST_ID}
```

---

## Pourquoi imgbb est nécessaire

L'API Instagram Graph (`/media`) exige que les images soient fournies via une URL publique HTTPS accessible depuis les serveurs Meta. Instagram va télécharger l'image depuis cette URL lors de la création du container.

**L'API n'accepte pas :**
- Upload direct (`multipart/form-data`)
- URLs locales (`file://`, `localhost`, IP privées)
- Data URLs (`data:image/jpeg;base64,...`)

**Ce que fait imgbb :** héberge l'image de manière publique le temps que Meta la télécharge. L'image reste accessible sur imgbb indéfiniment (sauf si une expiration est définie).

---

## Formats d'image supportés par Instagram

Source : [Instagram Platform — IG User Media](https://developers.facebook.com/docs/instagram-platform/instagram-graph-api/reference/ig-user/media/)

| Paramètre | Valeur |
|-----------|--------|
| Format | **JPEG uniquement** (MPO et JPS non supportés) |
| Taille max | 8 MB |
| Largeur min | 320 px |
| Largeur max | 1440 px (auto-scalé si dépassé) |
| Ratio accepté | Entre **4:5** (portrait) et **1.91:1** (paysage) |
| Espace colorimétrique | sRGB (conversion automatique) |

> **Getpostcraft V1 génère des PNG 1080×1080 (ratio 1:1).**  
> Ce ratio est dans la plage acceptée (entre 4:5 et 1.91:1). L'image est convertible par imgbb si nécessaire.  
> ⚠️ À vérifier : Instagram télécharge l'image depuis imgbb — si imgbb convertit le PNG en JPEG lors de l'upload (comportement par défaut), c'est conforme. Si imgbb conserve le PNG, une conversion explicite en JPEG côté Rust/sidecar serait nécessaire.

---

## Limites de publication Instagram

| Limite | Valeur | Note |
|--------|--------|------|
| Posts/24h via API | **100 posts** | Période glissante de 24h |
| Containers créés/24h | 400 containers | Inclut les containers non publiés |
| Longueur caption | 2 200 caractères | |
| Hashtags par post | 30 maximum | |
| Mentions (@) par post | 20 maximum | |

> ⚠️ À vérifier : Une source tierce mentionne une limite de 25 posts/24h pour les comptes Creator. La documentation officielle Meta indique 100. La limite effective peut dépendre du type de compte (Creator vs Business) et des métriques d'impression. À tester sur le compte @terminallearning.

---

## Alternatives à imgbb (pour le SaaS futur)

| Service | Avantages | Inconvénients |
|---------|-----------|---------------|
| **imgbb** (actuel) | Simple, gratuit, API directe | Tiers externe, pas de contrôle sur la rétention |
| **Cloudinary** | CDN mondial, transformations d'image, SDK riche | Payant au-delà du free tier |
| **AWS S3 + CloudFront** | Contrôle total, scalable | Setup plus complexe, coût réseau |
| **Supabase Storage** | Intégré avec la stack SaaS cible | Limite de bandwidth sur free tier |
| **Serveur propre** | Contrôle total | Nécessite une infrastructure serveur HTTPS |

En SaaS, la recommandation est **Supabase Storage** (cohérent avec la stack Next.js + Supabase) avec une expiration automatique des images après 48h post-publication.

---

## Ce qui changerait en SaaS

| Aspect | V1 Desktop | SaaS |
|--------|-----------|------|
| Hébergement image | imgbb (clé utilisateur) | Supabase Storage ou S3 (clé serveur) |
| Clé imgbb | Côté client (SQLite local) | Variable d'environnement serveur |
| Publication | Synchrone (attente du résultat) | Queue asynchrone (BullMQ, pg_notify) |
| Token Instagram | `oauth_tokens.json` local | PostgreSQL chiffré (pgcrypto) |
| Multi-comptes | 1 utilisateur | N utilisateurs avec isolation RLS |
| Scheduling | Non supporté V1 | Jobs cron ou workers |

La logique de publication elle-même (2 requêtes API Instagram) ne change pas — seul le contexte d'exécution (desktop Rust → server-side Node.js ou Rust actix) évolue.
