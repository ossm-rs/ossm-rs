import { createContext, useContext } from "react";

export type Appearance = "light" | "dark";

export const AppearanceContext = createContext<
  [Appearance, () => void] | null
>(null);

export function useAppearance(): [Appearance, () => void] {
  const ctx = useContext(AppearanceContext);
  if (!ctx) throw new Error("useAppearance must be used within AppearanceProvider");
  return ctx;
}
