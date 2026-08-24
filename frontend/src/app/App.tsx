import { useEffect, useMemo } from "react";
import { RouterProvider } from "react-router";

import { useClaySession } from "./use-clay-session";
import { createAppRouter } from "./router";

/**
 * Root component: owns the bridge session and the memory router. Reconnect
 * (generation bump) replaces the router wholesale; connection updates flow
 * through the session store so the tree does not remount per event.
 */
export function App() {
  const { generation, reconnect } = useClaySession();
  const router = useMemo(() => {
    void generation; // new session identity → new router world
    return createAppRouter({ onReconnect: reconnect });
  }, [generation, reconnect]);
  useEffect(() => {
    if (!import.meta.env.DEV) return;
    (window as unknown as Record<string, unknown>).__clayNavigate = (
      path: string,
    ) => router.navigate(path);
  }, [router]);
  return <RouterProvider router={router} />;
}
