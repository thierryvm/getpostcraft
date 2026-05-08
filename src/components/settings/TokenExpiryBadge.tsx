import { Badge } from "@/components/ui/badge";
import { AlertTriangle } from "lucide-react";

interface TokenExpiryBadgeProps {
  /** ISO 8601 UTC timestamp from `account.token_expires_at`, or null/undefined. */
  expiresAt: string | null | undefined;
}

/**
 * Visual indicator of an OAuth token's remaining lifetime.
 *
 * Colour coding:
 *   - hidden       → expiry unknown (legacy account, provider doesn't expose it)
 *   - green        → > 14 days remaining
 *   - amber        → 7-14 days, time to plan a reconnect
 *   - red          → < 7 days OR already expired, must reconnect to publish
 *
 * The badge stays compact (one chip) by design — it lives next to a username
 * inside an account row, where space is precious. The actual "what to do"
 * guidance is implicit: reconnect when amber/red.
 */
export function TokenExpiryBadge({ expiresAt }: TokenExpiryBadgeProps) {
  if (!expiresAt) return null;

  const expiryDate = new Date(expiresAt);
  // Invalid timestamp guard — if the backend ever stored garbage we silently
  // hide the badge rather than render "NaN jours".
  if (Number.isNaN(expiryDate.getTime())) return null;

  const msUntilExpiry = expiryDate.getTime() - Date.now();
  const daysUntilExpiry = Math.floor(msUntilExpiry / (1000 * 60 * 60 * 24));

  if (daysUntilExpiry > 14) {
    return (
      <Badge
        variant="outline"
        className="text-[10px] px-1.5 py-0 border-emerald-500/30 text-emerald-400 bg-emerald-500/5"
      >
        Expire dans {daysUntilExpiry} j
      </Badge>
    );
  }

  if (daysUntilExpiry > 7) {
    return (
      <Badge
        variant="outline"
        className="text-[10px] px-1.5 py-0 border-amber-500/30 text-amber-400 bg-amber-500/10"
      >
        Expire dans {daysUntilExpiry} j
      </Badge>
    );
  }

  if (daysUntilExpiry >= 0) {
    return (
      <Badge
        variant="outline"
        className="text-[10px] px-1.5 py-0 border-red-500/30 text-red-400 bg-red-500/10 gap-1"
      >
        <AlertTriangle className="h-2.5 w-2.5" />
        Expire dans {daysUntilExpiry} j
      </Badge>
    );
  }

  // Already expired (negative days). Show absolute message — "reconnect now".
  return (
    <Badge
      variant="outline"
      className="text-[10px] px-1.5 py-0 border-red-500/40 text-red-400 bg-red-500/15 gap-1"
    >
      <AlertTriangle className="h-2.5 w-2.5" />
      Expiré — reconnecte
    </Badge>
  );
}
