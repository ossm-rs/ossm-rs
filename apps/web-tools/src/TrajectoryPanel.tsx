import { useMemo, type ComponentProps } from "react";
import init, { TrajectoryRecorder } from "@ossm-rs/trajectory-recorder";
import wasmUrl from "@ossm-rs/trajectory-recorder/trajectory_recorder_bg.wasm?url";

await init({ module_or_path: wasmUrl });
import {
  Box,
  Flex,
  Heading,
  Text,
  Slider,
  Select,
  Separator,
  SegmentedControl,
  Button,
} from "@radix-ui/themes";
import { ReloadIcon } from "@radix-ui/react-icons";
import { type ChartSeries } from "./Chart";
import { type UnitMode } from "./hooks/useTrajectoryInputs";

const UNIT_LABELS: Record<UnitMode, { position: string; velocity: string; acceleration: string }> = {
  relative: { position: "Position", velocity: "Velocity", acceleration: "Acceleration" },
  absolute: { position: "Position (mm)", velocity: "Velocity (mm/s)", acceleration: "Accel (mm/s²)" },
};

interface TrajectoryData {
  time: number[];
  position: number[];
  velocity: number[];
  acceleration: number[];
}

let recorder: TrajectoryRecorder | null = null;
function getRecorder(): TrajectoryRecorder {
  if (!recorder) {
    recorder = new TrajectoryRecorder();
  }
  return recorder;
}

function generateTrajectory(
  pattern: number,
  depth: number,
  stroke: number,
  velocity: number,
  sensation: number,
  durationSecs: number,
): TrajectoryData {
  const rec = getRecorder();
  const dt = rec.timestep_ms() / 1000;
  const totalSteps = Math.ceil(durationSecs / dt);

  const result = rec.record(
    pattern, depth, stroke, velocity, sensation, totalSteps,
  );

  const stepCount = result.position.length;
  const time = new Array(stepCount);
  for (let i = 0; i < stepCount; i++) {
    time[i] = i * dt;
  }

  return {
    time,
    position: Array.from(result.position),
    velocity: Array.from(result.velocity),
    acceleration: Array.from(result.acceleration),
  };
}

export function useTrajectoryData(inputs: {
  pattern: number;
  depth: number;
  stroke: number;
  velocity: number;
  sensation: number;
  duration: number;
  unitMode: UnitMode;
}) {
  const { pattern, depth, stroke, velocity, sensation, duration, unitMode } = inputs;

  const data = useMemo(
    () => generateTrajectory(pattern, depth, stroke, velocity, sensation, duration),
    [pattern, depth, stroke, velocity, sensation, duration],
  );

  const rec = getRecorder();
  const minPosMm = rec.min_position_mm();
  const rangeMm = rec.max_position_mm() - minPosMm;

  const isAbsolute = unitMode === "absolute";
  const units = UNIT_LABELS[unitMode];

  const chartSeries = useMemo((): ChartSeries[] => {
    const scale = isAbsolute ? rangeMm : 1;
    return [
      {
        key: "position",
        label: "Position",
        color: "#8b5cf6",
        data: data.position,
        scale,
        offset: isAbsolute ? minPosMm : 0,
        fixedDomain: isAbsolute ? undefined : [0, 1],
        unit: units.position,
      },
      {
        key: "velocity",
        label: "Velocity",
        color: "#06b6d4",
        data: data.velocity,
        scale,
        offset: 0,
        unit: units.velocity,
      },
      {
        key: "acceleration",
        label: "Acceleration",
        color: "#f59e0b",
        data: data.acceleration,
        scale,
        offset: 0,
        unit: units.acceleration,
      },
    ];
  }, [data, isAbsolute, units, rangeMm, minPosMm]);

  const stats = useMemo(() => {
    if (data.time.length === 0) return null;
    const dur = data.time[data.time.length - 1];
    const scale = isAbsolute ? rangeMm : 1;
    const peakVel = Math.max(...data.velocity.map(Math.abs)) * scale;
    const peakAccel = Math.max(...data.acceleration.map(Math.abs)) * scale;
    return { duration: dur, peakVel, peakAccel, samples: data.time.length };
  }, [data, isAbsolute, rangeMm]);

  return { data, chartSeries, stats };
}

export function usePatternList() {
  return useMemo(() => {
    const rec = getRecorder();
    const count = rec.pattern_count();
    return Array.from({ length: count }, (_, i) => ({
      index: i,
      name: rec.pattern_name(i),
    }));
  }, []);
}

interface TrajectorySidebarProps extends Omit<ComponentProps<typeof Box>, "children"> {
  pattern: number;
  onPatternChange: (v: number) => void;
  depth: number;
  onDepthChange: (v: number) => void;
  stroke: number;
  onStrokeChange: (v: number) => void;
  velocity: number;
  onVelocityChange: (v: number) => void;
  sensation: number;
  onSensationChange: (v: number) => void;
  compact?: boolean;
  unitMode?: UnitMode;
  onUnitModeChange?: (v: UnitMode) => void;
  onResetDefaults?: () => void;
  duration?: number;
  onDurationValueChange?: (v: number) => void;
  stats?: { duration: number; peakVel: number; peakAccel: number; samples: number } | null;
}

export function TrajectorySidebar({
  pattern,
  onPatternChange,
  depth,
  onDepthChange,
  stroke,
  onStrokeChange,
  velocity,
  onVelocityChange,
  sensation,
  onSensationChange,
  compact = false,
  unitMode,
  onUnitModeChange,
  onResetDefaults,
  duration,
  onDurationValueChange,
  stats,
  ...boxProps
}: TrajectorySidebarProps) {
  const patterns = usePatternList();
  const isAbsolute = unitMode === "absolute";

  return (
    <Box {...boxProps}>
      <Flex direction="column" gap="4">
        <Heading size="4">Pattern Trajectory</Heading>
        <Separator size="4" />

        <Box>
          <Text size="2" weight="medium" mb="1" as="label">
            Pattern
          </Text>
          <Select.Root
            value={String(pattern)}
            onValueChange={(v) => onPatternChange(Number(v))}
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
        </Box>

        <Separator size="4" />

        <LabeledSlider label="Depth" value={depth} display={`${(depth * 100).toFixed(0)}%`} min={0} max={1} step={0.01} onChange={onDepthChange} />
        <LabeledSlider label="Stroke" value={stroke} display={`${(stroke * 100).toFixed(0)}%`} min={0} max={1} step={0.01} onChange={onStrokeChange} />
        <LabeledSlider label="Velocity" value={velocity} display={`${(velocity * 100).toFixed(0)}%`} min={0} max={1} step={0.01} onChange={onVelocityChange} />
        <LabeledSlider label="Sensation" value={sensation} display={sensation.toFixed(2)} min={-1} max={1} step={0.01} onChange={onSensationChange} />

        {!compact && unitMode != null && onUnitModeChange && (
          <>
            <Separator size="4" />
            <Box>
              <Text size="2" weight="medium" mb="1" as="label">Units</Text>
              <SegmentedControl.Root
                value={unitMode}
                onValueChange={(v) => onUnitModeChange(v as UnitMode)}
                style={{ width: "100%" }}
              >
                <SegmentedControl.Item value="relative">Relative</SegmentedControl.Item>
                <SegmentedControl.Item value="absolute">Absolute</SegmentedControl.Item>
              </SegmentedControl.Root>
            </Box>
          </>
        )}

        {!compact && duration != null && onDurationValueChange && (
          <>
            <Separator size="4" />
            <LabeledSlider label="Duration" value={duration} display={`${duration}s`} min={5} max={35} step={1} onChange={onDurationValueChange} />
          </>
        )}

        {!compact && stats && (
          <>
            <Separator size="4" />
            <Flex direction="column" gap="2">
              <Flex justify="between">
                <Text size="1" color="gray">Duration</Text>
                <Text size="1">{stats.duration.toFixed(3)}s</Text>
              </Flex>
              <Flex justify="between">
                <Text size="1" color="gray">Peak velocity</Text>
                <Text size="1">
                  {isAbsolute ? `${stats.peakVel.toFixed(1)} mm/s` : `${stats.peakVel.toFixed(3)} /s`}
                </Text>
              </Flex>
              <Flex justify="between">
                <Text size="1" color="gray">Peak accel</Text>
                <Text size="1">
                  {isAbsolute ? `${stats.peakAccel.toFixed(0)} mm/s²` : `${stats.peakAccel.toFixed(2)} /s²`}
                </Text>
              </Flex>
              <Flex justify="between">
                <Text size="1" color="gray">Samples</Text>
                <Text size="1">{stats.samples.toLocaleString()}</Text>
              </Flex>
            </Flex>
          </>
        )}
        {onResetDefaults && (
          <>
            <Separator size="4" />
            <Button variant="outline" style={{ width: "100%" }} onClick={onResetDefaults}>
              <ReloadIcon /> Reset Defaults
            </Button>
          </>
        )}
      </Flex>
    </Box>
  );
}

function LabeledSlider({
  label,
  value,
  display,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  value: number;
  display: string;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
}) {
  return (
    <Box>
      <Flex justify="between" mb="1">
        <Text size="2" weight="medium">{label}</Text>
        <Text size="2" color="gray">{display}</Text>
      </Flex>
      <Slider min={min} max={max} step={step} value={[value]} onValueChange={([v]) => onChange(v)} />
    </Box>
  );
}
