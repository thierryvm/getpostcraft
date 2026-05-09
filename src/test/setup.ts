import "@testing-library/jest-dom";
import { vi } from "vitest";

// Mock Tauri IPC — no native bridge in test environment
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Mock TanStack Router's useNavigate — the real router needs a memory-history
// Provider that we don't bootstrap in unit tests. Tests can read the spy via
// `vi.mocked(useNavigate)` if they need to assert navigation.
vi.mock("@tanstack/react-router", async () => {
  const actual = await vi.importActual<typeof import("@tanstack/react-router")>(
    "@tanstack/react-router",
  );
  return {
    ...actual,
    useNavigate: () => vi.fn(),
  };
});
