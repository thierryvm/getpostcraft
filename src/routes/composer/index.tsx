import { useState, useEffect } from "react";
import { FileText, Plus, X } from "lucide-react";
import { BriefForm } from "@/components/composer/BriefForm";
import { ContentPreview } from "@/components/composer/ContentPreview";
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
  const [providerInfo, setProviderInfo] = useState<ProviderInfo | null>(null);
  const draftId = useComposerStore((s) => s.draftId);
  const result = useComposerStore((s) => s.result);
  const resetForNewPost = useComposerStore((s) => s.resetForNewPost);

  useEffect(() => {
    getActiveProvider().then(setProviderInfo).catch(console.error);
    warmupSidecar(); // fire-and-forget — pre-loads Python interpreter
  }, []);

  // We're "in a draft session" if we have either a saved draft or a generated
  // result. A "Nouveau post" button shows up so the user can break out without
  // being forced through publish.
  const isDraftLoaded = draftId !== null || result !== null;

  return (
    <div className="flex h-full flex-col">
      {/* Top bar */}
      <div className="flex items-center justify-between border-b border-border px-4 sm:px-6 py-2 shrink-0 gap-3">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-sm font-medium text-foreground">Composer</span>
          {isDraftLoaded && draftId !== null && (
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
          {/* Preview panel — grows to fill on desktop, natural height on mobile */}
          <div className="flex-1 min-w-0">
            <ContentPreview />
          </div>
        </div>
      </div>
    </div>
  );
}
