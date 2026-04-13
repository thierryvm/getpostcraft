# Guide — Product Truth

> Dernière mise à jour : avril 2026

---

## C'est quoi le Product Truth ?

Quand tu demandes à l'IA de générer un post, elle ne sait pas qui tu es, ce que tu vends, ni ce qui existe vraiment sur ton compte.

Sans contexte, elle peut inventer des choses :
- mentionner une formation que tu n'as pas encore sortie
- parler d'un outil que tu ne recommandes plus
- utiliser un ton ou un angle qui ne correspond pas à ton compte

**Le Product Truth, c'est une note que tu écris une seule fois, par compte.**
L'IA la lit avant chaque génération pour rester dans les clous.

---

## Exemple concret

Tu gères le compte **@terminallearning** sur Instagram.
Voici ce que tu pourrais écrire comme Product Truth :

```
Compte @[username] — niche [domaine], communauté [cible].

Ce qui existe aujourd'hui :
- [produit ou contenu A] (disponible)
- [produit ou contenu B] (disponible)

Ce qui N'existe PAS encore :
- [sujet X] (ne pas en parler)
- [offre Y] (pas proposée)

Ton de voix : [ex: direct, praticien, pas de jargon inutile]
Audience : [ex: développeurs débutants, sysadmins francophones]
```

Avec ce texte, l'IA ne mentionnera jamais la formation Kubernetes
et ne proposera jamais un coaching qui n'existe pas.

---

## Comment configurer ça

### Étape 1 — Aller dans Paramètres

Dans la barre de gauche, clique sur **Paramètres**.
Puis clique sur l'onglet **Comptes**.

---

### Étape 2 — Trouver ton compte connecté

Tu vois ton compte Instagram (ou LinkedIn) avec ton nom d'utilisateur.

En dessous du nom, il y a une section **Product Truth**
avec une zone de texte vide.

---

### Étape 3 — Écrire ton contexte

Clique dans la zone de texte et écris librement.

**Pas de format imposé.** L'IA comprend le langage naturel.
Voici un exemple court qui fonctionne très bien :

```
Profil LinkedIn — développeur indie, projet passion en parallèle d'un emploi salarié.

Ce que je construis : [nom du projet], [description en 1 ligne].
Ce que je publie : retours d'expérience de construction, pas de théorie.
Ton : honnête, direct, amateur assumé — pas d'expert qui donne des leçons.
Ce que je ne prétends PAS être : [compétences que tu n'as pas encore].
```

---

### Étape 4 — Enregistrer

Clique sur le bouton **Enregistrer**.

Le bouton affiche **"Enregistré ✓"** pendant 2 secondes
pour confirmer la sauvegarde.

---

### Étape 5 — Vérifier dans le Composer

Retourne dans le **Composer** (l'écran principal de création).

Sous le sélecteur de réseau (Instagram / LinkedIn),
tu vois maintenant un sélecteur **Compte** avec ton nom d'utilisateur.

Si le Product Truth est bien rempli, tu vois **✓ Product Truth**
en vert à côté de ton nom.

C'est tout. L'IA l'utilise automatiquement à chaque génération.

---

## Comment ça marche en coulisse

Quand tu cliques sur **Générer** :

1. L'app récupère ton Product Truth depuis la base locale
2. Elle l'ajoute à la fin des instructions envoyées à l'IA :

```
[Instructions de base — format JSON, règles Instagram...]

═══ BRAND IDENTITY / PRODUCT TRUTH ═══
Ce contexte décrit ce que le compte publie réellement.
Contrains ta génération à ce qui est listé ici :

[ton texte ici]
```

3. L'IA reçoit tout ça et génère un post cohérent
   avec ce que tu proposes vraiment

Rien ne quitte ta machine. Le texte est stocké
dans la base SQLite locale de l'app.

---

## Ce qu'il vaut mieux écrire

### Ce qui marche bien

- Les produits ou services qui **existent déjà**
  (avec leur nom exact)
- Ce qui **n'existe pas encore**
  (pour éviter que l'IA l'invente)
- Le **ton de voix** que tu veux garder
- L'**audience** que tu cibles
- Les sujets que tu veux ou ne veux **pas aborder**

### Ce qui ne sert à rien

- Les détails techniques de tes outils internes
- Les chiffres de performance (likes, abonnés)
- Les informations qui changent tous les jours

---

## Plusieurs comptes, plusieurs Product Truths

Tu peux connecter plusieurs comptes et écrire
un Product Truth différent pour chacun.

**Exemple :**

- **@terminallearning** → niche Linux, formations vidéo
- **@irontrack_app** → app mobile de suivi de musculation,
  disponible sur iOS et Android, pas encore sur web

Dans le Composer, tu choisis le compte dans le dropdown
avant de générer. L'IA prend le bon contexte automatiquement.

---

## Modifier ou effacer le Product Truth

Pour modifier : retourne dans **Paramètres → Comptes**,
édite le texte, clique **Enregistrer**.

Pour effacer complètement : supprime tout le texte
de la zone et clique **Enregistrer**.
L'IA reviendra à la génération générique sans contexte.

---

## Résumé en 3 étapes

```
1. Paramètres → Comptes
   → écrire le Product Truth de ton compte
   → Enregistrer

2. Composer
   → choisir le compte dans le sélecteur
   → écrire ton brief comme d'habitude

3. Générer
   → l'IA tient compte de ce que tu proposes vraiment
```
