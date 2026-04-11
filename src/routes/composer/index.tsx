import { useState, useEffect } from "react";
import { BriefForm } from "@/components/composer/BriefForm";
import { ContentPreview } from "@/components/composer/ContentPreview";
import { Badge } from "@/components/ui/badge";
import { getActiveProvider } from "@/lib/tauri/settings";
import { warmupSidecar } from "@/lib/tauri/composer";
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

  useEffect(() => {
    getActiveProvider().then(setProviderInfo).catch(console.error);
    warmupSidecar(); // fire-and-forget — pre-loads Python interpreter
  }, []);

  return (
    <div className="flex h-full flex-col">
      {/* Top bar */}
      <div className="flex items-center justify-between border-b border-border px-4 sm:px-6 py-2 shrink-0">
        <span className="text-sm font-medium text-foreground">Composer</span>
        <ProviderBadge info={providerInfo} />
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
