import { RefreshCw, Copy, Check, X, Plus, ImageDown, Loader2, ChevronLeft, ChevronRight, Download, Layers, Pencil } from "lucide-react";
import { useState, useRef, useEffect } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useComposerStore } from "@/stores/composer.store";
import { generateContent, saveDraft, generateCarousel } from "@/lib/tauri/composer";
import type { CaptionVariant, CarouselSlide } from "@/lib/tauri/composer";
import { renderPostImage, renderCodeImage, renderTerminalImage, renderCarouselSlides, exportCarouselZip } from "@/lib/tauri/media";
import { publishPost, publishLinkedinPost, updateDraftImage } from "@/lib/tauri/publisher";
import { updatePostDraft } from "@/lib/tauri/calendar";
import { NETWORK_META } from "@/types/composer.types";
import { CaptionWithFold } from "@/components/shared/CaptionWithFold";

function CopyButton({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon" className="h-7 w-7" onClick={handleCopy}>
          {copied ? (
            <Check className="h-3.5 w-3.5 text-primary" />
          ) : (
            <Copy className="h-3.5 w-3.5 text-muted-foreground" />
          )}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{copied ? "Copié !" : `Copier ${label}`}</TooltipContent>
    </Tooltip>
  );
}

function EditableHashtags({
  hashtags,
  onChange,
  maxHashtags,
}: {
  hashtags: string[];
  onChange: (tags: string[]) => void;
  maxHashtags?: number;
}) {
  const [newTag, setNewTag] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const isAtLimit = maxHashtags !== undefined && hashtags.length >= maxHashtags;

  const remove = (tag: string) => onChange(hashtags.filter((t) => t !== tag));

  const add = () => {
    if (isAtLimit) return;
    const tag = newTag.trim().replace(/^#+/, "").toLowerCase();
    if (tag && !hashtags.includes(tag)) {
      onChange([...hashtags, tag]);
    }
    setNewTag("");
    inputRef.current?.focus();
  };

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex flex-wrap gap-1.5 items-center">
        {hashtags.map((tag) => (
          <span
            key={tag}
            className="inline-flex items-center gap-1 rounded-md bg-secondary px-2 py-0.5 text-xs text-secondary-foreground"
          >
            #{tag}
            <button
              type="button"
              onClick={() => remove(tag)}
              className="text-muted-foreground hover:text-destructive transition-colors"
              aria-label={`Supprimer #${tag}`}
            >
              <X className="h-3 w-3" />
            </button>
          </span>
        ))}
        {!isAtLimit && (
          <div className="inline-flex items-center gap-1">
            <input
              ref={inputRef}
              value={newTag}
              onChange={(e) => setNewTag(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") { e.preventDefault(); add(); }
              }}
              placeholder="ajouter..."
              className="w-20 bg-transparent text-xs text-foreground placeholder:text-muted-foreground border-b border-border focus:outline-none focus:border-primary"
            />
            <button
              type="button"
              onClick={add}
              className="text-muted-foreground hover:text-primary transition-colors"
              aria-label="Ajouter hashtag"
            >
              <Plus className="h-3.5 w-3.5" />
            </button>
          </div>
        )}
      </div>
      {isAtLimit && (
        <p className="text-xs text-muted-foreground">
          Limite de {maxHashtags} hashtags atteinte pour ce réseau.
        </p>
      )}
    </div>
  );
}

const TONE_LABELS: Record<string, string> = {
  educational: "Éducatif",
  casual: "Casual",
  punchy: "Percutant",
};

function VariantsPanel({
  variants,
  onSelect,
}: {
  variants: CaptionVariant[];
  onSelect: (v: CaptionVariant) => void;
}) {
  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm text-muted-foreground">
        Choisis un ton — il sera chargé dans l'éditeur.
      </p>
      {variants.map((v) => (
        <Card key={v.tone} className="cursor-pointer hover:border-primary transition-colors" onClick={() => onSelect(v)}>
          <CardContent className="pt-4 pb-3 flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold text-primary uppercase tracking-wide">
                {TONE_LABELS[v.tone] ?? v.tone}
              </span>
              <Button size="sm" variant="ghost" className="h-6 text-xs px-2" onClick={(e) => { e.stopPropagation(); onSelect(v); }}>
                Choisir
              </Button>
            </div>
            <p className="text-sm text-foreground line-clamp-3 whitespace-pre-line">{v.caption}</p>
            <div className="flex flex-wrap gap-1">
              {v.hashtags.slice(0, 5).map((t) => (
                <span key={t} className="text-xs text-primary">#{t}</span>
              ))}
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

export function ContentPreview() {
  const { result, variants, network, brief, imageFormat, draftId, setResult, setIsLoading, setError, setDraftId } = useComposerStore();
  const queryClient = useQueryClient();
  const { captionLimit, hashtagLimit, foldLimit, label: networkLabel } = NETWORK_META[network];
  const imageRef = useRef<HTMLDivElement>(null);

  type VisualTemplate = "post" | "code" | "terminal" | "carousel";

  // Must be declared before any early return
  const [hashtags, setHashtags] = useState<string[]>([]);
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [isRendering, setIsRendering] = useState(false);
  const [renderError, setRenderError] = useState<string | null>(null);
  const [template, setTemplate] = useState<VisualTemplate>("post");
  // Code template inputs
  const [code, setCode] = useState("");
  const [language, setLanguage] = useState("bash");
  const [filename, setFilename] = useState("");
  // Terminal template inputs
  const [termCommand, setTermCommand] = useState("");
  const [termOutput, setTermOutput] = useState("");
  // Carousel state
  const [slideCount, setSlideCount] = useState(5);
  const [carouselSlides, setCarouselSlides] = useState<CarouselSlide[] | null>(null);
  const [carouselImages, setCarouselImages] = useState<string[] | null>(null);
  const [carouselIndex, setCarouselIndex] = useState(0);
  const [isCarouselLoading, setIsCarouselLoading] = useState(false);
  const [carouselError, setCarouselError] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState(false);
  const [exportSuccess, setExportSuccess] = useState<string | null>(null);
  const [publishedInSession, setPublishedInSession] = useState(false);
  // Inline caption editing
  const [isEditingCaption, setIsEditingCaption] = useState(false);
  const [editCaption, setEditCaption] = useState("");

  // Publish to the selected network
  const publishMutation = useMutation({
    mutationFn: () => {
      if (draftId === null) throw new Error("Aucun brouillon enregistré");
      if (network === "linkedin") return publishLinkedinPost(draftId);
      return publishPost(draftId);
    },
    onSuccess: () => {
      setPublishedInSession(true);
      queryClient.invalidateQueries({ queryKey: ["post_history"] });
    },
  });

  // Sync local hashtag state whenever a new result arrives; reset image + publish state
  useEffect(() => {
    if (result) {
      setHashtags(result.hashtags);
      setImageUrl(null);
      setRenderError(null);
      setPublishedInSession(false);
      setIsEditingCaption(false);
    }
  }, [result]);

  // Scroll image into view once it loads
  useEffect(() => {
    if (imageUrl && imageRef.current) {
      imageRef.current.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }, [imageUrl]);

  const handleRegenerate = async () => {
    if (!brief) return;
    setIsLoading(true);
    setError(null);
    setResult(null);
    try {
      const newResult = await generateContent(brief, network);
      setResult(newResult);
      saveDraft(network, newResult.caption, newResult.hashtags)
        .then(setDraftId)
        .catch(() => {});
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const handleRenderImage = async () => {
    setIsRendering(true);
    setRenderError(null);
    setImageUrl(null);
    try {
      const { width, height } = imageFormat;
      let url: string;
      if (template === "code") {
        if (!code.trim()) { setRenderError("Colle du code d'abord."); setIsRendering(false); return; }
        url = await renderCodeImage(code, language, filename || undefined, width, height);
      } else if (template === "terminal") {
        if (!termCommand.trim()) { setRenderError("Saisis une commande d'abord."); setIsRendering(false); return; }
        url = await renderTerminalImage(termCommand, termOutput || undefined, width, height);
      } else {
        if (!result) { setRenderError("Génère du contenu d'abord."); setIsRendering(false); return; }
        url = await renderPostImage(result.caption, hashtags, width, height);
      }
      setImageUrl(url);
      // Persist the data URL in SQLite so publish_post can upload it to imgbb
      if (draftId !== null) {
        updateDraftImage(draftId, url).catch(() => {
          // Non-fatal: publish will fail with a clear error if image_path is missing
        });
      }
    } catch (err) {
      setRenderError(String(err));
    } finally {
      setIsRendering(false);
    }
  };

  const handleGenerateCarousel = async () => {
    if (!brief) return;
    setIsCarouselLoading(true);
    setCarouselError(null);
    setCarouselSlides(null);
    setCarouselImages(null);
    setExportSuccess(null);
    try {
      const slides = await generateCarousel(brief, network, slideCount);
      setCarouselSlides(slides);
      setCarouselIndex(0);
      const images = await renderCarouselSlides(slides);
      setCarouselImages(images);
    } catch (err) {
      setCarouselError(String(err));
    } finally {
      setIsCarouselLoading(false);
    }
  };

  const handleExportZip = async () => {
    if (!carouselImages) return;
    setIsExporting(true);
    setExportSuccess(null);
    try {
      const zipPath = await exportCarouselZip(carouselImages);
      const filename = zipPath.split(/[/\\]/).pop() ?? "carousel.zip";
      setExportSuccess(`Enregistré dans Téléchargements : ${filename}`);
    } catch (err) {
      setCarouselError(String(err));
    } finally {
      setIsExporting(false);
    }
  };

  const handleSelectVariant = (v: CaptionVariant) => {
    // setResult already clears variants in the store — don't call setVariants(null) after,
    // as it would reset result back to null.
    setResult({ caption: v.caption, hashtags: v.hashtags });
    saveDraft(network, v.caption, v.hashtags)
      .then(setDraftId)
      .catch(() => {});
  };

  if (!result && !variants) {
    return (
      <div className="flex min-h-40 items-center justify-center rounded-lg border border-dashed border-border">
        <p className="text-sm text-muted-foreground">
          Le contenu généré apparaîtra ici.
        </p>
      </div>
    );
  }

  if (variants) {
    return (
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-base">3 variantes générées</CardTitle>
        </CardHeader>
        <Separator />
        <CardContent className="pt-4">
          <VariantsPanel variants={variants} onSelect={handleSelectVariant} />
        </CardContent>
      </Card>
    );
  }

  // At this point result is guaranteed non-null (variants path returned above, null+null returned above)
  const safeResult = result!;
  const captionLength = safeResult.caption.length;
  const isOverLimit = captionLength > captionLimit;
  const hashtagsText = hashtags.map((t) => `#${t}`).join(" ");

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-base">Aperçu généré</CardTitle>
      </CardHeader>
      <Separator />
      <CardContent className="flex flex-col gap-4 pt-4">
        {/* Caption */}
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-foreground">Caption</span>
            <div className="flex items-center gap-1">
              <span className={`text-xs ${isOverLimit ? "text-destructive" : "text-muted-foreground"}`}>
                {captionLength} / {captionLimit}
              </span>
              {!isEditingCaption && (
                <>
                  <CopyButton text={safeResult.caption} label="la caption" />
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7"
                        onClick={() => { setEditCaption(safeResult.caption); setIsEditingCaption(true); }}
                      >
                        <Pencil className="h-3.5 w-3.5 text-muted-foreground" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>Modifier</TooltipContent>
                  </Tooltip>
                </>
              )}
              {isEditingCaption && (
                <div className="flex items-center gap-1">
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7"
                        onClick={() => {
                          const updated = { caption: editCaption, hashtags };
                          setResult(updated);
                          if (draftId !== null) {
                            updatePostDraft(draftId, editCaption, hashtags).catch(() => {});
                          }
                        }}
                      >
                        <Check className="h-3.5 w-3.5 text-primary" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>Confirmer</TooltipContent>
                  </Tooltip>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7"
                        onClick={() => setIsEditingCaption(false)}
                      >
                        <X className="h-3.5 w-3.5 text-muted-foreground" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>Annuler</TooltipContent>
                  </Tooltip>
                </div>
              )}
            </div>
          </div>
          {isEditingCaption ? (
            <textarea
              value={editCaption}
              onChange={(e) => setEditCaption(e.target.value)}
              rows={6}
              className="w-full bg-secondary/50 border border-border rounded-md px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:border-primary resize-none leading-relaxed"
            />
          ) : (
            <div className="max-h-48 overflow-y-auto rounded-md pr-1">
              <CaptionWithFold
                text={safeResult.caption}
                foldLimit={foldLimit}
                network={networkLabel}
              />
            </div>
          )}
        </div>

        <Separator />

        {/* Hashtags */}
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-foreground">
              Hashtags{" "}
              <span className="text-xs font-normal text-muted-foreground">
                ({hashtags.length})
              </span>
            </span>
            <CopyButton text={hashtagsText} label="les hashtags" />
          </div>
          <EditableHashtags hashtags={hashtags} onChange={setHashtags} maxHashtags={hashtagLimit} />
        </div>

        <Separator />

        {/* Visual generator */}
        <div ref={imageRef} className="flex flex-col gap-3">
          {/* Header + generate button (hidden for carousel which has its own) */}
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-foreground">
            Visuel {imageFormat.width}×{imageFormat.height}
            <span className="ml-1.5 text-xs font-normal text-muted-foreground">{imageFormat.ratio}</span>
          </span>
            {template !== "carousel" && (
              <Button
                variant="outline"
                size="sm"
                className="h-7 gap-1.5 text-xs"
                onClick={handleRenderImage}
                disabled={isRendering}
              >
                {isRendering ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <ImageDown className="h-3.5 w-3.5" />}
                {isRendering ? "Rendu…" : "Générer"}
              </Button>
            )}
          </div>

          {/* Template selector */}
          <div className="flex flex-wrap gap-1 p-1 bg-secondary/50 rounded-lg w-fit">
            {(["post", "code", "terminal", "carousel"] as const).map((t) => (
              <button
                key={t}
                type="button"
                onClick={() => {
                  setTemplate(t);
                  setImageUrl(null);
                  setRenderError(null);
                  setCarouselError(null);
                  setExportSuccess(null);
                }}
                className={`flex items-center gap-1 px-3 py-1 text-xs rounded-md transition-colors font-medium ${
                  template === t
                    ? "bg-background text-foreground shadow-sm"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                {t === "carousel" && <Layers className="h-3 w-3" />}
                {t === "post" ? "Post" : t === "code" ? "Code" : t === "terminal" ? "Terminal" : "Carrousel"}
              </button>
            ))}
          </div>

          {/* Template-specific inputs */}
          {template === "code" && (
            <div className="flex flex-col gap-2">
              <div className="flex gap-2">
                <input
                  value={language}
                  onChange={(e) => setLanguage(e.target.value)}
                  placeholder="langage (ex: bash)"
                  className="flex-1 bg-secondary/50 border border-border rounded-md px-3 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:border-primary"
                />
                <input
                  value={filename}
                  onChange={(e) => setFilename(e.target.value)}
                  placeholder="nom fichier (optionnel)"
                  className="flex-1 bg-secondary/50 border border-border rounded-md px-3 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:border-primary"
                />
              </div>
              <textarea
                value={code}
                onChange={(e) => setCode(e.target.value)}
                placeholder="Colle ton code ici…"
                rows={6}
                className="w-full bg-secondary/50 border border-border rounded-md px-3 py-2 text-xs text-foreground placeholder:text-muted-foreground font-mono focus:outline-none focus:border-primary resize-none"
              />
            </div>
          )}

          {template === "terminal" && (
            <div className="flex flex-col gap-2">
              <input
                value={termCommand}
                onChange={(e) => setTermCommand(e.target.value)}
                placeholder="commande (ex: grep -r 'error' /var/log)"
                className="w-full bg-secondary/50 border border-border rounded-md px-3 py-1.5 text-xs text-foreground placeholder:text-muted-foreground font-mono focus:outline-none focus:border-primary"
              />
              <textarea
                value={termOutput}
                onChange={(e) => setTermOutput(e.target.value)}
                placeholder="output (optionnel)"
                rows={4}
                className="w-full bg-secondary/50 border border-border rounded-md px-3 py-2 text-xs text-foreground placeholder:text-muted-foreground font-mono focus:outline-none focus:border-primary resize-none"
              />
            </div>
          )}

          {/* Carousel template UI */}
          {template === "carousel" && (
            <div className="flex flex-col gap-3">
              {/* Controls row */}
              <div className="flex items-center gap-3">
                <span className="text-xs text-muted-foreground">Slides :</span>
                <div className="flex gap-1">
                  {[3, 5, 7, 10].map((n) => (
                    <button
                      key={n}
                      type="button"
                      onClick={() => setSlideCount(n)}
                      className={`px-2.5 py-0.5 rounded text-xs font-medium transition-colors ${
                        slideCount === n
                          ? "bg-primary text-primary-foreground"
                          : "bg-secondary text-muted-foreground hover:text-foreground"
                      }`}
                    >
                      {n}
                    </button>
                  ))}
                </div>
                <Button
                  type="button"
                  size="sm"
                  className="ml-auto h-7 gap-1.5 text-xs"
                  onClick={handleGenerateCarousel}
                  disabled={isCarouselLoading || !brief}
                >
                  {isCarouselLoading
                    ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
                    : <Layers className="h-3.5 w-3.5" />}
                  {isCarouselLoading
                    ? (carouselSlides ? "Rendu…" : "Génération…")
                    : "Générer"}
                </Button>
              </div>

              {carouselError && (
                <p className="text-xs text-destructive bg-destructive/10 rounded-md px-3 py-2">
                  {carouselError}
                </p>
              )}

              {/* Slide preview + navigation */}
              {carouselImages && (
                <>
                  <div className="rounded-lg overflow-hidden border border-border flex justify-center bg-[#0d1117]">
                    <img
                      src={carouselImages[carouselIndex]}
                      alt={`Slide ${carouselIndex + 1}`}
                      className="max-h-64 w-auto object-contain"
                    />
                  </div>
                  <div className="flex items-center justify-between">
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-7 gap-1 text-xs"
                      onClick={() => setCarouselIndex((i) => Math.max(0, i - 1))}
                      disabled={carouselIndex === 0}
                    >
                      <ChevronLeft className="h-4 w-4" />
                      Précédent
                    </Button>
                    <span className="text-xs text-muted-foreground">
                      {carouselIndex + 1} / {carouselImages.length}
                    </span>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-7 gap-1 text-xs"
                      onClick={() => setCarouselIndex((i) => Math.min(carouselImages.length - 1, i + 1))}
                      disabled={carouselIndex === carouselImages.length - 1}
                    >
                      Suivant
                      <ChevronRight className="h-4 w-4" />
                    </Button>
                  </div>
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-7 gap-1.5 text-xs"
                    onClick={handleExportZip}
                    disabled={isExporting}
                  >
                    {isExporting
                      ? <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      : <Download className="h-3.5 w-3.5" />}
                    {isExporting ? "Export…" : "Exporter ZIP"}
                  </Button>
                  {exportSuccess && (
                    <p className="text-xs text-primary bg-primary/10 rounded-md px-3 py-2">
                      {exportSuccess}
                    </p>
                  )}
                </>
              )}

              {/* Slide titles list (shown after AI generation, before render completes) */}
              {carouselSlides && !carouselImages && !isCarouselLoading && (
                <div className="flex flex-col gap-1">
                  {carouselSlides.map((s) => (
                    <div key={s.index} className="flex items-center gap-2 text-xs text-muted-foreground">
                      <span className="text-sm">{s.emoji}</span>
                      <span>{s.title}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {renderError && template !== "carousel" && (
            <p className="text-xs text-destructive bg-destructive/10 rounded-md px-3 py-2">
              {renderError}
            </p>
          )}

          {template !== "carousel" && (
            imageUrl ? (
              <div className="rounded-lg overflow-hidden border border-border flex justify-center items-center bg-[#0d1117] p-2">
                <img src={imageUrl} alt="Visuel post" className="max-h-96 max-w-full object-contain rounded" />
              </div>
            ) : !renderError && !isRendering ? (
              <div className="flex h-32 items-center justify-center rounded-lg border border-dashed border-border">
                <p className="text-xs text-muted-foreground">Clique sur "Générer" pour créer le visuel</p>
              </div>
            ) : null
          )}
        </div>

        <Separator />

        {/* Actions */}
        <div className="flex flex-col gap-2">
          <div className="flex gap-2">
            {/* Publish button — visible once content is ready.
                LinkedIn allows text-only; Instagram requires an image. */}
            {(imageUrl !== null || network === "linkedin") && !publishedInSession && (
              <Button
                variant="default"
                className="flex-1"
                disabled={publishMutation.isPending || draftId === null}
                onClick={() => publishMutation.mutate()}
              >
                {publishMutation.isPending ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    Publication…
                  </>
                ) : (
                  `Publier sur ${networkLabel}`
                )}
              </Button>
            )}
            {/* Success badge replaces button after publish */}
            {publishedInSession && (
              <span className="flex-1 flex items-center justify-center gap-1.5 rounded-md border border-primary/30 bg-primary/10 px-4 py-2 text-sm font-medium text-primary">
                <Check className="h-4 w-4" />
                Publié ✓
              </span>
            )}
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="outline"
                  size="icon"
                  onClick={handleRegenerate}
                  aria-label="Regénérer"
                >
                  <RefreshCw className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Regénérer</TooltipContent>
            </Tooltip>
          </div>
          {/* Publish error */}
          {publishMutation.isError && (
            <p className="text-xs text-destructive bg-destructive/10 rounded-md px-3 py-2">
              {publishMutation.error instanceof Error
                ? publishMutation.error.message
                : String(publishMutation.error)}
            </p>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
