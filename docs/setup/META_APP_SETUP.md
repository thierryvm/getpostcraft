# Configuration de l'application Meta Developer

> Dernière vérification : avril 2026  
> Basé sur : [Instagram Platform — Create an Instagram App](https://developers.facebook.com/docs/instagram-platform/create-an-instagram-app/) et [Business Login for Instagram](https://developers.facebook.com/docs/instagram-platform/instagram-api-with-instagram-login/business-login/)

Ce guide couvre la création et la configuration de l'application Meta Developer nécessaire pour utiliser Getpostcraft. Il s'adresse au développeur ou à l'utilisateur qui installe l'outil pour la première fois.

---

## Prérequis

1. **Un compte Instagram professionnel** — Creator ou Business. Un compte Personnel ne fonctionne pas avec l'API de publication.
   - Pour convertir : Instagram → Paramètres → Compte → Passer à un compte professionnel
   - Choisir "Créateur" ou "Entreprise" selon votre usage
2. **Un compte Facebook** lié à votre compte Instagram (requis par Meta pour accéder au Meta Developer Dashboard)
3. **Un navigateur** avec accès à `developers.facebook.com`

---

## Étape 1 — Créer l'application Meta

1. Rendez-vous sur [https://developers.facebook.com/apps/](https://developers.facebook.com/apps/)
2. Cliquez sur **"Créer une application"** (bouton vert, en haut à droite)
3. Sur l'écran "Quel est votre cas d'utilisation ?" → sélectionnez **"Autre"**
4. Sur l'écran suivant "Sélectionner un type d'application" → sélectionnez **"Business"**

> **Pourquoi "Business" ?**  
> Le type Business est obligatoire pour ajouter le produit Instagram Login avec les scopes de publication (`instagram_business_content_publish`). Les autres types d'application ne supportent pas ce produit.

5. Renseignez :
   - **Nom de l'application** : par exemple `Getpostcraft Dev`
   - **Email de contact** : votre adresse email
   - **Portfolio Business** : laisser vide pour un usage personnel
6. Cliquez sur **"Créer une application"**

Vous êtes redirigé vers le tableau de bord de votre application.

---

## Étape 2 — Récupérer l'App ID et l'App Secret

1. Dans le menu de gauche, cliquez sur **"Paramètres"** → **"Général"** (ou Basic Settings)
2. En haut de la page, vous voyez :
   - **App ID** : un nombre à environ 15–17 chiffres — c'est votre `instagram_app_id`
   - **App Secret** : cliquez sur **"Afficher"** pour révéler la valeur — c'est votre `instagram_client_secret`

> **Sécurité :** Ne partagez jamais l'App Secret publiquement. Ne le commitez jamais dans un dépôt Git. Dans Getpostcraft, il est stocké dans un fichier protégé hors du repo.

---

## Étape 3 — Ajouter le produit Instagram Login

1. Dans le menu de gauche, cliquez sur **"Tableau de bord"** (ou "Dashboard")
2. Dans la section "Ajouter des produits à votre application", cherchez **"Instagram"** et cliquez sur **"Configurer"**

> ⚠️ À vérifier : Le libellé exact du produit peut être "Instagram Platform" ou "Instagram Login" selon l'interface Meta du moment. Cherchez le bloc avec le logo Instagram.

3. Une sous-section **"Instagram Login"** apparaît dans le menu de gauche. Cliquez dessus.
4. Cliquez sur **"Paramètres"** sous Instagram Login

---

## Étape 4 — Configurer les redirect URIs

Cette étape est critique. Getpostcraft utilise `https://localhost:7891/callback` comme URI de redirection OAuth.

1. Dans **Instagram Login → Paramètres**
2. Dans le champ **"Valid OAuth Redirect URIs"** (ou "URI de redirection OAuth valides"), ajoutez :

```
https://localhost:7891/callback
```

3. Cliquez sur **"Save Changes"** (bouton bleu, bas de page)

> **Pourquoi HTTPS et non HTTP pour localhost ?**  
> Meta Instagram Login exige une URI en HTTPS même pour localhost. Getpostcraft génère automatiquement un certificat TLS auto-signé pour localhost lors du flow OAuth — c'est pour cette raison que le navigateur affiche un avertissement de sécurité (voir section Erreurs courantes).

> ⚠️ À vérifier : Certaines configurations Meta acceptent `http://localhost` pour les environnements de développement. La documentation officielle exige HTTPS, mais des retours d'expérience terrain montrent que certains apps en mode Development acceptent HTTP. L'implémentation actuelle de Getpostcraft utilise HTTPS.

---

## Étape 5 — Configurer les permissions (scopes)

Toujours dans **Instagram Login → Paramètres**, vérifiez que les scopes suivants sont activés ou ajoutés :

| Scope | Description |
|-------|-------------|
| `instagram_business_basic` | Accès au profil (ID, username, nom) |
| `instagram_business_content_publish` | Publication de posts |

Ces scopes correspondent exactement à ce que demande Getpostcraft lors de l'autorisation OAuth.

---

## Étape 6 — Ajouter un compte testeur

En mode Development (avant App Review Meta), seuls les comptes ayant un rôle sur l'application peuvent s'authentifier.

1. Dans le menu de gauche, cliquez sur **"Rôles de l'application"** (App Roles) → **"Rôles"**
2. Cliquez sur **"Ajouter des testeurs Instagram"**
3. Saisissez le nom d'utilisateur Instagram du compte à tester (ex: `terminallearning`)
4. Cliquez sur **"Envoyer"**

**Sur l'application Instagram du compte testeur :**
1. Ouvrir Instagram → Paramètres → Activités → Invitations
2. Accepter l'invitation de testeur

> Si vous testez avec votre propre compte, vous n'avez pas besoin d'invitation — le compte propriétaire de l'app a automatiquement accès.

---

## Différence Creator vs Business pour l'API

| Type de compte | `instagram_business_basic` | `instagram_business_content_publish` |
|----------------|---------------------------|--------------------------------------|
| Creator        | ✅ Supporté               | ✅ Supporté                           |
| Business       | ✅ Supporté               | ✅ Supporté                           |
| Personnel      | ❌ Non supporté           | ❌ Non supporté                       |

Les deux types de comptes professionnels fonctionnent identiquement avec l'API Instagram Login. La distinction Creator/Business est purement liée aux fonctionnalités Instagram (Boutique, Outils créateur, etc.) et n'impacte pas l'accès à l'API.

---

## Passer en mode Live (pour utilisation publique)

En mode Development, seuls les testeurs déclarés peuvent utiliser l'app. Pour un usage public (SaaS futur) :

1. Compléter les informations légales de l'app (Politique de confidentialité, CGU, contact)
2. Soumettre l'app à la **Meta App Review** pour les scopes `instagram_business_content_publish`
3. Meta examine l'app sous 5–7 jours ouvrés
4. Après approbation, basculer le toggle **"Mode live"** dans Paramètres → Général

> ⚠️ Pour un usage desktop personnel (V1), rester en mode Development est suffisant. L'App Review n'est nécessaire que si des utilisateurs sans rôle sur l'app doivent s'authentifier.

---

## Erreurs courantes et solutions

### "Invalid redirect_uri"
**Cause :** L'URI enregistrée dans Meta ne correspond pas exactement à celle envoyée par l'app.  
**Solution :** Vérifier qu'il n'y a pas de slash final. L'URI doit être exactement :
```
https://localhost:7891/callback
```

### "App not set up: This app is still in development mode"
**Cause :** Le compte Instagram essayant de se connecter n'est pas testeur de l'app.  
**Solution :** Ajouter le compte en testeur (Étape 6) et accepter l'invitation Instagram.

### Avertissement de certificat dans le navigateur
**Cause :** Getpostcraft génère un certificat TLS auto-signé pour `localhost:7891`. Les navigateurs ne font pas confiance aux certificats auto-signés.  
**Solution :** Cliquer sur "Avancé" → "Continuer vers localhost (non sécurisé)". Ce comportement est normal et sans risque car la connexion reste locale à votre machine.

### "The user hasn't authorized the application to perform this action"
**Cause :** Le scope `instagram_business_content_publish` n'a pas été accordé lors de l'autorisation.  
**Solution :** Déconnecter le compte dans Getpostcraft (Settings → Comptes) et relancer le flow OAuth.
