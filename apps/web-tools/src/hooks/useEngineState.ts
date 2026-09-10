import { useRef, useCallback, useSyncExternalStore } from "react";
import type { Simulator } from "@ossm-rs/web-simulator";

export const EngineState = {
  Stopped: 0,
  Homing: 1,
  Playing: 2,
  Paused: 3,
} as const;

export type PlaybackState = "stopped" | "homing" | "playing" | "paused";

const STATE_LABELS: Record<number, PlaybackState> = {
  [EngineState.Stopped]: "stopped",
  [EngineState.Homing]: "homing",
  [EngineState.Playing]: "playing",
  [EngineState.Paused]: "paused",
};

export function useEngineState(simulator: Simulator): PlaybackState {
  const stateRef = useRef<PlaybackState>("stopped");

  const subscribe = useCallback(
    (onStoreChange: () => void) => {
      let raf: number;
      const poll = () => {
        const raw = simulator.get_engine_state();
        const next = STATE_LABELS[raw] ?? "stopped";
        if (next !== stateRef.current) {
          stateRef.current = next;
          onStoreChange();
        }
        raf = requestAnimationFrame(poll);
      };
      raf = requestAnimationFrame(poll);
      return () => cancelAnimationFrame(raf);
    },
    [simulator],
  );

  return useSyncExternalStore(subscribe, () => stateRef.current);
}
