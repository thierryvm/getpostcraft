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
  getLinkedInClientId,
  getLinkedInClientSecretStatus,
  listAccounts,
  saveInstagramAppId,
  saveInstagramClientSecret,
  saveLinkedInClientId,
  saveLinkedInClientSecret,
  startLinkedInOAuthFlow,
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

  // ── LinkedIn state ────────────────────────────────────────────────────────

  const [liClientIdInput, setLiClientIdInput] = useState("");
  const [liSecretInput, setLiSecretInput] = useState("");
  const [liConnectError, setLiConnectError] = useState<string | null>(null);

  const { data: savedLiClientId = null } = useQuery({
    queryKey: ["linkedin_client_id"],
    queryFn: getLinkedInClientId,
  });

  const { data: liSecretConfigured = false } = useQuery({
    queryKey: ["linkedin_client_secret_status"],
    queryFn: getLinkedInClientSecretStatus,
  });

  const saveLiClientId = useMutation({
    mutationFn: (id: string) => saveLinkedInClientId(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["linkedin_client_id"] }),
  });

  const saveLiSecret = useMutation({
    mutationFn: (s: string) => saveLinkedInClientSecret(s),
    onSuccess: () => {
      setLiSecretInput("");
      qc.invalidateQueries({ queryKey: ["linkedin_client_secret_status"] });
    },
  });

  const connectLinkedIn = useMutation({
    mutationFn: () => startLinkedInOAuthFlow(),
    onSuccess: () => {
      setLiConnectError(null);
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
    onError: (e: unknown) => {
      setLiConnectError(e instanceof Error ? e.message : String(e));
    },
  });

  // ── Derived values ─────────────────────────────────────────────────────────

  const instagramAccount = accounts.find((a) => a.provider === "instagram");
  const appId = savedAppId ?? "";
  const canConnect = !!appId && secretConfigured;

  const linkedInAccount = accounts.find((a) => a.provider === "linkedin");
  const canConnectLinkedIn = !!savedLiClientId && liSecretConfigured;

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

      {/* LinkedIn */}
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-2">
          <Link2 className="h-5 w-5 text-[#0A66C2]" />
          <span className="text-sm font-semibold text-foreground">LinkedIn</span>
          {linkedInAccount ? (
            <Badge className="text-xs bg-primary/20 text-primary border-0">Connecté</Badge>
          ) : (
            <Badge variant="secondary" className="text-xs">Non connecté</Badge>
          )}
        </div>

        {linkedInAccount ? (
          <div className="flex items-center justify-between rounded-lg border border-border p-3">
            <div className="flex items-center gap-3">
              <div className="flex h-9 w-9 items-center justify-center rounded-full bg-secondary">
                <User className="h-4 w-4 text-muted-foreground" />
              </div>
              <div>
                <p className="text-sm font-medium text-foreground">
                  {linkedInAccount.display_name ?? linkedInAccount.username}
                </p>
                <p className="text-xs text-muted-foreground">LinkedIn</p>
              </div>
            </div>
            <Button
              variant="ghost"
              size="sm"
              className="text-destructive hover:text-destructive gap-1.5"
              disabled={disconnect.isPending}
              onClick={() =>
                disconnect.mutate({
                  provider: linkedInAccount.provider,
                  userId: linkedInAccount.user_id,
                })
              }
            >
              <LogOut className="h-3.5 w-3.5" />
              Déconnecter
            </Button>
          </div>
        ) : (
          <div className="flex flex-col gap-4">
            {/* Client ID */}
            <div className="flex flex-col gap-2">
              <Label className="text-xs text-muted-foreground">
                LinkedIn Client ID
                {savedLiClientId && (
                  <span className="ml-1 text-primary">✓ configuré</span>
                )}
              </Label>
              <div className="flex gap-2">
                <Input
                  placeholder={savedLiClientId ?? "86xxxxxxxxxxxxxxxx"}
                  value={liClientIdInput}
                  onChange={(e) => setLiClientIdInput(e.target.value)}
                  className="font-mono text-sm"
                />
                <Button
                  variant="outline"
                  size="sm"
                  disabled={!liClientIdInput.trim() || saveLiClientId.isPending}
                  onClick={() => {
                    saveLiClientId.mutate(liClientIdInput.trim());
                    setLiClientIdInput("");
                  }}
                >
                  {saveLiClientId.isPending ? "…" : "Enregistrer"}
                </Button>
              </div>
            </div>

            {/* Client Secret — write-only */}
            <div className="flex flex-col gap-2">
              <Label className="text-xs text-muted-foreground">
                LinkedIn Client Secret
                {liSecretConfigured && (
                  <span className="ml-1 text-primary">✓ configuré</span>
                )}
              </Label>
              <div className="flex gap-2">
                <Input
                  type="password"
                  placeholder={liSecretConfigured ? "••••••••••••••••" : "Client Secret LinkedIn"}
                  value={liSecretInput}
                  onChange={(e) => setLiSecretInput(e.target.value)}
                  className="font-mono text-sm"
                  autoComplete="off"
                  spellCheck={false}
                />
                <Button
                  variant="outline"
                  size="sm"
                  disabled={!liSecretInput.trim() || saveLiSecret.isPending}
                  onClick={() => {
                    saveLiSecret.mutate(liSecretInput.trim());
                  }}
                >
                  {saveLiSecret.isPending ? "…" : "Enregistrer"}
                </Button>
              </div>
              <p className="text-xs text-muted-foreground flex items-center gap-1">
                <ExternalLink className="h-3 w-3 shrink-0" />
                developer.linkedin.com → App → Auth → Client ID &amp; Secret
              </p>
            </div>

            <Alert>
              <AlertDescription className="text-xs text-muted-foreground">
                La connexion LinkedIn utilise OAuth 2.0 PKCE — aucun mot de passe
                n'est stocké. Ton token est conservé localement. Enregistre{" "}
                <span className="font-mono">https://localhost:7892/callback</span>{" "}
                comme redirect URL dans ton app LinkedIn.
              </AlertDescription>
            </Alert>

            {liConnectError && (
              <p className="text-xs text-destructive">{liConnectError}</p>
            )}

            <Button
              className="w-fit gap-2"
              disabled={!canConnectLinkedIn || connectLinkedIn.isPending}
              onClick={() => connectLinkedIn.mutate()}
            >
              <Link2 className="h-4 w-4" />
              {connectLinkedIn.isPending
                ? "En attente de l'autorisation dans le navigateur…"
                : "Connecter LinkedIn"}
            </Button>
          </div>
        )}
      </div>

      {/* Future networks */}
      <div className="flex flex-col gap-2 opacity-40 pointer-events-none">
        <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          V3 — Prochainement
        </p>
        <div className="flex gap-2">
          {["Twitter/X", "TikTok"].map((n) => (
            <Badge key={n} variant="outline" className="text-xs">{n}</Badge>
          ))}
        </div>
      </div>
    </div>
  );
}
