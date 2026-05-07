# Releasing Getpostcraft

Guide pour publier une version multi-plateforme (Windows + macOS + Linux) avec
auto-update intégré, **à coût 0€**.

## Vue d'ensemble

- Build : GitHub Actions sur 3 OS (windows-latest, macos-latest, ubuntu-22.04)
- Distribution : GitHub Releases (assets téléchargeables)
- Auto-update : `tauri-plugin-updater` lit `latest.json` sur le dernier release
- Signature : Ed25519 (Tauri Signer) — gratuit, intégré
- **Pas** de code signing OS (Apple Dev / Windows OV) — skip pour V1 alpha

## Setup initial (une fois)

### 1. Générer la keypair updater

```bash
mkdir -p "$USERPROFILE/.tauri/getpostcraft"  # PowerShell : %USERPROFILE%
npx tauri signer generate \
  --ci \
  --password "" \
  --write-keys "$USERPROFILE/.tauri/getpostcraft/updater.key" \
  --force
```

**Sécurité** :
- La clé privée (`updater.key`) **ne quitte JAMAIS** ta machine sauf vers GitHub Secrets.
- Ne la commit jamais. `~/.tauri/` est en dehors du repo.
- Si tu la perds, tu ne peux plus signer de mises à jour pour les utilisateurs existants — il faut tout réinstaller depuis un nouveau bundle non-signé. Sauvegarde-la (gestionnaire de mots de passe, USB chiffré).

### 2. Injecter la clé publique dans `tauri.conf.json`

Le contenu de `~/.tauri/getpostcraft/updater.key.pub` (une longue chaîne base64)
va dans `plugins.updater.pubkey`. **Déjà fait** dans cette PR si tu as utilisé
le script ci-dessus. Sinon copie-colle manuellement.

### 3. Ajouter les secrets dans GitHub

Settings → Secrets and variables → Actions → New repository secret :

| Secret | Valeur |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | contenu de `~/.tauri/getpostcraft/updater.key` (le fichier entier) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | chaîne vide `""` (on a généré sans mot de passe) |

⚠️ Si tu régénères la keypair plus tard avec un mot de passe non-vide,
mets-le dans `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

## Publier une nouvelle version

### Workflow normal

```bash
# 1. Bumper les versions dans les 3 fichiers (doivent rester alignés)
#    package.json     : "version": "0.2.0"
#    src-tauri/Cargo.toml : version = "0.2.0"
#    src-tauri/tauri.conf.json : "version": "0.2.0"

# 2. Vérifier l'alignement avant de tagger
npm run check:versions

# 3. Tagger + pousser
git commit -am "chore(release): bump version to v0.2.0"
git tag v0.2.0
git push origin main --tags
```

Le push du tag déclenche `.github/workflows/release.yml` qui :

1. **version-check** — fait échouer la release si les versions divergent
2. **build (matrix)** — build + signe les 3 bundles, upload sur un draft Release
3. **publish-manifest** — collecte les `.sig` et écrit `latest.json` sur la Release

Au premier passage tu dois **manuellement promouvoir le draft en published** sur
GitHub Releases. Dès que c'est public, l'auto-update fonctionne pour tous les
utilisateurs ≤ v0.2.0.

### Bundles produits

| OS | Format | Path |
|---|---|---|
| Windows | NSIS installer (recommandé) + MSI | `getpostcraft_X.Y.Z_x64-setup.exe` (+ `.nsis.zip` pour updater) |
| macOS | DMG + .app.tar.gz (signed pour updater) | `getpostcraft_X.Y.Z_universal.dmg` |
| Linux | AppImage + .deb | `getpostcraft_X.Y.Z_amd64.AppImage` |

## Pour les utilisateurs

### Premier install (warnings attendus)

Comme on n'a pas de code signing OS, les 3 plateformes affichent un warning au
premier lancement. C'est **normal** et lié au coût V1 alpha.

**Windows :**
- SmartScreen → "Windows protected your PC"
- Cliquer "More info" → "Run anyway"
- Le warning disparaît après ~5000 installs (réputation Microsoft)

**macOS :**
- "Cannot be opened because it is from an unidentified developer"
- Solution 1 : right-click sur l'app → "Open" → confirmer
- Solution 2 : terminal `xattr -d com.apple.quarantine /Applications/getpostcraft.app`

**Linux :**
- AppImage : `chmod +x getpostcraft_X.Y.Z_amd64.AppImage` puis double-clic
- .deb : `sudo dpkg -i getpostcraft_X.Y.Z_amd64.deb`

### Mises à jour suivantes

Settings → À propos → **Mises à jour** → "Vérifier".
Si une version est dispo : bouton "Installer". Téléchargement + install + relaunch
en moins d'une minute. Aucun warning OS sur les updates (signature Ed25519
suffit pour Tauri).

## Sécurité — checklist senior

Cette release pipeline a été conçue avec ces garde-fous :

| Risque | Mitigation |
|---|---|
| Updater accepte un binaire malveillant | Signature Ed25519 obligatoire — invalid sig → install refusé par le plugin |
| MITM sur le manifest | HTTPS strict (GitHub Releases) + content-length verification |
| Clé privée commitée par erreur | `.gitignore` strict + clé hors repo (`~/.tauri/`) |
| Clé privée loggée dans CI | Secrets jamais echo, pas de `set -x`, masking GitHub auto |
| Version mismatch publish | `scripts/check-versions.mjs` bloque la release |
| Tag arbitraire (push --force) | Branche `main` protégée + workflow trigger sur tag pattern `v*` uniquement |
| Bundle uploadé sans `.sig` | `pickBundleAsset` dans `generate-update-manifest.mjs` skip ces platforms |
| Réutilisation d'une keypair compromise | Doc rappelle de re-générer + invalider les anciennes releases |

## Roadmap signing OS (post-V1)

Quand le SaaS deviendra public, ajouter (budget ~600€/an) :

1. Apple Developer Program — 99€/an → Gatekeeper accepte sans warning
2. Windows OV cert (Sectigo / SSL.com) — 200-500€/an → SmartScreen accepte
3. Linux : optionnel, pas critique

Workflow : ajouter `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, et
`WINDOWS_CERTIFICATE_THUMBPRINT` aux secrets. `tauri-action` gère le reste.
