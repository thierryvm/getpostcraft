import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { PublicationForm } from "./PublicationForm";
import { invoke } from "@tauri-apps/api/core";

const mockInvoke = vi.mocked(invoke);

function renderWithQuery(ui: React.ReactElement) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>);
}

beforeEach(() => {
  vi.clearAllMocks();
});

// ── Provider par défaut ───────────────────────────────────────────────────────

describe("PublicationForm — provider par défaut", () => {
  beforeEach(() => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_image_host") return Promise.resolve("catbox");
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      return Promise.resolve(null);
    });
  });

  it("affiche les deux options de provider", async () => {
    renderWithQuery(<PublicationForm />);
    await waitFor(() => {
      // Anchored: imgbb description mentions "catbox.moe" so plain /catbox\.moe/i matches both
      expect(screen.getByRole("button", { name: /^catbox\.moe/i })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: /^imgbb/i })).toBeInTheDocument();
    });
  });

  it("catbox est sélectionné par défaut", async () => {
    renderWithQuery(<PublicationForm />);
    await waitFor(() => {
      const catboxBtn = screen.getByRole("button", { name: /^catbox\.moe/i });
      expect(catboxBtn).toHaveClass("border-primary");
    });
  });

  it("le champ clé ImgBB est masqué quand catbox est sélectionné", async () => {
    renderWithQuery(<PublicationForm />);
    await waitFor(() => {
      expect(screen.queryByText(/clé api imgbb/i)).not.toBeInTheDocument();
    });
  });

  it("affiche le badge 'Recommandé · Aucune clé requise' sur l'option catbox", async () => {
    renderWithQuery(<PublicationForm />);
    await waitFor(() => {
      expect(screen.getByText(/aucune clé requise/i)).toBeInTheDocument();
    });
  });
});

// ── Sélection ImgBB ───────────────────────────────────────────────────────────

describe("PublicationForm — sélection ImgBB", () => {
  beforeEach(() => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_image_host") return Promise.resolve("catbox");
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      if (cmd === "save_image_host") return Promise.resolve();
      return Promise.resolve(null);
    });
  });

  it("cliquer sur ImgBB appelle save_image_host avec 'imgbb'", async () => {
    const user = userEvent.setup();
    renderWithQuery(<PublicationForm />);

    await waitFor(() => screen.getByRole("button", { name: /^imgbb/i }));
    await user.click(screen.getByRole("button", { name: /^imgbb/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_image_host", { provider: "imgbb" });
    });
  });

  it("le champ clé ImgBB apparaît quand imgbb est le provider actif", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_image_host") return Promise.resolve("imgbb");
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      return Promise.resolve(null);
    });

    renderWithQuery(<PublicationForm />);
    await waitFor(() => {
      expect(screen.getByText(/clé api imgbb/i)).toBeInTheDocument();
    });
  });

  it("le bouton Enregistrer est désactivé si la clé est vide", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_image_host") return Promise.resolve("imgbb");
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      return Promise.resolve(null);
    });

    renderWithQuery(<PublicationForm />);
    await waitFor(() => {
      const btn = screen.getByRole("button", { name: /enregistrer/i });
      expect(btn).toBeDisabled();
    });
  });

  it("le bouton Enregistrer est actif quand une clé est saisie", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_image_host") return Promise.resolve("imgbb");
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      return Promise.resolve(null);
    });

    renderWithQuery(<PublicationForm />);
    await waitFor(() => screen.getByRole("button", { name: /enregistrer/i }));

    const input = screen.getByPlaceholderText(/colle ta clé/i);
    await user.type(input, "my-imgbb-api-key");

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /enregistrer/i })).not.toBeDisabled();
    });
  });

  it("Enregistrer appelle save_imgbb_key avec la clé saisie", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_image_host") return Promise.resolve("imgbb");
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      if (cmd === "save_imgbb_key") return Promise.resolve();
      return Promise.resolve(null);
    });

    renderWithQuery(<PublicationForm />);
    await waitFor(() => screen.getByRole("button", { name: /enregistrer/i }));

    const input = screen.getByPlaceholderText(/colle ta clé/i);
    await user.type(input, "my-imgbb-api-key");
    await user.click(screen.getByRole("button", { name: /enregistrer/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_imgbb_key", { apiKey: "my-imgbb-api-key" });
    });
  });
});

// ── Statut clé ImgBB ─────────────────────────────────────────────────────────

describe("PublicationForm — statut clé ImgBB", () => {
  it("affiche '· Clé configurée' quand une clé ImgBB existe déjà", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_image_host") return Promise.resolve("imgbb");
      if (cmd === "get_imgbb_key_status") return Promise.resolve(true);
      return Promise.resolve(null);
    });

    renderWithQuery(<PublicationForm />);
    await waitFor(() => {
      expect(screen.getByText(/clé configurée/i)).toBeInTheDocument();
    });
  });

  it("affiche le placeholder masqué si une clé ImgBB existe", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_image_host") return Promise.resolve("imgbb");
      if (cmd === "get_imgbb_key_status") return Promise.resolve(true);
      return Promise.resolve(null);
    });

    renderWithQuery(<PublicationForm />);
    await waitFor(() => {
      const input = screen.getByPlaceholderText("••••••••••••••••");
      expect(input).toBeInTheDocument();
    });
  });

  it("cliquer sur catbox appelle save_image_host avec 'catbox'", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_image_host") return Promise.resolve("imgbb");
      if (cmd === "get_imgbb_key_status") return Promise.resolve(true);
      if (cmd === "save_image_host") return Promise.resolve();
      return Promise.resolve(null);
    });

    renderWithQuery(<PublicationForm />);
    await waitFor(() => screen.getByRole("button", { name: /^catbox\.moe/i }));
    await user.click(screen.getByRole("button", { name: /^catbox\.moe/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("save_image_host", { provider: "catbox" });
    });
  });
});
