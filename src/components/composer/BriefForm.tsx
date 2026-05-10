import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { Loader2, AlertCircle, Link2, FileText, Layers } from "lucide-react";
import { useState, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useComposerStore } from "@/stores/composer.store";
import {
  generateContent,
  generateVariants,
  generateAndSaveGroup,
  saveDraft,
  scrapeUrlForBrief,
} from "@/lib/tauri/composer";
import { listAccounts } from "@/lib/tauri/oauth";
import { NETWORK_META, FORMATS_BY_NETWORK, type Network } from "@/types/composer.types";
import { CostEstimateBanner } from "@/components/composer/CostEstimateBanner";

const briefSchema = z.object({
  brief: z
    .string()
    .min(10, "Minimum 10 caractères")
    .max(500, "Maximum 500 caractères"),
});

type BriefFormData = z.infer<typeof briefSchema>;

export function BriefForm() {
  const {
    brief,
    network,
    selectedNetworks,
    accountIds,
    imageFormat,
    isLoading,
    error,
    setBrief,
    toggleNetwork,
    setAccountIdFor,
    setImageFormat,
    setResult,
    setVariants,
    setGroupResult,
    setIsLoading,
    setError,
    setDraftId,
  } = useComposerStore();

  const { data: allAccounts = [] } = useQuery({
    queryKey: ["accounts"],
    queryFn: listAccounts,
  });

  // Auto-select an account when exactly one exists for a network the user
  // just ticked — same convenience the previous mono-network form had,
  // now applied independently per network so checking LinkedIn doesn't
  // wipe the Instagram pick.
  useEffect(() => {
    for (const net of selectedNetworks) {
      const matches = allAccounts.filter((a) => a.provider === net);
      const current = accountIds[net];
      if (matches.length === 1 && (current === undefined || current === null)) {
        setAccountIdFor(net, matches[0].id);
      } else if (
        current !== undefined &&
        current !== null &&
        !matches.some((a) => a.id === current)
      ) {
        // The selected account no longer matches the network (e.g. user
        // disconnected it in Settings while the form was open). Fall back
        // to the only remaining account, or NULL if there's none / many.
        setAccountIdFor(net, matches.length === 1 ? matches[0].id : null);
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedNetworks, allAccounts.length]);

  const [inputMode, setInputMode] = useState<"text" | "url">("text");
  const [urlValue, setUrlValue] = useState("");
  const [isScraping, setIsScraping] = useState(false);
  const [scrapeError, setScrapeError] = useState<string | null>(null);

  const {
    register,
    handleSubmit,
    watch,
    setValue,
    formState: { errors, isValid },
  } = useForm<BriefFormData>({
    resolver: zodResolver(briefSchema),
    defaultValues: { brief },
    mode: "onChange",
  });

  const briefValue = watch("brief");
  const selectedArray = Array.from(selectedNetworks) as Network[];
  const isMultiNetwork = selectedArray.length >= 2;

  const onSubmit = async (data: BriefFormData) => {
    setIsLoading(true);
    setError(null);
    setBrief(data.brief);
    try {
      if (isMultiNetwork) {
        // Multi-network path: one parallel call to the new Tauri command,
        // results land in `groupResult` and the preview switches to tabs.
        const result = await generateAndSaveGroup(
          data.brief,
          selectedArray.map((net) => ({
            network: net,
            account_id: accountIds[net] ?? null,
          })),
        );
        setGroupResult(result);
        // The first successful member becomes the active draft so the
        // existing publish/save shortcuts have something to point at —
        // the tabbed preview lets the user switch to siblings via UI.
        const firstOk = result.members.find((m) => m.status === "ok" && m.post_id !== null);
        setDraftId(firstOk?.post_id ?? null);
      } else {
        // Mono-network path: legacy command, no schema change. The new
        // `generateAndSaveGroup` could handle N=1 too, but keeping the
        // single-network flow on its dedicated command preserves the
        // historical UX (no transactional group wrapper for users who
        // never wanted multi-network in the first place).
        const primary = selectedArray[0];
        const acc = accountIds[primary] ?? null;
        const result = await generateContent(data.brief, primary, acc);
        setResult(result);
        saveDraft(primary, result.caption, result.hashtags, acc)
          .then(setDraftId)
          .catch(() => {});
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const onVariants = async (data: BriefFormData) => {
    // Variants ×3 stay mono-network — running 3 tones × 2 networks would be
    // 6 parallel AI calls, which both blows the cost banner and turns the
    // preview UI into a 6-tab grid that doesn't fit on a laptop screen.
    if (isMultiNetwork) {
      setError("Les variantes ×3 sont uniquement disponibles en mono-réseau.");
      return;
    }
    setIsLoading(true);
    setError(null);
    setBrief(data.brief);
    try {
      const primary = selectedArray[0];
      const acc = accountIds[primary] ?? null;
      const variants = await generateVariants(data.brief, primary, acc);
      setVariants(variants);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const handleScrape = async () => {
    const url = urlValue.trim();
    if (!url) return;
    setIsScraping(true);
    setScrapeError(null);
    try {
      const text = await scrapeUrlForBrief(url);
      const truncated = text.slice(0, 490);
      setValue("brief", truncated, { shouldValidate: true });
      setBrief(truncated);
      setInputMode("text");
    } catch (err) {
      setScrapeError(String(err));
    } finally {
      setIsScraping(false);
    }
  };

  return (
    <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-4">
      <div>
        <h1 className="text-xl font-semibold text-foreground">Nouveau post</h1>
        <p className="text-sm text-muted-foreground mt-0.5">
          Décris ton idée ou colle une URL, Claude génère le contenu.
        </p>
      </div>

      {/* Network multi-select — checkbox grid replaces the v0.3.8 dropdown.
          Pinned hint about the V1 ceiling lets users discover the cap before
          they try a fourth checkbox and get rejected by the form. */}
      <div className="flex flex-col gap-1.5">
        <div className="flex items-center justify-between gap-2">
          <label className="text-sm font-medium text-foreground">Réseaux</label>
          <span className="inline-flex items-center gap-1 text-[10px] text-muted-foreground">
            <Layers className="h-3 w-3" aria-hidden="true" />
            multi-réseau · max 3
          </span>
        </div>
        <div className="flex flex-col gap-1 rounded-md border border-border bg-card p-1.5">
          {(Object.keys(NETWORK_META) as Network[]).map((net) => {
            const meta = NETWORK_META[net];
            const checked = selectedNetworks.has(net);
            const matches = allAccounts.filter((a) => a.provider === net);
            const accId = accountIds[net] ?? null;
            const selectedAccount = matches.find((a) => a.id === accId);
            const disabledByCap = !checked && selectedNetworks.size >= 3;
            return (
              <div key={net} className="flex flex-col gap-1">
                <label
                  className={`flex items-center gap-2 rounded px-2 py-1.5 text-sm transition-colors ${
                    checked
                      ? "bg-primary/10 text-foreground"
                      : meta.v1
                        ? "text-foreground/90 hover:bg-secondary/50"
                        : "text-muted-foreground/60"
                  } ${(!meta.v1 || disabledByCap) ? "cursor-not-allowed" : "cursor-pointer"}`}
                >
                  <input
                    type="checkbox"
                    checked={checked}
                    disabled={!meta.v1 || disabledByCap}
                    onChange={() => toggleNetwork(net)}
                    className="h-4 w-4 accent-primary"
                    aria-label={`Activer ${meta.label}`}
                  />
                  <span className="flex-1 font-medium">{meta.label}</span>
                  {!meta.v1 && (
                    <span className="text-[10px] uppercase tracking-wider">
                      V2
                    </span>
                  )}
                </label>

                {/* Per-network account cascade — only visible when the
                    network is checked, indented under its own checkbox so
                    the visual association is unambiguous even with all
                    three networks expanded. */}
                {checked && (
                  <div className="ml-8 flex flex-col gap-1">
                    {matches.length === 0 ? (
                      <p className="text-[11px] text-muted-foreground">
                        Aucun compte {meta.label} connecté — génération sans Product Truth.
                      </p>
                    ) : (
                      <Select
                        value={accId !== null ? String(accId) : "none"}
                        onValueChange={(val) =>
                          setAccountIdFor(net, val === "none" ? null : Number(val))
                        }
                      >
                        <SelectTrigger className="h-8 text-xs">
                          <SelectValue placeholder="Choisir un compte…" />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="none">
                            <span className="text-muted-foreground">
                              Aucun (générique)
                            </span>
                          </SelectItem>
                          {matches.map((a) => (
                            <SelectItem key={a.id} value={String(a.id)}>
                              @{a.username}
                              {a.product_truth && (
                                <span className="ml-1.5 text-xs text-primary">
                                  ✓ Product Truth
                                </span>
                              )}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    )}
                    {selectedAccount && !selectedAccount.product_truth && (
                      <p className="text-[10px] text-orange-400/90 leading-snug">
                        ⚠ Pas de Product Truth pour @{selectedAccount.username} —
                        l'IA peut inventer des features.
                      </p>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* Cost estimate — appears as soon as at least one network is ticked,
          consumes the OpenRouter pricing snapshot the AI usage panel already
          uses, so a model change in Settings reflects here automatically. */}
      <CostEstimateBanner selectedNetworks={selectedArray} />

      {/* Image format selector — driven by the PRIMARY network's catalog.
          The format set is per-network on the renderer side, so showing the
          IG formats while LinkedIn is also ticked is fine: the renderer will
          adapt each sibling to that network's canvas. */}
      <div className="flex flex-col gap-1.5">
        <label className="text-sm font-medium text-foreground">Format image</label>
        <div className="flex flex-wrap gap-1.5">
          {FORMATS_BY_NETWORK[network].map((fmt) => (
            <button
              key={fmt.id}
              type="button"
              onClick={() => setImageFormat(fmt)}
              className={`flex flex-col items-center gap-0.5 px-3 py-1.5 rounded-md border text-xs font-medium transition-colors ${
                imageFormat.id === fmt.id
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border text-muted-foreground hover:text-foreground hover:border-border/80"
              }`}
            >
              <span>{fmt.label}</span>
              <span
                className={`text-[10px] ${
                  imageFormat.id === fmt.id
                    ? "text-primary/70"
                    : "text-muted-foreground/60"
                }`}
              >
                {fmt.width}×{fmt.height}
              </span>
            </button>
          ))}
        </div>
      </div>

      {/* Brief / URL toggle */}
      <div className="flex flex-col gap-1.5">
        <div className="flex items-center justify-between">
          <label className="text-sm font-medium text-foreground">Brief</label>
          <div className="flex gap-1 p-0.5 bg-secondary/50 rounded-md">
            <button
              type="button"
              onClick={() => {
                setInputMode("text");
                setScrapeError(null);
              }}
              className={`flex items-center gap-1 px-2 py-0.5 rounded text-xs transition-colors ${
                inputMode === "text"
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              <FileText className="h-3 w-3" />
              Texte
            </button>
            <button
              type="button"
              onClick={() => {
                setInputMode("url");
                setScrapeError(null);
              }}
              className={`flex items-center gap-1 px-2 py-0.5 rounded text-xs transition-colors ${
                inputMode === "url"
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              }`}
            >
              <Link2 className="h-3 w-3" />
              URL
            </button>
          </div>
        </div>

        {inputMode === "url" ? (
          <div className="flex flex-col gap-2">
            <div className="flex gap-2">
              <input
                value={urlValue}
                onChange={(e) => setUrlValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    handleScrape();
                  }
                }}
                placeholder="https://blog.example.com/article"
                className="flex-1 bg-secondary/50 border border-border rounded-md px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:border-primary"
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={handleScrape}
                disabled={isScraping || !urlValue.trim()}
                className="shrink-0"
              >
                {isScraping ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  "Extraire"
                )}
              </Button>
            </div>
            {scrapeError && (
              <p className="text-xs text-destructive">{scrapeError}</p>
            )}
            <p className="text-xs text-muted-foreground leading-snug">
              URL HTTP(S) publique uniquement (article de blog, README GitHub,
              page de doc, ta propre page produit déployée). Pas de chemin local{" "}
              <span className="font-mono">file://</span> ni de dossier de
              projet — utilise le mode <span className="font-mono">Texte</span>{" "}
              et colle ton README à la place.
            </p>
          </div>
        ) : (
          <>
            <Textarea
              {...register("brief")}
              placeholder="Décris ce que tu veux poster…"
              className="min-h-36 resize-none"
            />
            <div className="flex justify-between items-center">
              {errors.brief ? (
                <span className="text-xs text-destructive">
                  {errors.brief.message}
                </span>
              ) : (
                <span />
              )}
              <span
                className={`text-xs ${
                  (briefValue?.length ?? 0) > 450
                    ? "text-destructive"
                    : "text-muted-foreground"
                }`}
              >
                {briefValue?.length ?? 0} / 500
              </span>
            </div>
          </>
        )}
      </div>

      <div className="flex gap-2">
        <Button
          type="submit"
          disabled={!isValid || isLoading}
          className="flex-1"
        >
          {isLoading && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
          {isLoading
            ? "Génération…"
            : isMultiNetwork
              ? `Générer pour ${selectedArray.length} réseaux`
              : "Générer"}
        </Button>
        <Button
          type="button"
          variant="outline"
          disabled={!isValid || isLoading || isMultiNetwork}
          onClick={handleSubmit(onVariants)}
          className="shrink-0 text-xs"
          title={
            isMultiNetwork
              ? "Variantes ×3 disponibles uniquement en mono-réseau"
              : "Générer 3 variantes en parallèle (éducatif · casual · percutant)"
          }
        >
          {isLoading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : "×3"}
        </Button>
      </div>

      {error && (
        <Alert variant="destructive">
          <AlertCircle className="h-4 w-4" />
          <AlertDescription className="font-mono text-xs break-all">
            {error}
          </AlertDescription>
        </Alert>
      )}
    </form>
  );
}
