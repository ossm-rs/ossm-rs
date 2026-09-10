import {
  useRef,
  useMemo,
  useCallback,
  useImperativeHandle,
  forwardRef,
  memo,
} from "react";
import { Canvas, useFrame, useThree } from "@react-three/fiber";
import {
  Environment,
  OrbitControls,
  OrthographicCamera,
  useGLTF,
} from "@react-three/drei";
import type { OrbitControls as OrbitControlsImpl } from "three-stdlib";
import {
  ACESFilmicToneMapping,
  BufferGeometry,
  MeshStandardMaterial,
  Mesh as ThreeMesh,
  Vector3,
} from "three";
import type { Object3D } from "three";
import { mergeGeometries } from "three/examples/jsm/utils/BufferGeometryUtils.js";
import type { Simulator } from "@ossm-rs/web-simulator";
import { useAppearance } from "./hooks/useAppearance";

const MODEL_URL = "/models/ossm-alt.gltf";

const RAIL_TRAVEL = 0.25;

const purpleMaterial = new MeshStandardMaterial({ color: 0x6a1b9a });
const railMaterial = new MeshStandardMaterial({
  color: 0x888888,
  metalness: 0.8,
  roughness: 0.2,
});

function collectGeometries(root: Object3D): BufferGeometry | null {
  const geometries: BufferGeometry[] = [];
  root.updateWorldMatrix(true, true);
  root.traverse((child) => {
    if ((child as ThreeMesh).isMesh) {
      const mesh = child as ThreeMesh;
      const geo = mesh.geometry.clone();
      geo.applyMatrix4(mesh.matrixWorld);
      geometries.push(geo);
    }
  });
  if (geometries.length === 0) return null;
  return mergeGeometries(geometries);
}

function Model({
  simulator,
  onOrbitTarget,
  overridePosition,
}: {
  simulator: Simulator;
  onOrbitTarget?: (center: Vector3) => void;
  overridePosition?: number | null;
}) {
  const { scene } = useGLTF(MODEL_URL);
  const railRef = useRef<ThreeMesh>(null);

  const { housingGeo, railGeo } = useMemo(() => {
    const housingNode = scene.getObjectByName("housing");
    const railNode = scene.getObjectByName("rail");
    const hGeo = housingNode ? collectGeometries(housingNode) : null;

    if (hGeo) {
      hGeo.computeBoundingBox();
      const center = new Vector3();
      hGeo.boundingBox!.getCenter(center);
      onOrbitTarget?.(center);
    }

    return {
      housingGeo: hGeo,
      railGeo: railNode ? collectGeometries(railNode) : null,
    };
  }, [scene, onOrbitTarget]);

  useFrame(() => {
    if (railRef.current) {
      const pos = overridePosition != null ? overridePosition : simulator.get_position();
      railRef.current.position.z = -(1 - pos) * RAIL_TRAVEL;
    }
  });

  return (
    <>
      {housingGeo && <mesh geometry={housingGeo} material={purpleMaterial} />}
      {railGeo && (
        <mesh ref={railRef} geometry={railGeo} material={railMaterial} />
      )}
    </>
  );
}

export interface SceneHandle {
  resetView: () => void;
}

const INITIAL_CAMERA: [number, number, number] = [-0.4, 0.4, 0.4];
const RESET_LERP_SPEED = 0.08;
const RESET_SNAP_THRESHOLD = 0.0001;

interface ViewportInsets {
  top: number;
  left: number;
  width: number;
  height: number;
}

function SceneContent({
  simulator,
  handle,
  zoom,
  viewportInsets,
  overridePosition,
}: {
  simulator: Simulator;
  handle: React.Ref<SceneHandle>;
  zoom: number;
  viewportInsets?: ViewportInsets;
  overridePosition?: number | null;
}) {
  const [appearance] = useAppearance();
  const controlsRef = useRef<OrbitControlsImpl>(null);
  const resettingRef = useRef(false);
  const camera = useThree((s) => s.camera);
  const targetRef = useRef(new Vector3());

  const isDark = appearance === "dark";

  const goalPos = useMemo(() => new Vector3(...INITIAL_CAMERA), []);
  const goalTarget = targetRef.current;

  const onOrbitTarget = useCallback((center: Vector3) => {
    targetRef.current.copy(center);
    controlsRef.current?.target.copy(center);
    controlsRef.current?.update();
  }, []);

  useImperativeHandle(handle, () => ({
    resetView: () => {
      resettingRef.current = true;
    },
  }));

  const size = useThree((s) => s.size);

  useFrame(({ gl }) => {
    gl.toneMappingExposure = isDark ? 0.75 : 1.2;

    if (viewportInsets && viewportInsets.width > 0 && viewportInsets.height > 0) {
      const { top, left, width, height } = viewportInsets;
      // Center of the viewport sub-region vs center of the full canvas
      const vpCenterX = left + width / 2;
      const vpCenterY = top + height / 2;
      // setViewOffset's x/y is the top-left of the sub-window we want centered.
      // To shift the projection so vpCenter becomes the visual center:
      const offsetX = size.width / 2 - vpCenterX;
      const offsetY = size.height / 2 - vpCenterY;
      camera.setViewOffset(
        size.width,
        size.height,
        offsetX,
        offsetY,
        size.width,
        size.height,
      );
      camera.updateProjectionMatrix();
    } else {
      camera.clearViewOffset();
      camera.updateProjectionMatrix();
    }

    if (!resettingRef.current) return;
    const controls = controlsRef.current;
    if (!controls) return;

    camera.position.lerp(goalPos, RESET_LERP_SPEED);
    controls.target.lerp(goalTarget, RESET_LERP_SPEED);
    camera.zoom += (zoom - camera.zoom) * RESET_LERP_SPEED;
    camera.updateProjectionMatrix();
    controls.update();

    if (
      camera.position.distanceTo(goalPos) < RESET_SNAP_THRESHOLD &&
      controls.target.distanceTo(goalTarget) < RESET_SNAP_THRESHOLD &&
      Math.abs(camera.zoom - zoom) < 0.5
    ) {
      camera.position.copy(goalPos);
      controls.target.copy(goalTarget);
      camera.zoom = zoom;
      camera.updateProjectionMatrix();
      controls.update();
      resettingRef.current = false;
    }
  });

  return (
    <>
      <color attach="background" args={[isDark ? "#111113" : "#ffffff"]} />
      <ambientLight intensity={isDark ? 0.4 : 0.8} />
      <directionalLight position={[1, 2, 3]} intensity={isDark ? 0.8 : 1.5} />
      <directionalLight position={[-1, 1, -1]} intensity={isDark ? 0.3 : 0.5} />
      <Environment preset="studio" environmentIntensity={isDark ? 0.4 : 1} />
      <Model simulator={simulator} onOrbitTarget={onOrbitTarget} overridePosition={overridePosition} />
      <OrthographicCamera
        makeDefault
        position={INITIAL_CAMERA}
        zoom={zoom}
        near={0.001}
        far={10}
      />
      <OrbitControls ref={controlsRef} />
    </>
  );
}

const Scene = memo(
  forwardRef<
    SceneHandle,
    {
      simulator: Simulator;
      zoom?: number;
      viewportInsets?: ViewportInsets;
      overridePosition?: number | null;
    }
  >(function Scene({ simulator, zoom = 1500, viewportInsets, overridePosition }, ref) {
    return (
      <Canvas
        gl={{ toneMapping: ACESFilmicToneMapping, toneMappingExposure: 1.2 }}
        style={{ width: "100%", height: "100%" }}
      >
        <SceneContent
          simulator={simulator}
          handle={ref}
          zoom={zoom}
          viewportInsets={viewportInsets}
          overridePosition={overridePosition}
        />
      </Canvas>
    );
  }),
);

export default Scene;
