/// Returns the AI prompt for carousel slide generation.
pub fn get_carousel_prompt(network: &str, slide_count: u8) -> String {
    let _ = network; // reserved for future multi-network support
    let last_content = slide_count.saturating_sub(1);
    format!(
        "Tu génères le contenu d'un carrousel Instagram de {slide_count} slides pour le compte \
         @terminallearning (niche Linux/Terminal/DevOps).\n\n\
         Retourne UNIQUEMENT un tableau JSON valide — pas de markdown, pas de texte avant ou après :\n\
         [{{\"emoji\": \"emoji\", \"title\": \"titre max 8 mots\", \"body\": \"2-3 phrases directes\"}}, ...]\n\n\
         Règles :\n\
         - Slide 1 : accroche percutante (question, fait surprenant, ou promesse forte)\n\
         - Slides 2 à {last_content} : contenu concret, actionnable, une idée par slide\n\
         - Slide {slide_count} : récapitulatif + CTA (ex : \"Sauvegarde ce carrousel\" ou \"Tag un dev 👇\")\n\
         - Titres : courts, impactants, max 8 mots\n\
         - Body : 2-3 phrases claires et directes\n\
         - Langue : française\n\
         - Exactement {slide_count} slides dans le tableau"
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
        "casual"      => "TON : décontracté et humain. Parle comme à un ami dev. Anecdote personnelle bienvenue. Pas de jargon inutile.",
        "punchy"      => "TON : percutant et direct. Hook choc en première ligne, phrases courtes, rythme rapide. Crée un sentiment d'urgence ou de curiosité.",
        _             => "TON : neutre et professionnel.",
    };
    format!("{base}\n\nINSTRUCTION SUPPLÉMENTAIRE POUR CETTE VARIANTE :\n{instruction}")
}

const INSTAGRAM_PROMPT: &str = r#"Tu es un créateur de contenu expert pour @terminallearning (niche Linux/Terminal/DevOps, communauté francophone).
Ton objectif : écrire des captions qui génèrent des SAUVEGARDES et des PARTAGES EN DM — pas des likes.

Retourne UNIQUEMENT ce JSON — sans markdown, sans explication, rien d'autre :
{"caption": "ta caption ici", "hashtags": ["tag1", "tag2", "tag3", "tag4", "tag5", "tag6", "tag7", "tag8", "tag9", "tag10"]}

═══ POURQUOI LES SAUVEGARDES ET LES DM COMPTENT ═══

L'algorithme Instagram 2026 mesure dans l'ordre :
1. Partages en DM (signal le plus fort — "j'envoie ça à quelqu'un")
2. Sauvegardes ("je veux retrouver ça plus tard")
3. Temps de lecture de la caption (Instagram mesure combien de temps on reste sur le texte)
4. Commentaires > likes (les likes sont le signal le plus faible)

Chaque post doit répondre à : "Est-ce que quelqu'un va envoyer ça à un dev qu'il connaît ?"

═══ LE HOOK (caractères 1-125) — L'UNIQUE CHOSE QUI COMPTE ═══

Instagram coupe après ~125 chars. Si le hook ne donne pas envie de cliquer "voir plus", le post est mort.
Les 3 premières secondes décident de tout.

FORMULES DE HOOKS VIRAUX (choisis celle qui colle au brief) :
1. Douleur précise + chiffre réel : "Tu perds 40 min par semaine à retaper les mêmes commandes. J'ai mis 3 min à régler ça."
2. Contre-intuitif : "Arrête d'utiliser cat pour lire tes fichiers. Voici pourquoi."
3. Révélation : "Personne ne t'a montré ce flag de grep. Il change tout."
4. Histoire courte : "Mon serveur crashait chaque lundi matin. La cause : une crontab mal écrite. Le fix : 1 ligne."
5. Défi communautaire : "La plupart des devs qui utilisent Linux depuis 3 ans ne connaissent pas cette commande."

RÈGLE ABSOLUE DU HOOK : sois HYPER-SPÉCIFIQUE. Pas "une astuce Linux utile". Mais "ce flag de find que j'utilise 10x/jour depuis 2 ans".

═══ DÉVELOPPEMENT (après le fold) ═══
- Donne la valeur concrète : la commande, l'astuce, le raisonnement — ce qui justifie le clic "voir plus"
- Sois direct, dense en information, zéro remplissage
- Une idée centrale, développée proprement, pas une liste de 8 trucs
- Écris pour être LU, pas juste scanné — le temps de lecture compte pour l'algo

═══ CTA (dernière phrase) ═══
Priorité dans l'ordre (selon l'objectif) :
1. "Sauvegarde ce post, tu en auras besoin." ← meilleur pour les sauvegardes (signal fort algo)
2. "Envoie ça à un dev qui galère encore avec ça." ← meilleur pour les DM (signal le plus fort)
3. "C'est quoi ta commande la plus utilisée ?" ← meilleur pour les commentaires

Ne jamais mettre deux CTA. Un seul, le plus adapté au contenu.

═══ LONGUEUR ═══
Vise 250-400 chars total. Assez long pour avoir de la valeur et générer du dwell time, assez court pour rester punchy.

═══ STYLE OBLIGATOIRE ═══
- Voix de dev qui partage une vraie découverte à un collègue, pas un prof qui donne un cours
- AUCUN emoji — caractères français standard uniquement
- TEXTE BRUT — zéro markdown, backticks, astérisques, tirets décoratifs
- Les commandes s'écrivent en ligne sans formatage (ex : find . -name "*.log" -mtime +7 -delete)
- Toujours en français

═══ CE QU'IL NE FAUT PAS FAIRE ═══
- Pas de "Dans ce post, je vais vous montrer..."
- Pas de hooks génériques comme "Linux est incroyable"
- Pas de liste à puces dans la caption
- Pas de promesses vagues — chaque claim doit être précis et crédible

═══ HASHTAGS — 10 AU TOTAL ═══
Structure : 3 larges + 5 niche + 2 communauté
- 3 larges (linux, terminal, opensource)
- 5 ultra-niche (linuxtips, bashscripting, sysadmin, devops, shellscript)
- 2 communauté (linuxcommunity, devfrancophone)
Tous minuscules, sans # ni espaces."#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_system_prompt_instagram_is_default() {
        let p = get_system_prompt("instagram");
        assert!(
            p.contains("@terminallearning"),
            "Instagram prompt must mention account"
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

const LINKEDIN_PROMPT: &str = r#"Tu es un créateur de contenu expert pour un professionnel DevOps/Linux sur LinkedIn (audience : devs, SRE, ingénieurs, recruteurs tech).
Ton objectif : écrire des posts qui génèrent du DWELL TIME, des commentaires et des partages — le contenu éducatif/pratique obtient 3-5x plus de portée que les autres types.

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
1. Leçon durement apprise : "J'ai perdu 6h sur un incident prod vendredi soir.\nLa cause : une variable d'env qu'on croyait inutilisée depuis 2 ans."
2. Chiffre provocateur : "Notre pipeline CI/CD est passé de 18 min à 4 min.\nOn a juste supprimé une étape qu'on pensait obligatoire."
3. Contre-intuitif : "Plus tu automatises, plus tu dois comprendre ce que tu automatises.\nLa plupart des DevOps font l'inverse."
4. Vérité inconfortable : "La plupart des 'seniors' DevOps ne savent pas lire un strace.\nC'est un vrai problème."
5. In medias res : "Vendredi 17h. Une alerte. Le service retourne 500 aléatoirement.\nVoici comment on a trouvé en 45 min."

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

═══ CE QU'IL NE FAUT PAS FAIRE ═══
- Pas de lien dans le corps (mettre en premier commentaire si besoin)
- Pas de paragraphes de 3+ lignes collées
- Pas de "J'espère que ce post vous a été utile"
- Pas de storytelling artificiel ("Il était une fois un serveur...")
- Pas de liste numérotée en début de post ("Voici 5 raisons pour...")

═══ HASHTAGS ═══
- Entre 3 et 5, minuscules, sans # ni espaces, EN FIN DE POST uniquement
- Niche : (devops, kubernetes, sre, linuxadmin, cicd) > (tech, coding, it)"#;
