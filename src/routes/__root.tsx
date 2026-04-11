import { Outlet, Link, useRouterState } from "@tanstack/react-router";
import { TooltipProvider } from "@/components/ui/tooltip";
import {
  LayoutDashboard,
  PenLine,
  Settings,
  PanelLeftClose,
  PanelLeftOpen,
  Zap,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { useUIStore } from "@/stores/ui.store";
import { cn } from "@/lib/utils";

const navItems = [
  { to: "/", label: "Dashboard", icon: LayoutDashboard },
  { to: "/composer", label: "Composer", icon: PenLine },
  { to: "/settings", label: "Settings", icon: Settings },
] as const;

function NavLink({
  to,
  label,
  icon: Icon,
  collapsed,
}: {
  to: string;
  label: string;
  icon: React.ElementType;
  collapsed: boolean;
}) {
  const routerState = useRouterState();
  const isActive = routerState.location.pathname === to;

  return (
    <Link
      to={to}
      className={cn(
        "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
        "hover:bg-secondary hover:text-foreground",
        isActive
          ? "bg-secondary text-primary"
          : "text-muted-foreground",
        collapsed && "justify-center px-2"
      )}
      title={collapsed ? label : undefined}
    >
      <Icon className="h-4 w-4 shrink-0" />
      {!collapsed && <span>{label}</span>}
    </Link>
  );
}

export function RootLayout() {
  const { sidebarCollapsed, toggleSidebar } = useUIStore();

  return (
    <TooltipProvider>
    <div className="flex h-screen overflow-hidden bg-background">
      {/* Sidebar */}
      <aside
        className={cn(
          "flex flex-col border-r border-border bg-card transition-all duration-200",
          sidebarCollapsed ? "w-14" : "w-56"
        )}
      >
        {/* Logo */}
        <div
          className={cn(
            "flex h-14 items-center border-b border-border px-3",
            sidebarCollapsed ? "justify-center" : "gap-2"
          )}
        >
          <Zap className="h-5 w-5 shrink-0 text-primary" />
          {!sidebarCollapsed && (
            <span className="text-sm font-semibold tracking-tight text-foreground">
              Getpostcraft
            </span>
          )}
        </div>

        {/* Nav */}
        <nav className="flex flex-1 flex-col gap-1 p-2">
          {navItems.map((item) => (
            <NavLink key={item.to} {...item} collapsed={sidebarCollapsed} />
          ))}
        </nav>

        {/* Collapse toggle */}
        <div className="border-t border-border p-2">
          <Button
            variant="ghost"
            size="icon"
            onClick={toggleSidebar}
            className="h-8 w-full text-muted-foreground hover:text-foreground"
            aria-label={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          >
            {sidebarCollapsed ? (
              <PanelLeftOpen className="h-4 w-4" />
            ) : (
              <PanelLeftClose className="h-4 w-4" />
            )}
          </Button>
        </div>
      </aside>

      {/* Main content */}
      <main className="flex flex-1 flex-col overflow-auto">
        <Outlet />
      </main>
    </div>
    </TooltipProvider>
  );
}
