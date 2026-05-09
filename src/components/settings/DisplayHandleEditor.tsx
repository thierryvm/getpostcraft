import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { updateAccountDisplayHandle } from "@/lib/tauri/oauth";

/**
 * Lets the user override the brand handle that appears on rendered visuals
 * (the `>_ @handle` stamp). LinkedIn's OAuth fills `username` with the
 * owner's full personal name, so the default rendered "@Thierry Vanmeeteren"
 * is wrong for a brand account — this input fixes that without affecting
 * Instagram accounts whose username is already a handle.
 *
 * Empty input clears the override; the renderer then falls back to
 * `username`. The leading `@` is stripped server-side so users can type
 * either form.
 */
export function DisplayHandleEditor({
  accountId,
  fallbackUsername,
  initialValue,
}: {
  accountId: number;
  /** The platform-supplied username; shown as placeholder when no override is set. */
  fallbackUsername: string;
  initialValue: string | null;
}) {
  const qc = useQueryClient();
  const [value, setValue] = useState(initialValue ?? "");
  const [saved, setSaved] = useState(false);

  const save = useMutation({
    mutationFn: () => updateAccountDisplayHandle(accountId, value),
    onSuccess: () => {
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });

  const isDirty = value !== (initialValue ?? "");
  const previewHandle = value.trim().replace(/^@+/, "") || fallbackUsername;

  return (
    <div className="flex flex-col gap-2 mt-3 pt-3 border-t border-border">
      <Label className="text-xs text-muted-foreground">
        Handle affiché sur les visuels
        <span className="ml-1 font-normal opacity-70">
          — apparaît dans le stamp <span className="font-mono">{`>_ @handle`}</span> en bas
          des images. Vide = utilise <span className="font-mono">@{fallbackUsername}</span>.
        </span>
      </Label>
      <div className="flex items-center gap-2">
        <span className="text-sm text-muted-foreground select-none">@</span>
        <Input
          value={value}
          onChange={(e) => {
            setValue(e.target.value);
            setSaved(false);
          }}
          placeholder={fallbackUsername}
          className="font-mono text-xs h-8 w-56"
        />
        <Button
          size="sm"
          variant="outline"
          disabled={!isDirty || save.isPending}
          onClick={() => save.mutate()}
          className="w-fit"
        >
          {save.isPending ? "…" : saved ? "Enregistré ✓" : "Enregistrer"}
        </Button>
      </div>
      <p className="text-[11px] text-muted-foreground">
        Aperçu : <span className="font-mono text-foreground/80">{`>_ @${previewHandle}`}</span>
      </p>
    </div>
  );
}
