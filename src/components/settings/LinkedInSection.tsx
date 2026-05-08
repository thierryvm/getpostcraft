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
  getLinkedInClientId,
  getLinkedInClientSecretStatus,
  saveLinkedInClientId,
  saveLinkedInClientSecret,
  startLinkedInOAuthFlow,
} from "@/lib/tauri/oauth";
import { ProductTruthEditor } from "./ProductTruthEditor";
import { BrandColorsEditor } from "./BrandColorsEditor";
import { TokenExpiryBadge } from "./TokenExpiryBadge";
import type { ConnectedAccount } from "@/lib/tauri/oauth";

export function LinkedInSection({ account }: { account: ConnectedAccount | undefined }) {
  const qc = useQueryClient();
  const [clientIdInput, setClientIdInput] = useState("");
  const [secretInput, setSecretInput] = useState("");
  const [connectError, setConnectError] = useState<string | null>(null);

  const { data: savedClientId = null } = useQuery({
    queryKey: ["linkedin_client_id"],
    queryFn: getLinkedInClientId,
  });

  const { data: secretConfigured = false } = useQuery({
    queryKey: ["linkedin_client_secret_status"],
    queryFn: getLinkedInClientSecretStatus,
  });

  const saveClientId = useMutation({
    mutationFn: (id: string) => saveLinkedInClientId(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["linkedin_client_id"] }),
  });

  const saveSecret = useMutation({
    mutationFn: (s: string) => saveLinkedInClientSecret(s),
    onSuccess: () => {
      setSecretInput("");
      qc.invalidateQueries({ queryKey: ["linkedin_client_secret_status"] });
    },
  });

  const connect = useMutation({
    mutationFn: () => startLinkedInOAuthFlow(),
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

  const canConnect = !!savedClientId && secretConfigured;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        {/* `text-primary` matches the design token (`#3ddc84`) used on
            every other network icon in this tab — `InstagramSection` etc.
            Hardcoded LinkedIn brand blue would have rendered the two
            sections visually inconsistent for no semantic reason. */}
        <Link2 className="h-5 w-5 text-primary" />
        <span className="text-sm font-semibold text-foreground">LinkedIn</span>
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
                  <p className="text-sm font-medium text-foreground">
                    {account.display_name ?? account.username}
                  </p>
                  <TokenExpiryBadge expiresAt={account.token_expires_at} />
                </div>
                <p className="text-xs text-muted-foreground">LinkedIn</p>
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
            handle={account.display_name ?? account.username}
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
              LinkedIn Client ID
              {savedClientId && <span className="ml-1 text-primary">✓ configuré</span>}
            </Label>
            {savedClientId && clientIdInput === "" ? (
              <div className="flex gap-2">
                <Input value={savedClientId} readOnly className="font-mono text-sm bg-muted/30 cursor-text select-all" />
                <Button variant="outline" size="sm" onClick={() => setClientIdInput(savedClientId)}>Modifier</Button>
              </div>
            ) : (
              <div className="flex gap-2">
                <Input
                  placeholder="86xxxxxxxxxxxxxxxx"
                  value={clientIdInput}
                  onChange={(e) => setClientIdInput(e.target.value)}
                  className="font-mono text-sm"
                  autoFocus={!!savedClientId}
                />
                <Button
                  variant="outline" size="sm"
                  disabled={!clientIdInput.trim() || saveClientId.isPending}
                  onClick={() => { saveClientId.mutate(clientIdInput.trim()); setClientIdInput(""); }}
                >
                  {saveClientId.isPending ? "…" : "Enregistrer"}
                </Button>
                {savedClientId && (
                  <Button variant="ghost" size="sm" onClick={() => setClientIdInput("")}>Annuler</Button>
                )}
              </div>
            )}
          </div>

          <div className="flex flex-col gap-2">
            <Label className="text-xs text-muted-foreground">
              LinkedIn Client Secret
              {secretConfigured && <span className="ml-1 text-primary">✓ configuré</span>}
            </Label>
            <div className="flex gap-2">
              <Input
                type="password"
                placeholder={secretConfigured ? "••••••••••••••••" : "Client Secret LinkedIn"}
                value={secretInput}
                onChange={(e) => setSecretInput(e.target.value)}
                className="font-mono text-sm"
                autoComplete="off"
                spellCheck={false}
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
              developer.linkedin.com → App → Auth → Client ID &amp; Secret
            </p>
          </div>

          <Alert>
            <AlertDescription className="text-xs text-muted-foreground">
              La connexion LinkedIn utilise OAuth 2.0 PKCE — aucun mot de passe n'est stocké.
              Ton token est conservé localement. Enregistre{" "}
              <span className="font-mono">https://localhost:7892/callback</span>{" "}
              comme redirect URL dans ton app LinkedIn.
            </AlertDescription>
          </Alert>

          {connectError && <p className="text-xs text-destructive">{connectError}</p>}

          <Button
            className="w-fit gap-2"
            disabled={!canConnect || connect.isPending}
            onClick={() => connect.mutate()}
          >
            <Link2 className="h-4 w-4" />
            {connect.isPending ? "En attente de l'autorisation dans le navigateur…" : "Connecter LinkedIn"}
          </Button>
        </div>
      )}
    </div>
  );
}
