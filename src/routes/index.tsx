import { useState } from "react";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  PenLine,
  FileText,
  CheckCircle,
  Clock,
  Pencil,
  Check,
  Loader2,
} from "lucide-react";
import { useNavigate } from "@tanstack/react-router";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { getPostHistory } from "@/lib/tauri/composer";
import { updatePostDraft } from "@/lib/tauri/calendar";
import type { PostRecord } from "@/types/composer.types";
import { NETWORK_META } from "@/types/composer.types";
import { CaptionWithFold, FoldCounter } from "@/components/shared/CaptionWithFold";
import { NetworkBadge } from "@/components/shared/NetworkBadge";
import { PostThumbnail } from "@/components/shared/PostThumbnail";
import { GroupBadge } from "@/components/shared/GroupBadge";
import { PostActions } from "@/components/shared/PostActions";

const STATUS_META = {
  draft: { label: "Brouillon", variant: "secondary" as const, icon: Clock },
  published: { label: "Publié", variant: "default" as const, icon: CheckCircle },
  failed: { label: "Échec", variant: "destructive" as const, icon: FileText },
};

function StatCard({
  label,
  value,
  icon: Icon,
}: {
  label: string;
  value: number;
  icon: React.ElementType;
}) {
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

export function PostDetailSheet({
  post,
  onClose,
  onDelete,
  onUpdate,
}: {
  post: PostRecord | null;
  onClose: () => void;
  onDelete: (id: number) => void;
  onUpdate: (updated: PostRecord) => void;
}) {
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [editCaption, setEditCaption] = useState("");
  const [editHashtags, setEditHashtags] = useState("");

  // Reset edit state whenever the user opens a different post
  if (post && !isEditing && editCaption === "" && post.caption !== "") {
    setEditCaption(post.caption);
    setEditHashtags(post.hashtags.join(" "));
  }

  if (!post) return null;
  const meta = STATUS_META[post.status] ?? STATUS_META.draft;

  const handleSaveEdit = async () => {
    const hashtags = editHashtags
      .split(/[\s,]+/)
      .map((t) => t.replace(/^#/, "").trim())
      .filter(Boolean);
    setIsSaving(true);
    try {
      await updatePostDraft(post.id, editCaption, hashtags);
      onUpdate({ ...post, caption: editCaption, hashtags });
      setIsEditing(false);
    } finally {
      setIsSaving(false);
    }
  };

  const previewImage = post.images?.[0] ?? post.image_path ?? null;
  const isCarousel = (post.images?.length ?? 0) > 1;

  return (
    <Sheet
      open={!!post}
      onOpenChange={(open) => {
        if (!open) {
          setIsEditing(false);
          setEditCaption("");
          setEditHashtags("");
          onClose();
        }
      }}
    >
      <SheetContent side="right" className="flex flex-col overflow-hidden">
        <SheetHeader className="px-6 pt-6 pb-4 pr-12 border-b border-border shrink-0">
          <SheetTitle className="text-base font-semibold leading-snug">
            Détail du post
          </SheetTitle>
        </SheetHeader>

        <div className="flex-1 overflow-y-auto px-6 py-6 flex flex-col gap-5">
          {/* Status + Network + Group + Date */}
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant={meta.variant}>{meta.label}</Badge>
            <NetworkBadge network={post.network} />
            <GroupBadge groupId={post.group_id} variant="chip" />
            <span className="text-xs text-muted-foreground">
              {format(new Date(post.created_at), "d MMM yyyy · HH:mm", { locale: fr })}
            </span>
          </div>

          {/* Image preview — single or carousel strip */}
          {previewImage && (
            <div>
              <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-widest text-primary">
                {isCarousel ? `Carrousel · ${post.images.length} slides` : "Image"}
              </h3>
              {isCarousel ? (
                <div className="flex gap-2 overflow-x-auto pb-1">
                  {post.images.map((src, i) => (
                    <img
                      key={i}
                      src={src}
                      alt={`Slide ${i + 1}`}
                      className="h-32 w-32 shrink-0 rounded-md border border-border object-cover"
                      loading="lazy"
                    />
                  ))}
                </div>
              ) : (
                <img
                  src={previewImage}
                  alt=""
                  className="max-h-72 w-full rounded-md border border-border object-contain bg-secondary/30"
                  loading="lazy"
                />
              )}
            </div>
          )}

          {/* Caption */}
          <div>
            <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-widest text-primary">
              Légende
            </h3>
            {isEditing ? (
              <div className="flex flex-col gap-1">
                <Textarea
                  value={editCaption}
                  onChange={(e) => setEditCaption(e.target.value)}
                  className="text-sm min-h-32 resize-none"
                  autoFocus
                />
                <FoldCounter
                  length={editCaption.length}
                  foldLimit={NETWORK_META[post.network].foldLimit}
                />
              </div>
            ) : (
              <CaptionWithFold
                text={post.caption}
                foldLimit={NETWORK_META[post.network].foldLimit}
                network={NETWORK_META[post.network].label}
              />
            )}
          </div>

          {/* Hashtags */}
          {(post.hashtags.length > 0 || isEditing) && (
            <div>
              <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-widest text-primary">
                Hashtags
              </h3>
              {isEditing ? (
                <input
                  type="text"
                  value={editHashtags}
                  onChange={(e) => setEditHashtags(e.target.value)}
                  placeholder="hashtag1 hashtag2 …"
                  className="w-full rounded-md border border-input bg-background px-3 py-1.5 text-xs text-foreground outline-none focus:ring-1 focus:ring-ring"
                />
              ) : (
                <div className="flex flex-wrap gap-1.5">
                  {post.hashtags.map((tag) => (
                    <span
                      key={tag}
                      className="rounded-md bg-secondary px-2 py-0.5 text-xs text-foreground/80"
                    >
                      #{tag}
                    </span>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Published date */}
          {post.published_at && (
            <div>
              <h3 className="mb-1 text-[11px] font-semibold uppercase tracking-widest text-primary">
                Publié le
              </h3>
              <p className="text-sm text-foreground/80">
                {format(new Date(post.published_at), "d MMM yyyy · HH:mm", { locale: fr })}
              </p>
            </div>
          )}
        </div>

        {/* Actions footer */}
        <div className="shrink-0 px-6 py-4 border-t border-border flex flex-col gap-3">
          {isEditing ? (
            <div className="flex items-center justify-end gap-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setIsEditing(false)}
                disabled={isSaving}
              >
                Annuler
              </Button>
              <Button
                size="sm"
                onClick={handleSaveEdit}
                disabled={isSaving || !editCaption.trim()}
                className="gap-1.5"
              >
                {isSaving ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}
                Sauvegarder
              </Button>
            </div>
          ) : (
            <>
              <PostActions
                post={post}
                onDeleted={(id) => {
                  onDelete(id);
                  setEditCaption("");
                }}
                onPublished={(id) => {
                  onUpdate({ ...post, status: "published", published_at: new Date().toISOString() });
                  setEditCaption("");
                  // Keep the sheet open so the user sees the published badge.
                  void id;
                }}
                variant="full"
              />
              {post.status === "draft" && (
                <Button
                  variant="outline"
                  size="sm"
                  className="gap-1.5 self-start"
                  onClick={() => setIsEditing(true)}
                >
                  <Pencil className="h-3.5 w-3.5" />
                  Modifier le texte
                </Button>
              )}
            </>
          )}
        </div>
      </SheetContent>
    </Sheet>
  );
}

export function DashboardPage() {
  const queryClient = useQueryClient();
  const [selectedPost, setSelectedPost] = useState<PostRecord | null>(null);
  const navigate = useNavigate();

  const { data: posts = [] } = useQuery({
    queryKey: ["post_history"],
    queryFn: () => getPostHistory(20),
  });

  const published = posts.filter((p) => p.status === "published").length;
  const drafts = posts.filter((p) => p.status === "draft").length;

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
        <StatCard label="Posts générés" value={posts.length} icon={FileText} />
        <StatCard label="Publiés" value={published} icon={CheckCircle} />
        <StatCard label="Brouillons" value={drafts} icon={Clock} />
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
                  <div
                    key={post.id}
                    className="flex items-center gap-3 py-3 cursor-pointer rounded-md px-2 -mx-2 hover:bg-secondary/50 transition-colors"
                    onClick={() => setSelectedPost(post)}
                  >
                    <PostThumbnail post={post} size="md" />
                    <div className="flex-1 min-w-0">
                      <p className="text-sm text-foreground line-clamp-2">{post.caption}</p>
                      <div className="flex items-center gap-2 mt-1 flex-wrap">
                        <span className="text-xs text-muted-foreground">
                          {format(new Date(post.created_at), "d MMM · HH:mm", { locale: fr })}
                        </span>
                        <NetworkBadge network={post.network} />
                        <Badge variant={meta.variant} className="text-xs shrink-0">
                          {meta.label}
                        </Badge>
                        <GroupBadge groupId={post.group_id} />
                      </div>
                    </div>
                    <PostActions
                      post={post}
                      onDeleted={() => {
                        queryClient.invalidateQueries({ queryKey: ["post_history"] });
                        setSelectedPost(null);
                      }}
                      onPublished={() => {
                        queryClient.invalidateQueries({ queryKey: ["post_history"] });
                      }}
                      variant="compact"
                    />
                  </div>
                );
              })}
            </div>
          )}
        </CardContent>
      </Card>

      <PostDetailSheet
        post={selectedPost}
        onClose={() => setSelectedPost(null)}
        onDelete={() => {
          queryClient.invalidateQueries({ queryKey: ["post_history"] });
          setSelectedPost(null);
        }}
        onUpdate={(updated) => {
          queryClient.invalidateQueries({ queryKey: ["post_history"] });
          setSelectedPost(updated);
        }}
      />
    </div>
  );
}
