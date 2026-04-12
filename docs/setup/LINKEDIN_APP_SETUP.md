# Guide — Créer et configurer une LinkedIn Developer App pour Getpostcraft

> **Temps estimé :** 15–20 minutes  
> **Prérequis :** un compte LinkedIn personnel actif, et une page entreprise LinkedIn (requise par LinkedIn pour associer une app).

---

## Vue d'ensemble

Getpostcraft utilise OAuth 2.0 PKCE pour te connecter à LinkedIn. LinkedIn ne stocke jamais ton mot de passe dans l'app — seul un token d'accès est conservé localement sur ta machine.

| Ce que tu vas créer | Valeur |
|---|---|
| Redirect URL | `https://localhost:7892/callback` |
| Scopes requis | `openid` `profile` `w_member_social` `r_liteprofile` |
| Méthode auth | OAuth 2.0 PKCE + Client Secret |

---

## Étape 1 — Créer l'application

1. Va sur **https://www.linkedin.com/developers/apps**
2. Clique sur le bouton **"Create app"** (en haut à droite, fond bleu LinkedIn).
3. Remplis le formulaire :
   - **App name** → par exemple `Getpostcraft`
   - **LinkedIn Page** → sélectionne ta page entreprise LinkedIn dans le menu déroulant.
     > ⚠️ **Piège #1 :** LinkedIn exige qu'une page entreprise soit associée à chaque app, même pour un usage personnel. Si tu n'en as pas, crée-en une rapidement via linkedin.com/company/setup/new — elle peut rester vide/privée.
   - **App logo** → upload n'importe quelle image (requis pour valider le formulaire). Tu peux utiliser le logo de Getpostcraft ou une image quelconque.
   - **Legal agreement** → coche la case.
4. Clique **"Create app"**.

Tu arrives maintenant sur le tableau de bord de ton app, onglet **"Settings"** ouvert par défaut.

---

## Étape 2 — Activer "Sign In with LinkedIn using OpenID Connect"

Cet produit fournit les scopes `openid` et `profile` (nom, photo, identifiant).

1. Dans le menu de gauche (ou les onglets en haut), clique sur **"Products"**.
2. Tu vois une liste de produits disponibles. Cherche **"Sign In with LinkedIn using OpenID Connect"**.
3. Clique sur **"Request access"** à droite de ce produit.
4. Une modale s'ouvre — lis les conditions et clique **"I agree, request access"**.
5. Le statut passe de `Request access` à **`Added`** ou **`Pending`** (généralement immédiat pour OpenID Connect).

> ✅ Ce produit ajoute automatiquement les scopes `openid`, `profile`, et `email` à ton app.

---

## Étape 3 — Activer "Share on LinkedIn"

Ce produit fournit le scope `w_member_social` (nécessaire pour publier des posts).

1. Toujours dans l'onglet **"Products"**, cherche **"Share on LinkedIn"**.
2. Clique **"Request access"**.
3. Confirme dans la modale.
4. Le statut doit passer à **`Added`** rapidement.

> ⚠️ **Piège #2 — Mode Development :** En mode Development, tu es **limité à 100 membres LinkedIn** qui peuvent autoriser ton app. Pour un usage personnel (ton propre compte), c'est suffisant. Pour une utilisation multi-utilisateurs, il faut passer en "Verification" → voir la section **Pièges courants** en bas.

---

## Étape 4 — Ajouter la Redirect URL

1. Va sur l'onglet **"Auth"** (dans le menu de navigation de ton app).
2. Fais défiler jusqu'à la section **"OAuth 2.0 settings"**.
3. Dans **"Authorized redirect URLs for your app"**, clique sur le bouton **"Add redirect URL"** (icône crayon ou bouton "+").
4. Saisis exactement :
   ```
   https://localhost:7892/callback
   ```
5. Clique **"Update"** pour sauvegarder.

> ⚠️ **Piège #3 :** L'URL doit être **exactement** `https://localhost:7892/callback` — ni HTTP, ni un autre port, ni de slash final. LinkedIn est strict sur la correspondance exacte.

---

## Étape 5 — Vérifier les scopes autorisés

1. Toujours dans l'onglet **"Auth"**, fais défiler jusqu'à la section **"OAuth 2.0 scopes"**.
2. Vérifie que tu vois au minimum ces 4 scopes :

   | Scope | Source | Description |
   |---|---|---|
   | `openid` | Sign In with LinkedIn (OpenID) | Identification OIDC |
   | `profile` | Sign In with LinkedIn (OpenID) | Nom, photo, identifiant |
   | `w_member_social` | Share on LinkedIn | Publier des posts |
   | `r_liteprofile` | Sign In with LinkedIn (legacy) | Profil basique (legacy, encore supporté) |

   > Si `r_liteprofile` n'apparaît pas, ce n'est pas bloquant — il est remplacé par `profile` dans les nouvelles apps. Getpostcraft fonctionnera quand même.

---

## Étape 6 — Activer PKCE (si disponible)

1. Dans l'onglet **"Auth"**, cherche une section **"Security settings"** ou **"PKCE"**.
2. Si l'option **"Require PKCE"** ou **"Enable PKCE"** est disponible, active-la.

> ℹ️ **Note :** En 2025, LinkedIn intègre PKCE automatiquement si le paramètre `code_challenge` est présent dans la requête d'autorisation — ce que Getpostcraft envoie toujours. Il n'est pas forcément nécessaire d'activer une option manuelle. Si tu ne vois pas l'option, ne t'inquiète pas.

---

## Étape 7 — Récupérer le Client ID et générer un Client Secret

1. Toujours dans l'onglet **"Auth"**, en haut de page, tu vois :
   - **Client ID** → une chaîne de caractères type `86xxxxxxxxxxxxxxxxx` (alphanumérique, ~16 caractères)
   - **Client Secret** → masqué par défaut, avec un bouton **"Generate new secret"** ou un œil pour révéler

2. **Copie le Client ID** → tu en as besoin dans Getpostcraft.

3. Pour le Client Secret :
   - Clique sur **"Generate new secret"** si aucun secret n'existe, ou révèle le secret existant.
   - **Copie-le immédiatement** — LinkedIn ne l'affichera plus après que tu aies quitté la page.

> ⚠️ **Piège #4 :** Si tu génères un nouveau secret, l'ancien est immédiatement invalidé. Une seule génération par session.

---

## Étape 8 — Saisir les valeurs dans Getpostcraft

1. Ouvre **Getpostcraft** → **Settings** → onglet **Comptes**.
2. Dans la section **LinkedIn** :

   **Client ID :**
   - Colle le Client ID dans le champ **"LinkedIn Client ID"**
   - Clique **"Enregistrer"** → l'indicateur **"✓ configuré"** apparaît

   **Client Secret :**
   - Colle le Client Secret dans le champ **"LinkedIn Client Secret"** (type password — tu ne verras que des `•`)
   - Clique **"Enregistrer"** → l'indicateur **"✓ configuré"** apparaît
   - Le secret est stocké localement et ne transite jamais vers le renderer de l'app

3. Le bouton **"Connecter LinkedIn"** devient actif une fois les deux champs configurés.

---

## Étape 9 — Connecter ton compte LinkedIn

1. Clique **"Connecter LinkedIn"** dans Getpostcraft.
2. Ton navigateur par défaut s'ouvre sur la page d'autorisation LinkedIn.
   > ℹ️ **Note TLS :** La première fois, ton navigateur peut afficher un avertissement "connexion non sécurisée" ou "certificat auto-signé" pour `localhost:7892`. C'est attendu — Getpostcraft génère un certificat auto-signé temporaire pour recevoir le callback OAuth. Clique **"Avancé" → "Continuer vers localhost"** (Chrome) ou **"Accepter le risque" → "Continuer"** (Firefox).
3. Sur la page LinkedIn, connecte-toi si nécessaire, puis clique **"Allow"** pour autoriser Getpostcraft.
4. Le navigateur affiche une page de succès et se ferme automatiquement.
5. Dans Getpostcraft, la section LinkedIn affiche maintenant ton **nom LinkedIn** et un bouton "Déconnecter".

---

## Pièges courants et solutions

### ❌ "invalid_redirect_uri"
L'URL enregistrée dans LinkedIn ne correspond pas exactement. Vérifie dans l'onglet Auth que tu as bien `https://localhost:7892/callback` (HTTPS, port 7892, pas de slash final).

### ❌ "unauthorized_scope"
Un des scopes n'est pas autorisé. Vérifie que les deux produits ("Sign In with LinkedIn" et "Share on LinkedIn") ont bien le statut **"Added"** dans l'onglet Products.

### ❌ "LinkedIn OAuth flow timed out"
Le callback n'a pas été reçu dans les 5 minutes. Causes possibles : le navigateur a bloqué la redirection vers `localhost`, ou l'antivirus a bloqué le port 7892. Réessaie en désactivant temporairement l'antivirus pour le test.

### ⚠️ Mode Development — limite 100 membres
En mode Development, seules les personnes ajoutées manuellement comme "Authorized users" dans l'onglet **"Settings"** de ton app LinkedIn peuvent se connecter. Pour ton usage personnel (ton propre compte), tu es déjà autorisé en tant que propriétaire de l'app — pas d'action supplémentaire requise.

### ⚠️ Passer en Production
Pour permettre à d'autres utilisateurs de connecter leur LinkedIn, il faut soumettre l'app à la vérification LinkedIn (onglet **"Settings"** → **"Verify"**). Cela nécessite une page entreprise vérifiée et un email professionnel. Pour usage solo, le mode Development suffit.

### ❌ Client Secret régénéré par erreur
Si tu as généré un nouveau secret, retourne dans l'onglet Auth de ton app LinkedIn, copie le nouveau secret, et remets-le dans Settings → Comptes → LinkedIn Client Secret dans Getpostcraft.

---

## Résumé des valeurs à retenir

| Valeur | Où la trouver | Où la saisir |
|---|---|---|
| **Client ID** | LinkedIn Developer App → Auth | Getpostcraft → Settings → Comptes → LinkedIn Client ID |
| **Client Secret** | LinkedIn Developer App → Auth → Generate | Getpostcraft → Settings → Comptes → LinkedIn Client Secret |
| **Redirect URL à enregistrer** | (à saisir dans LinkedIn) | `https://localhost:7892/callback` |

---

*Guide rédigé pour Getpostcraft v0.1 — LinkedIn Developer Portal 2025.*
