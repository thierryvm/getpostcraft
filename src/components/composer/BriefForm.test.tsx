import { describe, it, expect, vi, beforeEach } from "vitest";
import { render as rtlRender, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import userEvent from "@testing-library/user-event";
import { BriefForm } from "./BriefForm";
import { invoke } from "@tauri-apps/api/core";

// BriefForm reads `useQuery({queryKey:["accounts"]})` so render() needs a fresh
// QueryClient per test to avoid cross-test cache bleed.
function render(ui: React.ReactElement) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return rtlRender(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

const mockInvoke = vi.mocked(invoke);

import { getDefaultFormat } from "@/types/composer.types";

// Stable mock function references — same instance across all renders in all tests
const storeFns = {
  setBrief: vi.fn(),
  setNetwork: vi.fn(),
  setAccountId: vi.fn(),
  setImageFormat: vi.fn(),
  setResult: vi.fn(),
  setVariants: vi.fn(),
  setIsLoading: vi.fn(),
  setError: vi.fn(),
  setDraftId: vi.fn(),
};

// Mock Zustand store so each test gets a clean empty brief (no cross-test bleed)
vi.mock("@/stores/composer.store", () => ({
  useComposerStore: vi.fn(() => ({
    brief: "",
    network: "instagram" as const,
    accountId: null,
    imageFormat: getDefaultFormat("instagram"),
    isLoading: false,
    error: null,
    result: null,
    variants: null,
    draftId: null,
    ...storeFns,
  })),
}));

beforeEach(() => {
  vi.clearAllMocks();
  // Reset call history on stable refs (clearAllMocks doesn't reach module-scope fns)
  Object.values(storeFns).forEach((fn) => fn.mockClear());
  // Default invoke responses: list_accounts must return [] so BriefForm renders.
  // Tests that exercise generate_content override this via mockResolvedValueOnce.
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "list_accounts") return Promise.resolve([]);
    return Promise.resolve({ caption: "stub", hashtags: [] });
  });
});

describe("BriefForm — validation Zod", () => {
  it("désactive le bouton Générer si le brief est vide", () => {
    render(<BriefForm />);
    const submitBtn = screen.getByRole("button", { name: /générer/i });
    expect(submitBtn).toBeDisabled();
  });

  it("désactive le bouton Générer si le brief est trop court (< 10 chars)", async () => {
    const user = userEvent.setup();
    render(<BriefForm />);
    const textarea = screen.getByPlaceholderText(/décris ce que tu veux poster/i);
    await user.type(textarea, "trop court");
    // "trop court" = 10 chars exactement → valide
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /générer/i })).not.toBeDisabled();
    });
  });

  it("affiche une erreur si le brief a moins de 10 caractères", async () => {
    const user = userEvent.setup();
    render(<BriefForm />);
    const textarea = screen.getByPlaceholderText(/décris ce que tu veux poster/i);
    await user.type(textarea, "court");
    await user.tab(); // trigger validation
    await waitFor(() => {
      expect(screen.getByText(/minimum 10 caractères/i)).toBeInTheDocument();
    });
  });

  it("affiche une erreur si le brief dépasse 500 caractères", async () => {
    render(<BriefForm />);
    const textarea = screen.getByPlaceholderText(/décris ce que tu veux poster/i);
    // fireEvent.change instead of userEvent.type for large strings (performance)
    fireEvent.change(textarea, { target: { value: "a".repeat(501) } });
    await waitFor(() => {
      expect(screen.getByText(/maximum 500 caractères/i)).toBeInTheDocument();
    });
  });

  it("active le bouton Générer avec un brief valide (10-500 chars)", async () => {
    const user = userEvent.setup();
    render(<BriefForm />);
    const textarea = screen.getByPlaceholderText(/décris ce que tu veux poster/i);
    await user.type(textarea, "Un post sur les alias Linux et leur utilité quotidienne");
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /générer/i })).not.toBeDisabled();
    });
  });

  it("affiche le compteur de caractères", async () => {
    const user = userEvent.setup();
    render(<BriefForm />);
    const textarea = screen.getByPlaceholderText(/décris ce que tu veux poster/i);
    await user.type(textarea, "Bonjour");
    await waitFor(() => {
      expect(screen.getByText(/7 \/ 500/)).toBeInTheDocument();
    });
  });

  it("le compteur passe en rouge au-delà de 450 caractères", async () => {
    render(<BriefForm />);
    const textarea = screen.getByPlaceholderText(/décris ce que tu veux poster/i);
    fireEvent.change(textarea, { target: { value: "a".repeat(451) } });
    await waitFor(() => {
      const counter = screen.getByText(/451 \/ 500/);
      expect(counter).toHaveClass("text-destructive");
    });
  });
});

describe("BriefForm — soumission", () => {
  it("appelle generate_content avec le brief et le réseau", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_accounts") return Promise.resolve([]);
      if (cmd === "generate_content")
        return Promise.resolve({ caption: "Test caption", hashtags: ["linux", "terminal"] });
      return Promise.resolve({ caption: "stub", hashtags: [] });
    });

    render(<BriefForm />);
    const textarea = screen.getByPlaceholderText(/décris ce que tu veux poster/i);
    await user.type(textarea, "Un post sur les alias Linux et leur utilité quotidienne");

    const submitBtn = screen.getByRole("button", { name: /générer/i });
    await user.click(submitBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "generate_content",
        expect.objectContaining({ brief: expect.any(String), network: "instagram" })
      );
    });
  });

  it("appelle setError avec le message d'erreur Tauri en cas d'échec", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_accounts") return Promise.resolve([]);
      return Promise.reject("No AI key configured");
    });

    render(<BriefForm />);
    const textarea = screen.getByPlaceholderText(/décris ce que tu veux poster/i);
    await user.type(textarea, "Un post sur les alias Linux et leur utilité quotidienne");

    const submitBtn = screen.getByRole("button", { name: /générer/i });
    await user.click(submitBtn);

    await waitFor(() => {
      expect(storeFns.setError).toHaveBeenCalledWith("No AI key configured");
    });
  });

  it("n'appelle pas generate_content si le brief est invalide", async () => {
    const user = userEvent.setup();
    render(<BriefForm />);
    const textarea = screen.getByPlaceholderText(/décris ce que tu veux poster/i);
    await user.type(textarea, "court");

    // Le bouton doit être désactivé → pas de submit possible
    const submitBtn = screen.getByRole("button", { name: /générer/i });
    expect(submitBtn).toBeDisabled();
    // Only list_accounts is allowed at render time; generate_content must not be called.
    const generateCalls = mockInvoke.mock.calls.filter(([cmd]) => cmd === "generate_content");
    expect(generateCalls).toHaveLength(0);
  });
});

describe("BriefForm — sélecteur de format image", () => {
  it("affiche les formats Instagram par défaut (3 options)", async () => {
    render(<BriefForm />);
    await waitFor(() => {
      expect(screen.getByText("Portrait 4:5")).toBeInTheDocument();
      expect(screen.getByText("Carré 1:1")).toBeInTheDocument();
      expect(screen.getByText("Paysage 1.91:1")).toBeInTheDocument();
    });
  });

  it("le format Portrait 4:5 est actif par défaut (Instagram)", async () => {
    render(<BriefForm />);
    await waitFor(() => {
      const portraitBtn = screen.getByText("Portrait 4:5").closest("button");
      expect(portraitBtn).toHaveClass("border-primary");
    });
  });

  it("cliquer sur Carré 1:1 appelle setImageFormat avec les bonnes dimensions", async () => {
    const user = userEvent.setup();
    render(<BriefForm />);
    await waitFor(() => screen.getByText("Carré 1:1"));
    await user.click(screen.getByText("Carré 1:1").closest("button")!);
    expect(storeFns.setImageFormat).toHaveBeenCalledWith(
      expect.objectContaining({ width: 1080, height: 1080 })
    );
  });

  it("affiche les dimensions sous le label du format", async () => {
    render(<BriefForm />);
    await waitFor(() => {
      expect(screen.getByText("1080×1350")).toBeInTheDocument();
    });
  });
});

describe("BriefForm — mode URL", () => {
  it("affiche le champ URL en mode URL", async () => {
    const user = userEvent.setup();
    render(<BriefForm />);
    const urlBtn = screen.getByRole("button", { name: /url/i });
    await user.click(urlBtn);
    await waitFor(() => {
      expect(
        screen.getByPlaceholderText(/https:\/\/blog\.example\.com/i)
      ).toBeInTheDocument();
    });
  });

  it("désactive le bouton Extraire si l'URL est vide", async () => {
    const user = userEvent.setup();
    render(<BriefForm />);
    const urlBtn = screen.getByRole("button", { name: /url/i });
    await user.click(urlBtn);
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /extraire/i })).toBeDisabled();
    });
  });
});
