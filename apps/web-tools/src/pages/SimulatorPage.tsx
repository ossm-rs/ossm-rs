import { Suspense, useCallback, useEffect, useRef, useState } from "react";
import { useSimulator } from "../hooks/useSimulator";
import { useEngineState } from "../hooks/useEngineState";
import { useIsMobile } from "../hooks/useIsMobile";
import { useAppearance } from "../hooks/useAppearance";
import { usePersistedState } from "../hooks/usePersistedState";
import { useTrajectoryInputs } from "../hooks/useTrajectoryInputs";
import { Box, Button, Card, Flex } from "@radix-ui/themes";
import { PlayIcon, PauseIcon, ReloadIcon } from "@radix-ui/react-icons";
import Scene from "../Scene";
import type { SceneHandle } from "../Scene";
import { Chart } from "../Chart";
import { TrajectorySidebar, useTrajectoryData } from "../TrajectoryPanel";
import styles from "./SimulatorPage.module.css";

export default function SimulatorPage() {
  const simulator = useSimulator();
  const sceneRef = useRef<SceneHandle>(null);
  const isMobile = useIsMobile();
  const [appearance] = useAppearance();
  const playbackState = useEngineState(simulator);
  const inputs = useTrajectoryInputs();

  const [wasPlaying, setWasPlaying] = usePersistedState("ossm:playing", false);
  const [focused, setFocused] = useState("position");

  const canvasRef = useRef<HTMLDivElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const [viewportInsets, setViewportInsets] = useState({ top: 0, left: 0, width: 0, height: 0 });
  const [scrubPosition, setScrubPosition] = useState<number | null>(null);
  const lastPatternRef = useRef<number | null>(null);
  const mountedRef = useRef(false);

  useEffect(() => {
    simulator.set_depth(inputs.depth);
    simulator.set_stroke(inputs.stroke);
    simulator.set_velocity(inputs.velocity);
    simulator.set_sensation(inputs.sensation);

    if (!mountedRef.current) {
      mountedRef.current = true;
      lastPatternRef.current = inputs.pattern;
      if (wasPlaying && inputs.pattern >= 0) {
        simulator.play(inputs.pattern);
      }
      return;
    }
  }, [simulator, inputs.depth, inputs.stroke, inputs.velocity, inputs.sensation]);

  useEffect(() => {
    if (!mountedRef.current) return;
    if (inputs.pattern >= 0 && inputs.pattern !== lastPatternRef.current) {
      lastPatternRef.current = inputs.pattern;
      setWasPlaying(true);
      simulator.play(inputs.pattern);
    }
  }, [simulator, inputs.pattern, setWasPlaying]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const viewport = viewportRef.current;
    if (!canvas || !viewport) {
      setViewportInsets({ top: 0, left: 0, width: 0, height: 0 });
      return;
    }

    const update = () => {
      const canvasRect = canvas.getBoundingClientRect();
      const vpRect = viewport.getBoundingClientRect();
      setViewportInsets({
        top: vpRect.top - canvasRect.top,
        left: vpRect.left - canvasRect.left,
        width: vpRect.width,
        height: vpRect.height,
      });
    };

    const observer = new ResizeObserver(update);
    observer.observe(canvas);
    observer.observe(viewport);
    return () => observer.disconnect();
  }, [isMobile]);

  const { data, chartSeries, stats } = useTrajectoryData(inputs);

  const handlePlay = useCallback(() => {
    setWasPlaying(true);
    lastPatternRef.current = inputs.pattern;
    simulator.play(inputs.pattern);
  }, [simulator, inputs.pattern, setWasPlaying]);

  const handlePause = useCallback(() => {
    simulator.pause();
  }, [simulator]);

  const handleResume = useCallback(() => {
    simulator.resume();
  }, [simulator]);

  const handleScrub = useCallback(
    (index: number | null) => {
      setScrubPosition(index != null ? data.position[index] : null);
    },
    [data],
  );

  return (
    <Box ref={canvasRef} className={styles.root}>
      <Suspense fallback={null}>
        <Scene
          ref={sceneRef}
          simulator={simulator}
          zoom={isMobile ? 900 : 1500}
          viewportInsets={viewportInsets}
          overridePosition={scrubPosition}
        />
      </Suspense>

      <div
        className={isMobile ? styles.overlayMobile : styles.overlayDesktop}
      >
        <Flex
          ref={viewportRef}
          direction="column"
          justify="end"
          className={styles.viewport}
        />

        <Flex gap="2" align="center" className={styles.buttons}>
          <Button
            variant="outline"
            size="2"
            className={styles.glass}
            onClick={
              playbackState === "playing" || playbackState === "homing"
                ? handlePause
                : playbackState === "paused"
                  ? handleResume
                  : handlePlay
            }
          >
            {playbackState === "playing" || playbackState === "homing" ? (
              <PauseIcon />
            ) : (
              <PlayIcon />
            )}
            {playbackState === "playing" || playbackState === "homing"
              ? "Pause"
              : playbackState === "paused"
                ? "Resume"
                : "Play"}
          </Button>
          <Box style={{ flex: 1 }} />
          <Button
            variant="outline"
            size="2"
            className={styles.glass}
            onClick={() => sceneRef.current?.resetView()}
          >
            <ReloadIcon />
            Reset View
          </Button>
        </Flex>

        {!isMobile && (
          <Card size="2" className={`${styles.glass} ${styles.chart}`}>
            <Chart.Legend
              series={chartSeries}
              focused={focused}
              onFocusChange={setFocused}
            />
            <Chart.Canvas
              series={chartSeries}
              xData={data.time}
              focused={focused}
              appearance={appearance}
              height={175}
              formatXTick={(v) => `${Math.round(v)}s`}
              onScrub={handleScrub}
              mt="2"
            />
          </Card>
        )}

        <Card size="2" className={`${styles.glass} ${styles.sidebar}`}>
          <TrajectorySidebar
            pattern={inputs.pattern}
            onPatternChange={inputs.setPattern}
            depth={inputs.depth}
            onDepthChange={inputs.setDepth}
            stroke={inputs.stroke}
            onStrokeChange={inputs.setStroke}
            velocity={inputs.velocity}
            onVelocityChange={inputs.setVelocity}
            sensation={inputs.sensation}
            onSensationChange={inputs.setSensation}
            compact={isMobile}
            unitMode={inputs.unitMode}
            onUnitModeChange={inputs.setUnitMode}
            duration={inputs.duration}
            onDurationValueChange={inputs.setDuration}
            stats={stats}
            onResetDefaults={inputs.resetDefaults}
          />
        </Card>
      </div>
    </Box>
  );
}
