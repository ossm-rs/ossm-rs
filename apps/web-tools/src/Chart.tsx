import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ComponentProps,
} from "react";
import { Box, Flex, Text } from "@radix-ui/themes";
import * as d3 from "d3";

export interface ChartSeries {
  key: string;
  label: string;
  color: string;
  data: number[];
  /** Transform: displayed = raw * scale + offset */
  scale?: number;
  offset?: number;
  /** Lock the Y domain instead of auto-fitting */
  fixedDomain?: [number, number];
  /** Y-axis label shown when this series is focused */
  unit?: string;
}

type RootProps = ComponentProps<typeof Box>;

function Root(props: RootProps) {
  return <Box {...props} />;
}

interface LegendProps extends Omit<ComponentProps<typeof Flex>, "children"> {
  series: ChartSeries[];
  focused: string;
  onFocusChange: (key: string) => void;
}

function Legend({ series, focused, onFocusChange, ...props }: LegendProps) {
  return (
    <Flex gap="4" {...props}>
      {series.map(({ key, label, color }) => (
        <Flex
          key={key}
          align="center"
          gap="2"
          onClick={() => onFocusChange(key)}
          style={{
            cursor: "pointer",
            opacity: key === focused ? 1 : 0.5,
          }}
        >
          <Box
            style={{
              width: 12,
              height: 12,
              borderRadius: 2,
              backgroundColor: color,
              outline: key === focused ? `2px solid ${color}` : "none",
              outlineOffset: 2,
            }}
          />
          <Text size="2" weight={key === focused ? "bold" : "regular"}>
            {label}
          </Text>
        </Flex>
      ))}
    </Flex>
  );
}

interface CanvasProps extends Omit<
  ComponentProps<typeof Box>,
  "children" | "height"
> {
  series: ChartSeries[];
  xData: number[];
  focused: string;
  appearance: string;
  height?: number;
  /** Format x-axis tick labels. Defaults to rounding to integer. */
  formatXTick?: (value: number) => string;
  /** Format y-axis tick labels. Auto-formats by default. */
  formatYTick?: (value: number) => string;
  /** Called with the index at the scrub position, or null on leave */
  onScrub?: (index: number | null) => void;
}

function defaultFormatY(v: number, yRange: number): string {
  const abs = Math.abs(v);
  if (abs >= 1000) return `${(v / 1000).toFixed(1)}k`;
  if (abs >= 10) return v.toFixed(0);
  if (yRange <= 2) return v.toFixed(1);
  if (abs >= 1) return v.toFixed(1);
  return v.toFixed(2);
}

function Canvas({
  series,
  xData,
  focused,
  appearance,
  height = 200,
  formatXTick,
  formatYTick,
  onScrub,
  ...boxProps
}: CanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 600, height: height ?? 200 });
  const [hoverX, setHoverX] = useState<number | null>(null);
  const onScrubRef = useRef(onScrub);
  onScrubRef.current = onScrub;
  const margin = useMemo(
    () => ({ top: 10, right: 8, bottom: 18, left: 52 }),
    [],
  );

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const observer = new ResizeObserver((entries) => {
      const { width } = entries[0].contentRect;
      setSize({ width, height: height ?? 200 });
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, [height]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || xData.length === 0 || series.length === 0) return;

    const dpr = window.devicePixelRatio || 1;
    const { width, height: h } = size;
    canvas.width = width * dpr;
    canvas.height = h * dpr;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${h}px`;

    const ctx = canvas.getContext("2d")!;
    ctx.scale(dpr, dpr);

    const isDark = appearance === "dark";
    const axisColor = isDark ? "rgba(255,255,255,0.3)" : "rgba(0,0,0,0.15)";
    const textColor = isDark ? "rgba(255,255,255,0.6)" : "rgba(0,0,0,0.5)";

    ctx.clearRect(0, 0, width, h);

    const plotW = width - margin.left - margin.right;
    const plotH = h - margin.top - margin.bottom;

    const xScale = d3
      .scaleLinear()
      .domain([xData[0], xData[xData.length - 1]])
      .range([0, plotW]);

    const seriesMap = new Map(
      series.map((s) => {
        const scale = s.scale ?? 1;
        const offset = s.offset ?? 0;
        const values =
          scale === 1 && offset === 0
            ? s.data
            : s.data.map((v) => v * scale + offset);
        const domain = (() => {
          if (s.fixedDomain) return s.fixedDomain;
          const ext = d3.extent(values) as [number, number];
          const pad = (ext[1] - ext[0]) * 0.1 || 1;
          return [ext[0] - pad, ext[1] + pad] as [number, number];
        })();
        return [
          s.key,
          {
            ...s,
            values,
            yScale: d3.scaleLinear().domain(domain).range([plotH, 0]),
          },
        ] as const;
      }),
    );

    const focusedSeries = seriesMap.get(focused);
    if (!focusedSeries) return;

    ctx.save();
    ctx.translate(margin.left, margin.top);

    const focusedY = focusedSeries.yScale;

    // Grid + axes
    ctx.strokeStyle = axisColor;
    ctx.lineWidth = 1;
    const yTicks = focusedY.ticks(5);
    const xTicks = xScale.ticks(8);

    for (const tick of xTicks) {
      const x = Math.round(xScale(tick)) + 0.5;
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, plotH);
      ctx.stroke();
    }
    ctx.beginPath();
    ctx.moveTo(0.5, 0);
    ctx.lineTo(0.5, plotH + 0.5);
    ctx.lineTo(plotW, plotH + 0.5);
    ctx.stroke();

    // Unfocused series
    for (const [key, s] of seriesMap) {
      if (key === focused) continue;
      ctx.globalAlpha = 0.25;
      ctx.strokeStyle = s.color;
      ctx.lineWidth = 1.5;
      ctx.lineJoin = "round";
      ctx.beginPath();
      for (let i = 0; i < xData.length; i++) {
        const x = xScale(xData[i]);
        const y = s.yScale(s.values[i]);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
    }

    // Focused series
    ctx.globalAlpha = 1.0;
    ctx.strokeStyle = focusedSeries.color;
    ctx.lineWidth = 2.5;
    ctx.lineJoin = "round";
    ctx.beginPath();
    for (let i = 0; i < xData.length; i++) {
      const x = xScale(xData[i]);
      const y = focusedY(focusedSeries.values[i]);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();

    ctx.globalAlpha = 1.0;

    // X-axis labels
    const fmtX = formatXTick ?? ((v: number) => String(Math.round(v)));
    ctx.fillStyle = textColor;
    ctx.font = "11px system-ui, sans-serif";
    ctx.textAlign = "center";
    for (const tick of xTicks) {
      ctx.fillText(fmtX(tick), xScale(tick), plotH + 14);
    }

    // Y-axis labels
    ctx.textAlign = "right";
    ctx.textBaseline = "middle";
    const focusedDomain = focusedY.domain();
    const yRange = Math.abs(focusedDomain[1] - focusedDomain[0]);
    const fmtY = formatYTick ?? ((v: number) => defaultFormatY(v, yRange));
    for (const tick of yTicks) {
      ctx.fillText(fmtY(tick), -6, focusedY(tick));
    }

    // Y-axis unit label
    if (focusedSeries.unit) {
      ctx.save();
      ctx.translate(-40, plotH / 2);
      ctx.rotate(-Math.PI / 2);
      ctx.textAlign = "center";
      ctx.fillText(focusedSeries.unit, 0, 0);
      ctx.restore();
    }

    // Scrub line
    if (hoverX !== null) {
      ctx.strokeStyle = isDark ? "rgba(255,255,255,0.6)" : "rgba(0,0,0,0.4)";
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 4]);
      const x = Math.round(hoverX) + 0.5;
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, plotH);
      ctx.stroke();
      ctx.setLineDash([]);
    }

    ctx.restore();
  }, [
    xData,
    series,
    focused,
    size,
    appearance,
    hoverX,
    margin,
    formatXTick,
    formatYTick,
  ]);

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas || xData.length === 0) return;
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left - margin.left;
      const plotW = size.width - margin.left - margin.right;
      if (x < 0 || x > plotW) {
        setHoverX(null);
        onScrubRef.current?.(null);
        return;
      }
      setHoverX(x);
      const t = xData[0] + (x / plotW) * (xData[xData.length - 1] - xData[0]);
      const dt = xData.length > 1 ? xData[1] - xData[0] : 1;
      const idx = Math.min(Math.round((t - xData[0]) / dt), xData.length - 1);
      onScrubRef.current?.(idx);
    },
    [xData, size, margin],
  );

  const handleMouseLeave = useCallback(() => {
    setHoverX(null);
    onScrubRef.current?.(null);
  }, []);

  return (
    <Box ref={containerRef} {...boxProps}>
      <canvas
        ref={canvasRef}
        style={{
          borderRadius: 6,
          display: "block",
          cursor: onScrub ? "crosshair" : undefined,
        }}
        onMouseMove={onScrub ? handleMouseMove : undefined}
        onMouseLeave={onScrub ? handleMouseLeave : undefined}
      />
    </Box>
  );
}

export const Chart = { Root, Legend, Canvas };
