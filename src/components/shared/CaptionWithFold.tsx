/**
 * Renders a caption with a visual fold indicator when the text exceeds foldLimit.
 *
 * The fold is the point where "Voir plus" / "...more" appears in the feed:
 * - Instagram: 125 chars
 * - LinkedIn:  140 chars
 *
 * Text after the fold is dimmed to signal it won't be visible without user interaction.
 */
export function CaptionWithFold({
  text,
  foldLimit,
  network,
}: {
  text: string;
  foldLimit: number;
  network: string;
}) {
  if (!foldLimit || text.length <= foldLimit) {
    return (
      <p className="text-sm text-foreground whitespace-pre-wrap leading-relaxed">
        {text}
      </p>
    );
  }

  const before = text.slice(0, foldLimit);
  const after = text.slice(foldLimit);

  return (
    <div className="flex flex-col gap-0">
      <p className="text-sm text-foreground whitespace-pre-wrap leading-relaxed">{before}</p>
      <div className="flex items-center gap-2 my-1.5">
        <div className="flex-1 border-t border-dashed border-amber-500/40" />
        <span className="text-[10px] font-semibold text-amber-500/80 shrink-0 select-none px-1">
          — fold {network} {foldLimit} car. —
        </span>
        <div className="flex-1 border-t border-dashed border-amber-500/40" />
      </div>
      <p className="text-sm text-foreground whitespace-pre-wrap leading-relaxed">{after}</p>
    </div>
  );
}

/**
 * Inline fold counter for use below a textarea in edit mode.
 * Returns null when foldLimit is 0 or text is within the fold.
 */
export function FoldCounter({
  length,
  foldLimit,
}: {
  length: number;
  foldLimit: number;
}) {
  if (!foldLimit) return null;

  if (length <= foldLimit) {
    return (
      <p className="text-xs text-muted-foreground">
        {length} car. · fold à {foldLimit}
      </p>
    );
  }

  const overflow = length - foldLimit;
  return (
    <p className="text-xs text-amber-500">
      {length} car. · {overflow} car. après le fold ({foldLimit})
    </p>
  );
}
