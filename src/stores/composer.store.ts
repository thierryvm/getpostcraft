import { create } from "zustand";
import type { GeneratedContent, Network, ImageFormat } from "@/types/composer.types";
import { getDefaultFormat } from "@/types/composer.types";
import type { CaptionVariant, GroupGenerationResult } from "@/lib/tauri/composer";

interface ComposerState {
  brief: string;
  /**
   * The network the Composer treats as "primary" — drives the image
   * format default, the recommendedLimit hints in the brief textarea,
   * and which account the legacy single-network flow targets. In
   * multi-network mode it's the first ticked checkbox; the rest live
   * in `selectedNetworks` and `accountIds`.
   */
  network: Network;
  /**
   * Multi-network selection (v0.3.9). Always contains at least
   * `network` so the legacy single-network code paths can read either
   * field interchangeably. When `size === 1`, the Composer routes to
   * the existing `generateContent` command; when `size >= 2`, it
   * routes to `generateAndSaveGroup`.
   */
  selectedNetworks: Set<Network>;
  /** ID of the connected account to generate for — injects product_truth into prompt. */
  accountId: number | null;
  /**
   * Per-network account ids for multi-network generation. Mirrors
   * `accountId` for the primary network so existing UI code that
   * reads `accountId` keeps working. Networks not present default
   * to `null` (no account → no Product Truth injection).
   */
  accountIds: Partial<Record<Network, number | null>>;
  imageFormat: ImageFormat;
  result: GeneratedContent | null;
  variants: CaptionVariant[] | null;
  /**
   * Outcome of a multi-network generation. Mutually exclusive with
   * `result` — setting one clears the other. The Composer preview
   * panel switches between the legacy single view and the tabbed
   * group view based on which one is set.
   */
  groupResult: GroupGenerationResult | null;
  isLoading: boolean;
  error: string | null;
  /** ID of the last draft saved to DB — used by publishPost */
  draftId: number | null;
  /**
   * When the calendar/history view wants to reopen a draft in the composer,
   * it sets this to the draft id and navigates to /composer. ContentPreview
   * picks it up on mount, fetches the post, populates state, then clears it.
   * Cross-route plumbing without router state, kept simple.
   */
  pendingDraftId: number | null;
  setBrief: (brief: string) => void;
  setNetwork: (network: Network) => void;
  toggleNetwork: (network: Network) => void;
  setAccountId: (id: number | null) => void;
  setAccountIdFor: (network: Network, id: number | null) => void;
  setImageFormat: (format: ImageFormat) => void;
  setResult: (result: GeneratedContent | null) => void;
  setVariants: (variants: CaptionVariant[] | null) => void;
  setGroupResult: (result: GroupGenerationResult | null) => void;
  setIsLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  setDraftId: (id: number | null) => void;
  setPendingDraftId: (id: number | null) => void;
  /**
   * Reset everything that's "the post we're working on" but keep the user's
   * current network + account selection so they can start a fresh draft on
   * the same target without re-picking it. Used by the "Nouveau post" button
   * to break out of an opened-draft session without forcing a publish.
   */
  resetForNewPost: () => void;
}

export const useComposerStore = create<ComposerState>((set) => ({
  brief: "",
  network: "instagram",
  selectedNetworks: new Set<Network>(["instagram"]),
  accountId: null,
  accountIds: { instagram: null },
  imageFormat: getDefaultFormat("instagram"),
  result: null,
  variants: null,
  groupResult: null,
  isLoading: false,
  error: null,
  draftId: null,
  pendingDraftId: null,
  setBrief: (brief) => set({ brief }),
  setNetwork: (network) =>
    set((s) => {
      // Switching the primary network always re-aligns the multi-select
      // so the legacy `network` field and the new `selectedNetworks` set
      // can never disagree. The image format follows the new primary.
      const nextSelected = new Set<Network>([network]);
      const nextAccountIds: Partial<Record<Network, number | null>> = {
        [network]: s.accountIds[network] ?? null,
      };
      return {
        network,
        selectedNetworks: nextSelected,
        accountId: nextAccountIds[network] ?? null,
        accountIds: nextAccountIds,
        imageFormat: getDefaultFormat(network),
      };
    }),
  toggleNetwork: (network) =>
    set((s) => {
      const next = new Set(s.selectedNetworks);
      if (next.has(network)) {
        // Refuse to drop the last network — at least one must stay
        // checked so the form has something to submit to.
        if (next.size === 1) return s;
        next.delete(network);
      } else {
        // V1 ceiling: the parallel sidecar fan-out is hard-capped at 3
        // networks per group on the Rust side; the UI mirrors that
        // limit so the user gets immediate feedback instead of a
        // backend rejection after they've already typed the brief.
        if (next.size >= 3) return s;
        next.add(network);
      }
      // Keep the primary network valid: if we just dropped the current
      // primary, promote the first remaining network so `network` and
      // `selectedNetworks` never disagree.
      const primary: Network = next.has(s.network)
        ? s.network
        : (next.values().next().value as Network);
      const nextAccountIds: Partial<Record<Network, number | null>> = {};
      for (const net of next) {
        nextAccountIds[net] = s.accountIds[net] ?? null;
      }
      return {
        network: primary,
        selectedNetworks: next,
        accountId: nextAccountIds[primary] ?? null,
        accountIds: nextAccountIds,
        imageFormat: primary === s.network ? s.imageFormat : getDefaultFormat(primary),
      };
    }),
  setAccountId: (accountId) =>
    set((s) => ({
      accountId,
      accountIds: { ...s.accountIds, [s.network]: accountId },
    })),
  setAccountIdFor: (network, id) =>
    set((s) => ({
      accountIds: { ...s.accountIds, [network]: id },
      // Mirror to the legacy `accountId` whenever we update the primary
      // network so the existing single-network code paths stay in sync.
      accountId: network === s.network ? id : s.accountId,
    })),
  setImageFormat: (imageFormat) => set({ imageFormat }),
  setResult: (result) => set({ result, variants: null, groupResult: null }),
  setVariants: (variants) => set({ variants, result: null, groupResult: null }),
  setGroupResult: (groupResult) => set({ groupResult, result: null, variants: null }),
  setIsLoading: (isLoading) => set({ isLoading }),
  setError: (error) => set({ error }),
  setDraftId: (draftId) => set({ draftId }),
  setPendingDraftId: (pendingDraftId) => set({ pendingDraftId }),
  resetForNewPost: () =>
    set((s) => ({
      brief: "",
      result: null,
      variants: null,
      groupResult: null,
      isLoading: false,
      error: null,
      draftId: null,
      pendingDraftId: null,
      // Reset image format to the primary network's default — switching
      // IG↔LinkedIn would otherwise stick with the previous draft's format.
      imageFormat: getDefaultFormat(s.network),
      // network + selectedNetworks + accountIds intentionally preserved.
    })),
}));
