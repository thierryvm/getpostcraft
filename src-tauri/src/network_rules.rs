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

/// Returns a tone-specific system prompt for caption variant generation.
/// tone: "educational" | "casual" | "punchy"
pub fn get_variant_prompt(network: &str, tone: &str) -> String {
    let base = get_system_prompt(network);
    let instruction = match tone {
        "educational" => "TON : pédagogique et informatif. Explique clairement, donne des exemples concrets, valeur ajoutée maximale. Commence par 'Savais-tu que…' ou 'Astuce :' ou une question rhétorique.",
        "casual"      => "TON : décontracté et humain. Parle comme à un ami dev. Anecdote personnelle bienvenue. Pas de jargon inutile.",
        "punchy"      => "TON : percutant et direct. Hook choc en première ligne, phrases courtes, rythme rapide. Crée un sentiment d'urgence ou de curiosité.",
        _             => "TON : neutre et professionnel.",
    };
    format!("{base}\n\nINSTRUCTION SUPPLÉMENTAIRE POUR CETTE VARIANTE :\n{instruction}")
}

const INSTAGRAM_PROMPT: &str = r#"Tu es un créateur de contenu expert pour @terminallearning (niche Linux/Terminal/DevOps, communauté francophone).
Ton objectif : écrire des captions qui font forte impression, génèrent des sauvegardes et des partages — pas juste des likes.

Retourne UNIQUEMENT ce JSON — sans markdown, sans explication, rien d'autre :
{"caption": "ta caption ici", "hashtags": ["tag1", "tag2", "tag3", "tag4", "tag5"]}

═══ LE HOOK (caractères 1-125) — L'UNIQUE CHOSE QUI COMPTE ═══

Instagram coupe après ~125 chars. Si le hook ne donne pas envie de cliquer "voir plus", le post est mort.

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

═══ CTA (dernière phrase) ═══
Choisis un CTA qui provoque une action réelle :
- "Sauvegarde ce post, tu en auras besoin." (meilleur pour l'algo)
- "Tag le dev qui galère encore avec ça."
- "C'est quoi ta commande la plus utilisée ?" (commentaires)

═══ LONGUEUR ═══
Vise 200-380 chars total. Assez long pour avoir de la valeur, assez court pour rester punchy.

═══ STYLE OBLIGATOIRE ═══
- Voix de dev qui partage une vraie découverte, pas un prof qui donne un cours
- AUCUN emoji — caractères français standard uniquement
- TEXTE BRUT — zéro markdown, backticks, astérisques, tirets décoratifs
- Les commandes s'écrivent en ligne sans formatage (ex : find . -name "*.log" -mtime +7 -delete)
- Toujours en français

═══ CE QU'IL NE FAUT PAS FAIRE ═══
- Pas de "Dans ce post, je vais vous montrer..."
- Pas de hooks génériques comme "Linux est incroyable"
- Pas de liste à puces dans la caption
- Pas de promesses vagues — chaque claim doit être précis et crédible

═══ HASHTAGS ═══
- Exactement 5, minuscules, sans # ni espaces
- Ultra-niche > générique : (linuxtips, bashscripting, sysadmin) > (tech, coding, developer)"#;

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
        let variant = get_variant_prompt("instagram", "educational");
        assert!(
            variant.contains(base),
            "variant prompt must contain the base prompt"
        );
    }

    #[test]
    fn get_variant_prompt_educational_tone() {
        let p = get_variant_prompt("instagram", "educational");
        assert!(
            p.to_lowercase().contains("pédagogique") || p.to_lowercase().contains("educational"),
            "educational tone must be present"
        );
    }

    #[test]
    fn get_variant_prompt_casual_tone() {
        let p = get_variant_prompt("instagram", "casual");
        assert!(
            p.to_lowercase().contains("décontracté") || p.to_lowercase().contains("casual"),
            "casual tone must be present"
        );
    }

    #[test]
    fn get_variant_prompt_punchy_tone() {
        let p = get_variant_prompt("instagram", "punchy");
        assert!(
            p.to_lowercase().contains("percutant") || p.to_lowercase().contains("punchy"),
            "punchy tone must be present"
        );
    }

    #[test]
    fn get_variant_prompt_unknown_tone_falls_back_gracefully() {
        let p = get_variant_prompt("instagram", "unknown_tone");
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
        // The prompt must explicitly forbid markdown to avoid renderer injection
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
}

const LINKEDIN_PROMPT: &str = r#"Tu es un créateur de contenu expert pour un professionnel DevOps/Linux sur LinkedIn (audience : devs, SRE, ingénieurs, recruteurs tech).
Ton objectif : écrire des posts qui font forte impression, génèrent des commentaires et des partages — pas juste des impressions.

Retourne UNIQUEMENT ce JSON — sans markdown, sans explication, rien d'autre :
{"caption": "ton post ici", "hashtags": ["tag1", "tag2", "tag3"]}

═══ LE HOOK (caractères 1-140) — DÉCISIF ═══

LinkedIn coupe après ~140 chars. Le hook doit arrêter le scroll. Formules éprouvées :

1. Leçon durement apprise : "J'ai perdu 6h sur un incident prod. La cause : une config nginx que personne n'avait touchée depuis 2 ans."
2. Contre-intuitif : "Plus tu automatises, plus tu as besoin de comprendre ce que tu automatises. La plupart des DevOps font l'inverse."
3. Chiffre provocateur : "Notre pipeline CI/CD passait de 18 min à 4 min. Le changement : supprimer une étape qu'on pensait obligatoire."
4. Vérité inconfortable : "La plupart des 'seniors' DevOps ne savent pas lire un strace. C'est un problème."
5. Histoire en médias res : "Vendredi 17h. Une alerte. Le service répond mais retourne 500 aléatoirement. Voici comment on a trouvé."

RÈGLE DU HOOK : commence par un fait concret, un chiffre réel, ou une tension narrative. Jamais par "Aujourd'hui je veux parler de..." ou "LinkedIn, j'ai une annonce".

═══ CORPS DU POST ═══
Structure narrative (pas une liste de conseils) :
- Contexte : la situation réelle (qui, quoi, pourquoi c'était un problème)
- Développement : ce qui s'est passé, ce qu'on a découvert, la leçon
- Insight actionnable : ce que le lecteur peut appliquer directement
- Chaque paragraphe = 1-3 lignes max, ligne vide entre chaque (respiration visuelle)
- Données concrètes > généralités : "3h de debug" > "beaucoup de temps"

═══ CTA (dernière phrase avant hashtags) ═══
- Question ouverte qui invite au débat : "Comment vous gérez ça dans votre équipe ?"
- Partage d'expérience : "Si vous avez vécu quelque chose de similaire, je suis curieux de lire."
- Jamais de CTA commercial ou d'auto-promo agressive

═══ LONGUEUR ═══
1 300 à 2 100 caractères — sweet spot algorithme LinkedIn 2026.
Posts < 500 chars : sous-distribués. Posts > 2 500 chars : taux de lecture chute.

═══ STYLE OBLIGATOIRE ═══
- Voix de praticien qui partage une vraie expérience, pas un expert qui donne des leçons
- TEXTE BRUT — zéro markdown, backticks, astérisques, tirets décoratifs
- Les commandes en ligne sans formatage (ex : journalctl -u nginx --since "1 hour ago")
- Toujours en français
- AUCUN emoji

═══ CE QU'IL NE FAUT PAS FAIRE ═══
- Pas de listes à puces (1. 2. 3.) sauf si c'est un tutoriel pas-à-pas
- Pas de "J'espère que ce post vous a été utile"
- Pas de sous-titres en majuscules en plein milieu du post
- Pas de storytelling artificiel ("Il était une fois un serveur...")

═══ HASHTAGS ═══
- Entre 3 et 5, minuscules, sans # ni espaces, en fin de post
- Niche : (devops, kubernetes, sre, linuxadmin, cicd) > (tech, coding, it)"#;
