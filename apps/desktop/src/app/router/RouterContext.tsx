import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { matchRoute, type RouteConfig } from "@/app/router";

type RouterState = {
  path: string;
  params: Record<string, string>;
  route: RouteConfig;
  push: (to: string) => Promise<void>;
};

const RouterContext = createContext<RouterState | null>(null);

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

  const value = useMemo(() => {
    const matched = matchRoute(path);
    return {
      path,
      params: matched.params,
      route: matched.route,
      push,
    };
  }, [path, push]);

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
