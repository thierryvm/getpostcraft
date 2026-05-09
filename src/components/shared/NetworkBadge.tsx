import { cn } from "@/lib/utils";
import { NETWORK_META, type Network } from "@/types/composer.types";

/** Single source of truth for per-network accent colors used in badges, dots,
 *  and calendar cells. Tailwind colors are kept on classnames so the design
 *  system stays grep-able from any component. */
export const NETWORK_COLORS: Record<string, string> = {
  instagram: "bg-pink-500/20 text-pink-300 border-pink-500/30",
  linkedin: "bg-blue-500/20 text-blue-300 border-blue-500/30",
  twitter: "bg-sky-500/20 text-sky-300 border-sky-500/30",
  tiktok: "bg-purple-500/20 text-purple-300 border-purple-500/30",
};

export const NETWORK_DOT_COLORS: Record<string, string> = {
  instagram: "bg-pink-500",
  linkedin: "bg-blue-500",
  twitter: "bg-sky-500",
  tiktok: "bg-purple-500",
};

interface NetworkBadgeProps {
  network: string;
  /** "pill" = full label in colored pill, "dot" = colored 8px dot only. */
  variant?: "pill" | "dot";
  className?: string;
}

export function NetworkBadge({ network, variant = "pill", className }: NetworkBadgeProps) {
  const label =
    NETWORK_META[network as Network]?.label ??
    network.charAt(0).toUpperCase() + network.slice(1);

  if (variant === "dot") {
    return (
      <span
        className={cn(
          "inline-block h-2 w-2 rounded-full shrink-0",
          NETWORK_DOT_COLORS[network] ?? "bg-muted-foreground",
          className,
        )}
        aria-label={label}
        title={label}
      />
    );
  }

  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium",
        NETWORK_COLORS[network] ?? "bg-secondary text-secondary-foreground",
        className,
      )}
    >
      {label}
    </span>
  );
}
