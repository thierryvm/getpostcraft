import { RefreshCw, Copy, Check, X, Plus, ImageDown, Loader2 } from "lucide-react";
import { useState, useRef, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useNavigate } from "@tanstack/react-router";
import { useComposerStore } from "@/stores/composer.store";
import { generateContent } from "@/lib/tauri/composer";
import { renderPostImage } from "@/lib/tauri/media";
import { NETWORK_META } from "@/types/composer.types";

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
}: {
  hashtags: string[];
  onChange: (tags: string[]) => void;
}) {
  const [newTag, setNewTag] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const remove = (tag: string) => onChange(hashtags.filter((t) => t !== tag));

  const add = () => {
    const tag = newTag.trim().replace(/^#+/, "").toLowerCase();
    if (tag && !hashtags.includes(tag)) {
      onChange([...hashtags, tag]);
    }
    setNewTag("");
    inputRef.current?.focus();
  };

  return (
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
    </div>
  );
}

export function ContentPreview() {
  const { result, network, brief, setResult, setIsLoading, setError } = useComposerStore();
  const navigate = useNavigate();
  const captionLimit = NETWORK_META[network].captionLimit;
  const imageRef = useRef<HTMLDivElement>(null);

  // Must be declared before any early return
  const [hashtags, setHashtags] = useState<string[]>([]);
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [isRendering, setIsRendering] = useState(false);
  const [renderError, setRenderError] = useState<string | null>(null);

  // Sync local hashtag state whenever a new result arrives; reset image
  useEffect(() => {
    if (result) {
      setHashtags(result.hashtags);
      setImageUrl(null);
      setRenderError(null);
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
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const handleRenderImage = async () => {
    if (!result) return;
    setIsRendering(true);
    setRenderError(null);
    setImageUrl(null);
    try {
      const url = await renderPostImage(result.caption, hashtags);
      setImageUrl(url);
    } catch (err) {
      setRenderError(String(err));
    } finally {
      setIsRendering(false);
    }
  };

  if (!result) {
    return (
      <div className="flex min-h-40 items-center justify-center rounded-lg border border-dashed border-border">
        <p className="text-sm text-muted-foreground">
          Le contenu généré apparaîtra ici.
        </p>
      </div>
    );
  }

  const captionLength = result.caption.length;
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
              <CopyButton text={result.caption} label="la caption" />
            </div>
          </div>
          <p className="text-sm text-foreground whitespace-pre-line leading-relaxed">
            {result.caption}
          </p>
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
          <EditableHashtags hashtags={hashtags} onChange={setHashtags} />
        </div>

        <Separator />

        {/* Image generation */}
        <div ref={imageRef} className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-foreground">Visuel 1080×1080</span>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 gap-1.5 text-xs"
                  onClick={handleRenderImage}
                  disabled={isRendering}
                >
                  {isRendering ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <ImageDown className="h-3.5 w-3.5" />
                  )}
                  {isRendering ? "Rendu en cours…" : "Générer l'image"}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                Rendu PNG 1080×1080 via Playwright
              </TooltipContent>
            </Tooltip>
          </div>

          {renderError && (
            <p className="text-xs text-destructive bg-destructive/10 rounded-md px-3 py-2">
              {renderError}
            </p>
          )}

          {imageUrl ? (
            <div className="rounded-lg overflow-hidden border border-border flex justify-center bg-[#0d1117]">
              <img
                src={imageUrl}
                alt="Visuel post Instagram"
                className="max-h-72 w-auto object-contain"
              />
            </div>
          ) : !renderError && !isRendering ? (
            <div className="flex h-16 items-center justify-center rounded-lg border border-dashed border-border">
              <p className="text-xs text-muted-foreground">
                Clique sur "Générer l'image" pour créer le visuel
              </p>
            </div>
          ) : null}
        </div>

        <Separator />

        {/* Actions */}
        <div className="flex gap-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="flex-1">
                <Button
                  variant="default"
                  className="w-full"
                  onClick={() => navigate({ to: "/settings", search: { tab: "accounts" } })}
                >
                  Publier sur Instagram
                </Button>
              </span>
            </TooltipTrigger>
            <TooltipContent>Connecter un compte Instagram</TooltipContent>
          </Tooltip>
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
      </CardContent>
    </Card>
  );
}
