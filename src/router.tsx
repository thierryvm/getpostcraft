import {
  createRouter,
  createRoute,
  createRootRoute,
} from "@tanstack/react-router";
import { RootLayout } from "./routes/__root";
import { DashboardPage } from "./routes/index";
import { ComposerPage } from "./routes/composer/index";
import { CalendarPage } from "./routes/calendar/index";
import { SettingsPage } from "./routes/settings/index";

const rootRoute = createRootRoute({ component: RootLayout });

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: DashboardPage,
});

const composerRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/composer",
  component: ComposerPage,
});

const calendarRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/calendar",
  component: CalendarPage,
});

const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  validateSearch: (search: Record<string, unknown>) => ({
    tab: (search.tab as string) ?? "ai",
  }),
  component: SettingsPage,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  composerRoute,
  calendarRoute,
  settingsRoute,
]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
