import { ExternalLink, LogOut, User, Link2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";

/**
 * Placeholder UI for Instagram account connection.
 * OAuth PKCE implementation is task #1 — this component will wire up
 * to start_oauth_flow / complete_oauth_flow Tauri commands once implemented.
 */
export function AccountsForm() {
  // TODO(task#1): replace with real account state from Tauri + SQLite
  const connected = false;
  const account = null as null | { username: string; name: string };

  return (
    <div className="flex flex-col gap-6">
      {/* Instagram section */}
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-2">
          <Link2 className="h-5 w-5 text-primary" />
          <span className="text-sm font-semibold text-foreground">Instagram</span>
          {connected
            ? <Badge className="text-xs bg-primary/20 text-primary border-0">Connecté</Badge>
            : <Badge variant="secondary" className="text-xs">Non connecté</Badge>
          }
        </div>

        {connected && account ? (
          <div className="flex items-center justify-between rounded-lg border border-border p-3">
            <div className="flex items-center gap-3">
              <div className="flex h-9 w-9 items-center justify-center rounded-full bg-secondary">
                <User className="h-4 w-4 text-muted-foreground" />
              </div>
              <div>
                <p className="text-sm font-medium text-foreground">@{account.username}</p>
                <p className="text-xs text-muted-foreground">{account.name}</p>
              </div>
            </div>
            <Button variant="ghost" size="sm" className="text-destructive hover:text-destructive gap-1.5">
              <LogOut className="h-3.5 w-3.5" />
              Déconnecter
            </Button>
          </div>
        ) : (
          <div className="flex flex-col gap-3">
            <Alert>
              <AlertDescription className="text-xs text-muted-foreground">
                La connexion Instagram utilise OAuth 2.0 PKCE — aucun mot de passe
                n'est stocké. Ton token est conservé localement sur ta machine.
              </AlertDescription>
            </Alert>
            <Button
              className="w-fit gap-2"
              disabled
              title="Bientôt disponible"
            >
              <Link2 className="h-4 w-4" />
              Connecter Instagram
            </Button>
            <p className="text-xs text-muted-foreground flex items-center gap-1">
              <ExternalLink className="h-3 w-3" />
              Nécessite un compte Instagram Business ou Creator
            </p>
          </div>
        )}
      </div>

      {/* Future networks */}
      <div className="flex flex-col gap-2 opacity-40 pointer-events-none">
        <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">V2 — Prochainement</p>
        <div className="flex gap-2">
          {["LinkedIn", "Twitter/X", "TikTok"].map((n) => (
            <Badge key={n} variant="outline" className="text-xs">{n}</Badge>
          ))}
        </div>
      </div>
    </div>
  );
}
