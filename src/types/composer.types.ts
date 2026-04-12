export type Network = "instagram" | "linkedin" | "twitter" | "tiktok";

export interface NetworkMeta {
  label: string;
  captionLimit: number;
  hashtagLimit: number;
  v1: boolean;
}

export const NETWORK_META: Record<Network, NetworkMeta> = {
  instagram: {
    label: "Instagram",
    captionLimit: 2200,
    hashtagLimit: 30,
    v1: true,
  },
  linkedin: { label: "LinkedIn", captionLimit: 3000, hashtagLimit: 5, v1: false },
  twitter: { label: "Twitter / X", captionLimit: 280, hashtagLimit: 2, v1: false },
  tiktok: { label: "TikTok", captionLimit: 2200, hashtagLimit: 100, v1: false },
};

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
