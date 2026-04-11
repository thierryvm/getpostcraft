import { useState, useEffect } from "react";
import { BriefForm } from "@/components/composer/BriefForm";
import { ContentPreview } from "@/components/composer/ContentPreview";
import { Badge } from "@/components/ui/badge";
import { getActiveProvider } from "@/lib/tauri/settings";
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
  }, []);

  return (
    <div className="flex h-full flex-col">
      {/* Top bar */}
      <div className="flex items-center justify-between border-b border-border px-6 py-2">
        <span className="text-sm font-medium text-foreground">Composer</span>
        <ProviderBadge info={providerInfo} />
      </div>

      {/* Content */}
      <div className="flex flex-1 flex-col md:flex-row gap-6 overflow-auto p-6">
        <div className="w-full md:w-80 md:shrink-0">
          <BriefForm />
        </div>
        <div className="flex-1 min-w-0">
          <ContentPreview />
        </div>
      </div>
    </div>
  );
}
