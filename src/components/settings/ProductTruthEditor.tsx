import { useState } from "react";
import { useQueryClient, useMutation } from "@tanstack/react-query";
import { Loader2, Sparkles, X } from "lucide-react";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { updateAccountProductTruth } from "@/lib/tauri/oauth";
import { synthesizeProductTruthFromUrl } from "@/lib/tauri/composer";

export function ProductTruthEditor({
  accountId,
  initialValue,
  handle,
}: {
  accountId: number;
  initialValue: string | null;
  /** Account handle (without @) — passed to the synthesis prompt to label the block. */
  handle: string;
}) {
  const qc = useQueryClient();
  const [value, setValue] = useState(initialValue ?? "");
  const [saved, setSaved] = useState(false);

  // URL-analysis flow state. We surface a small inline form rather than a modal
  // because the synthesis is one-shot and the user wants to review the output
  // in place before deciding to apply it.
  const [showUrlForm, setShowUrlForm] = useState(false);
  const [url, setUrl] = useState("");
  const [preview, setPreview] = useState<string | null>(null);
  const [analyzeError, setAnalyzeError] = useState<string | null>(null);

  const save = useMutation({
    mutationFn: () => updateAccountProductTruth(accountId, value),
    onSuccess: () => {
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });

  const analyze = useMutation({
    mutationFn: () => synthesizeProductTruthFromUrl(url.trim(), handle),
    onSuccess: (synthesized) => {
      setPreview(synthesized);
      setAnalyzeError(null);
    },
    onError: (e: unknown) => {
      setAnalyzeError(e instanceof Error ? e.message : String(e));
      setPreview(null);
    },
  });

  const isDirty = value !== (initialValue ?? "");

  const applyPreview = () => {
    if (!preview) return;
    setValue(preview);
    setSaved(false);
    setPreview(null);
    setShowUrlForm(false);
    setUrl("");
  };

  const cancelAnalyze = () => {
    setShowUrlForm(false);
    setUrl("");
    setPreview(null);
    setAnalyzeError(null);
  };

  return (
    <div className="flex flex-col gap-2 mt-3 pt-3 border-t border-border">
      <div className="flex items-center justify-between gap-2">
        <Label className="text-xs text-muted-foreground">
          Product Truth
          <span className="ml-1 font-normal opacity-70">
            — contexte marque injecté dans le prompt IA
          </span>
        </Label>
        {!showUrlForm && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-7 gap-1.5 text-xs text-primary"
            onClick={() => setShowUrlForm(true)}
            title="Synthétise un Product Truth à partir de ton site web"
          >
            <Sparkles className="h-3 w-3" />
            Analyser depuis URL
          </Button>
        )}
      </div>

      {/* URL analysis flow — collapsible inline form */}
      {showUrlForm && (
        <div className="flex flex-col gap-2 rounded-md border border-primary/30 bg-primary/5 p-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-primary">
              Analyser un site web
            </span>
            <button
              type="button"
              onClick={cancelAnalyze}
              className="text-muted-foreground hover:text-foreground"
              aria-label="Annuler"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
          <p className="text-xs text-muted-foreground">
            Colle l'URL de ton site / projet. L'app rend la page (Playwright) puis
            l'IA synthétise un Product Truth structuré (chiffres, modules, voix).
            Prend ~15-30 secondes.
          </p>
          <div className="flex gap-2">
            <Input
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && url.trim() && !analyze.isPending) {
                  e.preventDefault();
                  analyze.mutate();
                }
              }}
              placeholder="https://exemple.com"
              className="font-mono text-xs"
              disabled={analyze.isPending}
            />
            <Button
              size="sm"
              variant="default"
              disabled={!url.trim() || analyze.isPending}
              onClick={() => analyze.mutate()}
              className="shrink-0 h-9 gap-1.5"
            >
              {analyze.isPending ? (
                <>
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  Analyse…
                </>
              ) : (
                <>
                  <Sparkles className="h-3.5 w-3.5" />
                  Analyser
                </>
              )}
            </Button>
          </div>
          {analyzeError && (
            <p className="text-xs text-destructive font-mono break-all">
              {analyzeError}
            </p>
          )}
          {preview && (
            <div className="flex flex-col gap-2">
              <p className="text-xs font-medium text-foreground">
                Aperçu — relis avant d'appliquer :
              </p>
              <Textarea
                value={preview}
                onChange={(e) => setPreview(e.target.value)}
                className="min-h-48 text-xs font-mono [field-sizing:content]"
              />
              <div className="flex gap-2">
                <Button
                  size="sm"
                  onClick={applyPreview}
                  className="h-7 text-xs"
                >
                  Appliquer dans Product Truth
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => analyze.mutate()}
                  disabled={analyze.isPending}
                  className="h-7 text-xs"
                >
                  Régénérer
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                Tu peux modifier le texte ci-dessus avant de l'appliquer.
              </p>
            </div>
          )}
        </div>
      )}

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
