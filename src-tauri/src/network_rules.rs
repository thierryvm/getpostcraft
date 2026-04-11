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
