import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { FileEdit, Send, Trash2, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useComposerStore } from "@/stores/composer.store";
import { publishPost, publishLinkedinPost } from "@/lib/tauri/publisher";
import { deletePost } from "@/lib/tauri/calendar";
import type { PostRecord } from "@/types/composer.types";

interface PostActionsProps {
  post: PostRecord;
  /** Called after successful delete. Use it to optimistically remove the row from the parent list. */
  onDeleted?: (id: number) => void;
  /** Called after successful publish. Use it to refresh the parent list. */
  onPublished?: (id: number) => void;
  /** Layout variant. `compact` = single row of icon-only buttons; `full` = labeled buttons with destructive grouped at start. */
  variant?: "compact" | "full";
  /** When true, the publish button is hidden — useful in views that already
   *  show a parent control like the calendar reschedule menu. */
  hidePublish?: boolean;
}

/** Determines whether a draft can be published immediately from a list view.
 *  IG requires at least one image; LinkedIn accepts text-only posts. */
export function canPublishInline(post: PostRecord): boolean {
  if (post.status !== "draft") return false;
  if (post.network === "instagram") return post.images.length > 0;
  return true; // linkedin / twitter / tiktok — text-only allowed
}

/**
 * Shared action group used in the dashboard list, the calendar modal, and
 * any future surface that lists posts. Encapsulates the publish-network-routing
 * (`publish_post` for IG, `publish_linkedin_post` for LI) and the two-step
 * delete confirmation so callers don't reimplement it.
 */
export function PostActions({
  post,
  onDeleted,
  onPublished,
  variant = "compact",
  hidePublish = false,
}: PostActionsProps) {
  const navigate = useNavigate();
  const setPendingDraftId = useComposerStore((s) => s.setPendingDraftId);
  const queryClient = useQueryClient();
  const [confirmDelete, setConfirmDelete] = useState(false);

  const isDraft = post.status === "draft";
  const showPublish = !hidePublish && canPublishInline(post);

  const handleOpen = () => {
    setPendingDraftId(post.id);
    navigate({ to: "/composer" });
  };

  const publishMutation = useMutation({
    mutationFn: () =>
      post.network === "linkedin" ? publishLinkedinPost(post.id) : publishPost(post.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["post_history"] });
      queryClient.invalidateQueries({ queryKey: ["calendar_posts"] });
      onPublished?.(post.id);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: () => deletePost(post.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["post_history"] });
      queryClient.invalidateQueries({ queryKey: ["calendar_posts"] });
      onDeleted?.(post.id);
    },
  });

  const handleDelete = () => {
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    deleteMutation.mutate();
  };

  const isPublishing = publishMutation.isPending;
  const isDeleting = deleteMutation.isPending;

  if (variant === "compact") {
    return (
      <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
        {isDraft && (
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-muted-foreground hover:text-foreground"
            title="Ouvrir dans le Composer"
            onClick={handleOpen}
            aria-label="Ouvrir dans le Composer"
          >
            <FileEdit className="h-3.5 w-3.5" />
          </Button>
        )}
        {showPublish && (
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-primary hover:text-primary hover:bg-primary/10"
            title="Publier maintenant"
            onClick={() => publishMutation.mutate()}
            disabled={isPublishing}
            aria-label="Publier maintenant"
          >
            {isPublishing ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Send className="h-3.5 w-3.5" />}
          </Button>
        )}
        <Button
          variant="ghost"
          size="icon"
          className={confirmDelete ? "h-7 w-7 text-destructive hover:text-destructive" : "h-7 w-7 text-muted-foreground hover:text-destructive"}
          title={confirmDelete ? "Cliquer à nouveau pour confirmer" : "Supprimer"}
          onClick={handleDelete}
          disabled={isDeleting}
          aria-label={confirmDelete ? "Confirmer la suppression" : "Supprimer"}
        >
          {isDeleting ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Trash2 className="h-3.5 w-3.5" />}
        </Button>
      </div>
    );
  }

  // Full variant — labeled buttons, used inside detail panels.
  return (
    <div className="flex flex-wrap items-center gap-2" onClick={(e) => e.stopPropagation()}>
      {isDraft && (
        <Button variant="outline" size="sm" className="gap-1.5" onClick={handleOpen}>
          <FileEdit className="h-3.5 w-3.5" />
          Ouvrir
        </Button>
      )}
      {showPublish && (
        <Button
          size="sm"
          className="gap-1.5"
          onClick={() => publishMutation.mutate()}
          disabled={isPublishing}
        >
          {isPublishing ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Send className="h-3.5 w-3.5" />}
          Publier maintenant
        </Button>
      )}
      <Button
        variant="ghost"
        size="sm"
        className={
          confirmDelete
            ? "gap-1.5 text-destructive hover:text-destructive"
            : "gap-1.5 text-muted-foreground hover:text-destructive"
        }
        onClick={handleDelete}
        disabled={isDeleting}
      >
        {isDeleting ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Trash2 className="h-3.5 w-3.5" />}
        {confirmDelete ? "Confirmer ?" : "Supprimer"}
      </Button>

      {publishMutation.isError && (
        <p className="basis-full rounded-md bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {String(publishMutation.error)}
        </p>
      )}
    </div>
  );
}
