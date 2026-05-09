import { ImageOff, Layers } from "lucide-react";
import { cn } from "@/lib/utils";
import type { PostRecord } from "@/types/composer.types";
import { NETWORK_DOT_COLORS } from "./NetworkBadge";

interface PostThumbnailProps {
  post: Pick<PostRecord, "images" | "image_path" | "network">;
  size?: "sm" | "md" | "lg";
  className?: string;
}

const SIZE_CLASSES: Record<NonNullable<PostThumbnailProps["size"]>, string> = {
  sm: "h-12 w-12",
  md: "h-16 w-16",
  lg: "h-24 w-24",
};

/**
 * Renders the first image of a post as a square thumbnail. Falls back to a
 * neutral placeholder with a network-colored corner dot when the post has no
 * media (text-only LinkedIn drafts). Carousels show a stacked-layers icon
 * overlay so the user knows there's more than one slide.
 */
export function PostThumbnail({ post, size = "md", className }: PostThumbnailProps) {
  const src = post.images?.[0] ?? post.image_path ?? null;
  const isCarousel = (post.images?.length ?? 0) > 1;
  const sizeClass = SIZE_CLASSES[size];

  return (
    <div
      className={cn(
        "relative shrink-0 overflow-hidden rounded-md border border-border bg-secondary/40",
        sizeClass,
        className,
      )}
    >
      {src ? (
        <img
          src={src}
          alt=""
          className="h-full w-full object-cover"
          loading="lazy"
          draggable={false}
        />
      ) : (
        <div className="flex h-full w-full items-center justify-center text-muted-foreground/60">
          <ImageOff className="h-4 w-4" aria-hidden="true" />
        </div>
      )}

      {isCarousel && (
        <span className="absolute right-1 top-1 inline-flex items-center justify-center rounded-sm bg-black/70 px-1 py-0.5 text-[9px] font-mono text-white">
          <Layers className="mr-0.5 h-2.5 w-2.5" aria-hidden="true" />
          {post.images.length}
        </span>
      )}

      <span
        className={cn(
          "absolute bottom-1 left-1 h-1.5 w-1.5 rounded-full",
          NETWORK_DOT_COLORS[post.network] ?? "bg-muted-foreground",
        )}
        aria-hidden="true"
      />
    </div>
  );
}
