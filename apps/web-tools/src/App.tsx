import { Suspense, useRef, useCallback, useEffect, useMemo } from "react";
import { useSimulator } from "./hooks/useSimulator";
import { useEngineState } from "./hooks/useEngineState";
import { useAppearance } from "./hooks/useAppearance";
import { useIsMobile } from "./hooks/useIsMobile";
import { usePersistedState } from "./hooks/usePersistedState";
import {
  Theme,
  Box,
  Card,
  Flex,
  Heading,
  Text,
  Slider,
  Spinner,
  Select,
  Button,
  IconButton,
  ScrollArea,
  Separator,
  Tooltip,
} from "@radix-ui/themes";
import {
  SunIcon,
  MoonIcon,
  ResetIcon,
  GitHubLogoIcon,
  PlayIcon,
  PauseIcon,
  StopIcon,
} from "@radix-ui/react-icons";
import Scene from "./Scene";
import type { SceneHandle } from "./Scene";

export default function App() {
  const simulator = useSimulator();
  const sceneRef = useRef<SceneHandle>(null);
  const [depth, setDepth] = usePersistedState("ossm:depth", 0.5);
  const [stroke, setStroke] = usePersistedState("ossm:stroke", 0.4);
  const [velocity, setVelocity] = usePersistedState("ossm:velocity", 0.5);
  const [sensation, setSensation] = usePersistedState("ossm:sensation", 0.0);
  const [selectedPattern, setSelectedPattern] = usePersistedState(
    "ossm:pattern",
    0,
  );
  const playbackState = useEngineState(simulator);
  const [appearance, toggleAppearance] = useAppearance();
  const isMobile = useIsMobile();

  useEffect(() => {
    simulator.set_depth(depth);
    simulator.set_stroke(stroke);
    simulator.set_velocity(velocity);
    simulator.set_sensation(sensation);
    if (selectedPattern > 0) {
      simulator.play(selectedPattern);
    }
    // Only sync on mount, not on every state change
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [simulator]);

  const patterns = useMemo<
    { index: number; name: string; description: string }[]
  >(() => {
    const count = simulator.pattern_count();
    return Array.from({ length: count }, (_, i) => ({
      index: i,
      name: simulator.pattern_name(i),
      description: simulator.pattern_description(i),
    }));
  }, [simulator]);

  const updateDepth = useCallback(
    (v: number) => {
      setDepth(v);
      simulator.set_depth(v);
    },
    [simulator, setDepth],
  );

  const updateStroke = useCallback(
    (v: number) => {
      setStroke(v);
      simulator.set_stroke(v);
    },
    [simulator, setStroke],
  );

  const updateVelocity = useCallback(
    (v: number) => {
      setVelocity(v);
      simulator.set_velocity(v);
    },
    [simulator, setVelocity],
  );

  const updateSensation = useCallback(
    (v: number) => {
      setSensation(v);
      simulator.set_sensation(v);
    },
    [simulator, setSensation],
  );

  const handlePlay = useCallback(() => {
    simulator.play(selectedPattern);
  }, [simulator, selectedPattern]);

  const handlePause = useCallback(() => {
    simulator.pause();
  }, [simulator]);

  const handleResume = useCallback(() => {
    simulator.resume();
  }, [simulator]);

  const handleStop = useCallback(() => {
    simulator.stop();
  }, [simulator]);

  const handlePatternChange = useCallback(
    (value: string) => {
      const index = Number(value);
      setSelectedPattern(index);
      simulator.play(index);
    },
    [simulator, setSelectedPattern],
  );

  return (
    <Theme accentColor="purple" radius="large" appearance={appearance}>
      <Flex direction={isMobile ? "column" : "row"} height="100vh">
        <Box
          style={{
            flex: isMobile ? undefined : 1,
            height: isMobile ? "30vh" : "100vh",
            minHeight: 0,
            position: "relative",
          }}
        >
          <Suspense
            fallback={
              <Flex align="center" justify="center" height="100%" gap="3">
                <Spinner size="3" />
                <Text size="2">Loading model…</Text>
              </Flex>
            }
          >
            <Scene
              ref={sceneRef}
              simulator={simulator}
              zoom={isMobile ? 900 : 1500}
            />
          </Suspense>
          <Button
            asChild
            variant="outline"
            size="2"
            style={{
              position: "absolute",
              top: 8,
              right: 8,
              alignContent: "center",
            }}
          >
            <a
              href="https://github.com/ossm-rs/ossm"
              target="_blank"
              rel="noopener noreferrer"
            >
              <GitHubLogoIcon /> ossm-rs/ossm
            </a>
          </Button>
        </Box>

        <Box
          style={{
            width: isMobile ? undefined : "360px",
            height: isMobile ? "70vh" : "100vh",
            flexShrink: 0,
          }}
          p="3"
        >
          <Card
            size="2"
            style={{
              height: "100%",
              display: "flex",
              flexDirection: "column",
            }}
          >
            <Flex justify="between" align="center" mb="3">
              <Heading size="5">OSSM Simulator</Heading>
              <Tooltip
                content={
                  appearance === "light"
                    ? "Switch to dark mode"
                    : "Switch to light mode"
                }
              >
                <IconButton
                  variant="ghost"
                  size="2"
                  onClick={toggleAppearance}
                  aria-label="Toggle theme"
                >
                  {appearance === "light" ? <SunIcon /> : <MoonIcon />}
                </IconButton>
              </Tooltip>
            </Flex>

            <Separator size="4" mb="3" />

            <ScrollArea style={{ flex: 1 }} scrollbars="vertical">
              <Flex direction="column" gap="4" pr="1">
                <Box>
                  <Text size="2" weight="medium" mb="1" as="label">
                    Pattern
                  </Text>
                  <Select.Root
                    value={String(selectedPattern)}
                    onValueChange={handlePatternChange}
                  >
                    <Select.Trigger style={{ width: "100%" }} />
                    <Select.Content>
                      {patterns.map((p) => (
                        <Select.Item key={p.index} value={String(p.index)}>
                          {p.name}
                        </Select.Item>
                      ))}
                    </Select.Content>
                  </Select.Root>
                  {patterns[selectedPattern] && (
                    <Text size="1" color="gray" mt="1">
                      {patterns[selectedPattern].description}
                    </Text>
                  )}
                </Box>

                <Flex gap="2" align="center">
                  <IconButton
                    onClick={
                      playbackState === "playing" || playbackState === "homing"
                        ? handlePause
                        : playbackState === "paused"
                          ? handleResume
                          : handlePlay
                    }
                    variant="solid"
                    size="3"
                    aria-label={
                      playbackState === "playing" || playbackState === "homing"
                        ? "Pause"
                        : playbackState === "paused"
                          ? "Resume"
                          : "Play"
                    }
                  >
                    {playbackState === "playing" ||
                    playbackState === "homing" ? (
                      <PauseIcon />
                    ) : (
                      <PlayIcon />
                    )}
                  </IconButton>
                  <IconButton
                    onClick={handleStop}
                    variant="outline"
                    size="3"
                    disabled={playbackState === "stopped"}
                    aria-label="Stop"
                  >
                    <StopIcon />
                  </IconButton>
                  <Text size="2" color="gray" ml="1">
                    {playbackState.charAt(0).toUpperCase() +
                      playbackState.slice(1)}
                  </Text>
                </Flex>

                <Separator size="4" />

                <LabeledSlider
                  label="Depth"
                  value={depth}
                  min={0}
                  max={1}
                  step={0.01}
                  onChange={updateDepth}
                />
                <LabeledSlider
                  label="Stroke"
                  value={stroke}
                  min={0}
                  max={1}
                  step={0.01}
                  onChange={updateStroke}
                />
                <LabeledSlider
                  label="Velocity"
                  value={velocity}
                  min={0}
                  max={1}
                  step={0.01}
                  onChange={updateVelocity}
                />
                <LabeledSlider
                  label="Sensation"
                  value={sensation}
                  min={-1}
                  max={1}
                  step={0.01}
                  onChange={updateSensation}
                />

                <Separator size="4" />

                <Button
                  variant="outline"
                  onClick={() => sceneRef.current?.resetView()}
                  style={{ width: "100%" }}
                >
                  <ResetIcon /> Reset View
                </Button>
              </Flex>
            </ScrollArea>
          </Card>
        </Box>
      </Flex>
    </Theme>
  );
}

function LabeledSlider({
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
}) {
  return (
    <Box>
      <Flex justify="between" mb="1">
        <Text size="2" weight="medium">
          {label}
        </Text>
        <Text size="2" color="gray">
          {value}
        </Text>
      </Flex>
      <Slider
        min={min}
        max={max}
        step={step}
        value={[value]}
        onValueChange={(values) => onChange(values[0])}
      />
    </Box>
  );
}
