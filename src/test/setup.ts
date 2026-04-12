import "@testing-library/jest-dom";
import { vi } from "vitest";

// Mock Tauri IPC — no native bridge in test environment
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
