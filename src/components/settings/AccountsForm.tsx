import { useQuery } from "@tanstack/react-query";
import { Badge } from "@/components/ui/badge";
import { listAccounts } from "@/lib/tauri/oauth";
import { InstagramSection } from "./InstagramSection";
import { ImgbbSection } from "./ImgbbSection";
import { LinkedInSection } from "./LinkedInSection";

export function AccountsForm() {
  const { data: accounts = [] } = useQuery({
    queryKey: ["accounts"],
    queryFn: listAccounts,
  });

  const instagramAccount = accounts.find((a) => a.provider === "instagram");
  const linkedInAccount = accounts.find((a) => a.provider === "linkedin");

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
