import { useState, useCallback, useMemo, type ReactNode } from "react";
import { AppearanceContext, type Appearance } from "./hooks/useAppearance";

export function AppearanceProvider({ children }: { children: ReactNode }) {
  const [appearance, setAppearance] = useState<Appearance>(() => {
    const stored = localStorage.getItem("theme");
    if (stored === "dark" || stored === "light") return stored;
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  });

  const toggle = useCallback(() => {
    const update = () => {
      setAppearance((prev) => {
        const next = prev === "light" ? "dark" : "light";
        localStorage.setItem("theme", next);
        return next;
      });
    };

    if ("startViewTransition" in document) {
      (
        document as unknown as { startViewTransition: (cb: () => void) => void }
      ).startViewTransition(update);
    } else {
      update();
    }
  }, []);

  const value = useMemo(
    () => [appearance, toggle] as [Appearance, () => void],
    [appearance, toggle],
  );

  return (
    <AppearanceContext.Provider value={value}>
      {children}
    </AppearanceContext.Provider>
  );
}
