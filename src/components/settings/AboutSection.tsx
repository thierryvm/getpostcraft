import { useEffect, useState } from "react";
import { ExternalLink, Globe, Shield, Code2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { UpdateChecker } from "./UpdateChecker";
import { appVersion } from "@/lib/tauri/updater";

export function AboutSection() {
  const [version, setVersion] = useState<string>("…");

  useEffect(() => {
    appVersion()
      .then(setVersion)
      .catch(() => setVersion("inconnu"));
  }, []);

  return (
    <div className="flex flex-col gap-5">
      {/* App identity */}
      <div className="flex flex-col gap-1">
        <div className="flex items-center gap-2">
          <span className="text-lg font-semibold text-foreground">Getpostcraft</span>
          <Badge variant="secondary" className="font-mono text-xs">v{version}</Badge>
        </div>
        <p className="text-sm text-muted-foreground">
          AI-assisted social media content creation — desktop app
        </p>
      </div>

      <Separator />

      {/* Auto-updater */}
      <UpdateChecker />

      <Separator />

      {/* Links */}
      <div className="flex flex-col gap-2">
        <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">Liens</p>
        {[
          { icon: Globe, label: "getpostcraft.app", href: "https://getpostcraft.app" },
          { icon: Code2, label: "github.com/thierryvm/getpostcraft", href: "https://github.com/thierryvm/getpostcraft" },
        ].map(({ icon: Icon, label, href }) => (
          <a
            key={href}
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-2 text-sm text-muted-foreground hover:text-primary transition-colors w-fit"
          >
            <Icon className="h-3.5 w-3.5" />
            {label}
            <ExternalLink className="h-3 w-3 opacity-50" />
          </a>
        ))}
      </div>

      <Separator />

      {/* License */}
      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-2">
          <Shield className="h-3.5 w-3.5 text-muted-foreground" />
          <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">Licence</p>
        </div>
        <p className="text-sm text-foreground font-medium">BUSL-1.1</p>
        <p className="text-xs text-muted-foreground">
          Usage personnel uniquement jusqu'au 11 avril 2030, puis MIT.
          <br />
          © 2026 Thierry Vanmeeteren — tous droits réservés.
        </p>
      </div>

      {/* Stack */}
      <Separator />
      <div className="flex flex-col gap-2">
        <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">Stack</p>
        <div className="flex flex-wrap gap-1.5">
          {["Tauri 2", "React 18", "TypeScript", "Tailwind v4", "Rust", "Python"].map((t) => (
            <Badge key={t} variant="outline" className="text-xs font-mono">{t}</Badge>
          ))}
        </div>
      </div>
    </div>
  );
}
