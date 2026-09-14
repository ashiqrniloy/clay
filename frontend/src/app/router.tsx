// Application router. Memory data router: no server-style history fallback
// is needed inside the packaged webview, and routes stay testable headlessly.
// Top-level surfaces only — tabs, panes, documents, menus, and overlays are
// application state, never routes.

import { lazy, Suspense } from "react";
import { createMemoryRouter, type RouteObject } from "react-router";

import { AppShell } from "./layout/app-shell";
import { WorkspaceRoute } from "../routes/workspace";
import type { ConnectionState } from "../state/connection-store";

const LazyFixtureRoute = import.meta.env.DEV
  ? lazy(async () => {
      const { FixtureRoute } = await import("../routes/fixture");
      return { default: FixtureRoute };
    })
  : null;

function DevFixtureRoute() {
  if (!LazyFixtureRoute) return null;
  return (
    <Suspense fallback={null}>
      <LazyFixtureRoute />
    </Suspense>
  );
}

export interface RouterCallbacks {
  /** Optional test override. Production routes read the session store. */
  connection?: ConnectionState;
  onReconnect: () => void;
}

/**
 * Builds the production router. `callbacks` lets the shell render live
 * session state without a context provider layer.
 */
export function createAppRouter(
  callbacks: RouterCallbacks,
  initialPath = import.meta.env.DEV
    ? // DEV fixtures: /?fixture=<id> boots straight into that fixture route
      // (plain-browser visual/accessibility review without the Tauri webview).
      // The remaining query parameters carry to the memory router, so a fixture
      // can be opened in a named state: /?fixture=coding-agent&state=conversation.
      (() => {
        const params = new URLSearchParams(window.location.search);
        const fixture = params.get("fixture");
        if (!fixture) return "/workspace";
        params.delete("fixture");
        const rest = params.toString();
        return `/fixture/${fixture}${rest ? `?${rest}` : ""}`;
      })()
    : "/workspace",
) {
  const allRoutes: RouteObject[] = [
    {
      path: "/",
      element: (
        <AppShell
          status={
            callbacks.connection
              ? callbacks.connection.phase === "ready"
                ? "Connected"
                : callbacks.connection.phase === "disconnected"
                  ? "Disconnected"
                  : "Connecting…"
              : undefined
          }
        />
      ),
      children: [
        {
          index: false,
          path: "workspace",
          element: (
            <WorkspaceRoute
              connection={callbacks.connection}
              onReconnect={callbacks.onReconnect}
            />
          ),
        },
        ...(import.meta.env.DEV
          ? [
              {
                path: "fixture/:fixtureId",
                element: <DevFixtureRoute />,
              },
            ]
          : []),
      ],
    },
  ];
  return createMemoryRouter(allRoutes, { initialEntries: [initialPath] });
}
