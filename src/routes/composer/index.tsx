import { useState, useEffect } from "react";
import { FileText, Plus, X, ArrowLeft, Layers } from "lucide-react";
import { useNavigate } from "@tanstack/react-router";
import { BriefForm } from "@/components/composer/BriefForm";
import { ContentPreview } from "@/components/composer/ContentPreview";
import { GroupResultPanel } from "@/components/composer/GroupResultPanel";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { getActiveProvider } from "@/lib/tauri/settings";
import { warmupSidecar } from "@/lib/tauri/composer";
import { useComposerStore } from "@/stores/composer.store";
import type { ProviderInfo } from "@/types/settings.types";

function ProviderBadge({ info }: { info: ProviderInfo | null }) {
  if (!info) return null;
  const shortModel = info.model.split("/").pop() ?? info.model;
  return (
    <Badge
      variant="secondary"
      className="text-xs font-mono text-muted-foreground"
    >
      {info.provider} · {shortModel}
    </Badge>
  );
}

export function ComposerPage() {
  const navigate = useNavigate();
  const [providerInfo, setProviderInfo] = useState<ProviderInfo | null>(null);
  const draftId = useComposerStore((s) => s.draftId);
  const result = useComposerStore((s) => s.result);
  const groupResult = useComposerStore((s) => s.groupResult);
  const resetForNewPost = useComposerStore((s) => s.resetForNewPost);

  useEffect(() => {
    getActiveProvider().then(setProviderInfo).catch(console.error);
    warmupSidecar(); // fire-and-forget — pre-loads Python interpreter
  }, []);

  // We're "in a draft session" if we have a saved draft, a single-network
  // generation result, or a multi-network group. The "Nouveau post" button
  // shows up so the user can break out without being forced through publish.
  const isDraftLoaded = draftId !== null || result !== null || groupResult !== null;

  /** Reset state AND leave the composer page entirely. The chip × and
   *  "Nouveau post" buttons reset in place; this one closes the workspace
   *  for users who want "fermer" semantics rather than "fresh start". */
  const closeAndExit = () => {
    resetForNewPost();
    navigate({ to: "/" });
  };

  return (
    <div className="flex h-full flex-col">
      {/* Top bar */}
      <div className="flex items-center justify-between border-b border-border px-4 sm:px-6 py-2 shrink-0 gap-3">
        <div className="flex items-center gap-2 min-w-0">
          <Button
            variant="ghost"
            size="sm"
            className="h-7 text-xs gap-1 -ml-2 text-muted-foreground hover:text-foreground"
            onClick={closeAndExit}
            title="Retour au tableau de bord (le brouillon reste sauvegardé)"
          >
            <ArrowLeft className="h-3 w-3" />
            Retour
          </Button>
          <span className="text-sm font-medium text-foreground">Composer</span>
          {isDraftLoaded && draftId !== null && groupResult === null && (
            <span className="inline-flex items-center gap-1 rounded-md bg-primary/10 border border-primary/30 px-2 py-0.5 text-[11px] font-mono text-primary">
              <FileText className="h-3 w-3" aria-hidden="true" />
              Brouillon #{draftId}
              <button
                type="button"
                onClick={resetForNewPost}
                title="Quitter le brouillon (il reste sauvegardé)"
                aria-label="Quitter le brouillon"
                className="ml-0.5 text-primary/70 hover:text-primary"
              >
                <X className="h-3 w-3" />
              </button>
            </span>
          )}
          {groupResult !== null && groupResult.group_id !== null && (
            <span className="inline-flex items-center gap-1 rounded-md bg-primary/10 border border-primary/30 px-2 py-0.5 text-[11px] font-mono text-primary">
              <Layers className="h-3 w-3" aria-hidden="true" />
              Groupe #{groupResult.group_id}
              <button
                type="button"
                onClick={resetForNewPost}
                title="Quitter le groupe (les brouillons restent sauvegardés)"
                aria-label="Quitter le groupe"
                className="ml-0.5 text-primary/70 hover:text-primary"
              >
                <X className="h-3 w-3" />
              </button>
            </span>
          )}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {isDraftLoaded && (
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-xs gap-1"
              onClick={resetForNewPost}
              title="Garder le brouillon en l'état et démarrer un nouveau post"
            >
              <Plus className="h-3 w-3" />
              Nouveau post
            </Button>
          )}
          <ProviderBadge info={providerInfo} />
        </div>
      </div>

      {/* Scrollable content — natural flow, mobile-first */}
      <div className="flex-1 overflow-y-auto">
        <div className="flex flex-col lg:flex-row gap-6 p-4 sm:p-6">
          {/* Brief panel — full width mobile, fixed sidebar desktop */}
          <div className="w-full lg:w-80 lg:shrink-0">
            <BriefForm />
          </div>
          {/* Preview panel — grows to fill on desktop, natural height on mobile.
              Routes to the GroupResultPanel when the user just ran a multi-
              network generation (the lighter summary view), and to the rich
              ContentPreview otherwise (single-network: image render, edit,
              publish flow). The two are mutually exclusive in the store. */}
          <div className="flex-1 min-w-0">
            {groupResult !== null ? <GroupResultPanel /> : <ContentPreview />}
          </div>
        </div>
      </div>
    </div>
  );
}
