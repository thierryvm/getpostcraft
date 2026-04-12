# Flow OAuth PKCE — Documentation technique

> Dernière vérification : avril 2026  
> Fichiers source : `src-tauri/src/commands/oauth.rs`, `src-tauri/src/adapters/instagram.rs`, `src-tauri/src/token_store.rs`

---

## Vue d'ensemble

Getpostcraft implémente OAuth 2.0 avec PKCE (Proof Key for Code Exchange, RFC 7636) pour l'authentification Instagram. L'ensemble du flow est géré côté Rust — le renderer React ne voit jamais de token ni de secret.

---

## Diagramme séquentiel

```
Renderer (React)          Rust (Tauri)              OS / Browser             Instagram / Meta
      |                       |                           |                         |
      |--invoke("start_oauth_flow", {client_id})--------->|                         |
      |                       |                           |                         |
      |                   [1] Génère PKCE verifier + challenge (SHA-256 / base64url)|
      |                   [2] Génère CSRF state (16 bytes aléatoires)               |
      |                   [3] Récupère client_secret depuis api_keys.json           |
      |                   [4] Bind TcpListener sur 127.0.0.1:7891                  |
      |                   [5] Génère certificat TLS auto-signé (rcgen)             |
      |                   [6] Construit l'URL d'autorisation                        |
      |                       |--open_url(auth_url)------>|                         |
      |                       |                           |--GET /oauth/authorize-->|
      |                       |                           |<--302 instagram.com/---|
      |                       |                           |  (page de login)        |
      |                       |                           |   [Utilisateur se       |
      |                       |                           |    connecte et          |
      |                       |                           |    autorise l'app]      |
      |                       |                           |<--redirect callback-----|
      |                       |<--HTTPS GET /callback?code=AUTH_CODE&state=CSRF----|
      |                   [7] Valide CSRF state           |                         |
      |                   [8] Extrait le code d'autorisation                        |
      |                       |--HTTP 200 (page succès)-->|                         |
      |                       |                           |                         |
      |                   [9] POST /oauth/access_token (code + code_verifier + client_secret)
      |                       |-------------------------------------------------->|
      |                       |<--{ access_token }--------------------------------|
      |                       |                           |                         |
      |                  [10] GET /me?fields=id,username,name                      |
      |                       |-------------------------------------------------->|
      |                       |<--{ id, username, name }-----------------------|   |
      |                       |                           |                         |
      |                  [11] save_token("instagram:{user_id}", access_token)      |
      |                       |  → oauth_tokens.json (data dir, Rust only)         |
      |                  [12] upsert_and_get(db, "instagram", user_id, ...)        |
      |                       |  → SQLite: accounts table (metadata seulement)     |
      |<--Ok(ConnectedAccount {id, provider, user_id, username, display_name})-----|
```

---

## Détail de chaque étape

### [1] Génération PKCE

```rust
// code_verifier : 32 bytes aléatoires encodés base64url (no-pad)
let mut bytes = [0u8; 32];
rand::thread_rng().fill_bytes(&mut bytes);
let code_verifier = URL_SAFE_NO_PAD.encode(bytes);

// code_challenge = BASE64URL(SHA-256(code_verifier))
let hash = Sha256::digest(code_verifier.as_bytes());
let code_challenge = URL_SAFE_NO_PAD.encode(hash);
```

Le `code_verifier` ne quitte jamais Rust. Il est passé directement à l'étape d'échange de token.

### [2] CSRF State

16 bytes aléatoires en base64url. Envoyé dans l'URL d'autorisation (`&state=...`), vérifié à la réception du callback. Protège contre les attaques CSRF sur le callback local.

### [3] Récupération du client_secret

```rust
let client_secret = crate::ai_keys::get_key("instagram_client_secret")?;
```

Lu depuis `api_keys.json` dans le répertoire de données utilisateur. Ce fichier n'est jamais exposé à l'IPC.

### [4-5] Serveur callback TLS

Un `TcpListener` est lié sur `127.0.0.1:7891`. Un certificat TLS auto-signé est généré à la volée avec `rcgen`. Ce certificat est éphémère — valable seulement le temps du flow (quelques secondes à 5 minutes max).

L'URI de callback enregistrée dans Meta doit être exactement :
```
https://localhost:7891/callback
```

### [6] URL d'autorisation

```
https://www.instagram.com/oauth/authorize
  ?client_id={APP_ID}
  &redirect_uri=https%3A%2F%2Flocalhost%3A7891%2Fcallback
  &scope=instagram_business_basic,instagram_business_content_publish
  &response_type=code
  &code_challenge={CHALLENGE}
  &code_challenge_method=S256
  &state={CSRF}
```

> **Note :** La documentation officielle de Meta liste l'endpoint d'autorisation comme `https://api.instagram.com/oauth/authorize`. L'implémentation utilise `https://www.instagram.com/oauth/authorize` — les deux sont fonctionnels. La page de login utilisateur est identique.

### [7] Validation CSRF

```rust
let state_param = parse_query_param(first_line, "state");
if state_param.as_deref() != Some(expected_state) {
    return Err("CSRF state mismatch — potential attack detected");
}
```

Si le state ne correspond pas, la connexion est rejetée avec HTTP 400.

### [8] Extraction du code

Le code d'autorisation est extrait de la query string de la requête HTTP. Meta ajoute `#_` en fin d'URL dans certains browsers — le parser HTTP côté Rust ne voit pas le fragment (les fragments ne sont pas envoyés au serveur), donc ce n'est pas un problème.

### [9] Échange code → token

```
POST https://api.instagram.com/oauth/access_token
Content-Type: application/x-www-form-urlencoded

client_id={APP_ID}
&client_secret={APP_SECRET}
&code={AUTH_CODE}
&code_verifier={PKCE_VERIFIER}
&redirect_uri=https://localhost:7891/callback
&grant_type=authorization_code
```

**Pourquoi `client_secret` est requis avec PKCE :**

RFC 7636 spécifie que PKCE est conçu pour les clients publics (mobile, desktop) qui ne peuvent pas stocker un secret de manière sécurisée — PKCE *remplace* le client_secret dans ce contexte.

Meta fait un choix non-standard : ils **exigent les deux** simultanément. C'est documenté dans leur code source de référence et confirmé par les erreurs obtenues sans client_secret. Cette décision de Meta offre une protection en couche supplémentaire mais va à l'encontre de l'esprit de RFC 7636 pour les clients natifs.

Dans Getpostcraft V1 desktop, le client_secret est stocké dans `api_keys.json` (répertoire de données utilisateur, non accessible à d'autres processus sous Windows/macOS). Ce n'est pas idéal mais c'est le seul moyen de respecter la contrainte Meta sur un client desktop.

### [10] Profil utilisateur

```
GET https://graph.instagram.com/me?fields=id,username,name&access_token={TOKEN}
```

Retourne `{ id: "...", username: "...", name: "..." }`.

### [11] Stockage du token

```
Fichier : {data_dir}/getpostcraft/oauth_tokens.json
Format  : { "instagram:{user_id}": "{access_token}", ... }
```

Ce fichier n'est **jamais lu par le renderer**. Seul Rust y accède. La clé composite `instagram:{user_id}` permet de gérer plusieurs comptes.

### [12] Métadonnées SQLite

La table `accounts` ne stocke **que les métadonnées** :

```sql
id, provider, user_id, username, display_name, token_key, created_at, updated_at
```

`token_key` est la clé de lookup dans `oauth_tokens.json` — pas le token lui-même.

---

## Timeout et gestion d'erreurs

Le serveur callback a un timeout de 5 minutes :

```rust
tokio::time::timeout(Duration::from_secs(300), accept_oauth_callback(...))
```

Les requêtes non-callback (ex: `/favicon.ico` que le browser demande automatiquement) reçoivent une réponse `204 No Content` et la boucle continue.

---

## Ce qui ne quitte jamais Rust

| Donnée | Stockage | Accessible depuis |
|--------|---------|-------------------|
| `code_verifier` PKCE | Mémoire Rust (stack) | Rust uniquement |
| `client_secret` | `api_keys.json` | Rust uniquement |
| `access_token` | `oauth_tokens.json` | Rust uniquement |
| `csrf_state` | Mémoire Rust (stack) | Rust uniquement |

Le renderer React ne reçoit que : `{ id, provider, user_id, username, display_name }` — aucune donnée sensible.

---

## Durée de vie des tokens Instagram

| Type | Durée | Refresh |
|------|-------|---------|
| Short-lived token (initial) | ~1 heure | Non, remplacé par long-lived |
| Long-lived token | 60 jours | Via `GET /refresh_access_token` |

> ⚠️ À vérifier : L'implémentation actuelle (V1) ne gère pas le refresh automatique des tokens. Un token expiré nécessite de relancer le flow OAuth complet. À implémenter en V1.1.

**Endpoint de refresh :**
```
GET https://graph.instagram.com/refresh_access_token
  ?grant_type=ig_refresh_token
  &access_token={LONG_LIVED_TOKEN}
```

---

## Ce qui changerait en V2/SaaS

| Aspect | V1 Desktop | V2 SaaS |
|--------|-----------|---------|
| Serveur callback | localhost:7891 (Rust) | Endpoint HTTPS serveur (ex: `/api/oauth/callback`) |
| client_secret | Fichier local utilisateur | Variable d'environnement serveur |
| Stockage token | `oauth_tokens.json` local | PostgreSQL/Supabase, chiffré |
| PKCE | Maintenu (bonne pratique) | Optionnel côté serveur, mais à conserver |
| Token refresh | Manuel (re-auth) | Automatique (background job) |
| Multi-comptes | 1 utilisateur, N comptes | N utilisateurs, N comptes |

En SaaS, le client_secret vivrait exclusivement côté serveur — la contrainte de stockage local disparaît. Le callback serait une route HTTP ordinaire, plus besoin de TLS auto-signé.
