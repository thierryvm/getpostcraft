import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PostDetailSheet } from "./index";
import { invoke } from "@tauri-apps/api/core";
import type { PostRecord } from "@/types/composer.types";
import { renderWithQuery } from "@/test/renderWithQuery";

const mockInvoke = vi.mocked(invoke);

const mockPost: PostRecord = {
  id: 42,
  network: "instagram",
  caption: "Un post sur les alias Linux et leur utilité au quotidien",
  hashtags: ["linux", "terminal", "devops"],
  status: "draft",
  created_at: "2026-04-12T10:00:00.000Z",
  published_at: null,
  scheduled_at: null,
  image_path: null,
  // IG drafts without an image cannot publish inline; tests that exercise the
  // publish button override `images` with at least one entry.
  images: [],
  ig_media_id: null,
  account_id: null,
  published_url: null,
  group_id: null,
};

function renderSheet(overrides: Partial<Parameters<typeof PostDetailSheet>[0]> = {}) {
  const props = {
    post: mockPost,
    onClose: vi.fn(),
    onDelete: vi.fn(),
    onUpdate: vi.fn(),
    ...overrides,
  };
  return { ...renderWithQuery(<PostDetailSheet {...props} />), props };
}

beforeEach(() => {
  vi.clearAllMocks();
});

// ── Affichage ─────────────────────────────────────────────────────────────────

describe("PostDetailSheet — affichage", () => {
  it("n'affiche rien si post est null", () => {
    renderSheet({ post: null });
    expect(screen.queryByText(mockPost.caption)).not.toBeInTheDocument();
  });

  it("affiche la caption du post", async () => {
    renderSheet();
    await waitFor(() => {
      expect(screen.getByText(mockPost.caption)).toBeInTheDocument();
    });
  });

  it("affiche les hashtags du post", async () => {
    renderSheet();
    await waitFor(() => {
      expect(screen.getByText(/#linux/i)).toBeInTheDocument();
    });
  });

  it("affiche le badge Brouillon pour un post draft", async () => {
    renderSheet();
    await waitFor(() => {
      expect(screen.getByText("Brouillon")).toBeInTheDocument();
    });
  });
});

// ── Suppression — flux deux étapes (toggle button) ────────────────────────────
//
// New UX: a single button toggles between "Supprimer" and "Confirmer ?" — no
// separate Annuler button. Clicking outside the sheet (or moving the focus)
// implicitly cancels by leaving the toggle state.

describe("PostDetailSheet — suppression (toggle un seul bouton)", () => {
  it("affiche le bouton Supprimer à l'état initial", async () => {
    renderSheet();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /^supprimer$/i })).toBeInTheDocument();
    });
  });

  it("premier clic → bouton bascule en mode Confirmer ?", async () => {
    const user = userEvent.setup();
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /^supprimer$/i }));
    await user.click(screen.getByRole("button", { name: /^supprimer$/i }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /confirmer/i })).toBeInTheDocument();
    });
  });

  it("Confirmer → appelle delete_post avec le bon id", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue(undefined);
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /^supprimer$/i }));
    await user.click(screen.getByRole("button", { name: /^supprimer$/i }));
    await waitFor(() => screen.getByRole("button", { name: /confirmer/i }));
    await user.click(screen.getByRole("button", { name: /confirmer/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("delete_post", { postId: 42 });
    });
  });

  it("Confirmer succès → appelle onDelete avec l'id du post", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    mockInvoke.mockResolvedValue(undefined);
    renderSheet({ onDelete });

    await waitFor(() => screen.getByRole("button", { name: /^supprimer$/i }));
    await user.click(screen.getByRole("button", { name: /^supprimer$/i }));
    await waitFor(() => screen.getByRole("button", { name: /confirmer/i }));
    await user.click(screen.getByRole("button", { name: /confirmer/i }));

    await waitFor(() => {
      expect(onDelete).toHaveBeenCalledWith(42);
    });
  });

  it("Confirmer échec → n'appelle pas onDelete", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    mockInvoke.mockRejectedValue("error");
    renderSheet({ onDelete });

    await waitFor(() => screen.getByRole("button", { name: /^supprimer$/i }));
    await user.click(screen.getByRole("button", { name: /^supprimer$/i }));
    await waitFor(() => screen.getByRole("button", { name: /confirmer/i }));
    await user.click(screen.getByRole("button", { name: /confirmer/i }));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    expect(onDelete).not.toHaveBeenCalled();
  });
});

// ── Édition ───────────────────────────────────────────────────────────────────

describe("PostDetailSheet — édition (brouillon)", () => {
  it("affiche le bouton 'Modifier le texte' pour un brouillon", async () => {
    renderSheet();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /modifier le texte/i })).toBeInTheDocument();
    });
  });

  it("n'affiche pas le bouton 'Modifier le texte' pour un post publié", async () => {
    renderSheet({ post: { ...mockPost, status: "published" } });
    await waitFor(() => screen.getByText(mockPost.caption));
    expect(screen.queryByRole("button", { name: /modifier le texte/i })).not.toBeInTheDocument();
  });

  it("Modifier → affiche le textarea avec la caption actuelle", async () => {
    const user = userEvent.setup();
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /modifier le texte/i }));
    await user.click(screen.getByRole("button", { name: /modifier le texte/i }));

    await waitFor(() => {
      const textarea = screen.getAllByRole("textbox")[0];
      expect(textarea).toHaveValue(mockPost.caption);
    });
  });

  it("Annuler en mode édition → revient au bouton 'Modifier le texte'", async () => {
    const user = userEvent.setup();
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /modifier le texte/i }));
    await user.click(screen.getByRole("button", { name: /modifier le texte/i }));
    await waitFor(() => screen.getByRole("button", { name: /annuler/i }));
    await user.click(screen.getByRole("button", { name: /annuler/i }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /modifier le texte/i })).toBeInTheDocument();
    });
  });

  it("Sauvegarder appelle update_post_draft avec la nouvelle caption", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue(undefined);
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /modifier le texte/i }));
    await user.click(screen.getByRole("button", { name: /modifier le texte/i }));

    const textarea = await waitFor(() => screen.getAllByRole("textbox")[0]);
    await user.clear(textarea);
    await user.type(textarea, "Nouvelle caption modifiée pour le test");

    await user.click(screen.getByRole("button", { name: /sauvegarder/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "update_post_draft",
        expect.objectContaining({
          postId: 42,
          caption: "Nouvelle caption modifiée pour le test",
        }),
      );
    });
  });
});

// ── Publish action (only IG drafts with an image, or any LinkedIn draft) ──────

describe("PostDetailSheet — publication", () => {
  it("affiche 'Publier maintenant' pour un brouillon LinkedIn texte-seul", async () => {
    renderSheet({ post: { ...mockPost, network: "linkedin" } });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /publier maintenant/i })).toBeInTheDocument();
    });
  });

  it("masque 'Publier maintenant' pour un brouillon Instagram sans image", async () => {
    renderSheet();
    await waitFor(() => screen.getByText(mockPost.caption));
    expect(screen.queryByRole("button", { name: /publier maintenant/i })).not.toBeInTheDocument();
  });

  it("affiche 'Publier maintenant' pour un brouillon Instagram avec image", async () => {
    renderSheet({
      post: { ...mockPost, images: ["data:image/png;base64,XYZ"] },
    });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /publier maintenant/i })).toBeInTheDocument();
    });
  });
});
