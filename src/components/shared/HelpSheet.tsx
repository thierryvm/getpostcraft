import { HelpCircle } from "lucide-react";
import { useRouterState } from "@tanstack/react-router";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { cn } from "@/lib/utils";

// ── Help content per route ────────────────────────────────────────────────

type HelpSection = { title: string; items: string[] };

type RouteHelp = {
  title: string;
  sections: HelpSection[];
};

const HELP_CONTENT: Record<string, RouteHelp> = {
  "/composer": {
    title: "Composer — Guide",
    sections: [
      {
        title: "Workflow en 4 étapes",
        items: [
          "1. Écris un brief (10–500 chars) ou colle une URL pour extraire le contenu",
          "2. Clique Générer → Claude crée une légende + hashtags",
          "3. Choisis un template visuel et clique Générer l'image",
          "4. Clique Publier — l'image est uploadée sur imgbb puis postée sur Instagram",
        ],
      },
      {
        title: "Astuces",
        items: [
          "Variants : génère plusieurs versions du même brief avec le bouton ⟳",
          "Carousel : active le mode multi-slides pour un post carrousel",
          "Draft auto-sauvegardé à chaque génération — tu peux quitter et revenir",
          "Le brief URL scrape le contenu d'une page web pour en faire un post",
        ],
      },
    ],
  },
  "/settings": {
    title: "Settings — Checklist setup",
    sections: [
      {
        title: "Configuration Instagram",
        items: [
          "1. Crée une Meta App sur developers.facebook.com",
          "2. Copie l'App ID dans Comptes → Meta App ID",
          "3. Copie l'App Secret dans Comptes → Meta App Secret",
          "4. Clique Connecter Instagram et autorise l'accès",
        ],
      },
      {
        title: "Configuration publication",
        items: [
          "Obtiens une clé API gratuite sur imgbb.com (requis pour publier)",
          "Colle la clé dans Publication → imgbb API Key",
          "Sans clé imgbb, le bouton Publier retournera une erreur",
        ],
      },
      {
        title: "Configuration IA",
        items: [
          "Ajoute ta clé Anthropic Claude dans IA → Claude API Key",
          "La clé est stockée localement, elle ne quitte jamais ton appareil",
        ],
      },
    ],
  },
  "/": {
    title: "Dashboard — Indicateurs",
    sections: [
      {
        title: "Métriques affichées",
        items: [
          "Posts publiés : total des publications réussies",
          "Drafts en cours : posts générés non encore publiés",
          "Tokens IA utilisés : consommation de ta clé Claude API",
          "Compte Instagram : statut de connexion OAuth",
        ],
      },
      {
        title: "Actions rapides",
        items: [
          "Clique sur un post dans l'historique pour voir son détail",
          "Le statut 'published' confirme la publication Instagram",
          "En cas d'erreur de connexion, va dans Settings → Comptes",
        ],
      },
    ],
  },
  "/calendar": {
    title: "Calendrier — Planification",
    sections: [
      {
        title: "Utilisation",
        items: [
          "Le calendrier affiche tes drafts et posts planifiés par date",
          "Clique sur un créneau pour y assigner un draft existant",
          "Les posts planifiés passent en statut 'scheduled' dans l'historique",
        ],
      },
      {
        title: "Publication différée",
        items: [
          "La publication automatique à l'heure prévue n'est pas encore disponible (V1.1)",
          "Pour l'instant : planifie la date, publie manuellement depuis le Composer",
        ],
      },
    ],
  },
};

const DEFAULT_HELP: RouteHelp = {
  title: "Aide",
  sections: [
    {
      title: "Navigation",
      items: [
        "Dashboard : vue d'ensemble de tes publications",
        "Composer : crée et publie un post Instagram",
        "Calendrier : planifie tes publications",
        "Settings : configure tes comptes et clés API",
      ],
    },
  ],
};

// ── Component ─────────────────────────────────────────────────────────────

export function HelpSheet({ collapsed }: { collapsed: boolean }) {
  const routerState = useRouterState();
  const pathname = routerState.location.pathname;
  const help = HELP_CONTENT[pathname] ?? DEFAULT_HELP;

  return (
    <Sheet>
      <SheetTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className={cn(
            "h-8 w-full text-muted-foreground hover:text-foreground",
            !collapsed && "justify-start gap-3 px-3"
          )}
          title={collapsed ? "Aide" : undefined}
          aria-label="Aide contextuelle"
        >
          <HelpCircle className="h-4 w-4 shrink-0" />
          {!collapsed && <span className="text-sm font-medium">Aide</span>}
        </Button>
      </SheetTrigger>

      <SheetContent side="right" className="flex flex-col overflow-hidden">
        {/* Header — padding right élargi pour laisser place au bouton ✕ */}
        <SheetHeader className="px-6 pt-6 pb-4 pr-12 border-b border-border shrink-0">
          <SheetTitle className="text-base font-semibold leading-snug">
            {help.title}
          </SheetTitle>
        </SheetHeader>

        {/* Scrollable content */}
        <div className="flex-1 overflow-y-auto px-6 py-6">
          <div className="flex flex-col gap-7">
            {help.sections.map((section) => (
              <div key={section.title}>
                <h3 className="mb-3 text-[11px] font-semibold uppercase tracking-widest text-primary">
                  {section.title}
                </h3>
                <ul className="flex flex-col gap-3">
                  {section.items.map((item, i) => (
                    <li key={i} className="flex gap-3 text-sm text-foreground/90 leading-relaxed">
                      <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-primary/50" />
                      <span>{item}</span>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </div>
      </SheetContent>
    </Sheet>
  );
}
