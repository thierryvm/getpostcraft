import { create } from "zustand";
import type { GeneratedContent, Network, ImageFormat } from "@/types/composer.types";
import { getDefaultFormat } from "@/types/composer.types";
import type { CaptionVariant } from "@/lib/tauri/composer";

interface ComposerState {
  brief: string;
  network: Network;
  /** ID of the connected account to generate for — injects product_truth into prompt. */
  accountId: number | null;
  imageFormat: ImageFormat;
  result: GeneratedContent | null;
  variants: CaptionVariant[] | null;
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
  setAccountId: (id: number | null) => void;
  setImageFormat: (format: ImageFormat) => void;
  setResult: (result: GeneratedContent | null) => void;
  setVariants: (variants: CaptionVariant[] | null) => void;
  setIsLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  setDraftId: (id: number | null) => void;
  setPendingDraftId: (id: number | null) => void;
}

export const useComposerStore = create<ComposerState>((set) => ({
  brief: "",
  network: "instagram",
  accountId: null,
  imageFormat: getDefaultFormat("instagram"),
  result: null,
  variants: null,
  isLoading: false,
  error: null,
  draftId: null,
  pendingDraftId: null,
  setBrief: (brief) => set({ brief }),
  setNetwork: (network) => set({ network, imageFormat: getDefaultFormat(network) }),
  setAccountId: (accountId) => set({ accountId }),
  setImageFormat: (imageFormat) => set({ imageFormat }),
  setResult: (result) => set({ result, variants: null }),
  setVariants: (variants) => set({ variants, result: null }),
  setIsLoading: (isLoading) => set({ isLoading }),
  setError: (error) => set({ error }),
  setDraftId: (draftId) => set({ draftId }),
  setPendingDraftId: (pendingDraftId) => set({ pendingDraftId }),
}));
