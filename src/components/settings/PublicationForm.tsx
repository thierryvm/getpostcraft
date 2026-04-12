import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Check, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  getImageHost,
  saveImageHost,
  saveImgbbKey,
  getImgbbKeyStatus,
  type ImageHostProvider,
} from "@/lib/tauri/publisher";

export function PublicationForm() {
  const qc = useQueryClient();
  const [imgbbInput, setImgbbInput] = useState("");
  const [imgbbSaved, setImgbbSaved] = useState(false);

  const { data: host = "catbox" } = useQuery({
    queryKey: ["image_host"],
    queryFn: getImageHost,
  });

  const { data: hasImgbbKey = false } = useQuery({
    queryKey: ["imgbb_key_status"],
    queryFn: getImgbbKeyStatus,
  });

  const switchHost = useMutation({
    mutationFn: (provider: ImageHostProvider) => saveImageHost(provider),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["image_host"] }),
  });

  const saveKey = useMutation({
    mutationFn: (key: string) => saveImgbbKey(key),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["imgbb_key_status"] });
      setImgbbInput("");
      setImgbbSaved(true);
      setTimeout(() => setImgbbSaved(false), 2000);
    },
  });

  return (
    <div className="flex flex-col gap-6">
      {/* Provider selector */}
      <div className="flex flex-col gap-3">
        <p className="text-sm text-muted-foreground">
          Pour publier sur Instagram, l'image doit être hébergée sur une URL publique.
          Choisis le service utilisé pour cet hébergement intermédiaire.
        </p>

        <div className="flex flex-col gap-2">
          {/* Catbox option */}
          <button
            type="button"
            onClick={() => switchHost.mutate("catbox")}
            className={`flex items-start gap-3 rounded-lg border p-4 text-left transition-colors ${
              host === "catbox"
                ? "border-primary bg-primary/5"
                : "border-border hover:border-border/80"
            }`}
          >
            <div className={`mt-0.5 h-4 w-4 shrink-0 rounded-full border-2 flex items-center justify-center ${
              host === "catbox" ? "border-primary" : "border-muted-foreground"
            }`}>
              {host === "catbox" && (
                <div className="h-2 w-2 rounded-full bg-primary" />
              )}
            </div>
            <div className="flex flex-col gap-0.5">
              <span className="text-sm font-medium text-foreground">
                Catbox.moe
                <span className="ml-2 text-xs font-normal text-primary bg-primary/10 px-1.5 py-0.5 rounded">
                  Recommandé · Aucune clé requise
                </span>
              </span>
              <span className="text-xs text-muted-foreground">
                Hébergement anonyme gratuit, URLs permanentes. Aucune configuration nécessaire.
              </span>
            </div>
          </button>

          {/* ImgBB option */}
          <button
            type="button"
            onClick={() => switchHost.mutate("imgbb")}
            className={`flex items-start gap-3 rounded-lg border p-4 text-left transition-colors ${
              host === "imgbb"
                ? "border-primary bg-primary/5"
                : "border-border hover:border-border/80"
            }`}
          >
            <div className={`mt-0.5 h-4 w-4 shrink-0 rounded-full border-2 flex items-center justify-center ${
              host === "imgbb" ? "border-primary" : "border-muted-foreground"
            }`}>
              {host === "imgbb" && (
                <div className="h-2 w-2 rounded-full bg-primary" />
              )}
            </div>
            <div className="flex flex-col gap-0.5">
              <span className="text-sm font-medium text-foreground">
                ImgBB
                {hasImgbbKey && (
                  <span className="ml-2 text-xs font-normal text-muted-foreground">
                    · Clé configurée
                  </span>
                )}
              </span>
              <span className="text-xs text-muted-foreground">
                Clé API gratuite requise (imgbb.com). Option si catbox.moe n'est pas disponible.
              </span>
            </div>
          </button>
        </div>
      </div>

      {/* ImgBB key field — visible only when imgbb is selected */}
      {host === "imgbb" && (
        <div className="flex flex-col gap-2 border-t border-border pt-4">
          <label className="text-sm font-medium text-foreground">
            Clé API ImgBB
          </label>
          <p className="text-xs text-muted-foreground">
            Crée un compte gratuit sur{" "}
            <span className="font-mono text-foreground">imgbb.com</span>{" "}
            → API → ta clé.
          </p>
          <div className="flex gap-2">
            <Input
              type="password"
              value={imgbbInput}
              onChange={(e) => setImgbbInput(e.target.value)}
              placeholder={hasImgbbKey ? "••••••••••••••••" : "Colle ta clé ici…"}
              className="font-mono text-sm"
            />
            <Button
              variant="outline"
              size="sm"
              disabled={!imgbbInput.trim() || saveKey.isPending}
              onClick={() => saveKey.mutate(imgbbInput.trim())}
              className="shrink-0"
            >
              {saveKey.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : imgbbSaved ? (
                <Check className="h-4 w-4 text-primary" />
              ) : (
                "Enregistrer"
              )}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
