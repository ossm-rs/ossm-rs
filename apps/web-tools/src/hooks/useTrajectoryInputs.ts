import { useCallback } from "react";
import { usePersistedState } from "./usePersistedState";

export type UnitMode = "relative" | "absolute";

const DEFAULTS = {
  pattern: 0,
  depth: 0.75,
  stroke: 0.5,
  velocity: 0.75,
  sensation: 0.0,
  duration: 20,
  unitMode: "relative" as UnitMode,
};

export { DEFAULTS as TRAJECTORY_DEFAULTS };

export function useTrajectoryInputs() {
  const [pattern, setPattern] = usePersistedState("ossm:pattern", DEFAULTS.pattern);
  const [depth, setDepth] = usePersistedState("ossm:depth", DEFAULTS.depth);
  const [stroke, setStroke] = usePersistedState("ossm:stroke", DEFAULTS.stroke);
  const [velocity, setVelocity] = usePersistedState("ossm:velocity", DEFAULTS.velocity);
  const [sensation, setSensation] = usePersistedState("ossm:sensation", DEFAULTS.sensation);
  const [duration, setDuration] = usePersistedState("ossm:duration", DEFAULTS.duration);
  const [unitMode, setUnitMode] = usePersistedState<UnitMode>("ossm:unitMode", DEFAULTS.unitMode);

  const resetDefaults = useCallback(() => {
    setPattern(DEFAULTS.pattern);
    setDepth(DEFAULTS.depth);
    setStroke(DEFAULTS.stroke);
    setVelocity(DEFAULTS.velocity);
    setSensation(DEFAULTS.sensation);
    setDuration(DEFAULTS.duration);
    setUnitMode(DEFAULTS.unitMode);
  }, [setPattern, setDepth, setStroke, setVelocity, setSensation, setDuration, setUnitMode]);

  return {
    pattern, setPattern,
    depth, setDepth,
    stroke, setStroke,
    velocity, setVelocity,
    sensation, setSensation,
    duration, setDuration,
    unitMode, setUnitMode,
    resetDefaults,
  };
}
