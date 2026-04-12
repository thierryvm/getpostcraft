import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, Link2, LogOut, Upload, User } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  disconnectAccount,
  getInstagramAppId,
  getInstagramClientSecretStatus,
  listAccounts,
  saveInstagramAppId,
  saveInstagramClientSecret,
  startOAuthFlow,
} from "@/lib/tauri/oauth";
import { getImgbbKeyStatus, saveImgbbKey } from "@/lib/tauri/publisher";

export function AccountsForm() {
  const qc = useQueryClient();

  const { data: accounts = [] } = useQuery({
    queryKey: ["accounts"],
    queryFn: listAccounts,
  });

  const { data: savedAppId = null } = useQuery({
    queryKey: ["instagram_app_id"],
    queryFn: getInstagramAppId,
  });

  const [appIdInput, setAppIdInput] = useState("");
  const [secretInput, setSecretInput] = useState("");
  const [connectError, setConnectError] = useState<string | null>(null);
  const [imgbbInput, setImgbbInput] = useState("");

  const { data: secretConfigured = false } = useQuery({
    queryKey: ["instagram_client_secret_status"],
    queryFn: getInstagramClientSecretStatus,
  });

  const { data: imgbbConfigured = false } = useQuery({
    queryKey: ["imgbb_key_status"],
    queryFn: getImgbbKeyStatus,
  });

  const saveImgbb = useMutation({
    mutationFn: (key: string) => saveImgbbKey(key),
    onSuccess: () => {
      setImgbbInput("");
      qc.invalidateQueries({ queryKey: ["imgbb_key_status"] });
    },
  });

  // Save App ID to settings
  const saveAppId = useMutation({
    mutationFn: (id: string) => saveInstagramAppId(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["instagram_app_id"] }),
  });

  // Save client_secret (write-only — never read back)
  const saveSecret = useMutation({
    mutationFn: (s: string) => saveInstagramClientSecret(s),
    onSuccess: () => {
      setSecretInput("");
      qc.invalidateQueries({ queryKey: ["instagram_client_secret_status"] });
    },
  });

  // Start OAuth flow (long-running — opens browser)
  const connect = useMutation({
    mutationFn: (clientId: string) => startOAuthFlow(clientId),
    onSuccess: () => {
      setConnectError(null);
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
    onError: (e: unknown) => {
      setConnectError(e instanceof Error ? e.message : String(e));
    },
  });

  // Disconnect account
  const disconnect = useMutation({
    mutationFn: ({ provider, userId }: { provider: string; userId: string }) =>
      disconnectAccount(provider, userId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["accounts"] }),
  });

  const instagramAccount = accounts.find((a) => a.provider === "instagram");
  const appId = savedAppId ?? "";
  const canConnect = !!appId && secretConfigured;

  return (
    <div className="flex flex-col gap-6">
      {/* Instagram */}
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-2">
          <Link2 className="h-5 w-5 text-primary" />
          <span className="text-sm font-semibold text-foreground">Instagram</span>
          {instagramAccount ? (
            <Badge className="text-xs bg-primary/20 text-primary border-0">Connecté</Badge>
          ) : (
            <Badge variant="secondary" className="text-xs">Non connecté</Badge>
          )}
        </div>

        {instagramAccount ? (
          <div className="flex items-center justify-between rounded-lg border border-border p-3">
            <div className="flex items-center gap-3">
              <div className="flex h-9 w-9 items-center justify-center rounded-full bg-secondary">
                <User className="h-4 w-4 text-muted-foreground" />
              </div>
              <div>
                <p className="text-sm font-medium text-foreground">
                  @{instagramAccount.username}
                </p>
                {instagramAccount.display_name && (
                  <p className="text-xs text-muted-foreground">
                    {instagramAccount.display_name}
                  </p>
                )}
              </div>
            </div>
            <Button
              variant="ghost"
              size="sm"
              className="text-destructive hover:text-destructive gap-1.5"
              disabled={disconnect.isPending}
              onClick={() =>
                disconnect.mutate({
                  provider: instagramAccount.provider,
                  userId: instagramAccount.user_id,
                })
              }
            >
              <LogOut className="h-3.5 w-3.5" />
              Déconnecter
            </Button>
          </div>
        ) : (
          <div className="flex flex-col gap-4">
            {/* App ID configuration */}
            <div className="flex flex-col gap-2">
              <Label className="text-xs text-muted-foreground">
                Meta App ID
                {appId && (
                  <span className="ml-1 text-primary">✓ configuré</span>
                )}
              </Label>
              <div className="flex gap-2">
                <Input
                  placeholder={appId || "876077775447670"}
                  value={appIdInput}
                  onChange={(e) => setAppIdInput(e.target.value)}
                  className="font-mono text-sm"
                />
                <Button
                  variant="outline"
                  size="sm"
                  disabled={!appIdInput.trim() || saveAppId.isPending}
                  onClick={() => {
                    saveAppId.mutate(appIdInput.trim());
                    setAppIdInput("");
                  }}
                >
                  {saveAppId.isPending ? "…" : "Enregistrer"}
                </Button>
              </div>
            </div>

            {/* Client Secret configuration */}
            <div className="flex flex-col gap-2">
              <Label className="text-xs text-muted-foreground">
                Meta App Secret
                {secretConfigured && (
                  <span className="ml-1 text-primary">✓ configuré</span>
                )}
              </Label>
              <div className="flex gap-2">
                <Input
                  type="password"
                  placeholder={secretConfigured ? "••••••••••••••••" : "App Secret Meta"}
                  value={secretInput}
                  onChange={(e) => setSecretInput(e.target.value)}
                  className="font-mono text-sm"
                />
                <Button
                  variant="outline"
                  size="sm"
                  disabled={!secretInput.trim() || saveSecret.isPending}
                  onClick={() => {
                    saveSecret.mutate(secretInput.trim());
                  }}
                >
                  {saveSecret.isPending ? "…" : "Enregistrer"}
                </Button>
              </div>
              <p className="text-xs text-muted-foreground flex items-center gap-1">
                <ExternalLink className="h-3 w-3 shrink-0" />
                developers.facebook.com → Paramètres de base → App Secret
              </p>
            </div>

            <Alert>
              <AlertDescription className="text-xs text-muted-foreground">
                La connexion Instagram utilise OAuth 2.0 PKCE — aucun mot de
                passe n'est stocké. Ton token est conservé localement sur ta machine.
                Nécessite un compte Instagram Business ou Creator.
              </AlertDescription>
            </Alert>

            {connectError && (
              <p className="text-xs text-destructive">{connectError}</p>
            )}

            <Button
              className="w-fit gap-2"
              disabled={!canConnect || connect.isPending}
              onClick={() => connect.mutate(appId)}
            >
              <Link2 className="h-4 w-4" />
              {connect.isPending ? "En attente du navigateur…" : "Connecter Instagram"}
            </Button>
          </div>
        )}
      </div>

      {/* imgbb — image hosting key */}
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
              onKeyDown={(e) => e.key === "Enter" && imgbbInput.trim() && saveImgbb.mutate(imgbbInput.trim())}
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
            >
              {saveImgbb.isPending ? "…" : "Enregistrer"}
            </Button>
          </div>
        </div>
      </div>

      {/* Future networks */}
      <div className="flex flex-col gap-2 opacity-40 pointer-events-none">
        <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          V2 — Prochainement
        </p>
        <div className="flex gap-2">
          {["LinkedIn", "Twitter/X", "TikTok"].map((n) => (
            <Badge key={n} variant="outline" className="text-xs">{n}</Badge>
          ))}
        </div>
      </div>
    </div>
  );
}
