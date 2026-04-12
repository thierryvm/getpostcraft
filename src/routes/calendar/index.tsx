import { useState, useEffect, useCallback } from "react";
import { ChevronLeft, ChevronRight, Calendar, CalendarDays, Loader2, X, Pencil, Trash2, Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import { getCalendarPosts, schedulePost, unschedulePost, deletePost, updatePostDraft } from "@/lib/tauri/calendar";
import type { PostRecord } from "@/types/composer.types";
import { NETWORK_META } from "@/types/composer.types";
import { cn } from "@/lib/utils";

// ── Date helpers ──────────────────────────────────────────────────────────────

function isoDate(d: Date): string {
  return d.toISOString().slice(0, 10);
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
  return a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate();
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

function getPostDate(post: PostRecord): string {
  return (post.scheduled_at ?? post.created_at).slice(0, 10);
}

const MONTH_NAMES = [
  "Janvier", "Février", "Mars", "Avril", "Mai", "Juin",
  "Juillet", "Août", "Septembre", "Octobre", "Novembre", "Décembre",
];

const DAY_LABELS_SHORT = ["Lun", "Mar", "Mer", "Jeu", "Ven", "Sam", "Dim"];

const NETWORK_COLORS: Record<string, string> = {
  instagram: "bg-pink-500/20 text-pink-300 border-pink-500/30",
  linkedin: "bg-blue-500/20 text-blue-300 border-blue-500/30",
  twitter: "bg-sky-500/20 text-sky-300 border-sky-500/30",
  tiktok: "bg-purple-500/20 text-purple-300 border-purple-500/30",
};

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
  const [isDeleting, setIsDeleting] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [editCaption, setEditCaption] = useState(post.caption);
  const [editHashtags, setEditHashtags] = useState(post.hashtags.join(" "));

  const isDraft = post.status === "draft";

  const handleUnschedule = async () => {
    setIsRemoving(true);
    try {
      await unschedulePost(post.id);
      onUnschedule(post.id);
    } finally {
      setIsRemoving(false);
    }
  };

  const handleDelete = async () => {
    if (!confirmDelete) { setConfirmDelete(true); return; }
    setIsDeleting(true);
    try {
      await deletePost(post.id);
      onDelete(post.id);
    } finally {
      setIsDeleting(false);
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

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={() => { if (!isEditing) onClose(); }}
    >
      <Card
        className="w-full max-w-md mx-4"
        onClick={(e) => e.stopPropagation()}
      >
        <CardContent className="pt-5 pb-4 flex flex-col gap-3">
          {/* Header row */}
          <div className="flex items-start justify-between gap-2">
            <span
              className={cn(
                "inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium",
                NETWORK_COLORS[post.network] ?? "bg-secondary text-secondary-foreground"
              )}
            >
              {NETWORK_META[post.network as keyof typeof NETWORK_META]?.label ?? post.network}
            </span>
            <button
              type="button"
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground transition-colors"
            >
              <X className="h-4 w-4" />
            </button>
          </div>

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
                    <span key={t} className="text-xs text-primary">#{t}</span>
                  ))}
                </div>
              )}
            </>
          )}

          {/* Footer row */}
          <div className="flex items-center justify-between pt-1 border-t border-border gap-2">
            <span className="text-xs text-muted-foreground shrink-0">
              {post.scheduled_at
                ? `Planifié · ${new Date(post.scheduled_at).toLocaleDateString("fr-FR")}`
                : `Créé · ${new Date(post.created_at).toLocaleDateString("fr-FR")}`}
            </span>

            <div className="flex items-center gap-1">
              {isEditing ? (
                <>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 text-xs"
                    onClick={() => setIsEditing(false)}
                    disabled={isSaving}
                  >
                    Annuler
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 text-xs text-primary hover:text-primary"
                    onClick={handleSaveEdit}
                    disabled={isSaving || !editCaption.trim()}
                  >
                    {isSaving ? <Loader2 className="h-3 w-3 animate-spin" /> : <><Check className="h-3 w-3 mr-1" />Sauvegarder</>}
                  </Button>
                </>
              ) : (
                <>
                  {isDraft && (
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-6 w-6 text-muted-foreground hover:text-foreground"
                      title="Modifier"
                      onClick={() => { setIsEditing(true); setConfirmDelete(false); }}
                    >
                      <Pencil className="h-3 w-3" />
                    </Button>
                  )}
                  {post.scheduled_at && (
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-6 text-xs text-muted-foreground hover:text-foreground"
                      onClick={handleUnschedule}
                      disabled={isRemoving}
                    >
                      {isRemoving ? <Loader2 className="h-3 w-3 animate-spin" /> : "Retirer"}
                    </Button>
                  )}
                  <Button
                    variant="ghost"
                    size="sm"
                    className={cn(
                      "h-6 text-xs",
                      confirmDelete
                        ? "text-destructive hover:text-destructive font-semibold"
                        : "text-muted-foreground hover:text-destructive"
                    )}
                    onClick={handleDelete}
                    disabled={isDeleting}
                    title={confirmDelete ? "Cliquer à nouveau pour confirmer" : "Supprimer"}
                  >
                    {isDeleting
                      ? <Loader2 className="h-3 w-3 animate-spin" />
                      : confirmDelete
                        ? "Confirmer ?"
                        : <Trash2 className="h-3 w-3" />}
                  </Button>
                </>
              )}
            </div>
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
}: {
  date: Date;
  isToday: boolean;
  isCurrentMonth: boolean;
  posts: PostRecord[];
  isWeekView: boolean;
  onPostClick: (post: PostRecord) => void;
  onScheduleDrop?: (postId: number, date: Date) => void;
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
    if (postId && onScheduleDrop) onScheduleDrop(postId, date);
  };

  return (
    <div
      className={cn(
        "flex flex-col min-h-0 border-b border-r border-border p-1.5 gap-1 transition-colors",
        isWeekView ? "min-h-32" : "min-h-20",
        !isCurrentMonth && "opacity-40",
        isDragOver && "bg-primary/10",
      )}
      onDragOver={handleDragOver}
      onDragLeave={() => setIsDragOver(false)}
      onDrop={handleDrop}
    >
      <span
        className={cn(
          "self-start flex h-6 w-6 items-center justify-center rounded-full text-xs font-medium",
          isToday
            ? "bg-primary text-primary-foreground"
            : "text-muted-foreground",
        )}
      >
        {date.getDate()}
      </span>

      <div className="flex flex-col gap-0.5 overflow-hidden">
        {posts.slice(0, isWeekView ? 6 : 3).map((post) => (
          <button
            key={post.id}
            type="button"
            draggable
            onDragStart={(e) => e.dataTransfer.setData("postId", String(post.id))}
            onClick={() => onPostClick(post)}
            className={cn(
              "w-full text-left truncate rounded px-1.5 py-0.5 text-[10px] border",
              "leading-4 hover:opacity-80 transition-opacity cursor-pointer",
              NETWORK_COLORS[post.network] ?? "bg-secondary/50 text-muted-foreground border-border",
            )}
          >
            {post.caption.slice(0, 40)}
          </button>
        ))}
        {posts.length > (isWeekView ? 6 : 3) && (
          <span className="text-[9px] text-muted-foreground pl-1">
            +{posts.length - (isWeekView ? 6 : 3)} de plus
          </span>
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
  const [posts, setPosts] = useState<PostRecord[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [selectedPost, setSelectedPost] = useState<PostRecord | null>(null);

  const loadPosts = useCallback(async () => {
    setIsLoading(true);
    try {
      const [from, to] =
        view === "month"
          ? monthRangeISO(year, month)
          : weekRangeISO(weekStart);
      const data = await getCalendarPosts(from, to);
      setPosts(data);
    } catch {
      // silently fail — calendar is non-critical
    } finally {
      setIsLoading(false);
    }
  }, [view, year, month, weekStart]);

  useEffect(() => {
    loadPosts();
  }, [loadPosts]);

  const handleUnschedule = (id: number) => {
    setPosts((prev) => prev.filter((p) => p.id !== id));
    setSelectedPost(null);
  };

  const handleDelete = (id: number) => {
    setPosts((prev) => prev.filter((p) => p.id !== id));
    setSelectedPost(null);
  };

  const handleUpdate = (updated: PostRecord) => {
    setPosts((prev) => prev.map((p) => (p.id === updated.id ? updated : p)));
    setSelectedPost(updated);
  };

  const handleScheduleDrop = async (postId: number, date: Date) => {
    const iso = date.toISOString().slice(0, 10) + "T09:00:00Z";
    try {
      await schedulePost(postId, iso);
      await loadPosts();
    } catch {
      // ignore
    }
  };

  // ── Navigation ──

  const prevPeriod = () => {
    if (view === "month") {
      if (month === 0) { setYear((y) => y - 1); setMonth(11); }
      else setMonth((m) => m - 1);
    } else {
      setWeekStart((ws) => addDays(ws, -7));
    }
  };

  const nextPeriod = () => {
    if (view === "month") {
      if (month === 11) { setYear((y) => y + 1); setMonth(0); }
      else setMonth((m) => m + 1);
    } else {
      setWeekStart((ws) => addDays(ws, 7));
    }
  };

  const goToToday = () => {
    setYear(today.getFullYear());
    setMonth(today.getMonth());
    setWeekStart(startOfWeek(today));
  };

  // Group posts by ISO date
  const postsByDate: Record<string, PostRecord[]> = {};
  for (const post of posts) {
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

  return (
    <div className="flex flex-col h-full p-4 gap-3">
      {/* Toolbar */}
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold text-foreground">Calendrier éditorial</h1>
        <div className="flex items-center gap-2">
          {/* View toggle */}
          <div className="flex gap-1 p-0.5 bg-secondary/50 rounded-md">
            <button
              type="button"
              onClick={() => setView("month")}
              className={cn(
                "flex items-center gap-1 px-2 py-1 rounded text-xs font-medium transition-colors",
                view === "month"
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              <Calendar className="h-3 w-3" />
              Mois
            </button>
            <button
              type="button"
              onClick={() => setView("week")}
              className={cn(
                "flex items-center gap-1 px-2 py-1 rounded text-xs font-medium transition-colors",
                view === "week"
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              <CalendarDays className="h-3 w-3" />
              Semaine
            </button>
          </div>

          {/* Nav */}
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={prevPeriod}>
            <ChevronLeft className="h-4 w-4" />
          </Button>
          <button
            type="button"
            onClick={goToToday}
            className="text-xs font-medium text-foreground hover:text-primary transition-colors min-w-40 text-center"
          >
            {headerLabel}
          </button>
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={nextPeriod}>
            <ChevronRight className="h-4 w-4" />
          </Button>

          {isLoading && <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />}
        </div>
      </div>

      {/* Legend */}
      <div className="flex items-center gap-3 flex-wrap">
        {Object.entries(NETWORK_COLORS).map(([net, cls]) => (
          <span key={net} className={cn("flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-medium", cls)}>
            {NETWORK_META[net as keyof typeof NETWORK_META]?.label ?? net}
          </span>
        ))}
        <span className="text-[10px] text-muted-foreground ml-auto">
          Glisse-dépose un post pour le déplacer
        </span>
      </div>

      {/* Calendar grid */}
      <div className="flex-1 overflow-auto rounded-lg border border-border bg-card">
        {/* Day headers */}
        <div className="grid grid-cols-7 border-b border-border">
          {DAY_LABELS_SHORT.map((d) => (
            <div key={d} className="py-2 text-center text-xs font-medium text-muted-foreground border-r border-border last:border-r-0">
              {d}
            </div>
          ))}
        </div>

        {view === "month" ? (
          /* Month grid — 6 rows × 7 cols */
          <div className="grid grid-cols-7" style={{ gridTemplateRows: "repeat(6, minmax(80px, 1fr))" }}>
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
              />
            ))}
          </div>
        ) : (
          /* Week grid — 1 row × 7 cols */
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
              />
            ))}
          </div>
        )}
      </div>

      {/* Post detail modal */}
      {selectedPost && (
        <PostModal
          post={selectedPost}
          onClose={() => setSelectedPost(null)}
          onUnschedule={handleUnschedule}
          onDelete={handleDelete}
          onUpdate={handleUpdate}
        />
      )}
    </div>
  );
}
