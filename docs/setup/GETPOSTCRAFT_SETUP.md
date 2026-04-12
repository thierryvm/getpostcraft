# Installation et configuration de Getpostcraft

> Dernière vérification : avril 2026

Ce guide couvre l'installation de Getpostcraft et sa configuration initiale. Prérequis : avoir suivi [META_APP_SETUP.md](./META_APP_SETUP.md) pour créer l'application Meta.

---

## Prérequis système

| OS | Version minimale | Notes |
|----|-----------------|-------|
| Windows | 10 x64 (build 1903+) | Testé sur Windows 11 Pro |
| macOS | 11 (Big Sur) | Apple Silicon (M1+) natif |
| Linux | glibc 2.31+ (Ubuntu 20.04+) | AppImage ou .deb |

**Dépendances runtime :**
- WebView2 (Windows) — installé automatiquement si absent
- WebKit (macOS/Linux) — fourni par le système

---

## Option A — Installer depuis un binaire (recommandé)

1. Télécharger la dernière release depuis [GitHub Releases](https://github.com/thierryvm/getpostcraft/releases)
2. Selon votre OS :
   - **Windows** : exécuter le `.exe` ou `.msi`
   - **macOS** : ouvrir le `.dmg`, glisser l'app dans `/Applications`
   - **Linux** : `chmod +x getpostcraft.AppImage && ./getpostcraft.AppImage`

---

## Option B — Compiler depuis les sources

### Prérequis de build

```bash
# Rust (stable toolchain)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable

# Node.js 20+ et npm
# https://nodejs.org/

# Tauri CLI
npm install -g @tauri-apps/cli

# Dépendances Linux uniquement
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

### Build

```bash
git clone https://github.com/thierryvm/getpostcraft.git
cd getpostcraft
npm install
npm run tauri build
```

Le binaire se trouve dans `src-tauri/target/release/`.

### Développement

```bash
npm run tauri dev    # Lance l'app en hot-reload
```

---

## Configuration initiale

Au premier lancement, l'app affiche l'écran de configuration. Renseigner les éléments suivants dans l'ordre.

### 1. App ID Instagram (requis pour OAuth)

Dans **Settings → Comptes** :

- **App ID** : le numéro à 15–17 chiffres depuis `developers.facebook.com` → votre app → Paramètres → Général → App ID
- Cliquer sur **"Sauvegarder"**

Stockage : SQLite local (`settings` table, clé `instagram_app_id`). Visible dans l'UI.

### 2. App Secret Instagram (requis pour OAuth)

Dans **Settings → Comptes** :

- **App Secret** : la valeur depuis Meta Developer → Paramètres → Général → App Secret (cliquer "Afficher")
- Cliquer sur **"Sauvegarder le secret"**

Stockage : fichier `api_keys.json` dans le répertoire de données utilisateur de l'OS, **jamais dans SQLite en clair, jamais accessible depuis l'interface**.

| OS | Chemin du répertoire de données |
|----|--------------------------------|
| Windows | `%APPDATA%\getpostcraft\` |
| macOS | `~/Library/Application Support/getpostcraft/` |
| Linux | `~/.local/share/getpostcraft/` |

### 3. Clé API imgbb (requis pour la publication)

imgbb est utilisé pour héberger temporairement les images avant de les envoyer à Instagram (l'API Instagram exige une URL publique HTTPS, pas un upload direct).

1. Créer un compte sur [https://imgbb.com/signup](https://imgbb.com/signup)
2. Une fois connecté, aller sur [https://api.imgbb.com/](https://api.imgbb.com/) pour obtenir votre clé API
3. Dans Getpostcraft → **Settings → Publication** : coller la clé et cliquer **"Sauvegarder"**

Stockage : SQLite local (`settings` table, clé `imgbb_api_key`).

> ⚠️ **Limitation V1** : Le token Instagram obtenu est un token de courte durée (1 heure). Après expiration, l'application retourne une erreur de publication et il faudra relancer le flow OAuth (déconnecter et reconnecter le compte). La gestion automatique des tokens long-lived (60 jours) est prévue en V1.1.

> **Note de sécurité :** La clé imgbb permet d'uploader des images sur votre compte. Elle est stockée localement et ne quitte jamais votre machine. Pour la V1 desktop, c'est acceptable. En SaaS, cette clé devra migrer côté serveur.

### 4. Clé API IA (requis pour la génération de contenu)

Dans **Settings → IA** :

- **Fournisseur** : Anthropic (Claude) est supporté en V1
- **Clé API** : obtenir sur [https://console.anthropic.com/](https://console.anthropic.com/)
- Cliquer sur **"Sauvegarder"**

Stockage : fichier `api_keys.json` dans le répertoire de données utilisateur (même que les tokens OAuth).

---

## Première connexion Instagram (flow OAuth)

Une fois l'App ID et l'App Secret configurés :

1. Aller dans **Settings → Comptes**
2. Cliquer sur **"Connecter Instagram"**
3. Le navigateur système s'ouvre sur la page d'autorisation Instagram

**Ce que vous voyez dans le navigateur :**

```
instagram.com/oauth/authorize?client_id=...&scope=instagram_business_basic,...
```

4. Se connecter avec le compte Instagram testeur (ou le compte propriétaire de l'app)
5. Cliquer **"Autoriser"** sur la page Meta
6. Le navigateur est redirigé vers `https://localhost:7891/callback`

---

## Avertissement certificat auto-signé

Lors de l'étape 6, le navigateur affiche un avertissement similaire à :

> **"Votre connexion n'est pas privée"**  
> Attaquants potentiels peuvent essayer de voler vos informations sur localhost

**Pourquoi cet avertissement apparaît :**  
Meta exige que l'URI de callback soit en HTTPS même pour localhost. Getpostcraft génère un certificat TLS auto-signé à la volée pour satisfaire cette contrainte. Ce certificat n'est pas signé par une autorité de certification reconnue (CA) — d'où l'avertissement.

**Est-ce dangereux ?**  
Non. La connexion reste 100% locale à votre machine. Personne sur le réseau ne peut intercepter ce callback. L'avertissement est un faux positif dû à la nature des certificats auto-signés.

**Comment l'accepter :**

- **Chrome / Edge** : cliquer sur "Avancé" → "Continuer vers localhost (non sécurisé)"
- **Firefox** : cliquer sur "Avancé" → "Accepter le risque et continuer"
- **Safari** : cliquer sur "Afficher le détail" → "visiter ce site web"

Après l'acceptation, la page affiche un message de succès et la fenêtre de navigateur peut être fermée. Retourner dans Getpostcraft — le compte apparaît maintenant dans Settings → Comptes.

---

## Vérification du bon fonctionnement

1. **Compte connecté** : Settings → Comptes affiche le nom d'utilisateur Instagram
2. **Test IA** : Composer → saisir un brief → cliquer "Générer" → une caption et des hashtags apparaissent
3. **Test publication** : créer un post avec une image et une caption → cliquer "Publier" → le post apparaît sur Instagram

---

## Réinitialisation complète

Pour repartir de zéro (utile en debug) :

```bash
# Windows
Remove-Item -Recurse "$env:APPDATA\getpostcraft"

# macOS / Linux
rm -rf ~/Library/Application\ Support/getpostcraft   # macOS
rm -rf ~/.local/share/getpostcraft                    # Linux
```

Cela supprime la base SQLite, les tokens OAuth et les clés API. L'application repart en configuration initiale.
