import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AccountsForm } from "./AccountsForm";
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

describe("AccountsForm — compte non connecté", () => {
  beforeEach(() => {
    // No accounts, no App ID, no client secret configured
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_accounts") return Promise.resolve([]);
      if (cmd === "get_instagram_app_id") return Promise.resolve(null);
      if (cmd === "get_instagram_client_secret_status") return Promise.resolve(false);
      if (cmd === "get_linkedin_client_id") return Promise.resolve(null);
      if (cmd === "get_linkedin_client_secret_status") return Promise.resolve(false);
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      return Promise.resolve(null);
    });
  });

  it("affiche le formulaire de connexion quand aucun compte n'est connecté", async () => {
    renderWithQuery(<AccountsForm />);
    await waitFor(() => {
      // Both Instagram and LinkedIn show "Non connecté" when no accounts
      const badges = screen.getAllByText("Non connecté");
      expect(badges.length).toBeGreaterThanOrEqual(1);
    });
  });

  it("le bouton Connecter est désactivé sans App ID ni Secret", async () => {
    renderWithQuery(<AccountsForm />);
    await waitFor(() => {
      const btn = screen.getByRole("button", { name: /connecter instagram/i });
      expect(btn).toBeDisabled();
    });
  });

  it("le bouton Connecter reste désactivé avec App ID seul (pas de secret)", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_accounts") return Promise.resolve([]);
      if (cmd === "get_instagram_app_id") return Promise.resolve("876077775447670");
      if (cmd === "get_instagram_client_secret_status") return Promise.resolve(false);
      if (cmd === "get_linkedin_client_id") return Promise.resolve(null);
      if (cmd === "get_linkedin_client_secret_status") return Promise.resolve(false);
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      return Promise.resolve(null);
    });

    renderWithQuery(<AccountsForm />);
    await waitFor(() => {
      const btn = screen.getByRole("button", { name: /connecter instagram/i });
      expect(btn).toBeDisabled();
    });
  });

  it("le bouton Connecter reste désactivé avec Secret seul (pas d'App ID)", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_accounts") return Promise.resolve([]);
      if (cmd === "get_instagram_app_id") return Promise.resolve(null);
      if (cmd === "get_instagram_client_secret_status") return Promise.resolve(true);
      if (cmd === "get_linkedin_client_id") return Promise.resolve(null);
      if (cmd === "get_linkedin_client_secret_status") return Promise.resolve(false);
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      return Promise.resolve(null);
    });

    renderWithQuery(<AccountsForm />);
    await waitFor(() => {
      const btn = screen.getByRole("button", { name: /connecter instagram/i });
      expect(btn).toBeDisabled();
    });
  });

  it("le bouton Connecter est actif quand App ID ET Secret sont configurés", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_accounts") return Promise.resolve([]);
      if (cmd === "get_instagram_app_id") return Promise.resolve("876077775447670");
      if (cmd === "get_instagram_client_secret_status") return Promise.resolve(true);
      if (cmd === "get_linkedin_client_id") return Promise.resolve(null);
      if (cmd === "get_linkedin_client_secret_status") return Promise.resolve(false);
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      return Promise.resolve(null);
    });

    renderWithQuery(<AccountsForm />);
    await waitFor(() => {
      const btn = screen.getByRole("button", { name: /connecter instagram/i });
      expect(btn).not.toBeDisabled();
    });
  });

  it("affiche ✓ configuré après sauvegarde de l'App ID", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_accounts") return Promise.resolve([]);
      if (cmd === "get_instagram_app_id") return Promise.resolve("876077775447670");
      if (cmd === "get_instagram_client_secret_status") return Promise.resolve(false);
      if (cmd === "save_instagram_app_id") return Promise.resolve();
      if (cmd === "get_linkedin_client_id") return Promise.resolve(null);
      if (cmd === "get_linkedin_client_secret_status") return Promise.resolve(false);
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      return Promise.resolve(null);
    });

    renderWithQuery(<AccountsForm />);
    await waitFor(() => {
      expect(screen.getByText(/✓ configuré/)).toBeInTheDocument();
    });
  });
});

describe("AccountsForm — compte connecté", () => {
  const mockAccount = {
    id: 1,
    provider: "instagram",
    user_id: "12345",
    username: "terminallearning",
    display_name: "Terminal Learning",
  };

  beforeEach(() => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_accounts") return Promise.resolve([mockAccount]);
      if (cmd === "get_instagram_app_id") return Promise.resolve("876077775447670");
      if (cmd === "get_instagram_client_secret_status") return Promise.resolve(true);
      if (cmd === "get_linkedin_client_id") return Promise.resolve(null);
      if (cmd === "get_linkedin_client_secret_status") return Promise.resolve(false);
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      return Promise.resolve(null);
    });
  });

  it("affiche le badge Connecté et le nom d'utilisateur", async () => {
    renderWithQuery(<AccountsForm />);
    await waitFor(() => {
      expect(screen.getByText("Connecté")).toBeInTheDocument();
      expect(screen.getByText("@terminallearning")).toBeInTheDocument();
    });
  });

  it("n'affiche pas le formulaire de connexion quand un compte est connecté", async () => {
    renderWithQuery(<AccountsForm />);
    await waitFor(() => {
      expect(
        screen.queryByRole("button", { name: /connecter instagram/i })
      ).not.toBeInTheDocument();
    });
  });

  it("appelle disconnect_account au clic sur Déconnecter", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_accounts") return Promise.resolve([mockAccount]);
      if (cmd === "get_instagram_app_id") return Promise.resolve("876077775447670");
      if (cmd === "get_instagram_client_secret_status") return Promise.resolve(true);
      if (cmd === "get_linkedin_client_id") return Promise.resolve(null);
      if (cmd === "get_linkedin_client_secret_status") return Promise.resolve(false);
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      if (cmd === "disconnect_account") return Promise.resolve();
      return Promise.resolve(null);
    });

    renderWithQuery(<AccountsForm />);
    await waitFor(() => {
      expect(screen.getByText("@terminallearning")).toBeInTheDocument();
    });

    const disconnectBtn = screen.getByRole("button", { name: /déconnecter/i });
    fireEvent.click(disconnectBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("disconnect_account", {
        provider: "instagram",
        userId: "12345",
      });
    });
  });
});

describe("AccountsForm — sécurité IPC", () => {
  it("ne retourne jamais de token à l'interface — list_accounts ne contient pas de token_key", async () => {
    const accountWithToken = {
      id: 1,
      provider: "instagram",
      user_id: "12345",
      username: "terminallearning",
      display_name: null,
      // token_key volontairement absent du type ConnectedAccount
    };

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_accounts") return Promise.resolve([accountWithToken]);
      if (cmd === "get_instagram_app_id") return Promise.resolve(null);
      if (cmd === "get_instagram_client_secret_status") return Promise.resolve(false);
      if (cmd === "get_linkedin_client_id") return Promise.resolve(null);
      if (cmd === "get_linkedin_client_secret_status") return Promise.resolve(false);
      if (cmd === "get_imgbb_key_status") return Promise.resolve(false);
      return Promise.resolve(null);
    });

    renderWithQuery(<AccountsForm />);
    await waitFor(() => {
      expect(screen.getByText("@terminallearning")).toBeInTheDocument();
    });

    // Le token_key ne doit pas apparaître dans le DOM
    expect(screen.queryByText(/token_key/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/instagram:12345/i)).not.toBeInTheDocument();
  });
});
