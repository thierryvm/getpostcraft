import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { Loader2, AlertCircle, Link2, FileText } from "lucide-react";
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
import { generateContent, generateVariants, saveDraft, scrapeUrlForBrief } from "@/lib/tauri/composer";
import { listAccounts } from "@/lib/tauri/oauth";
import { NETWORK_META, FORMATS_BY_NETWORK, type Network } from "@/types/composer.types";

const briefSchema = z.object({
  brief: z
    .string()
    .min(10, "Minimum 10 caractères")
    .max(500, "Maximum 500 caractères"),
  network: z.enum(["instagram", "linkedin", "twitter", "tiktok"]),
});

type BriefFormData = z.infer<typeof briefSchema>;

export function BriefForm() {
  const {
    brief, network, accountId, imageFormat, isLoading, error,
    setBrief, setNetwork, setAccountId, setImageFormat,
    setResult, setVariants, setIsLoading, setError, setDraftId,
  } = useComposerStore();

  const { data: allAccounts = [] } = useQuery({
    queryKey: ["accounts"],
    queryFn: listAccounts,
  });

  // Accounts that match the currently selected network
  const networkAccounts = allAccounts.filter((a) => a.provider === network);

  // Auto-select when there's exactly one account for the network
  useEffect(() => {
    if (networkAccounts.length === 1 && accountId === null) {
      setAccountId(networkAccounts[0].id);
    }
    // Clear selection if the current account doesn't belong to this network
    if (
      accountId !== null &&
      !networkAccounts.some((a) => a.id === accountId)
    ) {
      setAccountId(networkAccounts.length === 1 ? networkAccounts[0].id : null);
    }
  }, [network, networkAccounts.length]); // eslint-disable-line react-hooks/exhaustive-deps

  const [inputMode, setInputMode] = useState<"text" | "url">("text");
  const [urlValue, setUrlValue] = useState("");
  const [isScraping, setIsScraping] = useState(false);
  const [scrapeError, setScrapeError] = useState<string | null>(null);

  const {
    register,
    handleSubmit,
    watch,
    control,
    setValue,
    formState: { errors, isValid },
  } = useForm<BriefFormData>({
    resolver: zodResolver(briefSchema),
    defaultValues: { brief, network },
    mode: "onChange",
  });

  const briefValue = watch("brief");

  const onSubmit = async (data: BriefFormData) => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await generateContent(data.brief, data.network as Network, accountId);
      setResult(result);
      setBrief(data.brief);
      setNetwork(data.network as Network);
      saveDraft(data.network as Network, result.caption, result.hashtags, accountId)
        .then(setDraftId)
        .catch(() => {});
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const onVariants = async (data: BriefFormData) => {
    setIsLoading(true);
    setError(null);
    try {
      const variants = await generateVariants(data.brief, data.network as Network, accountId);
      setBrief(data.brief);
      setNetwork(data.network as Network);
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
      // Truncate to 500 chars for the brief field
      const truncated = text.slice(0, 490);
      setValue("brief", truncated, { shouldValidate: true });
      setBrief(truncated);
      setInputMode("text"); // switch to text mode so user can review/edit
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

      {/* Network + Format selectors */}
      <div className="flex flex-col gap-1.5">
        <label className="text-sm font-medium text-foreground">Réseau</label>
        <Controller
          name="network"
          control={control}
          render={({ field }) => (
            <Select
              value={field.value}
              onValueChange={(val) => {
                field.onChange(val);
                setNetwork(val as Network);
              }}
            >
              <SelectTrigger className="w-48">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {(Object.keys(NETWORK_META) as Network[]).map((net) => (
                  <SelectItem
                    key={net}
                    value={net}
                    disabled={!NETWORK_META[net].v1}
                  >
                    {NETWORK_META[net].label}
                    {!NETWORK_META[net].v1 && " (V2)"}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          )}
        />
      </div>

      {/* Account selector */}
      <div className="flex flex-col gap-1.5">
        <label className="text-sm font-medium text-foreground">Compte</label>
        {networkAccounts.length === 0 ? (
          <p className="text-xs text-muted-foreground">
            Aucun compte {network} connecté — génération sans Product Truth.
          </p>
        ) : (
          <>
            <Select
              value={accountId !== null ? String(accountId) : "none"}
              onValueChange={(val) =>
                setAccountId(val === "none" ? null : Number(val))
              }
            >
              <SelectTrigger className="w-64">
                <SelectValue placeholder="Choisir un compte…" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="none">
                  <span className="text-muted-foreground">Aucun (générique)</span>
                </SelectItem>
                {networkAccounts.map((a) => (
                  <SelectItem key={a.id} value={String(a.id)}>
                    @{a.username}
                    {a.product_truth && (
                      <span className="ml-1.5 text-xs text-primary">✓ Product Truth</span>
                    )}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {/* Clarify the role of ProductTruth — the user shouldn't feel
                they have to also paste their site URL in the brief if the
                ProductTruth is already filled. */}
            {(() => {
              const selected = networkAccounts.find((a) => a.id === accountId);
              if (selected?.product_truth) {
                return (
                  <p className="text-[11px] text-muted-foreground leading-snug">
                    ✓ Product Truth chargé pour <span className="font-mono">@{selected.username}</span>{" "}
                    — le brief n'a besoin que du{" "}
                    <span className="text-foreground">sujet précis</span> de ce
                    post (l'IA connaît déjà ta marque, ton stack, tes chiffres).
                  </p>
                );
              }
              if (selected && !selected.product_truth) {
                return (
                  <p className="text-[11px] text-orange-400/90 leading-snug">
                    ⚠ Pas de Product Truth pour <span className="font-mono">@{selected.username}</span>.
                    Génération générique — l'IA peut inventer des features.
                    Configure-le dans Paramètres → Comptes.
                  </p>
                );
              }
              return null;
            })()}
          </>
        )}
      </div>

      {/* Image format selector */}
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
              <span className={`text-[10px] ${imageFormat.id === fmt.id ? "text-primary/70" : "text-muted-foreground/60"}`}>
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
              onClick={() => { setInputMode("text"); setScrapeError(null); }}
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
              onClick={() => { setInputMode("url"); setScrapeError(null); }}
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
                onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); handleScrape(); } }}
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
                {isScraping ? <Loader2 className="h-4 w-4 animate-spin" /> : "Extraire"}
              </Button>
            </div>
            {scrapeError && (
              <p className="text-xs text-destructive">{scrapeError}</p>
            )}
            <p className="text-xs text-muted-foreground leading-snug">
              URL HTTP(S) publique uniquement (article de blog, README GitHub,
              page de doc, ta propre page produit déployée). Pas de chemin
              local <span className="font-mono">file://</span> ni de dossier
              de projet — utilise le mode <span className="font-mono">Texte</span>{" "}
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
                <span className="text-xs text-destructive">{errors.brief.message}</span>
              ) : (
                <span />
              )}
              <span
                className={`text-xs ${(briefValue?.length ?? 0) > 450 ? "text-destructive" : "text-muted-foreground"}`}
              >
                {briefValue?.length ?? 0} / 500
              </span>
            </div>
          </>
        )}
      </div>

      <div className="flex gap-2">
        <Button type="submit" disabled={!isValid || isLoading} className="flex-1">
          {isLoading && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
          {isLoading ? "Génération…" : "Générer"}
        </Button>
        <Button
          type="button"
          variant="outline"
          disabled={!isValid || isLoading}
          onClick={handleSubmit(onVariants)}
          className="shrink-0 text-xs"
          title="Générer 3 variantes en parallèle (éducatif · casual · percutant)"
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
