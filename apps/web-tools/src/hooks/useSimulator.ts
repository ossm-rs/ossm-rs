import { useContext } from "react";
import { SimulatorContext } from "../SimulatorProvider";

export function useSimulator() {
  const sim = useContext(SimulatorContext);
  if (!sim) throw new Error("useSimulator must be used within SimulatorProvider");
  return sim;
}
