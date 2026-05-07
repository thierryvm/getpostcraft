import { useState } from "react";
import { useQueryClient, useMutation } from "@tanstack/react-query";
import { Loader2, Sparkles, X } from "lucide-react";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { updateAccountProductTruth, updateAccountBranding } from "@/lib/tauri/oauth";
import { analyzeUrlVisual, type WebsiteAnalysis } from "@/lib/tauri/composer";

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

  // URL-analysis flow state — surfaces a small inline form rather than a modal
  // because the analyzer is one-shot and the user wants to review in place.
  const [showUrlForm, setShowUrlForm] = useState(false);
  const [url, setUrl] = useState("");
  /** Preview holds BOTH the textual ProductTruth (editable) and the visual profile (swatches). */
  const [preview, setPreview] = useState<WebsiteAnalysis | null>(null);
  const [previewText, setPreviewText] = useState<string>("");
  const [analyzeError, setAnalyzeError] = useState<string | null>(null);
  const [colorsApplied, setColorsApplied] = useState(false);

  const save = useMutation({
    mutationFn: () => updateAccountProductTruth(accountId, value),
    onSuccess: () => {
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });

  const analyze = useMutation({
    mutationFn: () => analyzeUrlVisual(url.trim(), handle, accountId),
    onSuccess: (result) => {
      setPreview(result);
      setPreviewText(result.product_truth);
      setAnalyzeError(null);
      setColorsApplied(false);
    },
    onError: (e: unknown) => {
      setAnalyzeError(e instanceof Error ? e.message : String(e));
      setPreview(null);
    },
  });

  /** Persist the first 2 extracted colors as brand_color + accent_color on the account. */
  const applyColors = useMutation({
    mutationFn: () => {
      if (!preview) throw new Error("Aucune analyse en cours");
      const [brand, accent] = preview.visual_profile.colors;
      // If we got fewer than 2 colors, reuse the first as both — better than empty.
      return updateAccountBranding(
        accountId,
        brand ?? "",
        accent ?? brand ?? "",
      );
    },
    onSuccess: () => {
      setColorsApplied(true);
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });

  const isDirty = value !== (initialValue ?? "");

  const applyPreview = () => {
    if (!previewText) return;
    setValue(previewText);
    setSaved(false);
    setPreview(null);
    setPreviewText("");
    setShowUrlForm(false);
    setUrl("");
    setColorsApplied(false);
  };

  const cancelAnalyze = () => {
    setShowUrlForm(false);
    setUrl("");
    setPreview(null);
    setPreviewText("");
    setAnalyzeError(null);
    setColorsApplied(false);
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
            <div className="flex flex-col gap-3">
              {/* Visual profile — color swatches + typography hint */}
              {preview.visual_profile.colors.length > 0 && (
                <div className="flex flex-col gap-1.5">
                  <p className="text-xs font-medium text-foreground">
                    Identité visuelle extraite
                    <span className="ml-1 font-normal text-muted-foreground">
                      — {preview.visual_profile.typography.family} ·{" "}
                      {preview.visual_profile.mood.slice(0, 3).join(" · ")}
                    </span>
                  </p>
                  <div className="flex items-center gap-2">
                    {preview.visual_profile.colors.map((c, i) => (
                      <div
                        key={`${c}-${i}`}
                        className="flex flex-col items-center gap-0.5"
                        title={c}
                      >
                        <span
                          className="block h-8 w-8 rounded border border-border"
                          style={{ backgroundColor: c }}
                        />
                        <span className="text-[10px] font-mono text-muted-foreground">
                          {c}
                        </span>
                      </div>
                    ))}
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={
                        applyColors.isPending ||
                        colorsApplied ||
                        preview.visual_profile.colors.length === 0
                      }
                      onClick={() => applyColors.mutate()}
                      className="ml-2 h-8 text-xs"
                    >
                      {applyColors.isPending
                        ? "…"
                        : colorsApplied
                          ? "Couleurs appliquées ✓"
                          : "Appliquer les couleurs"}
                    </Button>
                  </div>
                </div>
              )}

              <p className="text-xs font-medium text-foreground">
                Aperçu Product Truth — relis avant d'appliquer :
              </p>
              <Textarea
                value={previewText}
                onChange={(e) => setPreviewText(e.target.value)}
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
