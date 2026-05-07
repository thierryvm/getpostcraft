import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { updateAccountBranding } from "@/lib/tauri/oauth";

const DEFAULT_BRAND = "#3ddc84";
const DEFAULT_ACCENT = "#3ddc84";

const HEX_RE = /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/;

function isValidHex(value: string): boolean {
  return value === "" || HEX_RE.test(value);
}

function ColorRow({
  label,
  value,
  onChange,
  fallback,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  fallback: string;
}) {
  // Native <input type="color"> requires #rrggbb (no shortcuts) — pad if needed.
  const swatch =
    value && HEX_RE.test(value)
      ? value.length === 4
        ? "#" + value.slice(1).split("").map((c) => c + c).join("")
        : value
      : fallback;
  const valid = isValidHex(value);
  return (
    <div className="flex items-center gap-2">
      <Label className="text-xs text-muted-foreground w-20 shrink-0">{label}</Label>
      <input
        type="color"
        value={swatch}
        onChange={(e) => onChange(e.target.value)}
        className="h-8 w-10 cursor-pointer rounded border border-border bg-transparent p-0"
        aria-label={`${label} swatch`}
      />
      <Input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={fallback}
        className={`font-mono text-xs h-8 w-32 ${valid ? "" : "border-destructive"}`}
        aria-invalid={!valid}
      />
      {!valid && (
        <span className="text-xs text-destructive">Format #rgb ou #rrggbb</span>
      )}
    </div>
  );
}

export function BrandColorsEditor({
  accountId,
  initialBrand,
  initialAccent,
}: {
  accountId: number;
  initialBrand: string | null;
  initialAccent: string | null;
}) {
  const qc = useQueryClient();
  const [brand, setBrand] = useState(initialBrand ?? "");
  const [accent, setAccent] = useState(initialAccent ?? "");
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = useMutation({
    mutationFn: () => updateAccountBranding(accountId, brand, accent),
    onSuccess: () => {
      setSaved(true);
      setError(null);
      setTimeout(() => setSaved(false), 2000);
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
    onError: (e: unknown) => {
      setError(e instanceof Error ? e.message : String(e));
    },
  });

  const isDirty = brand !== (initialBrand ?? "") || accent !== (initialAccent ?? "");
  const allValid = isValidHex(brand) && isValidHex(accent);

  return (
    <div className="flex flex-col gap-2 mt-3 pt-3 border-t border-border">
      <Label className="text-xs text-muted-foreground">
        Couleurs de marque
        <span className="ml-1 font-normal opacity-70">
          — appliquées aux visuels générés ; vide = défaut #3ddc84
        </span>
      </Label>
      <div className="flex flex-col gap-2">
        <ColorRow label="Brand" value={brand} onChange={(v) => { setBrand(v); setSaved(false); }} fallback={DEFAULT_BRAND} />
        <ColorRow label="Accent" value={accent} onChange={(v) => { setAccent(v); setSaved(false); }} fallback={DEFAULT_ACCENT} />
      </div>
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          disabled={!isDirty || !allValid || save.isPending}
          onClick={() => save.mutate()}
          className="w-fit"
        >
          {save.isPending ? "…" : saved ? "Enregistré ✓" : "Enregistrer"}
        </Button>
        {isDirty && allValid && (
          <span className="text-xs text-muted-foreground">Modifications non sauvegardées</span>
        )}
      </div>
      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}
