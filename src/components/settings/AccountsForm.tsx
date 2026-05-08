import { useQuery } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { listAccounts } from "@/lib/tauri/oauth";
import { InstagramSection } from "./InstagramSection";
import { ImgbbSection } from "./ImgbbSection";
import { LinkedInSection } from "./LinkedInSection";

/**
 * Settings → Comptes container.
 *
 * Surface the loading and error states explicitly. The previous version
 * destructured only `data` and rendered the per-network sections with
 * `account={undefined}` while the IPC call was still in flight — visually
 * indistinguishable from the genuine "not connected" state, so the user
 * could click "Connecter" on a connected account because the UI hadn't
 * yet hydrated.
 */
export function AccountsForm() {
  const { data: accounts, isLoading, error, refetch } = useQuery({
    queryKey: ["accounts"],
    queryFn: listAccounts,
  });

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground py-4">
        <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />
        Chargement des comptes connectés…
      </div>
    );
  }

  if (error) {
    return (
      <div className="space-y-2">
        <p className="text-sm text-destructive bg-destructive/10 rounded p-2">
          Impossible de charger les comptes : {String(error)}
        </p>
        <button
          type="button"
          onClick={() => refetch()}
          className="text-xs text-muted-foreground hover:text-foreground underline"
        >
          Réessayer
        </button>
      </div>
    );
  }

  const list = accounts ?? [];
  const instagramAccount = list.find((a) => a.provider === "instagram");
  const linkedInAccount = list.find((a) => a.provider === "linkedin");

  return (
    <div className="flex flex-col gap-6">
      <InstagramSection account={instagramAccount} />
      <ImgbbSection />
      <LinkedInSection account={linkedInAccount} />

      {/* Future networks */}
      <div className="flex flex-col gap-2 opacity-40 pointer-events-none">
        <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          V3 — Prochainement
        </p>
        <div className="flex gap-2">
          {["Twitter/X", "TikTok"].map((n) => (
            <Badge key={n} variant="outline" className="text-xs">{n}</Badge>
          ))}
        </div>
      </div>
    </div>
  );
}
