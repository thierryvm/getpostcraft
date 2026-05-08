import { useEffect, useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Check, Upload } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { getImgbbKeyStatus, saveImgbbKey } from "@/lib/tauri/publisher";

export function ImgbbSection() {
  const qc = useQueryClient();
  const [imgbbInput, setImgbbInput] = useState("");
  // Local "saved" flag → shows the success check on the button for ~2s
  // after a successful save. Pattern matches `PublicationForm` and
  // `AiKeyForm` so feedback is consistent across panels.
  const [savedFlash, setSavedFlash] = useState(false);

  const { data: imgbbConfigured = false } = useQuery({
    queryKey: ["imgbb_key_status"],
    queryFn: getImgbbKeyStatus,
  });

  const saveImgbb = useMutation({
    mutationFn: (key: string) => saveImgbbKey(key),
    onSuccess: () => {
      setImgbbInput("");
      setSavedFlash(true);
      qc.invalidateQueries({ queryKey: ["imgbb_key_status"] });
    },
  });

  // Reset the success flash after 2.5 s. Long enough to be noticed,
  // short enough to clear before the user types a new value.
  useEffect(() => {
    if (!savedFlash) return;
    const t = setTimeout(() => setSavedFlash(false), 2500);
    return () => clearTimeout(t);
  }, [savedFlash]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <Upload className="h-5 w-5 text-primary" />
        <span className="text-sm font-semibold text-foreground">Hébergement d'images</span>
        {imgbbConfigured && (
          <Badge className="text-xs bg-primary/20 text-primary border-0">✓ configuré</Badge>
        )}
      </div>
      <div className="flex flex-col gap-2">
        <Label htmlFor="imgbb-key">
          Clé API imgbb
          {imgbbConfigured && (
            <span className="ml-1 text-xs font-normal text-primary">✓ configuré</span>
          )}
        </Label>
        <p className="text-xs text-muted-foreground">
          Nécessaire pour héberger l'image avant publication Instagram
        </p>
        <div className="flex gap-2">
          <Input
            id="imgbb-key"
            type="password"
            value={imgbbInput}
            onChange={(e) => setImgbbInput(e.target.value)}
            onKeyDown={(e) =>
              e.key === "Enter" && imgbbInput.trim() && saveImgbb.mutate(imgbbInput.trim())
            }
            placeholder={imgbbConfigured ? "••••••••••••••••" : "imgbb.com → API → Add API key"}
            className="font-mono text-sm"
            autoComplete="off"
            spellCheck={false}
          />
          <Button
            variant="outline"
            size="sm"
            disabled={!imgbbInput.trim() || saveImgbb.isPending}
            onClick={() => saveImgbb.mutate(imgbbInput.trim())}
            className="gap-1.5"
          >
            {saveImgbb.isPending ? (
              "…"
            ) : savedFlash ? (
              <>
                <Check className="h-3.5 w-3.5" aria-hidden="true" />
                Enregistré
              </>
            ) : (
              "Enregistrer"
            )}
          </Button>
        </div>
        {saveImgbb.isError && (
          <p className="text-xs text-destructive bg-destructive/10 rounded p-2">
            Échec de l'enregistrement : {String(saveImgbb.error)}
          </p>
        )}
      </div>
    </div>
  );
}
