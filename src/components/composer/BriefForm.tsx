import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { Loader2, AlertCircle } from "lucide-react";
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
import { generateContent, generateVariants, saveDraft } from "@/lib/tauri/composer";
import { NETWORK_META, type Network } from "@/types/composer.types";

const briefSchema = z.object({
  brief: z
    .string()
    .min(10, "Minimum 10 caractères")
    .max(500, "Maximum 500 caractères"),
  network: z.enum(["instagram", "linkedin", "twitter", "tiktok"]),
});

type BriefFormData = z.infer<typeof briefSchema>;

export function BriefForm() {
  const { brief, network, isLoading, error, setBrief, setNetwork, setResult, setVariants, setIsLoading, setError } =
    useComposerStore();

  const {
    register,
    handleSubmit,
    watch,
    control,
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
      const result = await generateContent(data.brief, data.network as Network);
      setResult(result);
      setBrief(data.brief);
      setNetwork(data.network as Network);
      // Auto-save as draft — fire-and-forget, never blocks or surfaces errors
      saveDraft(data.network as Network, result.caption, result.hashtags).catch(() => {});
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
      const variants = await generateVariants(data.brief, data.network as Network);
      setBrief(data.brief);
      setNetwork(data.network as Network);
      setVariants(variants);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-4">
      <div>
        <h1 className="text-xl font-semibold text-foreground">Nouveau post</h1>
        <p className="text-sm text-muted-foreground mt-0.5">
          Décris ton idée, Claude génère le contenu.
        </p>
      </div>

      {/* Network select */}
      <div className="flex flex-col gap-1.5">
        <label className="text-sm font-medium text-foreground">Réseau</label>
        <Controller
          name="network"
          control={control}
          render={({ field }) => (
            <Select value={field.value} onValueChange={field.onChange}>
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

      {/* Brief textarea */}
      <div className="flex flex-col gap-1.5">
        <label className="text-sm font-medium text-foreground">Brief</label>
        <Textarea
          {...register("brief")}
          placeholder="Décris ce que tu veux poster..."
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
