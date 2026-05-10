import { useState, useMemo, useEffect, useCallback } from "react";
import { useQuery, useQueryClient, useMutation } from "@tanstack/react-query";
import {
  ChevronLeft,
  ChevronRight,
  Calendar,
  CalendarDays,
  Loader2,
  X,
  Pencil,
  Check,
  Plus,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import {
  getCalendarPosts,
  schedulePost,
  unschedulePost,
  updatePostDraft,
} from "@/lib/tauri/calendar";
import { getPostHistory } from "@/lib/tauri/composer";
import type { PostRecord } from "@/types/composer.types";
import { cn } from "@/lib/utils";
import { NetworkBadge, NETWORK_DOT_COLORS } from "@/components/shared/NetworkBadge";
import { PostThumbnail } from "@/components/shared/PostThumbnail";
import { PostActions } from "@/components/shared/PostActions";
import { format, parse, isValid } from "date-fns";
import { fr } from "date-fns/locale";

// ── Date helpers ──────────────────────────────────────────────────────────────

/**
 * Format a Date as `YYYY-MM-DD` using the LOCAL calendar day, not UTC.
 *
 * `toISOString().slice(0,10)` was the previous implementation but it returns
 * the UTC date — so a post created at 20:25 CET on May 9 (= 18:25 UTC, ISO
 * "2026-05-09T18:25Z") had key "2026-05-09" while the May 9 cell, built from
 * `new Date(year, month, 1)` at local midnight, had its toISOString() snap
 * back to "2026-05-08T22:00Z" → key "2026-05-08". Posts ended up one day
 * forward in any UTC+N timezone (CET in summer = +2). This helper keeps the
 * keys in the user's local frame so cells and posts agree on the day.
 */
function isoDate(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function startOfMonth(y: number, m: number): Date {
  return new Date(y, m, 1);
}

function startOfWeek(d: Date): Date {
  const day = d.getDay(); // 0 = Sunday
  const diff = day === 0 ? -6 : 1 - day; // shift to Monday
  const result = new Date(d);
  result.setDate(d.getDate() + diff);
  return result;
}

function addDays(d: Date, n: number): Date {
  const result = new Date(d);
  result.setDate(d.getDate() + n);
  return result;
}

function sameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

function monthRangeISO(year: number, month: number): [string, string] {
  const first = new Date(year, month, 1);
  const last = new Date(year, month + 1, 0, 23, 59, 59);
  return [first.toISOString(), last.toISOString()];
}

function weekRangeISO(weekStart: Date): [string, string] {
  const start = new Date(weekStart);
  start.setHours(0, 0, 0, 0);
  const end = addDays(weekStart, 6);
  end.setHours(23, 59, 59, 999);
  return [start.toISOString(), end.toISOString()];
}

/**
 * Bucket a post into a calendar day. Same local-time discipline as `isoDate`:
 * parse the stored UTC ISO timestamp and read its LOCAL day components so
 * a post created at 20:25 CET on May 9 lands on May 9, not May 10.
 *
 * Precedence is **most-concrete-event-wins**:
 *   1. `published_at` — the post actually shipped on this day (Meta /
 *      LinkedIn timestamp). After publish this is the authoritative date.
 *   2. `scheduled_at` — the day the user planned the post for.
 *   3. `created_at`  — fallback for unscheduled drafts.
 *
 * Pre-v0.3.8 the helper used `scheduled_at ?? created_at`, so a draft
 * scheduled for May 9 and published on May 10 stayed glued on May 9 in
 * the calendar — confusing for users tracking what actually went out.
 */
function getPostDate(post: PostRecord): string {
  const iso = post.published_at ?? post.scheduled_at ?? post.created_at;
  return isoDate(new Date(iso));
}

/**
 * Visual status applied as a tiny coloured pill on each calendar tile.
 * Maps the post status to the same palette the dashboard / detail modal
 * already use so the user sees one visual language across surfaces.
 */
type CalendarPostStatus = "published" | "scheduled" | "draft" | "failed";

function postStatusForCalendar(post: PostRecord): CalendarPostStatus {
  if (post.status === "published") return "published";
  if (post.status === "failed") return "failed";
  return post.scheduled_at ? "scheduled" : "draft";
}

const STATUS_PILL_CLASS: Record<CalendarPostStatus, string> = {
  published: "bg-primary/15 text-primary border-primary/30",
  scheduled: "bg-amber-500/15 text-amber-300 border-amber-500/30",
  draft: "bg-muted text-muted-foreground border-border",
  failed: "bg-destructive/15 text-destructive border-destructive/30",
};

const STATUS_PILL_LABEL: Record<CalendarPostStatus, string> = {
  published: "Publié",
  scheduled: "Planifié",
  draft: "Brouillon",
  failed: "Échec",
};

const MONTH_NAMES = [
  "Janvier", "Février", "Mars", "Avril", "Mai", "Juin",
  "Juillet", "Août", "Septembre", "Octobre", "Novembre", "Décembre",
];

const DAY_LABELS_SHORT = ["Lun", "Mar", "Mer", "Jeu", "Ven", "Sam", "Dim"];

// ── Schedule-existing-draft picker ────────────────────────────────────────────

function SchedulePickerDialog({
  date,
  onClose,
  onScheduled,
}: {
  date: Date;
  onClose: () => void;
  onScheduled: () => void;
}) {
  const [time, setTime] = useState("09:00");
  const queryClient = useQueryClient();

  const { data: drafts = [], isLoading } = useQuery({
    queryKey: ["unscheduled_drafts"],
    queryFn: async () => {
      const all = await getPostHistory(50);
      return all.filter((p) => p.status === "draft" && !p.scheduled_at);
    },
  });

  const scheduleMutation = useMutation({
    mutationFn: async (postId: number) => {
      const [hh, mm] = time.split(":").map(Number);
      const target = new Date(date);
      target.setHours(hh, mm, 0, 0);
      await schedulePost(postId, target.toISOString());
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["calendar_posts"] });
      queryClient.invalidateQueries({ queryKey: ["unscheduled_drafts"] });
      queryClient.invalidateQueries({ queryKey: ["post_history"] });
      onScheduled();
    },
  });

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-label="Planifier un brouillon"
    >
      <Card
        className="w-full max-w-lg max-h-[80vh] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        <CardContent className="flex-1 overflow-hidden flex flex-col gap-4 pt-5 pb-4">
          {/* Header */}
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-base font-semibold text-foreground">
                Planifier un brouillon
              </h2>
              <p className="text-xs text-muted-foreground mt-0.5">
                {format(date, "EEEE d MMMM yyyy", { locale: fr })}
              </p>
            </div>
            <button
              type="button"
              onClick={onClose}
              aria-label="Fermer"
              className="text-muted-foreground hover:text-foreground transition-colors"
            >
              <X className="h-4 w-4" />
            </button>
          </div>

          {/* Time picker */}
          <div className="flex items-center gap-2">
            <label htmlFor="schedule-time" className="text-xs font-medium text-muted-foreground">
              Heure :
            </label>
            <input
              id="schedule-time"
              type="time"
              value={time}
              onChange={(e) => setTime(e.target.value)}
              className="rounded-md border border-input bg-background px-2 py-1 text-sm text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
            />
          </div>

          {/* Drafts list */}
          <div className="flex-1 overflow-y-auto -mx-2 px-2">
            {isLoading ? (
              <div className="flex items-center justify-center py-8">
                <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
              </div>
            ) : drafts.length === 0 ? (
              <div className="flex flex-col items-center gap-2 py-8 text-center">
                <p className="text-sm text-muted-foreground">
                  Aucun brouillon non planifié.
                </p>
                <p className="text-xs text-muted-foreground">
                  Crée un post depuis le Composer pour le retrouver ici.
                </p>
              </div>
            ) : (
              <div className="flex flex-col gap-2">
                {drafts.map((draft) => (
                  <button
                    key={draft.id}
                    type="button"
                    onClick={() => scheduleMutation.mutate(draft.id)}
                    disabled={scheduleMutation.isPending}
                    className="flex items-center gap-3 rounded-md border border-border p-2 text-left hover:border-primary hover:bg-secondary/50 transition-colors disabled:opacity-50"
                  >
                    <PostThumbnail post={draft} size="sm" />
                    <div className="flex-1 min-w-0">
                      <p className="text-sm text-foreground line-clamp-2">
                        {draft.caption}
                      </p>
                      <div className="flex items-center gap-2 mt-1">
                        <NetworkBadge network={draft.network} variant="dot" />
                        <span className="text-[10px] text-muted-foreground">
                          {format(new Date(draft.created_at), "d MMM", { locale: fr })}
                        </span>
                      </div>
                    </div>
                    {scheduleMutation.isPending && scheduleMutation.variables === draft.id ? (
                      <Loader2 className="h-3.5 w-3.5 animate-spin text-primary shrink-0" />
                    ) : (
                      <Plus className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                    )}
                  </button>
                ))}
              </div>
            )}
          </div>

          {scheduleMutation.isError && (
            <p className="text-xs text-destructive bg-destructive/10 rounded-md px-3 py-2">
              {String(scheduleMutation.error)}
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

// ── Post detail modal ─────────────────────────────────────────────────────────

function PostModal({
  post,
  onClose,
  onUnschedule,
  onDelete,
  onUpdate,
}: {
  post: PostRecord;
  onClose: () => void;
  onUnschedule: (id: number) => void;
  onDelete: (id: number) => void;
  onUpdate: (updated: PostRecord) => void;
}) {
  const [isRemoving, setIsRemoving] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [editCaption, setEditCaption] = useState(post.caption);
  const [editHashtags, setEditHashtags] = useState(post.hashtags.join(" "));

  // Reschedule controls
  const initialScheduledDate = post.scheduled_at
    ? format(new Date(post.scheduled_at), "yyyy-MM-dd")
    : "";
  const initialScheduledTime = post.scheduled_at
    ? format(new Date(post.scheduled_at), "HH:mm")
    : "09:00";
  const [editDate, setEditDate] = useState(initialScheduledDate);
  const [editTime, setEditTime] = useState(initialScheduledTime);
  const [isRescheduling, setIsRescheduling] = useState(false);

  const handleUnschedule = async () => {
    setIsRemoving(true);
    try {
      await unschedulePost(post.id);
      onUnschedule(post.id);
    } finally {
      setIsRemoving(false);
    }
  };

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

  const handleReschedule = async () => {
    if (!editDate) return;
    const target = parse(`${editDate}T${editTime}`, "yyyy-MM-dd'T'HH:mm", new Date());
    if (!isValid(target)) return;
    setIsRescheduling(true);
    try {
      await schedulePost(post.id, target.toISOString());
      onUpdate({ ...post, scheduled_at: target.toISOString() });
    } finally {
      setIsRescheduling(false);
    }
  };

  const previewImage = post.images?.[0] ?? post.image_path ?? null;
  const isCarousel = (post.images?.length ?? 0) > 1;
  const isDraft = post.status === "draft";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
      onClick={() => {
        if (!isEditing) onClose();
      }}
      role="dialog"
      aria-modal="true"
    >
      <Card
        className="w-full max-w-lg max-h-[90vh] flex flex-col overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <CardContent className="pt-5 pb-4 flex flex-col gap-4 overflow-y-auto">
          {/* Header */}
          <div className="flex items-start justify-between gap-2">
            <div className="flex items-center gap-2 flex-wrap">
              <NetworkBadge network={post.network} />
              {post.status === "published" && (
                <span className="inline-flex items-center rounded-full border border-primary/30 bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
                  Publié
                </span>
              )}
              {post.status === "failed" && (
                <span className="inline-flex items-center rounded-full border border-destructive/30 bg-destructive/10 px-2 py-0.5 text-xs font-medium text-destructive">
                  Échec
                </span>
              )}
            </div>
            <button
              type="button"
              onClick={onClose}
              aria-label="Fermer"
              className="text-muted-foreground hover:text-foreground transition-colors"
            >
              <X className="h-4 w-4" />
            </button>
          </div>

          {/* Image preview */}
          {previewImage && (
            <div>
              {isCarousel ? (
                <div className="flex gap-1.5 overflow-x-auto pb-1">
                  {post.images.map((src, i) => (
                    <img
                      key={i}
                      src={src}
                      alt={`Slide ${i + 1}`}
                      className="h-28 w-28 shrink-0 rounded-md border border-border object-cover"
                      loading="lazy"
                    />
                  ))}
                </div>
              ) : (
                <img
                  src={previewImage}
                  alt=""
                  className="max-h-56 w-full rounded-md border border-border object-contain bg-secondary/30"
                  loading="lazy"
                />
              )}
            </div>
          )}

          {/* Caption — view or edit */}
          {isEditing ? (
            <div className="flex flex-col gap-2">
              <Textarea
                value={editCaption}
                onChange={(e) => setEditCaption(e.target.value)}
                className="text-sm min-h-28 resize-none"
                autoFocus
              />
              <input
                type="text"
                value={editHashtags}
                onChange={(e) => setEditHashtags(e.target.value)}
                placeholder="hashtag1 hashtag2 …"
                className="w-full rounded-md border border-input bg-background px-3 py-1.5 text-xs text-foreground outline-none focus:ring-1 focus:ring-ring"
              />
            </div>
          ) : (
            <>
              <p className="text-sm text-foreground whitespace-pre-line leading-relaxed line-clamp-6">
                {post.caption}
              </p>
              {post.hashtags.length > 0 && (
                <div className="flex flex-wrap gap-1">
                  {post.hashtags.map((t) => (
                    <span key={t} className="text-xs text-primary">
                      #{t}
                    </span>
                  ))}
                </div>
              )}
            </>
          )}

          {/* Reschedule controls — only for drafts already on the calendar */}
          {!isEditing && isDraft && post.scheduled_at && (
            <div className="border-t border-border pt-3 flex flex-col gap-2">
              <p className="text-[11px] font-semibold uppercase tracking-widest text-primary">
                Replanifier
              </p>
              <div className="flex flex-wrap items-center gap-2">
                <input
                  type="date"
                  value={editDate}
                  onChange={(e) => setEditDate(e.target.value)}
                  className="rounded-md border border-input bg-background px-2 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                />
                <input
                  type="time"
                  value={editTime}
                  onChange={(e) => setEditTime(e.target.value)}
                  className="rounded-md border border-input bg-background px-2 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                />
                <Button
                  size="sm"
                  variant="outline"
                  className="h-7 text-xs"
                  onClick={handleReschedule}
                  disabled={isRescheduling || !editDate}
                >
                  {isRescheduling ? <Loader2 className="h-3 w-3 animate-spin" /> : "Mettre à jour"}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-7 text-xs text-muted-foreground"
                  onClick={handleUnschedule}
                  disabled={isRemoving}
                >
                  {isRemoving ? <Loader2 className="h-3 w-3 animate-spin" /> : "Retirer du calendrier"}
                </Button>
              </div>
            </div>
          )}

          {/* Footer */}
          <div className="flex items-center justify-between pt-2 border-t border-border gap-2 flex-wrap">
            <span className="text-xs text-muted-foreground shrink-0">
              {post.scheduled_at
                ? `Planifié · ${format(new Date(post.scheduled_at), "d MMM · HH:mm", { locale: fr })}`
                : `Créé · ${format(new Date(post.created_at), "d MMM · HH:mm", { locale: fr })}`}
            </span>

            {isEditing ? (
              <div className="flex items-center gap-1">
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 text-xs"
                  onClick={() => setIsEditing(false)}
                  disabled={isSaving}
                >
                  Annuler
                </Button>
                <Button
                  size="sm"
                  className="h-7 text-xs gap-1"
                  onClick={handleSaveEdit}
                  disabled={isSaving || !editCaption.trim()}
                >
                  {isSaving ? <Loader2 className="h-3 w-3 animate-spin" /> : <Check className="h-3 w-3" />}
                  Sauvegarder
                </Button>
              </div>
            ) : (
              <div className="flex items-center gap-1">
                {isDraft && (
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 text-xs gap-1 text-muted-foreground hover:text-foreground"
                    onClick={() => setIsEditing(true)}
                  >
                    <Pencil className="h-3 w-3" />
                    Modifier le texte
                  </Button>
                )}
                <PostActions
                  post={post}
                  variant="compact"
                  onDeleted={onDelete}
                  onPublished={(id) => {
                    onUpdate({
                      ...post,
                      status: "published",
                      published_at: new Date().toISOString(),
                    });
                    void id;
                  }}
                />
              </div>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

// ── Day cell (shared between month and week views) ────────────────────────────

function DayCell({
  date,
  isToday,
  isCurrentMonth,
  posts,
  isWeekView,
  onPostClick,
  onScheduleDrop,
  onEmptyClick,
}: {
  date: Date;
  isToday: boolean;
  isCurrentMonth: boolean;
  posts: PostRecord[];
  isWeekView: boolean;
  onPostClick: (post: PostRecord) => void;
  onScheduleDrop: (postId: number, date: Date) => void;
  onEmptyClick: (date: Date) => void;
}) {
  const [isDragOver, setIsDragOver] = useState(false);

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(true);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
    const postId = Number(e.dataTransfer.getData("postId"));
    if (postId) onScheduleDrop(postId, date);
  };

  const maxVisible = isWeekView ? 6 : 3;
  const overflow = posts.length - maxVisible;

  return (
    <div
      className={cn(
        "group relative flex flex-col min-h-0 border-b border-r border-border p-1.5 gap-1 transition-colors",
        isWeekView ? "min-h-32" : "min-h-20",
        !isCurrentMonth && "opacity-40",
        isDragOver && "bg-primary/10",
      )}
      onDragOver={handleDragOver}
      onDragLeave={() => setIsDragOver(false)}
      onDrop={handleDrop}
    >
      <div className="flex items-center justify-between">
        <span
          className={cn(
            "flex h-6 w-6 items-center justify-center rounded-full text-xs font-medium",
            isToday ? "bg-primary text-primary-foreground" : "text-muted-foreground",
          )}
        >
          {date.getDate()}
        </span>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onEmptyClick(date);
          }}
          className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100 text-muted-foreground hover:text-primary transition-opacity"
          aria-label={`Planifier un brouillon le ${date.getDate()}`}
        >
          <Plus className="h-3.5 w-3.5" />
        </button>
      </div>

      <div className="flex flex-col gap-0.5 overflow-hidden">
        {posts.slice(0, maxVisible).map((post) => {
          const status = postStatusForCalendar(post);
          return (
            <button
              key={post.id}
              type="button"
              draggable
              onDragStart={(e) => e.dataTransfer.setData("postId", String(post.id))}
              onClick={() => onPostClick(post)}
              className={cn(
                "flex items-center gap-1 w-full rounded px-1 py-0.5 text-left",
                "border border-transparent hover:border-border hover:bg-secondary/50 transition-colors",
              )}
              title={`${STATUS_PILL_LABEL[status]} · ${post.caption}`}
            >
              <span
                className={cn(
                  "h-1.5 w-1.5 rounded-full shrink-0",
                  NETWORK_DOT_COLORS[post.network] ?? "bg-muted-foreground",
                )}
                aria-hidden="true"
              />
              {post.images?.[0] && (
                <img
                  src={post.images[0]}
                  alt=""
                  className="h-4 w-4 rounded-sm object-cover shrink-0"
                  loading="lazy"
                />
              )}
              {/* Status pill — single source of visual truth on each tile.
                  Lets the user see at a glance whether a slot holds a
                  draft, a scheduled draft, a published post, or a failed
                  attempt, without having to open the detail modal. */}
              <span
                className={cn(
                  "shrink-0 rounded border px-1 text-[8px] uppercase tracking-wider leading-3",
                  STATUS_PILL_CLASS[status],
                )}
                aria-label={STATUS_PILL_LABEL[status]}
              >
                {STATUS_PILL_LABEL[status]}
              </span>
              <span className="text-[10px] leading-4 text-foreground/90 truncate">
                {post.caption}
              </span>
            </button>
          );
        })}
        {overflow > 0 && (
          <span className="text-[9px] text-muted-foreground pl-1">+{overflow} de plus</span>
        )}
      </div>
    </div>
  );
}

// ── Main calendar page ────────────────────────────────────────────────────────

export function CalendarPage() {
  const today = new Date();
  const [view, setView] = useState<"month" | "week">("month");
  const [year, setYear] = useState(today.getFullYear());
  const [month, setMonth] = useState(today.getMonth());
  const [weekStart, setWeekStart] = useState(() => startOfWeek(today));
  const [selectedPost, setSelectedPost] = useState<PostRecord | null>(null);
  const [scheduleForDate, setScheduleForDate] = useState<Date | null>(null);
  const [networkFilter, setNetworkFilter] = useState<string | null>(null);

  const queryClient = useQueryClient();

  const range = useMemo<[string, string]>(
    () =>
      view === "month"
        ? monthRangeISO(year, month)
        : weekRangeISO(weekStart),
    [view, year, month, weekStart],
  );

  const { data: posts = [], isLoading } = useQuery({
    queryKey: ["calendar_posts", range[0], range[1]],
    queryFn: () => getCalendarPosts(range[0], range[1]),
  });

  // Keep keyboard arrow navigation working without competing with form fields.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target?.matches("input, textarea")) return;
      if (e.key === "ArrowLeft") prevPeriod();
      if (e.key === "ArrowRight") nextPeriod();
      if (e.key.toLowerCase() === "t") goToToday();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, year, month, weekStart]);

  const handleScheduleDrop = useCallback(
    async (postId: number, date: Date) => {
      const target = new Date(date);
      target.setHours(9, 0, 0, 0);
      try {
        await schedulePost(postId, target.toISOString());
        queryClient.invalidateQueries({ queryKey: ["calendar_posts"] });
        queryClient.invalidateQueries({ queryKey: ["unscheduled_drafts"] });
      } catch {
        // ignore — error surfaces at the next refresh
      }
    },
    [queryClient],
  );

  // ── Navigation ──

  const prevPeriod = () => {
    if (view === "month") {
      if (month === 0) {
        setYear((y) => y - 1);
        setMonth(11);
      } else setMonth((m) => m - 1);
    } else {
      setWeekStart((ws) => addDays(ws, -7));
    }
  };

  const nextPeriod = () => {
    if (view === "month") {
      if (month === 11) {
        setYear((y) => y + 1);
        setMonth(0);
      } else setMonth((m) => m + 1);
    } else {
      setWeekStart((ws) => addDays(ws, 7));
    }
  };

  const goToToday = () => {
    setYear(today.getFullYear());
    setMonth(today.getMonth());
    setWeekStart(startOfWeek(today));
  };

  // ── Filtering & grouping ──

  const filteredPosts = networkFilter
    ? posts.filter((p) => p.network === networkFilter)
    : posts;

  const postsByDate: Record<string, PostRecord[]> = {};
  for (const post of filteredPosts) {
    const key = getPostDate(post);
    if (!postsByDate[key]) postsByDate[key] = [];
    postsByDate[key].push(post);
  }

  // ── Month grid cells ──

  const firstOfMonth = startOfMonth(year, month);
  const gridStart = startOfWeek(firstOfMonth);
  const cells: Date[] = Array.from({ length: 42 }, (_, i) => addDays(gridStart, i));

  // ── Header text ──

  const headerLabel =
    view === "month"
      ? `${MONTH_NAMES[month]} ${year}`
      : (() => {
          const end = addDays(weekStart, 6);
          if (weekStart.getMonth() === end.getMonth())
            return `${weekStart.getDate()}–${end.getDate()} ${MONTH_NAMES[weekStart.getMonth()]} ${weekStart.getFullYear()}`;
          return `${weekStart.getDate()} ${MONTH_NAMES[weekStart.getMonth()]} – ${end.getDate()} ${MONTH_NAMES[end.getMonth()]} ${end.getFullYear()}`;
        })();

  // ── Week days ──

  const weekDays = Array.from({ length: 7 }, (_, i) => addDays(weekStart, i));

  // ── Available networks for the filter ──
  const availableNetworks = Array.from(new Set(posts.map((p) => p.network)));

  return (
    <div className="flex flex-col h-full p-4 gap-3">
      {/* Toolbar */}
      <div className="flex items-center justify-between flex-wrap gap-2">
        <div>
          <h1 className="text-lg font-semibold text-foreground">Calendrier éditorial</h1>
          <p className="text-xs text-muted-foreground mt-0.5">
            Glisse-dépose pour replanifier · clique <Plus className="inline h-3 w-3" /> pour ajouter un brouillon
          </p>
        </div>
        <div className="flex items-center gap-2 flex-wrap">
          <div className="flex gap-1 p-0.5 bg-secondary/50 rounded-md">
            <button
              type="button"
              onClick={() => setView("month")}
              className={cn(
                "flex items-center gap-1 px-2 py-1 rounded text-xs font-medium transition-colors",
                view === "month"
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              <Calendar className="h-3 w-3" /> Mois
            </button>
            <button
              type="button"
              onClick={() => setView("week")}
              className={cn(
                "flex items-center gap-1 px-2 py-1 rounded text-xs font-medium transition-colors",
                view === "week"
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              <CalendarDays className="h-3 w-3" /> Semaine
            </button>
          </div>

          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={prevPeriod} aria-label="Période précédente">
            <ChevronLeft className="h-4 w-4" />
          </Button>
          <button
            type="button"
            onClick={goToToday}
            className="text-xs font-medium text-foreground hover:text-primary transition-colors min-w-40 text-center"
          >
            {headerLabel}
          </button>
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={nextPeriod} aria-label="Période suivante">
            <ChevronRight className="h-4 w-4" />
          </Button>

          {isLoading && <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />}
        </div>
      </div>

      {/* Network filter */}
      {availableNetworks.length > 1 && (
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-[10px] uppercase tracking-wider text-muted-foreground">
            Filtrer :
          </span>
          <button
            type="button"
            onClick={() => setNetworkFilter(null)}
            className={cn(
              "rounded-full border px-2 py-0.5 text-[10px] transition-colors",
              networkFilter === null
                ? "border-primary bg-primary/10 text-primary"
                : "border-border text-muted-foreground hover:text-foreground",
            )}
          >
            Tous · {posts.length}
          </button>
          {availableNetworks.map((net) => (
            <button
              key={net}
              type="button"
              onClick={() => setNetworkFilter(net === networkFilter ? null : net)}
              className={cn(
                "rounded-full border px-2 py-0.5 text-[10px] flex items-center gap-1 transition-opacity",
                networkFilter && networkFilter !== net && "opacity-40",
              )}
            >
              <NetworkBadge network={net} />
              <span className="text-muted-foreground">
                · {posts.filter((p) => p.network === net).length}
              </span>
            </button>
          ))}
        </div>
      )}

      {/* Calendar grid */}
      <div className="flex-1 overflow-auto rounded-lg border border-border bg-card">
        <div className="grid grid-cols-7 border-b border-border">
          {DAY_LABELS_SHORT.map((d) => (
            <div
              key={d}
              className="py-2 text-center text-xs font-medium text-muted-foreground border-r border-border last:border-r-0"
            >
              {d}
            </div>
          ))}
        </div>

        {view === "month" ? (
          <div
            className="grid grid-cols-7"
            style={{ gridTemplateRows: "repeat(6, minmax(80px, 1fr))" }}
          >
            {cells.map((date) => (
              <DayCell
                key={isoDate(date)}
                date={date}
                isToday={sameDay(date, today)}
                isCurrentMonth={date.getMonth() === month}
                posts={postsByDate[isoDate(date)] ?? []}
                isWeekView={false}
                onPostClick={setSelectedPost}
                onScheduleDrop={handleScheduleDrop}
                onEmptyClick={setScheduleForDate}
              />
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-7">
            {weekDays.map((date) => (
              <DayCell
                key={isoDate(date)}
                date={date}
                isToday={sameDay(date, today)}
                isCurrentMonth={true}
                posts={postsByDate[isoDate(date)] ?? []}
                isWeekView={true}
                onPostClick={setSelectedPost}
                onScheduleDrop={handleScheduleDrop}
                onEmptyClick={setScheduleForDate}
              />
            ))}
          </div>
        )}
      </div>

      {/* Modals */}
      {selectedPost && (
        <PostModal
          post={selectedPost}
          onClose={() => setSelectedPost(null)}
          onUnschedule={() => {
            queryClient.invalidateQueries({ queryKey: ["calendar_posts"] });
            setSelectedPost(null);
          }}
          onDelete={() => {
            queryClient.invalidateQueries({ queryKey: ["calendar_posts"] });
            setSelectedPost(null);
          }}
          onUpdate={(updated) => {
            queryClient.invalidateQueries({ queryKey: ["calendar_posts"] });
            setSelectedPost(updated);
          }}
        />
      )}

      {scheduleForDate && (
        <SchedulePickerDialog
          date={scheduleForDate}
          onClose={() => setScheduleForDate(null)}
          onScheduled={() => setScheduleForDate(null)}
        />
      )}
    </div>
  );
}
