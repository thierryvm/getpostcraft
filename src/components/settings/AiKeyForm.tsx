import { useState, useEffect } from "react";
import { format } from "date-fns";
import { CheckCircle, XCircle, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader,
  AlertDialogTitle, AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { saveAiKey, getAiKeyStatus, deleteAiKey, setActiveProvider } from "@/lib/tauri/settings";
import {
  PROVIDER_META, OPENROUTER_MODELS, PROVIDER_DEFAULT_MODELS,
  type AiProvider, type AiKeyStatus, type KeyValidationResult,
} from "@/types/settings.types";

export function AiKeyForm() {
  const [provider, setProvider] = useState<AiProvider>("openrouter");
  const [model, setModel] = useState(PROVIDER_DEFAULT_MODELS.openrouter);
  const [inputKey, setInputKey] = useState("");
  const [ollamaUrl, setOllamaUrl] = useState("http://localhost:11434");
  const [keyStatus, setKeyStatus] = useState<AiKeyStatus | null>(null);
  const [result, setResult] = useState<KeyValidationResult | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [validatedAt, setValidatedAt] = useState<Date | null>(null);

  useEffect(() => {
    if (provider !== "ollama") {
      getAiKeyStatus(provider).then(setKeyStatus).catch(console.error);
    } else {
      setKeyStatus(null);
    }
    setResult(null);
    setInputKey("");
  }, [provider]);

  const handleProviderChange = (p: AiProvider) => {
    setProvider(p);
    setModel(PROVIDER_DEFAULT_MODELS[p]);
  };

  const handleSave = async () => {
    setIsLoading(true);
    setResult(null);
    try {
      if (provider === "ollama") {
        await setActiveProvider(provider, model);
        setResult({ valid: true });
        setValidatedAt(new Date());
        return;
      }
      if (!inputKey.trim()) return;
      const r = await saveAiKey(provider, inputKey.trim());
      setResult(r);
      if (r.valid) {
        await setActiveProvider(provider, model);
        setValidatedAt(new Date());
        setInputKey("");
        getAiKeyStatus(provider).then(setKeyStatus).catch(console.error);
      }
    } catch (err) {
      setResult({ valid: false, error: String(err) });
    } finally {
      setIsLoading(false);
    }
  };

  const handleDelete = async () => {
    await deleteAiKey(provider).catch(console.error);
    setKeyStatus({ configured: false, masked: null });
    setResult(null);
    setValidatedAt(null);
  };

  const needsKey = PROVIDER_META[provider].keyPrefix !== null;
  const canSave = provider === "ollama" || inputKey.trim().length > 0;

  return (
    <div className="flex flex-col gap-6">
      {/* Provider */}
      <div className="flex flex-col gap-2">
        <Label>Fournisseur</Label>
        <Select value={provider} onValueChange={(v) => handleProviderChange(v as AiProvider)}>
          <SelectTrigger className="w-72"><SelectValue /></SelectTrigger>
          <SelectContent>
            {(Object.keys(PROVIDER_META) as AiProvider[]).map((p) => (
              <SelectItem key={p} value={p}>{PROVIDER_META[p].label}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* Model */}
      <div className="flex flex-col gap-2">
        <Label>Modèle</Label>
        {provider === "openrouter" ? (
          <Select value={model} onValueChange={setModel}>
            <SelectTrigger className="w-72"><SelectValue /></SelectTrigger>
            <SelectContent>
              {OPENROUTER_MODELS.map((m) => (
                <SelectItem key={m.value} value={m.value}>{m.label}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        ) : (
          <Input
            value={model}
            onChange={(e) => setModel(e.target.value)}
            className="w-72 font-mono text-sm"
            placeholder={PROVIDER_DEFAULT_MODELS[provider]}
          />
        )}
      </div>

      {/* Ollama URL */}
      {provider === "ollama" && (
        <div className="flex flex-col gap-2">
          <Label>URL Ollama</Label>
          <Input
            value={ollamaUrl}
            onChange={(e) => setOllamaUrl(e.target.value)}
            className="w-72 font-mono text-sm"
          />
        </div>
      )}

      {/* API Key */}
      {needsKey && (
        <div className="flex flex-col gap-2">
          <Label htmlFor="api-key">
            Clé API{keyStatus?.configured ? " (remplacement)" : ""}
          </Label>
          {keyStatus?.configured && (
            <div className="flex items-center gap-2 mb-1">
              <code className="text-sm bg-secondary px-2 py-0.5 rounded text-muted-foreground">
                {keyStatus.masked}
              </code>
              <Badge className="text-xs bg-primary/20 text-primary border-0">Configurée</Badge>
              {validatedAt && (
                <span className="text-xs text-muted-foreground">
                  · validée le {format(validatedAt, "dd/MM/yyyy HH:mm")}
                </span>
              )}
            </div>
          )}
          <Input
            id="api-key"
            type="password"
            value={inputKey}
            onChange={(e) => setInputKey(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSave()}
            placeholder={PROVIDER_META[provider].keyPrefix + "..."}
            className="w-72 font-mono text-sm"
            autoComplete="off"
            spellCheck={false}
          />
        </div>
      )}

      {/* Action */}
      <Button onClick={handleSave} disabled={!canSave || isLoading} className="w-fit">
        {isLoading && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
        {provider === "ollama" ? "Définir comme actif" : "Tester & Sauvegarder"}
      </Button>

      {/* Result */}
      {result && (
        <Alert variant={result.valid ? "default" : "destructive"}>
          {result.valid
            ? <CheckCircle className="h-4 w-4 text-primary" />
            : <XCircle className="h-4 w-4" />}
          <AlertDescription>
            {result.valid
              ? "✓ Clé valide — provider actif"
              : (result.error ?? "Erreur inconnue")}
          </AlertDescription>
        </Alert>
      )}

      {/* Delete */}
      {needsKey && keyStatus?.configured && (
        <AlertDialog>
          <AlertDialogTrigger asChild>
            <Button variant="destructive" size="sm" className="w-fit">
              Supprimer la clé
            </Button>
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Supprimer la clé API ?</AlertDialogTitle>
              <AlertDialogDescription>
                Supprimée du trousseau système. La génération ne sera plus possible
                avec ce provider.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Annuler</AlertDialogCancel>
              <AlertDialogAction
                onClick={handleDelete}
                className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              >
                Supprimer
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      )}
    </div>
  );
}
