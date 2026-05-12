import { useEffect, useState } from "react";
import { ShieldCheck, ShieldAlert, ShieldQuestion, Loader2, LogOut } from "lucide-react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  isSecurityPasswordSet,
  setupSecurityPassword,
  verifySecurityPassword,
  checkSecuritySession,
  endSecuritySession,
  listRecentSecurityAttempts,
  type SecurityAttemptRow,
} from "@/lib/tauri/securityAdmin";
import { TauriRuntimeUnavailableError } from "@/lib/tauri/invoke";

type GateState =
  | { kind: "loading" }
  | { kind: "unavailable"; reason: string }
  | { kind: "setup-needed" }
  | { kind: "locked"; sessionToken: string | null }
  | { kind: "unlocked"; sessionToken: string };

const SESSION_STORAGE_KEY = "gpc.security.session";

export function SecurityCenter() {
  const [gate, setGate] = useState<GateState>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;
    async function bootstrap() {
      try {
        const hasPassword = await isSecurityPasswordSet();
        if (cancelled) return;
        if (!hasPassword) {
          setGate({ kind: "setup-needed" });
          return;
        }
        // Try to revive a session from sessionStorage. Session token
        // lives only in RAM on the Rust side too — a refresh of the
        // renderer would otherwise force re-prompt on every render.
        const stored = sessionStorage.getItem(SESSION_STORAGE_KEY);
        if (stored && (await checkSecuritySession(stored))) {
          setGate({ kind: "unlocked", sessionToken: stored });
          return;
        }
        if (stored) sessionStorage.removeItem(SESSION_STORAGE_KEY);
        setGate({ kind: "locked", sessionToken: null });
      } catch (err) {
        if (cancelled) return;
        if (err instanceof TauriRuntimeUnavailableError) {
          setGate({
            kind: "unavailable",
            reason:
              "Le centre de sécurité n'est disponible que dans l'app desktop. Lance `npm run tauri dev`.",
          });
        } else {
          setGate({
            kind: "unavailable",
            reason: err instanceof Error ? err.message : String(err),
          });
        }
      }
    }
    void bootstrap();
    return () => {
      cancelled = true;
    };
  }, []);

  if (gate.kind === "loading") {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        Initialisation du centre de sécurité…
      </div>
    );
  }

  if (gate.kind === "unavailable") {
    return (
      <Alert>
        <ShieldQuestion className="h-4 w-4" />
        <AlertTitle>Centre de sécurité indisponible</AlertTitle>
        <AlertDescription>{gate.reason}</AlertDescription>
      </Alert>
    );
  }

  if (gate.kind === "setup-needed") {
    return (
      <SecurityPasswordSetup
        onSetupComplete={() =>
          setGate({ kind: "locked", sessionToken: null })
        }
      />
    );
  }

  if (gate.kind === "locked") {
    return (
      <SecurityPasswordGate
        onUnlock={(token) => {
          sessionStorage.setItem(SESSION_STORAGE_KEY, token);
          setGate({ kind: "unlocked", sessionToken: token });
        }}
      />
    );
  }

  return (
    <SecurityCenterUnlocked
      onLock={() => {
        sessionStorage.removeItem(SESSION_STORAGE_KEY);
        void endSecuritySession();
        setGate({ kind: "locked", sessionToken: null });
      }}
    />
  );
}

// ── Setup wizard ───────────────────────────────────────────────────────────

function SecurityPasswordSetup({ onSetupComplete }: { onSetupComplete: () => void }) {
  const [pwd, setPwd] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const tooShort = pwd.length > 0 && pwd.length < 12;
  const mismatch = confirm.length > 0 && pwd !== confirm;
  const canSubmit = pwd.length >= 12 && pwd === confirm && !busy;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    setBusy(true);
    setError(null);
    try {
      await setupSecurityPassword(pwd);
      onSetupComplete();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <ShieldCheck className="h-4 w-4" />
          Création du mot de passe Sécurité
        </CardTitle>
        <CardDescription>
          Définis un mot de passe maître pour protéger l'accès au centre de
          sécurité (audits LLM + rapports). Stocké en local via Argon2id dans
          ton trousseau système — il ne quitte jamais ta machine et ne peut
          pas être récupéré s'il est perdu.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form className="space-y-4" onSubmit={handleSubmit}>
          <div className="space-y-2">
            <label className="text-sm font-medium" htmlFor="security-pwd">
              Mot de passe (12 caractères minimum)
            </label>
            <Input
              id="security-pwd"
              type="password"
              autoComplete="new-password"
              value={pwd}
              onChange={(e) => setPwd(e.target.value)}
              placeholder="•••••••••••••"
              disabled={busy}
            />
            {tooShort && (
              <p className="text-xs text-destructive">Encore {12 - pwd.length} caractère(s).</p>
            )}
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium" htmlFor="security-pwd-confirm">
              Confirme le mot de passe
            </label>
            <Input
              id="security-pwd-confirm"
              type="password"
              autoComplete="new-password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              disabled={busy}
            />
            {mismatch && (
              <p className="text-xs text-destructive">Les deux saisies ne correspondent pas.</p>
            )}
          </div>
          {error && (
            <Alert variant="destructive">
              <ShieldAlert className="h-4 w-4" />
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}
          <Button type="submit" disabled={!canSubmit}>
            {busy && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Activer le centre de sécurité
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}

// ── Verify gate ────────────────────────────────────────────────────────────

function SecurityPasswordGate({ onUnlock }: { onUnlock: (token: string) => void }) {
  const [pwd, setPwd] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lockoutSeconds, setLockoutSeconds] = useState<number | null>(null);

  useEffect(() => {
    if (lockoutSeconds === null || lockoutSeconds <= 0) return;
    const handle = setInterval(() => {
      setLockoutSeconds((s) => (s === null ? null : Math.max(0, s - 1)));
    }, 1000);
    return () => clearInterval(handle);
  }, [lockoutSeconds]);

  const locked = lockoutSeconds !== null && lockoutSeconds > 0;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (busy || locked) return;
    setBusy(true);
    setError(null);
    try {
      const r = await verifySecurityPassword(pwd);
      switch (r.kind) {
        case "ok":
          onUnlock(r.token);
          break;
        case "locked_out":
          setLockoutSeconds(r.wait_seconds);
          setError(
            `Trop d'échecs récents. Réessaie dans ${r.wait_seconds} seconde(s).`,
          );
          break;
        case "no_password_set":
          setError(
            "Aucun mot de passe configuré. Recharge la page pour démarrer la configuration.",
          );
          break;
        case "wrong":
          setError("Mot de passe incorrect.");
          setPwd("");
          break;
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <ShieldAlert className="h-4 w-4" />
          Centre de sécurité verrouillé
        </CardTitle>
        <CardDescription>
          Saisis le mot de passe maître pour accéder aux audits et rapports.
          Session active pendant 30 minutes une fois déverrouillé.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form className="space-y-4" onSubmit={handleSubmit}>
          <Input
            type="password"
            autoComplete="current-password"
            autoFocus
            value={pwd}
            onChange={(e) => setPwd(e.target.value)}
            placeholder="Mot de passe maître"
            disabled={busy || locked}
          />
          {error && (
            <Alert variant="destructive">
              <ShieldAlert className="h-4 w-4" />
              <AlertDescription>
                {error}
                {locked && lockoutSeconds !== null && lockoutSeconds > 0 && (
                  <span className="ml-1 font-mono">({lockoutSeconds}s)</span>
                )}
              </AlertDescription>
            </Alert>
          )}
          <Button type="submit" disabled={busy || locked || pwd.length === 0}>
            {busy && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Déverrouiller
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}

// ── Unlocked panel ─────────────────────────────────────────────────────────

function SecurityCenterUnlocked({ onLock }: { onLock: () => void }) {
  const [attempts, setAttempts] = useState<SecurityAttemptRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const rows = await listRecentSecurityAttempts(20);
        if (!cancelled) setAttempts(rows);
      } catch (err) {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <div className="flex items-start justify-between gap-4">
            <div>
              <CardTitle className="flex items-center gap-2 text-base">
                <ShieldCheck className="h-4 w-4 text-emerald-500" />
                Centre de sécurité déverrouillé
              </CardTitle>
              <CardDescription>
                Session active. L'agent runner et les rapports d'audit
                arrivent en Phase B.
              </CardDescription>
            </div>
            <Button variant="outline" size="sm" onClick={onLock}>
              <LogOut className="mr-2 h-4 w-4" />
              Verrouiller
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <div className="rounded-md border border-dashed border-border bg-muted/30 p-6 text-center text-sm text-muted-foreground">
            <ShieldQuestion className="mx-auto mb-2 h-6 w-6 opacity-60" />
            <p className="font-medium text-foreground">
              Agent runner — bientôt
            </p>
            <p className="mt-1">
              Phase B branchera ici le lancement de `llm-security-auditor` et
              `prompt-guardrail-auditor` avec affichage des rapports persistés.
            </p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Tentatives récentes</CardTitle>
          <CardDescription>
            20 dernières tentatives de déverrouillage. Trace forensique
            locale — jamais transmise.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {error ? (
            <Alert variant="destructive">
              <ShieldAlert className="h-4 w-4" />
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : attempts === null ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              Chargement…
            </div>
          ) : attempts.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              Aucune tentative enregistrée.
            </p>
          ) : (
            <ul className="space-y-2 text-sm">
              {attempts.map((row) => (
                <li
                  key={row.id}
                  className="flex items-center justify-between gap-3 rounded-md border border-border bg-muted/30 px-3 py-2"
                >
                  <div className="flex items-center gap-2">
                    {row.success === 1 ? (
                      <ShieldCheck className="h-4 w-4 text-emerald-500" />
                    ) : (
                      <ShieldAlert className="h-4 w-4 text-destructive" />
                    )}
                    <span className="font-mono text-xs text-muted-foreground">
                      {new Date(row.attempted_at).toLocaleString("fr-FR")}
                    </span>
                  </div>
                  <span className="text-xs text-muted-foreground">
                    {row.note ?? (row.success === 1 ? "OK" : "Échec")}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
