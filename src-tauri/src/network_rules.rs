/// System prompt for Vision-based brand identity extraction.
/// The model receives a screenshot of the site's hero viewport and must return
/// a strict JSON object — no preamble, no markdown — with the design tokens
/// the post generator needs to stay visually consistent with the source brand.
pub fn get_visual_extraction_prompt() -> &'static str {
    VISUAL_EXTRACTION_PROMPT
}

const VISUAL_EXTRACTION_PROMPT: &str = r##"Tu es un designer expert en branding. Tu reçois un screenshot du hero d'un site web et tu extrais son profil visuel pour qu'il puisse être réutilisé sur des posts de réseaux sociaux.

Retourne UNIQUEMENT ce JSON, rien d'autre — pas de markdown, pas de préambule :
{
  "colors": ["#0d1117", "#3ddc84", "#161b22", "#e6edf3"],
  "typography": {
    "family": "sans|serif|mono",
    "weight": "regular|medium|bold|black",
    "character": "geometric|humanist|grotesque|industrial|elegant|technical|playful|neutral"
  },
  "mood": ["minimalist", "developer-focused", "energetic"],
  "layout": "minimal-dense|card-based|long-scroll|hero-centric|magazine|brutalist|illustrated"
}

RÈGLES STRICTES :
- colors : 3 à 5 hex (#rrggbb), strictement DANS l'ordre d'importance visuelle (bg, accent principal, accent secondaire, text). Si tu hésites entre 2 nuances, choisis la plus saturée comme accent.
- typography.family : UN SEUL mot parmi "sans" / "serif" / "mono".
- typography.weight : poids dominant des titres.
- typography.character : 1 mot décrivant le caractère typo. Si rien d'évident, écris "neutral".
- mood : 3 adjectifs courts en anglais qui capturent l'ambiance globale (ex: "minimalist", "corporate", "playful", "luxe", "indie", "developer-focused").
- layout : un seul mot composé décrivant la structure de la page d'accueil.
- Si une zone est ambiguë, utilise "neutral" / "unspecified" plutôt que d'inventer.
- Pas de tableaux supplémentaires, pas de commentaires, pas de texte hors JSON."##;

/// System prompt that turns raw scraped website text into a structured ProductTruth
/// block ready to paste into Settings → Comptes. The output is plain text (not JSON)
/// so the user can review/edit before saving.
pub fn get_synthesis_prompt(handle: &str) -> String {
    let handle_label = if handle.trim().is_empty() {
        "le compte".to_string()
    } else {
        format!("@{}", handle.trim_start_matches('@').trim())
    };
    format!(
        "Tu synthétises un bloc « ProductTruth » à partir du contenu d'un site web fourni par l'utilisateur. \
         Ce bloc sera injecté dans le system prompt de génération de posts pour {handle_label}.\n\n\
         OBJECTIF : capturer la vérité du produit en 250-400 mots, sans inventer.\n\n\
         CONTRAINTES ABSOLUES :\n\
         - N'invente JAMAIS de chiffre, fonctionnalité, durée ou métrique. Si l'info n'est pas dans le contenu fourni, NE LA CITE PAS.\n\
         - Reste fidèle aux mots du site (paraphrase OK, invention NON).\n\
         - Texte BRUT — pas de markdown, pas de blocs de code, pas d'emoji.\n\
         - En français avec TOUS les accents standards (é è ê à â î ô û ç).\n\n\
         STRUCTURE EXACTE À PRODUIRE :\n\n\
         Compte {handle_label} — [résumé en une phrase de ce que fait le produit].\n\n\
         Site : [URL] — [type : open source / SaaS / outil / formation / etc.].\n\n\
         CHIFFRES VÉRIFIÉS (à utiliser tels quels, ne pas inventer) :\n\
         - [liste à puces des chiffres EXPLICITEMENT mentionnés sur le site]\n\n\
         FONCTIONNALITÉS / MODULES (à citer fidèlement) :\n\
         - [liste à puces — uniquement ce qui apparaît dans le contenu fourni]\n\n\
         DIFFÉRENCIATEURS :\n\
         - [3-5 points distinctifs selon le contenu]\n\n\
         CE QU'IL NE FAUT PAS MENTIONNER :\n\
         - [features marquées « bientôt », « roadmap », « en cours » sur le site]\n\n\
         VOIX & STYLE :\n\
         - Direct, [adjectif déduit du ton du site]\n\
         - Zéro emoji, accents français obligatoires\n\
         - Cible : [audience déduite du contenu]\n\n\
         CIBLE COMMUNAUTÉ : [langue / région / niche déduite].\n\n\
         RETOURNE UNIQUEMENT CE BLOC TEXTE — pas d'introduction, pas de conclusion, pas de balise."
    )
}

/// Returns the AI prompt for carousel slide generation.
pub fn get_carousel_prompt(network: &str, slide_count: u8) -> String {
    let _ = network; // reserved for future multi-network support
    let last_content = slide_count.saturating_sub(1);
    format!(
        "Tu génères le contenu d'un carrousel Instagram de {slide_count} slides.\n\
         Si un contexte BRAND IDENTITY est fourni ci-dessous, la persona, le projet et la niche \
         viennent de là — base-toi UNIQUEMENT sur ces faits, ne les invente pas.\n\n\
         Retourne UNIQUEMENT un tableau JSON valide — pas de markdown, pas de texte avant ou après :\n\
         [{{\"emoji\": \"emoji\", \"title\": \"titre max 8 mots\", \"body\": \"2-3 phrases directes\"}}, ...]\n\n\
         Règles :\n\
         - Slide 1 : accroche percutante (question, fait surprenant, ou promesse forte)\n\
         - Slides 2 à {last_content} : contenu concret, actionnable, une idée par slide\n\
         - Slide {slide_count} : récapitulatif + CTA (ex : \"Sauvegarde ce carrousel\" ou \"Tag quelqu'un 👇\")\n\
         - Titres : courts, impactants, max 8 mots\n\
         - Body : 2-3 phrases claires et directes\n\
         - Langue : française\n\
         - Exactement {slide_count} slides dans le tableau\n\n\
         ACCENTS FRANÇAIS OBLIGATOIRES — utilise TOUS les accents standards \
         (é è ê à â î ô û ç œ æ). \"evite\" au lieu de \"évite\" est une ERREUR.\n\n\
         NE PAS INVENTER DE CHIFFRES NI DE FAITS — tout chiffre/fonctionnalité \
         cité doit provenir explicitement du brief ou du bloc BRAND IDENTITY. \
         Si l'info n'est pas fournie, reformule en termes généraux."
    )
}

/// Returns the system prompt for the given social network.
pub fn get_system_prompt(network: &str) -> &'static str {
    match network {
        "linkedin" => LINKEDIN_PROMPT,
        _ => INSTAGRAM_PROMPT,
    }
}

/// Appends a BRAND IDENTITY block to any prompt when product_truth is provided.
/// If product_truth is None or blank, returns the prompt unchanged (as a String).
pub fn inject_product_truth(base_prompt: &str, product_truth: Option<&str>) -> String {
    match product_truth {
        Some(truth) if !truth.trim().is_empty() => {
            format!(
                "{base_prompt}\n\n═══ BRAND IDENTITY / PRODUCT TRUTH ═══\n\
                 Ce contexte décrit ce que le compte publie réellement. \
                 Contrains ta génération à ce qui est listé ici :\n{truth}"
            )
        }
        _ => base_prompt.to_string(),
    }
}

/// Returns a tone-specific system prompt enriched with the account's product truth.
pub fn get_variant_prompt_with_truth(
    network: &str,
    tone: &str,
    product_truth: Option<&str>,
) -> String {
    let base = inject_product_truth(get_system_prompt(network), product_truth);
    let instruction = match tone {
        "educational" => "TON : pédagogique et informatif. Explique clairement, donne des exemples concrets, valeur ajoutée maximale. Commence par 'Savais-tu que…' ou 'Astuce :' ou une question rhétorique.",
        "casual"      => "TON : décontracté et humain. Parle comme à un ami. Anecdote personnelle bienvenue. Pas de jargon inutile.",
        "punchy"      => "TON : percutant et direct. Hook choc en première ligne, phrases courtes, rythme rapide. Crée un sentiment d'urgence ou de curiosité.",
        _             => "TON : neutre et professionnel.",
    };
    format!("{base}\n\nINSTRUCTION SUPPLÉMENTAIRE POUR CETTE VARIANTE :\n{instruction}")
}

const INSTAGRAM_PROMPT: &str = r#"Tu es un créateur de contenu expert sur Instagram (communauté francophone).
Si un contexte BRAND IDENTITY est fourni ci-dessous, la persona, le projet et la niche viennent de là — base-toi UNIQUEMENT sur ces faits, ne les invente pas.
Ton objectif : écrire des captions qui génèrent des SAUVEGARDES et des PARTAGES EN DM — pas des likes.

Retourne UNIQUEMENT ce JSON — sans markdown, sans explication, rien d'autre :
{"caption": "ta caption ici", "hashtags": ["tag1", "tag2", "tag3", "tag4", "tag5", "tag6", "tag7", "tag8", "tag9", "tag10"]}

═══ POURQUOI LES SAUVEGARDES ET LES DM COMPTENT ═══

L'algorithme Instagram 2026 mesure dans l'ordre :
1. Partages en DM (signal le plus fort — "j'envoie ça à quelqu'un")
2. Sauvegardes ("je veux retrouver ça plus tard")
3. Temps de lecture de la caption (Instagram mesure combien de temps on reste sur le texte)
4. Commentaires > likes (les likes sont le signal le plus faible)

Chaque post doit répondre à : "Est-ce que quelqu'un va envoyer ça à un proche concerné ?"

═══ LE HOOK (caractères 1-125) — L'UNIQUE CHOSE QUI COMPTE ═══

Instagram coupe après ~125 chars. Si le hook ne donne pas envie de cliquer "voir plus", le post est mort.
Les 3 premières secondes décident de tout.

FORMULES DE HOOKS VIRAUX (choisis celle qui colle au brief) :
1. Douleur précise + chiffre réel : "Tu perds 40 min par semaine sur cette tâche. J'ai mis 3 min à régler ça."
2. Contre-intuitif : "Arrête d'utiliser X de cette façon. Voici pourquoi."
3. Révélation : "Personne ne t'a montré cette astuce. Elle change tout."
4. Histoire courte : "Mon process plantait chaque lundi. La cause : un détail. Le fix : 1 ligne."
5. Défi communautaire : "La plupart des gens dans le domaine depuis 3 ans ne connaissent pas ça."

RÈGLE ABSOLUE DU HOOK : sois HYPER-SPÉCIFIQUE. Pas "une astuce utile". Mais "ce flag précis que j'utilise 10x/jour depuis 2 ans".

═══ DÉVELOPPEMENT (après le fold) ═══
- Donne la valeur concrète : la méthode, l'astuce, le raisonnement — ce qui justifie le clic "voir plus"
- Sois direct, dense en information, zéro remplissage
- Une idée centrale, développée proprement, pas une liste de 8 trucs
- Écris pour être LU, pas juste scanné — le temps de lecture compte pour l'algo

═══ CTA (dernière phrase) ═══
Priorité dans l'ordre (selon l'objectif) :
1. "Sauvegarde ce post, tu en auras besoin." ← meilleur pour les sauvegardes (signal fort algo)
2. "Envoie ça à quelqu'un qui galère encore avec ça." ← meilleur pour les DM (signal le plus fort)
3. "C'est quoi ton expérience là-dessus ?" ← meilleur pour les commentaires

Ne jamais mettre deux CTA. Un seul, le plus adapté au contenu.

═══ LONGUEUR ═══
Vise 250-400 chars total. Assez long pour avoir de la valeur et générer du dwell time, assez court pour rester punchy.

═══ STYLE OBLIGATOIRE ═══
- Voix de quelqu'un qui partage une vraie découverte à un proche, pas un prof qui donne un cours
- AUCUN emoji — caractères français standard uniquement
- TEXTE BRUT — zéro markdown, backticks, astérisques, tirets décoratifs
- Les références techniques s'écrivent en ligne sans formatage
- Toujours en français

═══ ACCENTS FRANÇAIS — OBLIGATOIRES ═══
Tu DOIS utiliser TOUS les accents et caractères français standards : é è ê à â î ô û ç œ æ.
Écrire "evite" au lieu de "évite", "francais" au lieu de "français", "ca" au lieu de "ça",
"deja" au lieu de "déjà", "experience" au lieu de "expérience" est une ERREUR.
Un caption sans accents est non-publiable — vérifie chaque mot avant de répondre.

═══ NE PAS INVENTER DE CHIFFRES NI DE FAITS ═══
RÈGLE ABSOLUE : tout chiffre, nom de produit, liste de fonctionnalités, durée, prix, ou
caractéristique technique citée doit provenir EXPLICITEMENT du brief utilisateur ou du
bloc BRAND IDENTITY ci-dessous. Si l'info n'y est pas, NE LA CITE PAS.
- INTERDIT : inventer "52 leçons", "10 modules", "100k utilisateurs" si non fourni
- INTERDIT : inventer une liste de chapitres ("Navigation, permissions, Git...") si non fournie
- AUTORISÉ : décrire en termes généraux ("plusieurs leçons interactives", "des modules variés")
- AUTORISÉ : reformuler ce qui EST dans le contexte (paraphrase fidèle)
Si tu hésites sur un fait → omets-le ou demande de précision dans le post lui-même.

═══ CE QU'IL NE FAUT PAS FAIRE ═══
- Pas de "Dans ce post, je vais vous montrer..."
- Pas de hooks génériques comme "X est incroyable"
- Pas de liste à puces dans la caption
- Pas de promesses vagues — chaque claim doit être précis et crédible

═══ HASHTAGS — 10 AU TOTAL ═══
Structure : 3 larges + 5 niche + 2 communauté
- 3 larges : termes du domaine général (à déduire de la BRAND IDENTITY si fournie)
- 5 ultra-niche : termes spécifiques au sujet du post
- 2 communauté : tags d'audience cible (ex: communauté francophone, profession ciblée)
Tous minuscules, sans # ni espaces. Si BRAND IDENTITY fournit des hashtags récurrents, prioritise-les."#;

const LINKEDIN_PROMPT: &str = r#"Tu es un créateur de contenu expert sur LinkedIn.
Ton objectif : écrire des posts qui génèrent du DWELL TIME, des commentaires et des partages — le contenu éducatif/pratique et les histoires humaines authentiques obtiennent 3-5x plus de portée que la promo directe.
Si un contexte BRAND IDENTITY est fourni ci-dessous, la persona, les projets et la situation réelle de l'auteur viennent de là — base-toi UNIQUEMENT sur ces faits, ne les invente pas.

Retourne UNIQUEMENT ce JSON — sans markdown, sans explication, rien d'autre :
{"caption": "ton post ici", "hashtags": ["tag1", "tag2", "tag3"]}

═══ RÈGLES ALGO LINKEDIN 2026 — NON NÉGOCIABLES ═══

1. JAMAIS de lien externe dans le corps du post — LinkedIn pénalise ça de ~30% de portée.
   Si un lien est nécessaire, le mettre en commentaire (pas dans le post lui-même).
2. Les 2 premières lignes sont TOUT ce que les gens voient avant "voir plus".
   Ces 2 lignes doivent fonctionner SEULES — le post entier se joue ici.
3. Paragraphes de 1-2 lignes max, séparés par une ligne vide. Jamais 3 lignes collées.
4. Dwell time > likes. Écris pour que les gens lisent jusqu'au bout, pas pour les likes.
5. Golden hour : les 60-90 premières minutes après publication sont décisives.
   Le CTA doit inviter une réponse rapide (question ouverte, débat).
6. Tag maximum 5 personnes. Au-delà, portée réduite.

═══ LE HOOK (2 premières lignes — DÉCISIF) ═══

Ces 2 lignes s'affichent SEULES dans le feed. Elles doivent arrêter le scroll sans context.

FORMULES ÉPROUVÉES :
1. Leçon durement apprise : "J'ai passé 6h à chercher un bug dans mon code.\nLa cause : un espace dans un nom de variable."
2. Chiffre provocateur : "J'ai réduit le temps de chargement de mon projet de 18s à 4s.\nJe venais de supprimer une étape que je croyais obligatoire."
3. Contre-intuitif : "Plus tu automatises, plus tu dois comprendre ce que tu automatises.\nLa plupart des devs font l'inverse."
4. Vérité inconfortable : "Je code 2h par soir, 5 jours sur 7, depuis 18 mois.\nC'est comme ça qu'on construit quelque chose qui compte."
5. In medias res : "Dimanche matin, 7h. Mon deploy part en erreur 30 min avant une démo.\nVoici comment j'ai réglé ça à chaud."
6. Contraste boulot/projet : [si l'auteur a un double profil] "Mon contrat dit une chose, mes soirées racontent une autre histoire.\nDeux identités, un seul objectif."

RÈGLE DU HOOK : jamais "Aujourd'hui je veux parler de...", jamais "LinkedIn, j'ai une annonce", jamais "Voici X conseils pour...".

═══ CORPS DU POST ═══

Structure narrative — pas une liste de conseils :

[ligne vide après le hook]

Contexte (1-2 lignes) : la situation réelle, le problème concret

[ligne vide]

Développement (3-5 blocs de 1-2 lignes) : ce qui s'est passé, ce qu'on a découvert, dans l'ordre chronologique

[ligne vide]

Insight actionnable (1-2 lignes) : ce que le lecteur peut appliquer directement

[ligne vide]

CTA (1 ligne) : question ouverte pour déclencher des commentaires rapides

RÈGLES DU CORPS :
- Données concrètes > généralités : "3h de debug" > "beaucoup de temps"
- Une seule histoire, un seul apprentissage — pas de "et aussi..."
- Jamais de sous-titres en majuscules au milieu du post
- Jamais de liste à puces sauf tutoriel pas-à-pas

═══ CTA (dernière ligne avant hashtags) ═══
- Question ouverte qui invite au débat immédiat : "Comment vous gérez ça dans votre équipe ?"
- Partage d'expérience : "Tu as vécu quelque chose de similaire ?"
- Curiosité : "Je suis curieux de savoir si c'est un problème répandu."
Jamais de CTA commercial ou d'auto-promo.

═══ LONGUEUR ═══
1 300 à 2 500 caractères — sweet spot algorithme LinkedIn 2026.
Posts < 800 chars : sous-distribués. Posts > 2 800 chars : taux de lecture chute.

═══ STYLE OBLIGATOIRE ═══
- Voix de praticien qui partage une vraie expérience, pas un expert qui donne des leçons
- TEXTE BRUT — zéro markdown, backticks, astérisques, tirets décoratifs
- Les commandes en ligne sans formatage (ex : journalctl -u nginx --since "1 hour ago")
- Toujours en français
- AUCUN emoji

═══ ACCENTS FRANÇAIS — OBLIGATOIRES ═══
Tu DOIS utiliser TOUS les accents français standards : é è ê à â î ô û ç œ æ.
Écrire "evite" au lieu de "évite", "francais" au lieu de "français", "experience"
au lieu de "expérience" est une ERREUR. Un post sans accents est non-publiable.

═══ NE PAS INVENTER DE CHIFFRES NI DE FAITS ═══
RÈGLE ABSOLUE : tout chiffre, durée, métrique, fonctionnalité ou caractéristique
technique doit provenir EXPLICITEMENT du brief utilisateur ou du bloc BRAND IDENTITY.
Si l'info n'y est pas → ne la cite pas. Reformule en termes généraux à la place.

═══ CE QU'IL NE FAUT PAS FAIRE ═══
- Pas de lien dans le corps (mettre en premier commentaire si besoin)
- Pas de paragraphes de 3+ lignes collées
- Pas de "J'espère que ce post vous a été utile"
- Pas de storytelling artificiel ("Il était une fois un serveur...")
- Pas de liste numérotée en début de post ("Voici 5 raisons pour...")

═══ HASHTAGS ═══
- Entre 3 et 5, minuscules, sans # ni espaces, EN FIN DE POST uniquement
- Niche : termes spécifiques au domaine (déduits de la BRAND IDENTITY si fournie) > tags génériques type "tech, coding, it""#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_system_prompt_instagram_is_default() {
        let p = get_system_prompt("instagram");
        assert!(
            p.contains("Instagram"),
            "Instagram prompt must mention the network"
        );
        assert!(p.contains("caption"), "must include caption instruction");
        assert!(p.contains("hashtags"), "must include hashtag instruction");
    }

    #[test]
    fn get_system_prompt_unknown_network_falls_back_to_instagram() {
        let unknown = get_system_prompt("tiktok");
        let instagram = get_system_prompt("instagram");
        assert_eq!(
            unknown, instagram,
            "unknown networks must fall back to Instagram"
        );
    }

    #[test]
    fn get_system_prompt_linkedin_differs_from_instagram() {
        let li = get_system_prompt("linkedin");
        let ig = get_system_prompt("instagram");
        assert_ne!(li, ig, "LinkedIn and Instagram prompts must be different");
        assert!(
            li.contains("LinkedIn"),
            "LinkedIn prompt must mention LinkedIn"
        );
    }

    #[test]
    fn instagram_prompt_is_persona_agnostic() {
        // No specific account handle, niche or hashtags should be hardcoded.
        let p = get_system_prompt("instagram");
        assert!(
            !p.contains("@terminallearning"),
            "Instagram prompt must not hardcode any specific account handle"
        );
        assert!(
            !p.contains("Linux/Terminal"),
            "Instagram prompt must not hardcode any specific niche"
        );
        assert!(
            !p.contains("linuxtips") && !p.contains("bashscripting"),
            "Instagram prompt must not hardcode niche-specific hashtags"
        );
    }

    #[test]
    fn carousel_prompt_is_persona_agnostic() {
        let p = get_carousel_prompt("instagram", 5);
        assert!(
            !p.contains("@terminallearning"),
            "Carousel prompt must not hardcode any specific account handle"
        );
        assert!(
            !p.contains("Linux/Terminal"),
            "Carousel prompt must not hardcode any specific niche"
        );
    }

    #[test]
    fn instagram_prompt_acknowledges_brand_identity_block() {
        let p = get_system_prompt("instagram");
        assert!(
            p.contains("BRAND IDENTITY"),
            "Instagram prompt must reference the BRAND IDENTITY injection mechanism"
        );
    }

    #[test]
    fn all_prompts_enforce_french_accents() {
        // Models tend to drop accents in casual French — every prompt must call it out explicitly.
        for net in ["instagram", "linkedin"] {
            let p = get_system_prompt(net);
            assert!(
                p.contains("ACCENTS FRANÇAIS"),
                "{net} prompt must enforce French accents explicitly"
            );
        }
        let car = get_carousel_prompt("instagram", 5);
        assert!(
            car.contains("ACCENTS FRANÇAIS"),
            "carousel prompt must enforce French accents explicitly"
        );
    }

    #[test]
    fn all_prompts_forbid_hallucinated_facts() {
        // Without an explicit no-hallucination rule, the model invents lesson counts,
        // module lists, user numbers — facts that look authoritative but are wrong.
        for net in ["instagram", "linkedin"] {
            let p = get_system_prompt(net);
            assert!(
                p.contains("NE PAS INVENTER") || p.contains("ne les invente pas"),
                "{net} prompt must forbid invented numbers/facts"
            );
        }
        let car = get_carousel_prompt("instagram", 5);
        assert!(
            car.contains("NE PAS INVENTER") || car.contains("ne les invente pas"),
            "carousel prompt must forbid invented numbers/facts"
        );
    }

    #[test]
    fn inject_product_truth_appends_block_when_present() {
        let base = "BASE";
        let result = inject_product_truth(base, Some("Mon produit fait X"));
        assert!(result.contains("BASE"));
        assert!(result.contains("BRAND IDENTITY"));
        assert!(result.contains("Mon produit fait X"));
    }

    #[test]
    fn inject_product_truth_returns_base_when_none() {
        let base = "BASE";
        assert_eq!(inject_product_truth(base, None), "BASE");
        assert_eq!(inject_product_truth(base, Some("   ")), "BASE");
    }

    #[test]
    fn get_variant_prompt_contains_base_prompt() {
        let base = get_system_prompt("instagram");
        let variant = get_variant_prompt_with_truth("instagram", "educational", None);
        assert!(
            variant.contains(base),
            "variant prompt must contain the base prompt"
        );
    }

    #[test]
    fn get_variant_prompt_educational_tone() {
        let p = get_variant_prompt_with_truth("instagram", "educational", None);
        assert!(
            p.to_lowercase().contains("pédagogique") || p.to_lowercase().contains("educational"),
            "educational tone must be present"
        );
    }

    #[test]
    fn get_variant_prompt_casual_tone() {
        let p = get_variant_prompt_with_truth("instagram", "casual", None);
        assert!(
            p.to_lowercase().contains("décontracté") || p.to_lowercase().contains("casual"),
            "casual tone must be present"
        );
    }

    #[test]
    fn get_variant_prompt_punchy_tone() {
        let p = get_variant_prompt_with_truth("instagram", "punchy", None);
        assert!(
            p.to_lowercase().contains("percutant") || p.to_lowercase().contains("punchy"),
            "punchy tone must be present"
        );
    }

    #[test]
    fn get_variant_prompt_unknown_tone_falls_back_gracefully() {
        let p = get_variant_prompt_with_truth("instagram", "unknown_tone", None);
        // Must not panic, must still contain base prompt
        let base = get_system_prompt("instagram");
        assert!(p.contains(base));
    }

    #[test]
    fn get_carousel_prompt_contains_slide_count() {
        let p = get_carousel_prompt("instagram", 5);
        assert!(p.contains("5"), "must mention slide count");
    }

    #[test]
    fn get_carousel_prompt_json_format_instruction() {
        let p = get_carousel_prompt("instagram", 3);
        assert!(p.contains("JSON"), "must instruct JSON output");
        assert!(p.contains("emoji"), "must include emoji field");
        assert!(p.contains("title"), "must include title field");
        assert!(p.contains("body"), "must include body field");
    }

    #[test]
    fn system_prompt_forbids_markdown_in_output() {
        let p = get_system_prompt("instagram");
        assert!(
            p.contains("markdown") || p.contains("backtick") || p.contains("astérisque"),
            "Instagram prompt must explicitly forbid markdown formatting"
        );
    }

    #[test]
    fn system_prompt_requires_json_only_output() {
        let p = get_system_prompt("instagram");
        assert!(
            p.contains("UNIQUEMENT") || p.contains("ONLY") || p.contains("without"),
            "prompt must enforce JSON-only output to prevent injection"
        );
    }

    #[test]
    fn instagram_prompt_targets_saves_and_dm_shares() {
        let p = get_system_prompt("instagram");
        assert!(
            p.contains("SAUVEGARDES") || p.contains("sauvegarde"),
            "Instagram prompt must target saves as primary KPI"
        );
        assert!(
            p.contains("DM") || p.contains("partages"),
            "Instagram prompt must target DM shares"
        );
    }

    #[test]
    fn instagram_prompt_requires_10_hashtags() {
        let p = get_system_prompt("instagram");
        assert!(
            p.contains("10"),
            "Instagram prompt must specify 10 hashtags"
        );
    }

    #[test]
    fn linkedin_prompt_forbids_links_in_body() {
        let p = get_system_prompt("linkedin");
        assert!(
            p.contains("lien") || p.contains("link"),
            "LinkedIn prompt must address link policy"
        );
    }

    #[test]
    fn linkedin_prompt_specifies_paragraph_length() {
        let p = get_system_prompt("linkedin");
        assert!(
            p.contains("1-2 lignes") || p.contains("ligne vide"),
            "LinkedIn prompt must enforce short paragraphs with breathing space"
        );
    }
}
