import { Box, Card, Flex, Text } from "@radix-ui/themes";
import { useAppearance } from "../hooks/useAppearance";
import { useIsMobile } from "../hooks/useIsMobile";
import { useTrajectoryInputs } from "../hooks/useTrajectoryInputs";
import { Chart } from "../Chart";
import { TrajectorySidebar, useTrajectoryData } from "../TrajectoryPanel";
import styles from "./GraphPage.module.css";

export default function GraphPage() {
  const [appearance] = useAppearance();
  const isMobile = useIsMobile();
  const inputs = useTrajectoryInputs();
  const { data, chartSeries, stats } = useTrajectoryData(inputs);

  const charts = (
    <Flex direction="column" p="3" gap="3">
      {chartSeries.map((s) => (
        <Card size="2" key={s.key}>
          <Flex align="center" gap="2" mb="2">
            <Box
              className={styles.legendSwatch}
              style={{ backgroundColor: s.color }}
            />
            <Text size="2" weight="medium">{s.label}</Text>
            {s.unit && <Text size="1" color="gray">({s.unit})</Text>}
          </Flex>
          <Chart.Canvas
            series={[s]}
            xData={data.time}
            focused={s.key}
            appearance={appearance}
            height={180}
            formatXTick={(v) => `${Math.round(v)}s`}
          />
        </Card>
      ))}
    </Flex>
  );

  const sidebar = (
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
      unitMode={inputs.unitMode}
      onUnitModeChange={inputs.setUnitMode}
      duration={inputs.duration}
      onDurationValueChange={inputs.setDuration}
      stats={stats}
      onResetDefaults={inputs.resetDefaults}
      compact={isMobile}
      p="3"
    />
  );

  return (
    <Flex
      direction={isMobile ? "column-reverse" : "row"}
      className={styles.root}
    >
      <Box className={isMobile ? styles.sidebarMobile : styles.sidebarDesktop}>
        {sidebar}
      </Box>
      <Box className={styles.content}>
        {charts}
      </Box>
    </Flex>
  );
}
