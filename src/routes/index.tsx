import { useEffect, useState } from "react";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import { PenLine, FileText, CheckCircle, Clock } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useNavigate } from "@tanstack/react-router";
import { Button } from "@/components/ui/button";
import { getPostHistory } from "@/lib/tauri/composer";
import type { PostRecord } from "@/types/composer.types";

const STATUS_META = {
  draft:     { label: "Brouillon", variant: "secondary" as const, icon: Clock },
  published: { label: "Publié",    variant: "default"   as const, icon: CheckCircle },
  failed:    { label: "Échec",     variant: "destructive" as const, icon: FileText },
};

function StatCard({ label, value, icon: Icon }: { label: string; value: number; icon: React.ElementType }) {
  return (
    <Card>
      <CardContent className="flex items-center gap-4 pt-5">
        <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-secondary">
          <Icon className="h-5 w-5 text-primary" />
        </div>
        <div>
          <p className="text-2xl font-bold text-foreground">{value}</p>
          <p className="text-xs text-muted-foreground">{label}</p>
        </div>
      </CardContent>
    </Card>
  );
}

export function DashboardPage() {
  const [posts, setPosts] = useState<PostRecord[]>([]);
  const navigate = useNavigate();

  useEffect(() => {
    getPostHistory(20).then(setPosts).catch(console.error);
  }, []);

  const published = posts.filter((p) => p.status === "published").length;
  const drafts    = posts.filter((p) => p.status === "draft").length;

  return (
    <div className="flex flex-col gap-6 p-6 overflow-auto">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold text-foreground">Dashboard</h1>
          <p className="text-sm text-muted-foreground mt-0.5">
            Vue d'ensemble de ton activité
          </p>
        </div>
        <Button onClick={() => navigate({ to: "/composer" })} className="gap-2">
          <PenLine className="h-4 w-4" />
          Nouveau post
        </Button>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <StatCard label="Posts générés"  value={posts.length} icon={FileText} />
        <StatCard label="Publiés"        value={published}    icon={CheckCircle} />
        <StatCard label="Brouillons"     value={drafts}       icon={Clock} />
      </div>

      {/* History */}
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="text-base">Historique récent</CardTitle>
        </CardHeader>
        <CardContent>
          {posts.length === 0 ? (
            <div className="flex flex-col items-center gap-3 py-10 text-center">
              <p className="text-sm text-muted-foreground">
                Aucun post généré pour l'instant.
              </p>
              <Button variant="outline" size="sm" onClick={() => navigate({ to: "/composer" })}>
                Créer mon premier post
              </Button>
            </div>
          ) : (
            <div className="flex flex-col divide-y divide-border">
              {posts.map((post) => {
                const meta = STATUS_META[post.status] ?? STATUS_META.draft;
                return (
                  <div key={post.id} className="flex items-start gap-3 py-3">
                    <div className="flex-1 min-w-0">
                      <p className="text-sm text-foreground line-clamp-2">{post.caption}</p>
                      <div className="flex items-center gap-2 mt-1">
                        <span className="text-xs text-muted-foreground">
                          {format(new Date(post.created_at), "d MMM yyyy · HH:mm", { locale: fr })}
                        </span>
                        <span className="text-xs text-muted-foreground">·</span>
                        <span className="text-xs text-muted-foreground capitalize">{post.network}</span>
                      </div>
                    </div>
                    <Badge variant={meta.variant} className="text-xs shrink-0">
                      {meta.label}
                    </Badge>
                  </div>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
