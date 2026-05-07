import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PostDetailSheet } from "./index";
import { invoke } from "@tauri-apps/api/core";
import type { PostRecord } from "@/types/composer.types";

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
  images: [],
  ig_media_id: null,
  account_id: null,
};

function renderSheet(overrides: Partial<Parameters<typeof PostDetailSheet>[0]> = {}) {
  const props = {
    post: mockPost,
    onClose: vi.fn(),
    onDelete: vi.fn(),
    onUpdate: vi.fn(),
    ...overrides,
  };
  return { ...render(<PostDetailSheet {...props} />), props };
}

beforeEach(() => {
  vi.clearAllMocks();
});

// ── Affichage ─────────────────────────────────────────────────────────────────

describe("PostDetailSheet — affichage", () => {
  it("n'affiche rien si post est null", () => {
    const { container } = renderSheet({ post: null });
    // Sheet is closed — nothing in the accessible DOM
    expect(screen.queryByText(mockPost.caption)).not.toBeInTheDocument();
    expect(container).toBeDefined(); // component mounts without crash
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

// ── Suppression — flux deux étapes ────────────────────────────────────────────

describe("PostDetailSheet — suppression (flux deux étapes)", () => {
  it("affiche le bouton Supprimer à l'état initial", async () => {
    renderSheet();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /supprimer/i })).toBeInTheDocument();
    });
  });

  it("premier clic → mode confirmation : boutons Confirmer + Annuler visibles", async () => {
    const user = userEvent.setup();
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /supprimer/i }));
    await user.click(screen.getByRole("button", { name: /supprimer/i }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /confirmer/i })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: /annuler/i })).toBeInTheDocument();
    });
  });

  it("premier clic → le bouton Supprimer d'origine disparaît", async () => {
    const user = userEvent.setup();
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /supprimer/i }));
    await user.click(screen.getByRole("button", { name: /supprimer/i }));

    await waitFor(() => {
      // "Supprimer" ne doit plus apparaître comme bouton autonome
      expect(screen.queryByRole("button", { name: /^supprimer$/i })).not.toBeInTheDocument();
    });
  });

  it("Annuler en mode confirm → revient au bouton Supprimer initial", async () => {
    const user = userEvent.setup();
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /supprimer/i }));
    await user.click(screen.getByRole("button", { name: /supprimer/i }));
    await waitFor(() => screen.getByRole("button", { name: /annuler/i }));
    await user.click(screen.getByRole("button", { name: /annuler/i }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /supprimer/i })).toBeInTheDocument();
    });
    expect(screen.queryByRole("button", { name: /confirmer/i })).not.toBeInTheDocument();
  });

  it("Confirmer → appelle delete_post avec le bon id", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue(undefined);
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /supprimer/i }));
    await user.click(screen.getByRole("button", { name: /supprimer/i }));
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

    await waitFor(() => screen.getByRole("button", { name: /supprimer/i }));
    await user.click(screen.getByRole("button", { name: /supprimer/i }));
    await waitFor(() => screen.getByRole("button", { name: /confirmer/i }));
    await user.click(screen.getByRole("button", { name: /confirmer/i }));

    await waitFor(() => {
      expect(onDelete).toHaveBeenCalledWith(42);
    });
  });

  it("Confirmer échec → affiche le message d'erreur", async () => {
    const user = userEvent.setup();
    mockInvoke.mockRejectedValue("Database error: foreign key constraint");
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /supprimer/i }));
    await user.click(screen.getByRole("button", { name: /supprimer/i }));
    await waitFor(() => screen.getByRole("button", { name: /confirmer/i }));
    await user.click(screen.getByRole("button", { name: /confirmer/i }));

    await waitFor(() => {
      expect(screen.getByText(/database error/i)).toBeInTheDocument();
    });
  });

  it("Confirmer échec → revient au bouton Supprimer (pas bloqué en mode confirm)", async () => {
    const user = userEvent.setup();
    mockInvoke.mockRejectedValue("error");
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /supprimer/i }));
    await user.click(screen.getByRole("button", { name: /supprimer/i }));
    await waitFor(() => screen.getByRole("button", { name: /confirmer/i }));
    await user.click(screen.getByRole("button", { name: /confirmer/i }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /supprimer/i })).toBeInTheDocument();
    });
  });

  it("Confirmer échec → n'appelle pas onDelete", async () => {
    const user = userEvent.setup();
    const onDelete = vi.fn();
    mockInvoke.mockRejectedValue("error");
    renderSheet({ onDelete });

    await waitFor(() => screen.getByRole("button", { name: /supprimer/i }));
    await user.click(screen.getByRole("button", { name: /supprimer/i }));
    await waitFor(() => screen.getByRole("button", { name: /confirmer/i }));
    await user.click(screen.getByRole("button", { name: /confirmer/i }));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    expect(onDelete).not.toHaveBeenCalled();
  });
});

// ── Édition ───────────────────────────────────────────────────────────────────

describe("PostDetailSheet — édition (brouillon)", () => {
  it("affiche le bouton Modifier pour un brouillon", async () => {
    renderSheet();
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /modifier/i })).toBeInTheDocument();
    });
  });

  it("n'affiche pas le bouton Modifier pour un post publié", async () => {
    renderSheet({ post: { ...mockPost, status: "published" } });
    await waitFor(() => screen.getByText(mockPost.caption));
    expect(screen.queryByRole("button", { name: /modifier/i })).not.toBeInTheDocument();
  });

  it("Modifier → affiche le champ textarea avec la caption actuelle", async () => {
    const user = userEvent.setup();
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /modifier/i }));
    await user.click(screen.getByRole("button", { name: /modifier/i }));

    await waitFor(() => {
      // Edit mode shows two textboxes: textarea (caption) + input (hashtags)
      const textarea = screen.getAllByRole("textbox")[0];
      expect(textarea).toHaveValue(mockPost.caption);
    });
  });

  it("Annuler en mode édition → revient à l'affichage normal", async () => {
    const user = userEvent.setup();
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /modifier/i }));
    await user.click(screen.getByRole("button", { name: /modifier/i }));
    await waitFor(() => screen.getByRole("button", { name: /annuler/i }));
    await user.click(screen.getByRole("button", { name: /annuler/i }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /modifier/i })).toBeInTheDocument();
    });
  });

  it("Sauvegarder appelle update_post_draft avec la nouvelle caption", async () => {
    const user = userEvent.setup();
    mockInvoke.mockResolvedValue(undefined);
    renderSheet();

    await waitFor(() => screen.getByRole("button", { name: /modifier/i }));
    await user.click(screen.getByRole("button", { name: /modifier/i }));

    // Edit mode: [0] = caption textarea, [1] = hashtags input
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
        })
      );
    });
  });
});
