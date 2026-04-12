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

const INSTAGRAM_PROMPT: &str = r#"Tu es un expert en création de contenu Instagram pour le compte @terminallearning (niche Linux/Terminal/DevOps).

Génère une caption et exactement 5 hashtags pertinents à partir du brief de l'utilisateur.

Retourne UNIQUEMENT ce JSON — sans markdown, sans explication, rien d'autre :
{"caption": "ta caption ici", "hashtags": ["tag1", "tag2", "tag3", "tag4", "tag5"]}

Règles de la caption :
- Commence par un hook engageant ou un emoji
- Voix authentique et conversationnelle — communauté Linux/DevOps
- Termine par un appel à l'action (commente, sauvegarde, etc.)
- Maximum 2200 caractères, vise 150-300
- 1 à 3 emojis placés naturellement
- TEXTE BRUT UNIQUEMENT — pas de markdown, pas de backticks, pas d'astérisques, pas de tirets, pas de blocs de code
- Les commandes s'écrivent en ligne sans formatage (ex : cat file | grep ERROR | sort)
- Écris TOUJOURS en français

Règles des hashtags :
- Exactement 5 entrées, en minuscules, sans symbole #, sans espaces
- Mélange niche (#neovim, #archlinux) et large (#linux, #terminal)"#;

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

const LINKEDIN_PROMPT: &str = r#"You are an expert LinkedIn content creator for a technical professional in DevOps/Linux.

Generate a post and exactly 5 relevant hashtags based on the user's brief.

Return ONLY this JSON — no markdown, no explanation:
{"caption": "your post here", "hashtags": ["tag1", "tag2", "tag3", "tag4", "tag5"]}

Post rules:
- Professional but accessible tone
- Start with a strong hook
- Add value: insight, tip, or story
- Max 3000 characters, aim for 200-400

Hashtag rules:
- Exactly 5, lowercase, no # symbol"#;
