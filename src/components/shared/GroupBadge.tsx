import { Layers } from "lucide-react";

/**
 * Compact badge surfaced next to a post row whenever it belongs to a
 * multi-network group (`post.group_id !== null`). Lets the user see at
 * a glance which drafts were generated together by the multi-network
 * composer — the alternative was to reveal the relation only after
 * opening the detail modal, which made siblings feel like accidental
 * duplicates in the history list.
 *
 * Click is a no-op for now: V1 just shows the badge. Linking to a
 * dedicated group view (filtered list of all members) is a V2
 * follow-up; it requires a new route + the as-yet-unconsumed
 * `db::groups::get_with_members` Tauri command.
 */
interface GroupBadgeProps {
  groupId: number | null;
  /**
   * Visual weight. `inline` is the compact pill used in dashboard /
   * calendar lists; `chip` is the slightly larger variant used in the
   * detail modal header where the badge has more room.
   */
  variant?: "inline" | "chip";
}

export function GroupBadge({ groupId, variant = "inline" }: GroupBadgeProps) {
  if (groupId === null) return null;
  const isInline = variant === "inline";
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-md border border-primary/30 bg-primary/10 text-primary ${
        isInline
          ? "px-1.5 py-0 text-[10px] font-mono"
          : "px-2 py-0.5 text-xs font-medium"
      }`}
      title={`Brouillon issu d'un groupe multi-réseau (#${groupId})`}
      aria-label={`Membre du groupe #${groupId}`}
    >
      <Layers
        className={isInline ? "h-2.5 w-2.5" : "h-3 w-3"}
        aria-hidden="true"
      />
      Groupe #{groupId}
    </span>
  );
}
