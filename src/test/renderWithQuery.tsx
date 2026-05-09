import { render, type RenderOptions } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { type ReactElement } from "react";

/**
 * Render a component inside a fresh QueryClientProvider. Tests that touch
 * components using `useQueryClient` / `useQuery` / `useMutation` need this
 * to avoid "No QueryClient set" errors.
 *
 * Each call gets its own QueryClient with retries disabled so a transient
 * mock-rejection doesn't cascade into unrelated tests.
 */
export function renderWithQuery(
  ui: ReactElement,
  options?: RenderOptions,
) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0 },
      mutations: { retry: false },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>,
    options,
  );
}
