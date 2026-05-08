import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, Link2, LogOut, User } from "lucide-react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  disconnectAccount,
  getInstagramAppId,
  getInstagramClientSecretStatus,
  saveInstagramAppId,
  saveInstagramClientSecret,
  startOAuthFlow,
} from "@/lib/tauri/oauth";
import { ProductTruthEditor } from "./ProductTruthEditor";
import { BrandColorsEditor } from "./BrandColorsEditor";
import { TokenExpiryBadge } from "./TokenExpiryBadge";
import type { ConnectedAccount } from "@/lib/tauri/oauth";

export function InstagramSection({ account }: { account: ConnectedAccount | undefined }) {
  const qc = useQueryClient();
  const [appIdInput, setAppIdInput] = useState("");
  const [secretInput, setSecretInput] = useState("");
  const [connectError, setConnectError] = useState<string | null>(null);

  const { data: savedAppId = null } = useQuery({
    queryKey: ["instagram_app_id"],
    queryFn: getInstagramAppId,
  });

  const { data: secretConfigured = false } = useQuery({
    queryKey: ["instagram_client_secret_status"],
    queryFn: getInstagramClientSecretStatus,
  });

  const saveAppId = useMutation({
    mutationFn: (id: string) => saveInstagramAppId(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["instagram_app_id"] }),
  });

  const saveSecret = useMutation({
    mutationFn: (s: string) => saveInstagramClientSecret(s),
    onSuccess: () => {
      setSecretInput("");
      qc.invalidateQueries({ queryKey: ["instagram_client_secret_status"] });
    },
  });

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

  const disconnect = useMutation({
    mutationFn: ({ provider, userId }: { provider: string; userId: string }) =>
      disconnectAccount(provider, userId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["accounts"] }),
  });

  const appId = savedAppId ?? "";
  const canConnect = !!appId && secretConfigured;

  return (
    <div className="flex flex-col gap-3">
      {/* Instagram account */}
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-2">
          <Link2 className="h-5 w-5 text-primary" />
          <span className="text-sm font-semibold text-foreground">Instagram</span>
          {account ? (
            <Badge className="text-xs bg-primary/20 text-primary border-0">Connecté</Badge>
          ) : (
            <Badge variant="secondary" className="text-xs">Non connecté</Badge>
          )}
        </div>

        {account ? (
          <div className="rounded-lg border border-border p-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="flex h-9 w-9 items-center justify-center rounded-full bg-secondary">
                  <User className="h-4 w-4 text-muted-foreground" />
                </div>
                <div>
                  <div className="flex items-center gap-2">
                    <p className="text-sm font-medium text-foreground">@{account.username}</p>
                    <TokenExpiryBadge expiresAt={account.token_expires_at} />
                  </div>
                  {account.display_name && (
                    <p className="text-xs text-muted-foreground">{account.display_name}</p>
                  )}
                </div>
              </div>
              <Button
                variant="ghost"
                size="sm"
                className="text-destructive hover:text-destructive gap-1.5"
                disabled={disconnect.isPending}
                onClick={() => disconnect.mutate({ provider: account.provider, userId: account.user_id })}
              >
                <LogOut className="h-3.5 w-3.5" />
                Déconnecter
              </Button>
            </div>
            <ProductTruthEditor
              accountId={account.id}
              initialValue={account.product_truth}
              handle={account.username}
            />
            <BrandColorsEditor
              accountId={account.id}
              initialBrand={account.brand_color}
              initialAccent={account.accent_color}
            />
          </div>
        ) : (
          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label className="text-xs text-muted-foreground">
                Meta App ID
                {appId && <span className="ml-1 text-primary">✓ configuré</span>}
              </Label>
              {appId && appIdInput === "" ? (
                <div className="flex gap-2">
                  <Input value={appId} readOnly className="font-mono text-sm bg-muted/30 cursor-text select-all" />
                  <Button variant="outline" size="sm" onClick={() => setAppIdInput(appId)}>Modifier</Button>
                </div>
              ) : (
                <div className="flex gap-2">
                  <Input
                    placeholder="876077775447670"
                    value={appIdInput}
                    onChange={(e) => setAppIdInput(e.target.value)}
                    className="font-mono text-sm"
                    autoFocus={!!appId}
                  />
                  <Button
                    variant="outline" size="sm"
                    disabled={!appIdInput.trim() || saveAppId.isPending}
                    onClick={() => { saveAppId.mutate(appIdInput.trim()); setAppIdInput(""); }}
                  >
                    {saveAppId.isPending ? "…" : "Enregistrer"}
                  </Button>
                  {appId && (
                    <Button variant="ghost" size="sm" onClick={() => setAppIdInput("")}>Annuler</Button>
                  )}
                </div>
              )}
            </div>

            <div className="flex flex-col gap-2">
              <Label className="text-xs text-muted-foreground">
                Meta App Secret
                {secretConfigured && <span className="ml-1 text-primary">✓ configuré</span>}
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
                  variant="outline" size="sm"
                  disabled={!secretInput.trim() || saveSecret.isPending}
                  onClick={() => saveSecret.mutate(secretInput.trim())}
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
                La connexion Instagram utilise OAuth 2.0 PKCE — aucun mot de passe n'est stocké.
                Ton token est conservé localement. Nécessite un compte Instagram Business ou Creator.
              </AlertDescription>
            </Alert>

            {connectError && <p className="text-xs text-destructive">{connectError}</p>}

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

    </div>
  );
}
