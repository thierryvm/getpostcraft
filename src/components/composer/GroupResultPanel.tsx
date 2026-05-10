import { useState, useMemo } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import {
  CheckCircle2,
  AlertTriangle,
  ExternalLink,
  Layers,
  RefreshCw,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useComposerStore } from "@/stores/composer.store";
import {
  generateAndSaveGroup,
  type GroupMemberResult,
} from "@/lib/tauri/composer";
import { NETWORK_META, type Network } from "@/types/composer.types";
import { NETWORK_DOT_COLORS } from "@/components/shared/NetworkBadge";

/**
 * Compact summary of a multi-network generation. The Composer route
 * renders this whenever the store holds a `groupResult` instead of the
 * single-network ContentPreview.
 *
 * Why a separate panel rather than extending ContentPreview to a tab
 * mode: ContentPreview is 1000+ lines of image rendering / inline edit
 * / publish-flow logic that's all built around a single PostRecord. The
 * multi-network composer's job is to *create* the sibling drafts in
 * one pass — the rich preview / edit experience for each individual
 * post belongs to the single-network view, reachable via "Continuer
 * sur ce réseau" on each member tile here.
 *
 * The panel surfaces:
 *   - Per-member status (ok / error) with a coloured tile + caption
 *     preview + post id.
 *   - "Continuer sur Instagram" / "Continuer sur LinkedIn" actions
 *     that load the member as the active draft (single-network mode)
 *     so the user lands in the familiar ContentPreview to render
 *     images, edit captions, and publish.
 *   - "Recommencer" to clear the group and re-prompt.
 *   - Inline retry on failed members — re-runs `generateAndSaveGroup`
 *     for just that network and merges the result back.
 */
export function GroupResultPanel() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const groupResult = useComposerStore((s) => s.groupResult);
  const brief = useComposerStore((s) => s.brief);
  const accountIds = useComposerStore((s) => s.accountIds);
  const setGroupResult = useComposerStore((s) => s.setGroupResult);
  const setDraftId = useComposerStore((s) => s.setDraftId);
  const resetForNewPost = useComposerStore((s) => s.resetForNewPost);

  const [retrying, setRetrying] = useState<Network | null>(null);
  const [retryError, setRetryError] = useState<string | null>(null);

  if (!groupResult) return null;

  const okMembers = useMemo(
    () => groupResult.members.filter((m) => m.status === "ok"),
    [groupResult],
  );

  /** Re-run generation for a single failed network and merge its result. */
  const handleRetry = async (network: Network) => {
    setRetrying(network);
    setRetryError(null);
    try {
      const partial = await generateAndSaveGroup(brief, [
        { network, account_id: accountIds[network] ?? null },
      ]);
      // Merge: replace the failed member entry with the fresh outcome.
      // The retry uses a fresh group_id (one network = one group of 1
      // in the backend), so we keep the original group's id but adopt
      // the new member's post_id.
      const newMember = partial.members.find((m) => m.network === network);
      if (newMember) {
        const merged = {
          ...groupResult,
          members: groupResult.members.map((m) =>
            m.network === network ? newMember : m,
          ),
        };
        setGroupResult(merged);
        if (newMember.status === "ok" && newMember.post_id !== null) {
          // Refresh the draft list so the dashboard sees the new sibling
          // immediately without waiting for the next mount.
          queryClient.invalidateQueries({ queryKey: ["post_history"] });
          queryClient.invalidateQueries({ queryKey: ["calendar_posts"] });
        }
      }
    } catch (err) {
      setRetryError(`Retry ${network} : ${String(err)}`);
    } finally {
      setRetrying(null);
    }
  };

  /** Load a member as the active single-network draft and re-route the
   *  composer state so the user lands in the rich ContentPreview. */
  const handleContinueSingle = (member: GroupMemberResult) => {
    if (member.status !== "ok" || member.post_id === null) return;
    // Clear the group state and pin the chosen post as the draft to
    // reload — ContentPreview's mount effect picks it up via
    // `pendingDraftId`.
    resetForNewPost();
    useComposerStore.setState({
      pendingDraftId: member.post_id,
      network: member.network,
      selectedNetworks: new Set([member.network]),
      accountIds: { [member.network]: accountIds[member.network] ?? null },
      accountId: accountIds[member.network] ?? null,
    });
    setDraftId(member.post_id);
    navigate({ to: "/composer" });
  };

  return (
    <div className="flex flex-col gap-4">
      {/* Header with overall summary */}
      <div className="flex items-center justify-between gap-2 flex-wrap">
        <div className="flex items-center gap-2">
          <Layers className="h-4 w-4 text-primary" aria-hidden="true" />
          <h2 className="text-base font-semibold text-foreground">
            Groupe multi-réseau
          </h2>
          {groupResult.group_id !== null && (
            <Badge variant="outline" className="text-xs font-mono text-muted-foreground">
              #{groupResult.group_id}
            </Badge>
          )}
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs text-muted-foreground">
            <span className="text-primary font-medium">{okMembers.length}</span>
            {" / "}
            {groupResult.members.length} générés
          </span>
          <Button
            variant="ghost"
            size="sm"
            onClick={resetForNewPost}
            className="h-7 text-xs"
          >
            Recommencer
          </Button>
        </div>
      </div>

      {retryError && (
        <p className="text-xs text-destructive bg-destructive/10 rounded-md px-3 py-2">
          {retryError}
        </p>
      )}

      {/* Per-network member tiles. Order matches the user's checkbox
          selection (the backend preserves it through the parallel call
          collection step), so the leftmost tile is always the first
          network the user ticked. */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
        {groupResult.members.map((member) => {
          const meta = NETWORK_META[member.network];
          const isOk = member.status === "ok";
          return (
            <Card
              key={member.network}
              className={`border ${
                isOk
                  ? "border-border"
                  : "border-destructive/30 bg-destructive/5"
              }`}
            >
              <CardContent className="pt-4 pb-3 flex flex-col gap-3">
                <div className="flex items-center justify-between gap-2 flex-wrap">
                  <div className="flex items-center gap-2">
                    <span
                      className={`h-2 w-2 rounded-full ${
                        NETWORK_DOT_COLORS[member.network] ?? "bg-muted-foreground"
                      }`}
                      aria-hidden="true"
                    />
                    <span className="text-sm font-medium text-foreground">
                      {meta.label}
                    </span>
                    {isOk ? (
                      <Badge
                        variant="outline"
                        className="text-[10px] border-primary/30 bg-primary/10 text-primary gap-1"
                      >
                        <CheckCircle2 className="h-3 w-3" aria-hidden="true" />
                        Brouillon
                      </Badge>
                    ) : (
                      <Badge
                        variant="outline"
                        className="text-[10px] border-destructive/30 bg-destructive/10 text-destructive gap-1"
                      >
                        <AlertTriangle className="h-3 w-3" aria-hidden="true" />
                        Échec
                      </Badge>
                    )}
                  </div>
                  {isOk && member.post_id !== null && (
                    <span className="text-[10px] font-mono text-muted-foreground">
                      #{member.post_id}
                    </span>
                  )}
                </div>

                {isOk ? (
                  <>
                    <p className="text-xs text-foreground/90 line-clamp-4 leading-relaxed whitespace-pre-line">
                      {member.caption}
                    </p>
                    {member.hashtags && member.hashtags.length > 0 && (
                      <div className="flex flex-wrap gap-1">
                        {member.hashtags.slice(0, 6).map((tag) => (
                          <span
                            key={tag}
                            className="text-[10px] text-primary/80"
                          >
                            #{tag}
                          </span>
                        ))}
                        {member.hashtags.length > 6 && (
                          <span className="text-[10px] text-muted-foreground">
                            +{member.hashtags.length - 6}
                          </span>
                        )}
                      </div>
                    )}
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-7 text-xs gap-1.5 self-start"
                      onClick={() => handleContinueSingle(member)}
                    >
                      <ExternalLink className="h-3 w-3" />
                      Continuer sur {meta.label}
                    </Button>
                  </>
                ) : (
                  <>
                    <p className="text-xs text-destructive font-mono break-all">
                      {member.error_message ?? "Erreur inconnue"}
                    </p>
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-7 text-xs gap-1.5 self-start"
                      onClick={() => handleRetry(member.network)}
                      disabled={retrying === member.network}
                    >
                      {retrying === member.network ? (
                        <>
                          <RefreshCw className="h-3 w-3 animate-spin" />
                          Réessai…
                        </>
                      ) : (
                        <>
                          <RefreshCw className="h-3 w-3" />
                          Réessayer {meta.label}
                        </>
                      )}
                    </Button>
                  </>
                )}
              </CardContent>
            </Card>
          );
        })}
      </div>

      {okMembers.length > 0 && (
        <p className="text-xs text-muted-foreground leading-snug">
          Les brouillons sont sauvegardés et liés par le groupe #
          {groupResult.group_id ?? "—"}. Clique « Continuer sur {NETWORK_META[okMembers[0].network].label} »
          pour générer l'image et publier sur ce réseau, ou retrouve-les dans le
          tableau de bord.
        </p>
      )}
    </div>
  );
}
