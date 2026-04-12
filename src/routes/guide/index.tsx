import {
  BookOpen,
  PenLine,
  Zap,
  Image,
  Send,
  Settings,
  LayoutDashboard,
  CalendarDays,
  Hash,
  ChevronRight,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";

// ── Types ─────────────────────────────────────────────────────────────────

type Step = { icon: React.ElementType; title: string; desc: string };
type SectionProps = { id: string; icon: React.ElementType; title: string; children: React.ReactNode };

// ── Section wrapper ────────────────────────────────────────────────────────

function Section({ id, icon: Icon, title, children }: SectionProps) {
  return (
    <section id={id} className="flex flex-col gap-4">
      <div className="flex items-center gap-3 pb-2 border-b border-border">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary/10">
          <Icon className="h-4 w-4 text-primary" />
        </div>
        <h2 className="text-base font-semibold text-foreground">{title}</h2>
      </div>
      {children}
    </section>
  );
}

// ── Step card ─────────────────────────────────────────────────────────────

function StepCard({ steps }: { steps: Step[] }) {
  return (
    <div className="flex flex-col gap-2">
      {steps.map((s, i) => (
        <div key={i} className="flex items-start gap-4 rounded-lg border border-border bg-card p-4">
          <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-primary text-[11px] font-bold text-primary-foreground">
            {i + 1}
          </div>
          <div className="flex flex-col gap-0.5">
            <div className="flex items-center gap-2">
              <s.icon className="h-3.5 w-3.5 text-primary" />
              <span className="text-sm font-medium text-foreground">{s.title}</span>
            </div>
            <p className="text-xs text-muted-foreground leading-relaxed">{s.desc}</p>
          </div>
        </div>
      ))}
    </div>
  );
}

// ── Info block ────────────────────────────────────────────────────────────

function Info({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-border bg-card p-4 flex flex-col gap-1.5">
      <span className="text-[11px] font-semibold uppercase tracking-widest text-primary">{label}</span>
      <div className="text-sm text-foreground/90 leading-relaxed">{children}</div>
    </div>
  );
}

// ── Tip block ─────────────────────────────────────────────────────────────

function Tip({ children }: { children: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-primary/20 bg-primary/5 px-4 py-3 text-sm text-foreground/80 leading-relaxed">
      <span className="font-semibold text-primary mr-1">💡</span>
      {children}
    </div>
  );
}

// ── Visual previews ───────────────────────────────────────────────────────

function PreviewPost() {
  return (
    <div className="w-full aspect-square rounded-md overflow-hidden bg-[#0d1117] flex flex-col items-center justify-center p-3 gap-2 border border-white/5">
      <div className="w-2 h-2 rounded-full bg-[#3ddc84]" />
      <div className="w-3/4 h-2 rounded bg-white/20" />
      <div className="w-full h-1.5 rounded bg-white/10" />
      <div className="w-5/6 h-1.5 rounded bg-white/10" />
      <div className="w-4/6 h-1.5 rounded bg-white/10" />
      <div className="mt-1 flex gap-1">
        <div className="h-1 w-8 rounded-full bg-[#3ddc84]/40" />
        <div className="h-1 w-10 rounded-full bg-[#3ddc84]/40" />
        <div className="h-1 w-7 rounded-full bg-[#3ddc84]/40" />
      </div>
    </div>
  );
}

function PreviewCode() {
  return (
    <div className="w-full aspect-square rounded-md overflow-hidden bg-[#0d1117] border border-white/5 flex flex-col">
      {/* Title bar */}
      <div className="flex items-center gap-1.5 px-3 py-2 bg-[#161b22] border-b border-white/5">
        <div className="h-2 w-2 rounded-full bg-red-500/60" />
        <div className="h-2 w-2 rounded-full bg-yellow-500/60" />
        <div className="h-2 w-2 rounded-full bg-green-500/60" />
        <span className="ml-2 text-[9px] text-white/30 font-mono">script.sh</span>
      </div>
      {/* Code lines */}
      <div className="flex-1 flex flex-col gap-1.5 p-3 font-mono">
        <div className="flex gap-2">
          <span className="text-[8px] text-[#3ddc84]/70">#!/bin/bash</span>
        </div>
        <div className="flex gap-1.5">
          <span className="text-[8px] text-blue-400/70">echo</span>
          <span className="text-[8px] text-yellow-300/70">"Hello world"</span>
        </div>
        <div className="flex gap-1.5">
          <span className="text-[8px] text-purple-400/70">for</span>
          <span className="text-[8px] text-white/50">i in</span>
          <span className="text-[8px] text-yellow-300/70">1 2 3</span>
        </div>
        <div className="flex gap-1.5 pl-3">
          <span className="text-[8px] text-blue-400/70">echo</span>
          <span className="text-[8px] text-white/40">$i</span>
        </div>
        <div className="text-[8px] text-purple-400/70">done</div>
      </div>
    </div>
  );
}

function PreviewTerminal() {
  return (
    <div className="w-full aspect-square rounded-md overflow-hidden bg-[#0d1117] border border-white/5 flex flex-col">
      {/* Title bar */}
      <div className="flex items-center gap-1.5 px-3 py-2 bg-[#161b22] border-b border-white/5">
        <div className="h-2 w-2 rounded-full bg-red-500/60" />
        <div className="h-2 w-2 rounded-full bg-yellow-500/60" />
        <div className="h-2 w-2 rounded-full bg-green-500/60" />
        <span className="ml-2 text-[9px] text-white/30 font-mono">bash</span>
      </div>
      {/* Terminal output */}
      <div className="flex-1 flex flex-col gap-1 p-3 font-mono">
        <div className="flex gap-1">
          <span className="text-[8px] text-[#3ddc84]">$</span>
          <span className="text-[8px] text-white/70">ls -la</span>
        </div>
        <div className="text-[8px] text-white/40">total 48</div>
        <div className="text-[8px] text-white/40">drwxr-xr-x  .config</div>
        <div className="text-[8px] text-white/40">-rw-r--r--  .bashrc</div>
        <div className="flex gap-1 mt-1">
          <span className="text-[8px] text-[#3ddc84]">$</span>
          <span className="text-[8px] text-white/70">git status</span>
        </div>
        <div className="text-[8px] text-[#3ddc84]/70">On branch main</div>
        <div className="text-[8px] text-[#3ddc84]/50">nothing to commit</div>
      </div>
    </div>
  );
}

function PreviewCarousel() {
  return (
    <div className="w-full aspect-square rounded-md overflow-hidden bg-transparent flex items-center justify-center">
      {/* Stacked slides effect */}
      <div className="relative w-4/5 h-4/5">
        {/* Slide 3 (back) */}
        <div className="absolute inset-0 translate-x-3 translate-y-3 rounded-md bg-[#161b22] border border-white/5" />
        {/* Slide 2 (middle) */}
        <div className="absolute inset-0 translate-x-1.5 translate-y-1.5 rounded-md bg-[#161b22] border border-white/5" />
        {/* Slide 1 (front) */}
        <div className="absolute inset-0 rounded-md bg-[#0d1117] border border-white/10 flex flex-col items-center justify-center gap-2 p-3">
          <span className="text-lg">💡</span>
          <div className="w-3/4 h-2 rounded bg-[#3ddc84]/30" />
          <div className="w-full h-1.5 rounded bg-white/10" />
          <div className="w-5/6 h-1.5 rounded bg-white/10" />
          <div className="flex gap-1 mt-1">
            <div className="h-1 w-1 rounded-full bg-[#3ddc84]" />
            <div className="h-1 w-1 rounded-full bg-white/20" />
            <div className="h-1 w-1 rounded-full bg-white/20" />
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Visual tab card ───────────────────────────────────────────────────────

function VisualCard({
  name,
  badge,
  desc,
  preview: Preview,
}: {
  name: string;
  badge?: string;
  desc: string;
  preview: () => React.ReactElement;
}) {
  return (
    <div className="flex flex-col rounded-lg border border-border bg-card overflow-hidden">
      {/* Mini preview */}
      <div className="bg-[#0d1117] p-3">
        <div className="w-24 mx-auto">
          <Preview />
        </div>
      </div>
      {/* Info */}
      <div className="flex flex-col gap-1 p-4">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-foreground">{name}</span>
          {badge && <Badge variant="secondary" className="text-[10px] px-1.5 py-0">{badge}</Badge>}
        </div>
        <p className="text-xs text-muted-foreground leading-relaxed">{desc}</p>
      </div>
    </div>
  );
}

// ── Main guide page ───────────────────────────────────────────────────────

export function GuidePage() {
  return (
    <div className="flex flex-col min-h-full">
      {/* Header */}
      <div className="border-b border-border px-8 py-6 bg-card/50">
        <div className="flex items-center gap-3">
          <BookOpen className="h-6 w-6 text-primary" />
          <div>
            <h1 className="text-xl font-bold text-foreground">Guide d'utilisation</h1>
            <p className="text-sm text-muted-foreground mt-0.5">
              Tout ce qu'il faut savoir pour créer et publier des posts avec Getpostcraft
            </p>
          </div>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 px-8 py-8">
        <div className="max-w-3xl mx-auto flex flex-col gap-10">

          {/* ── Workflow global ── */}
          <Section id="workflow" icon={Zap} title="Workflow en 5 étapes">
            <StepCard steps={[
              {
                icon: PenLine,
                title: "Écris un brief",
                desc: "Décris en quelques mots le sujet de ton post (ex : 'Script bash pour automatiser les backups Linux'). Minimum 10 caractères. Tu peux aussi coller une URL — l'app extrait automatiquement le texte de la page.",
              },
              {
                icon: Zap,
                title: "Génère le contenu AI",
                desc: "Clique 'Générer' pour que l'IA crée une légende optimisée + hashtags pertinents. Tu peux aussi générer 3 variantes (éducatif / casual / percutant) avec le bouton variantes.",
              },
              {
                icon: Image,
                title: "Crée le visuel",
                desc: "Dans la section Visuel 1080×1080, choisis un template (Post / Code / Terminal / Carrousel) et clique 'Générer' pour créer l'image. Le draft est auto-sauvegardé.",
              },
              {
                icon: Send,
                title: "Publie sur Instagram ou LinkedIn",
                desc: "Clique 'Publier' pour envoyer le post directement. Instagram nécessite une image, LinkedIn peut publier en texte seul ou avec image.",
              },
              {
                icon: LayoutDashboard,
                title: "Suis tes publications",
                desc: "Le Dashboard affiche l'historique complet. Chaque post publié montre la date et l'ID de publication retourné par la plateforme.",
              },
            ]} />
          </Section>

          {/* ── Composer ── */}
          <Section id="composer" icon={PenLine} title="Composer — Détail des options">

            <Info label="Brief">
              Zone de texte principale. Décris ton post en français ou en anglais. Plus le brief est précis, meilleure est la génération. Tu peux aussi coller une URL complète (ex : <code className="bg-secondary px-1 rounded text-xs">https://github.com/...</code>) — l'app scrape le contenu automatiquement.
            </Info>

            <Info label="Réseau (Instagram / LinkedIn)">
              Sélectionne la plateforme cible avant de générer. Le choix influence le style de la légende (Instagram = émojis + hashtags, LinkedIn = ton professionnel, max 5 hashtags). Change-le avant de cliquer Générer.
            </Info>

            <Info label="Caption">
              La légende générée par l'IA. Tu peux l'éditer directement. Le compteur <code className="bg-secondary px-1 rounded text-xs">393 / 2200</code> indique le nombre de caractères (limite Instagram : 2200). L'icône copier permet de copier la légende en un clic.
            </Info>

            <Info label="Hashtags">
              Badges cliquables. Clique <code className="bg-secondary px-1 rounded text-xs">×</code> pour supprimer un hashtag. Clique <code className="bg-secondary px-1 rounded text-xs">ajouter…</code> pour en ajouter un. LinkedIn utilise max 5 hashtags (au-delà, les suivants sont ignorés).
            </Info>

            <div className="flex flex-col gap-2">
              <span className="text-xs font-semibold uppercase tracking-widest text-primary">Templates visuels 1080×1080</span>
              <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                <VisualCard
                  name="Post"
                  preview={PreviewPost}
                  desc="Texte de ta légende sur fond sombre avec accent vert. Parfait pour les annonces, citations ou présentations de projet."
                />
                <VisualCard
                  name="Code"
                  preview={PreviewCode}
                  desc="Snippet de code avec syntax highlighting coloré. Renseigne le langage (bash, python, js…) et colle ton code."
                />
                <VisualCard
                  name="Terminal"
                  preview={PreviewTerminal}
                  desc="Fenêtre de terminal simulée style macOS/Linux. Tape tes commandes et leur output. Idéal pour DevOps et Linux."
                />
                <VisualCard
                  name="Carrousel"
                  badge="Multi-slides"
                  preview={PreviewCarousel}
                  desc="Plusieurs slides empilées. L'IA génère le contenu de chaque slide automatiquement depuis ton brief."
                />
              </div>
            </div>

            <Tip>
              Le visuel est optionnel pour LinkedIn (peut publier en texte seul). Pour Instagram, une image est obligatoire.
            </Tip>
          </Section>

          {/* ── Settings ── */}
          <Section id="settings" icon={Settings} title="Settings — Configuration">

            <Info label="Clés IA (Intelligence Artificielle)">
              <div className="flex flex-col gap-2">
                <p>Getpostcraft est <strong>BYOK</strong> (Bring Your Own Key) — tu utilises ta propre clé API, jamais partagée.</p>
                <div className="flex flex-col gap-1 mt-1">
                  <div className="flex items-center gap-2">
                    <ChevronRight className="h-3 w-3 text-primary shrink-0" />
                    <span><strong>OpenRouter</strong> (recommandé) : donne accès à Claude, GPT-4, Mistral, etc. Crée un compte sur <code className="bg-secondary px-1 rounded text-xs">openrouter.ai</code> → API Keys.</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <ChevronRight className="h-3 w-3 text-primary shrink-0" />
                    <span><strong>Anthropic</strong> : clé directe Claude. Crée un compte sur <code className="bg-secondary px-1 rounded text-xs">console.anthropic.com</code> → API Keys.</span>
                  </div>
                </div>
                <p className="text-muted-foreground text-xs mt-1">La clé est stockée localement sur ton appareil — elle ne quitte jamais ta machine.</p>
              </div>
            </Info>

            <Info label="Comptes — Instagram">
              <div className="flex flex-col gap-1.5">
                <div className="flex items-start gap-2">
                  <span className="shrink-0 font-semibold text-primary">1.</span>
                  <span>Crée une Meta App sur <code className="bg-secondary px-1 rounded text-xs">developers.facebook.com</code> → Mon App → Créer une app.</span>
                </div>
                <div className="flex items-start gap-2">
                  <span className="shrink-0 font-semibold text-primary">2.</span>
                  <span>Copie le <strong>Meta App ID</strong> (pas l'Instagram App ID) dans Settings → Comptes → Meta App ID.</span>
                </div>
                <div className="flex items-start gap-2">
                  <span className="shrink-0 font-semibold text-primary">3.</span>
                  <span>Copie l'<strong>App Secret</strong> (Paramètres de base → App Secret) dans Settings → Comptes → Meta App Secret.</span>
                </div>
                <div className="flex items-start gap-2">
                  <span className="shrink-0 font-semibold text-primary">4.</span>
                  <span>Ajoute ton compte Instagram comme <strong>testeur</strong> dans l'onglet Rôles de l'app Meta.</span>
                </div>
                <div className="flex items-start gap-2">
                  <span className="shrink-0 font-semibold text-primary">5.</span>
                  <span>Clique <strong>Connecter Instagram</strong> — une fenêtre de navigation s'ouvre. Autorise l'accès.</span>
                </div>
              </div>
            </Info>

            <Info label="Comptes — LinkedIn">
              <div className="flex flex-col gap-1.5">
                <div className="flex items-start gap-2">
                  <span className="shrink-0 font-semibold text-primary">1.</span>
                  <span>Crée une app LinkedIn sur <code className="bg-secondary px-1 rounded text-xs">developer.linkedin.com</code> → Create app.</span>
                </div>
                <div className="flex items-start gap-2">
                  <span className="shrink-0 font-semibold text-primary">2.</span>
                  <span>Ajoute les produits <strong>"Sign In with LinkedIn using OpenID Connect"</strong> et <strong>"Share on LinkedIn"</strong> dans l'onglet Products.</span>
                </div>
                <div className="flex items-start gap-2">
                  <span className="shrink-0 font-semibold text-primary">3.</span>
                  <span>Dans Auth → Redirect URLs, ajoute exactement : <code className="bg-secondary px-1 rounded text-xs">https://localhost:7892/callback</code></span>
                </div>
                <div className="flex items-start gap-2">
                  <span className="shrink-0 font-semibold text-primary">4.</span>
                  <span>Copie le <strong>Client ID</strong> et le <strong>Client Secret</strong> dans Settings → Comptes → LinkedIn.</span>
                </div>
                <div className="flex items-start gap-2">
                  <span className="shrink-0 font-semibold text-primary">5.</span>
                  <span>Clique <strong>Connecter LinkedIn</strong> et autorise dans le navigateur.</span>
                </div>
              </div>
            </Info>

            <Info label="Hébergement d'images (imgbb) — requis pour Instagram">
              Instagram nécessite une URL publique pour l'image. imgbb est un hébergeur gratuit. Crée un compte sur <code className="bg-secondary px-1 rounded text-xs">imgbb.com</code> → API → Add API key → colle la clé dans Settings → Comptes → Clé API imgbb.
            </Info>

            <Tip>LinkedIn n'a pas besoin d'imgbb — l'image est uploadée directement via l'API LinkedIn.</Tip>
          </Section>

          {/* ── Dashboard ── */}
          <Section id="dashboard" icon={LayoutDashboard} title="Dashboard — Suivi des publications">
            <Info label="Historique">
              Le Dashboard liste tous tes posts (publiés + drafts). Clique sur un post pour voir le détail : légende complète, hashtags, date de publication, statut. Depuis le détail, tu peux éditer un draft ou le supprimer.
            </Info>
            <Info label="Statuts">
              <div className="flex flex-col gap-1">
                <div className="flex items-center gap-2"><Badge variant="secondary" className="text-xs">draft</Badge><span>Post généré, non encore publié</span></div>
                <div className="flex items-center gap-2"><Badge className="text-xs bg-primary/20 text-primary border-0">published</Badge><span>Post publié avec succès sur la plateforme</span></div>
                <div className="flex items-center gap-2"><Badge variant="secondary" className="text-xs">scheduled</Badge><span>Post assigné à une date dans le Calendrier</span></div>
              </div>
            </Info>
          </Section>

          {/* ── Calendrier ── */}
          <Section id="calendar" icon={CalendarDays} title="Calendrier — Planification">
            <Info label="Comment utiliser le calendrier">
              Le calendrier affiche tes drafts et posts par date. Clique sur un post existant pour lui assigner une date de publication, l'éditer ou le supprimer. La publication automatique à l'heure planifiée n'est pas encore disponible — tu publies manuellement depuis le Composer quand le moment est venu.
            </Info>
            <Tip>Utilise le calendrier pour visualiser ta fréquence de publication et éviter les trous ou les doublons dans ta stratégie de contenu.</Tip>
          </Section>

          {/* ── Glossaire ── */}
          <Section id="glossaire" icon={Hash} title="Glossaire rapide">
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
              {[
                { term: "Brief", def: "Description courte du sujet de ton post (entrée pour l'IA)" },
                { term: "Caption", def: "Légende du post générée par l'IA, modifiable librement" },
                { term: "Draft", def: "Post généré mais non encore publié, sauvegardé localement" },
                { term: "BYOK", def: "Bring Your Own Key — tu fournis ta propre clé API IA" },
                { term: "PKCE", def: "Protocole OAuth sécurisé utilisé pour la connexion aux réseaux sociaux" },
                { term: "imgbb", def: "Service d'hébergement d'images requis pour publier sur Instagram" },
                { term: "Carrousel", def: "Format Instagram/LinkedIn avec plusieurs slides (images) dans un même post" },
                { term: "URN", def: "Identifiant LinkedIn de ton profil (ex: urn:li:person:xxx)" },
              ].map(({ term, def }) => (
                <div key={term} className="rounded-lg border border-border bg-card px-4 py-3">
                  <span className="text-sm font-medium text-primary">{term}</span>
                  <p className="text-xs text-muted-foreground mt-0.5 leading-relaxed">{def}</p>
                </div>
              ))}
            </div>
          </Section>

        </div>
      </div>
    </div>
  );
}
