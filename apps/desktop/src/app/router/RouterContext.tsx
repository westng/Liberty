import { createContext, useCallback, useContext, useEffect, useMemo, useState, type Context } from "react";
import { matchRoute, type RouteConfig } from "@/app/router";

type RouterState = {
  path: string;
  params: Record<string, string>;
  route: RouteConfig;
  push: (to: string) => Promise<void>;
  replace: (to: string) => Promise<void>;
};

const ROUTER_CONTEXT_KEY = "__libertyRouterContext";

type RouterGlobal = typeof globalThis & {
  [ROUTER_CONTEXT_KEY]?: Context<RouterState | null>;
};

const routerGlobal = globalThis as RouterGlobal;
const RouterContext = routerGlobal[ROUTER_CONTEXT_KEY] ?? createContext<RouterState | null>(null);
routerGlobal[ROUTER_CONTEXT_KEY] = RouterContext;

export function RouterProvider({ children }: { children: React.ReactNode }) {
  const [path, setPath] = useState(() => window.location.pathname || "/");

  useEffect(() => {
    const handlePopState = () => setPath(window.location.pathname || "/");
    window.addEventListener("popstate", handlePopState);
    return () => window.removeEventListener("popstate", handlePopState);
  }, []);

  const push = useCallback(async (to: string) => {
    window.history.pushState({}, "", to);
    setPath(window.location.pathname || "/");
  }, []);

  const replace = useCallback(async (to: string) => {
    window.history.replaceState({}, "", to);
    setPath(window.location.pathname || "/");
  }, []);

  const value = useMemo(() => {
    const matched = matchRoute(path);
    return {
      path,
      params: matched.params,
      route: matched.route,
      push,
      replace,
    };
  }, [path, push, replace]);

  return <RouterContext.Provider value={value}>{children}</RouterContext.Provider>;
}

export function useRouter() {
  const context = useContext(RouterContext);
  if (!context) {
    throw new Error("useRouter must be used inside RouterProvider.");
  }
  return context;
}

type LinkProps = React.AnchorHTMLAttributes<HTMLAnchorElement> & {
  to: string;
};

export function Link({ to, onClick, ...props }: LinkProps) {
  const router = useRouter();

  return (
    <a
      {...props}
      href={to}
      onClick={(event) => {
        onClick?.(event);
        if (
          event.defaultPrevented
          || event.button !== 0
          || event.metaKey
          || event.altKey
          || event.ctrlKey
          || event.shiftKey
        ) {
          return;
        }
        event.preventDefault();
        void router.push(to);
      }}
    />
  );
}
