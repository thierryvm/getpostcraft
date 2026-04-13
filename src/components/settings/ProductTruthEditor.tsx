import { useState } from "react";
import { useQueryClient, useMutation } from "@tanstack/react-query";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { updateAccountProductTruth } from "@/lib/tauri/oauth";

export function ProductTruthEditor({
  accountId,
  initialValue,
}: {
  accountId: number;
  initialValue: string | null;
}) {
  const qc = useQueryClient();
  const [value, setValue] = useState(initialValue ?? "");
  const [saved, setSaved] = useState(false);

  const save = useMutation({
    mutationFn: () => updateAccountProductTruth(accountId, value),
    onSuccess: () => {
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });

  const isDirty = value !== (initialValue ?? "");

  return (
    <div className="flex flex-col gap-2 mt-3 pt-3 border-t border-border">
      <Label className="text-xs text-muted-foreground">
        Product Truth
        <span className="ml-1 font-normal opacity-70">
          — contexte marque injecté dans le prompt IA
        </span>
      </Label>
      <Textarea
        value={value}
        onChange={(e) => { setValue(e.target.value); setSaved(false); }}
        placeholder={
          "Ex :\n" +
          "Compte @[username] — niche [domaine], communauté [cible].\n" +
          "Produits réels : [liste ce qui existe].\n" +
          "Ne pas mentionner : [liste ce qui n'existe pas encore]."
        }
        className="min-h-28 resize-y text-xs font-mono [field-sizing:content]"
      />
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          disabled={!isDirty || save.isPending}
          onClick={() => save.mutate()}
          className="w-fit"
        >
          {save.isPending ? "…" : saved ? "Enregistré ✓" : "Enregistrer"}
        </Button>
        {isDirty && (
          <span className="text-xs text-muted-foreground">Modifications non sauvegardées</span>
        )}
      </div>
    </div>
  );
}
