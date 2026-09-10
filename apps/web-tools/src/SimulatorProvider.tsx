import { createContext, useEffect, useState, type ReactNode } from "react";
import init, { Simulator } from "@ossm-rs/web-simulator";
import wasmUrl from "@ossm-rs/web-simulator/web_simulator_bg.wasm?url";

let wasmReady: Promise<void> | null = null;
function ensureWasmInit() {
  if (!wasmReady) {
    wasmReady = init({ module_or_path: wasmUrl }).then(() => {});
  }
  return wasmReady;
}

export const SimulatorContext = createContext<Simulator | null>(null);

export function SimulatorProvider({
  children,
  fallback,
}: {
  children: ReactNode;
  fallback: ReactNode;
}) {
  const [simulator, setSimulator] = useState<Simulator | null>(null);

  useEffect(() => {
    let cancelled = false;
    ensureWasmInit().then(() => {
      if (cancelled) return;
      setSimulator(new Simulator(10.0));
    });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!simulator) return <>{fallback}</>;

  return (
    <SimulatorContext.Provider value={simulator}>
      {children}
    </SimulatorContext.Provider>
  );
}
