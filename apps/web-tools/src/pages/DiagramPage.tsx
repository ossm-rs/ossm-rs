import { Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Box,
  Button,
  Card,
  Flex,
  Heading,
  IconButton,
  Slider,
  Text,
} from "@radix-ui/themes";
import { Cross2Icon, ReloadIcon } from "@radix-ui/react-icons";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import AssemblyScene from "../AssemblyScene";
import type { AssemblySceneHandle } from "../AssemblyScene";
import { PART_CATALOG } from "../parts";
import { useIsMobile } from "../hooks/useIsMobile";
import styles from "./DiagramPage.module.css";

export default function DiagramPage() {
  const sceneRef = useRef<AssemblySceneHandle>(null);
  const isMobile = useIsMobile();
  const [explodeAmount, setExplodeAmount] = useState(0);
  const [selectedPartId, setSelectedPartId] = useState<string | null>(null);
  const partsById = useMemo(
    () => new Map(PART_CATALOG.map((p) => [p.id, p])),
    [],
  );
  const selectedPart = selectedPartId ? partsById.get(selectedPartId) : null;
  const introAnimRef = useRef(true);
  const rafRef = useRef(0);
  const introTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleSceneReady = useCallback(() => {
    if (!introAnimRef.current) return;
    introTimeoutRef.current = setTimeout(() => {
      if (!introAnimRef.current) return;
      const duration = 1500;
      const start = performance.now();
      const tick = (now: number) => {
        if (!introAnimRef.current) return;
        const t = Math.min((now - start) / duration, 1);
        const eased =
          t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
        setExplodeAmount(eased);
        if (t < 1) {
          rafRef.current = requestAnimationFrame(tick);
        } else {
          introAnimRef.current = false;
        }
      };
      rafRef.current = requestAnimationFrame(tick);
    }, 1000);
  }, []);

  useEffect(() => {
    return () => {
      introAnimRef.current = false;
      cancelAnimationFrame(rafRef.current);
      if (introTimeoutRef.current) clearTimeout(introTimeoutRef.current);
    };
  }, []);

  const canvasRef = useRef<HTMLDivElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const [viewportInsets, setViewportInsets] = useState({
    top: 0,
    left: 0,
    width: 0,
    height: 0,
  });

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

  return (
    <Box ref={canvasRef} className={styles.root}>
      <Suspense fallback={null}>
        <AssemblyScene
          ref={sceneRef}
          zoom={isMobile ? 900 : 1500}
          viewportInsets={viewportInsets}
          explodeAmount={explodeAmount}
          onReady={handleSceneReady}
          onPartSelect={setSelectedPartId}
        />
      </Suspense>

      <div className={isMobile ? styles.overlayMobile : styles.overlayDesktop}>
        <Flex
          ref={viewportRef}
          direction="column"
          justify="end"
          className={styles.viewport}
        />

        {selectedPart && (
          <Card size="1" className={`${styles.glass} ${styles.tooltip}`}>
            <Flex direction="column" gap="3" p="1">
              <Flex justify="between" align="start" gap="2">
                <Heading size="4">{selectedPart.name}</Heading>
                <IconButton
                  variant="ghost"
                  size="1"
                  onClick={() => setSelectedPartId(null)}
                  aria-label="Close"
                >
                  <Cross2Icon />
                </IconButton>
              </Flex>
              {selectedPart.description ? (
                <div className={styles.markdown}>
                  <Markdown
                    remarkPlugins={[remarkGfm]}
                    components={{
                      a: (props) => (
                        <a
                          {...props}
                          target="_blank"
                          rel="noreferrer noopener"
                        />
                      ),
                    }}
                  >
                    {selectedPart.description}
                  </Markdown>
                </div>
              ) : (
                <Text size="2" color="gray">
                  No description yet.
                </Text>
              )}
            </Flex>
          </Card>
        )}

        <div className={styles.reset}>
          <Button
            variant="outline"
            size="2"
            className={styles.glass}
            onClick={() => sceneRef.current?.resetView()}
          >
            <ReloadIcon />
            Reset View
          </Button>
        </div>

        <Card size="2" className={`${styles.glass} ${styles.timeline}`}>
          <Flex align="center" gap="3" p="1">
            <Text size="1" color="gray" style={{ minWidth: 60 }}>
              Explode
            </Text>
            <Slider
              value={[explodeAmount]}
              min={0}
              max={1.75}
              step={0.01}
              onValueChange={([v]) => {
                introAnimRef.current = false;
                if (introTimeoutRef.current) {
                  clearTimeout(introTimeoutRef.current);
                  introTimeoutRef.current = null;
                }
                cancelAnimationFrame(rafRef.current);
                setExplodeAmount(v);
              }}
              style={{ flex: 1 }}
            />
          </Flex>
        </Card>
      </div>
    </Box>
  );
}
