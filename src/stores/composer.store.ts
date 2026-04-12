import { create } from "zustand";
import type { GeneratedContent, Network } from "@/types/composer.types";
import type { CaptionVariant } from "@/lib/tauri/composer";

interface ComposerState {
  brief: string;
  network: Network;
  result: GeneratedContent | null;
  variants: CaptionVariant[] | null;
  isLoading: boolean;
  error: string | null;
  /** ID of the last draft saved to DB — used by publishPost */
  draftId: number | null;
  setBrief: (brief: string) => void;
  setNetwork: (network: Network) => void;
  setResult: (result: GeneratedContent | null) => void;
  setVariants: (variants: CaptionVariant[] | null) => void;
  setIsLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  setDraftId: (id: number | null) => void;
}

export const useComposerStore = create<ComposerState>((set) => ({
  brief: "",
  network: "instagram",
  result: null,
  variants: null,
  isLoading: false,
  error: null,
  draftId: null,
  setBrief: (brief) => set({ brief }),
  setNetwork: (network) => set({ network }),
  setResult: (result) => set({ result, variants: null }),
  setVariants: (variants) => set({ variants, result: null }),
  setIsLoading: (isLoading) => set({ isLoading }),
  setError: (error) => set({ error }),
  setDraftId: (draftId) => set({ draftId }),
}));
