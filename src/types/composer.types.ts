export type Network = "instagram" | "linkedin" | "twitter" | "tiktok";

export interface NetworkMeta {
  label: string;
  captionLimit: number;
  hashtagLimit: number;
  /** Characters visible before "Voir plus" / "...more" in the feed. 0 = no fold. */
  foldLimit: number;
  v1: boolean;
}

export const NETWORK_META: Record<Network, NetworkMeta> = {
  instagram: {
    label: "Instagram",
    captionLimit: 2200,
    hashtagLimit: 30,
    foldLimit: 125,
    v1: true,
  },
  linkedin: { label: "LinkedIn", captionLimit: 3000, hashtagLimit: 5, foldLimit: 140, v1: true },
  twitter: { label: "Twitter / X", captionLimit: 280, hashtagLimit: 2, foldLimit: 0, v1: false },
  tiktok: { label: "TikTok", captionLimit: 2200, hashtagLimit: 100, foldLimit: 0, v1: false },
};

export interface ImageFormat {
  id: string;
  label: string;
  width: number;
  height: number;
  /** Human-readable ratio, e.g. "4:5" */
  ratio: string;
}

export const FORMATS_BY_NETWORK: Record<Network, ImageFormat[]> = {
  instagram: [
    { id: "portrait", label: "Portrait 4:5", width: 1080, height: 1350, ratio: "4:5" },
    { id: "square",   label: "Carré 1:1",    width: 1080, height: 1080, ratio: "1:1" },
    { id: "landscape",label: "Paysage 1.91:1",width: 1080,height: 566,  ratio: "1.91:1" },
  ],
  linkedin: [
    { id: "landscape",label: "Bannière 1.91:1",width: 1200,height: 628, ratio: "1.91:1" },
    { id: "square",   label: "Carré 1:1",    width: 1080, height: 1080, ratio: "1:1" },
  ],
  twitter: [
    { id: "landscape",label: "Paysage 16:9", width: 1200, height: 675,  ratio: "16:9" },
    { id: "square",   label: "Carré 1:1",    width: 1080, height: 1080, ratio: "1:1" },
  ],
  tiktok: [
    { id: "portrait", label: "Portrait 9:16",width: 1080, height: 1920, ratio: "9:16" },
  ],
};

export function getDefaultFormat(network: Network): ImageFormat {
  return FORMATS_BY_NETWORK[network][0];
}

export interface Brief {
  network: Network;
  brief: string;
}

export interface GeneratedContent {
  caption: string;
  hashtags: string[];
}

export interface PostRecord {
  id: number;
  network: Network;
  caption: string;
  hashtags: string[];
  status: "draft" | "published" | "failed";
  created_at: string;
  published_at: string | null;
  scheduled_at: string | null;
}
