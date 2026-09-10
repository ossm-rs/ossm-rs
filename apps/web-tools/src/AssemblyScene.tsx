import {
  useRef,
  useMemo,
  useCallback,
  useImperativeHandle,
  useEffect,
  forwardRef,
  memo,
} from "react";
import { Canvas, useFrame, useThree } from "@react-three/fiber";
import type { ThreeEvent } from "@react-three/fiber";
import {
  Environment,
  OrbitControls,
  OrthographicCamera,
  useGLTF,
} from "@react-three/drei";
import type { OrbitControls as OrbitControlsImpl } from "three-stdlib";
import {
  ACESFilmicToneMapping,
  Box3,
  Color,
  Matrix4,
  Mesh,
  MeshStandardMaterial,
  Object3D,
  Quaternion,
  Vector3,
} from "three";
import { useAppearance } from "./hooks/useAppearance";
import { PART_CATALOG } from "./parts";
import type { CatalogPart } from "./parts";

const MODEL_URL = "/models/ossm-alt.glb";

useGLTF.preload(MODEL_URL, true);

type CatalogState = {
  part: CatalogPart;
  nodes: Object3D[];
  originalPositions: Vector3[];
};

const transformTmp = new Vector3();

export interface AssemblySceneHandle {
  resetView: () => void;
}

const INITIAL_CAMERA: [number, number, number] = [-0.4, 0.4, 0.4];
const RESET_DURATION = 0.55;

const cameraTmpMat = new Matrix4();
const cameraEndQuat = new Quaternion();

function easeOutCubicScalar(t: number) {
  return 1 - Math.pow(1 - t, 3);
}

function createMaterialForType(
  type: CatalogPart["materialType"],
): MeshStandardMaterial | null {
  switch (type) {
    case "printed":
      return new MeshStandardMaterial({
        color: new Color(0x6a1b9a),
        roughness: 0.85,
        metalness: 0,
      });
    default:
      return null;
  }
}

interface ViewportInsets {
  top: number;
  left: number;
  width: number;
  height: number;
}

function Model({
  onOrbitTarget,
  onReady,
  onPartSelect,
  explodeAmount = 1,
}: {
  onOrbitTarget?: (center: Vector3) => void;
  onReady?: () => void;
  onPartSelect?: (partId: string | null) => void;
  explodeAmount?: number;
}) {
  const { scene: original } = useGLTF(MODEL_URL, true);
  const scene = useMemo(() => original.clone(true), [original]);

  const catalogStatesRef = useRef<CatalogState[]>([]);
  const parentInvQuatsRef = useRef<Map<Object3D, Quaternion | null>>(new Map());
  const catalogIdByNodeRef = useRef<Map<Object3D, string>>(new Map());
  const readyFiredRef = useRef(false);

  useMemo(() => {
    scene.updateMatrixWorld(true);

    const catalogStates: CatalogState[] = [];
    const catalogNodeMap = new Map<Object3D, string>();
    for (const catalogPart of PART_CATALOG) {
      const nodes: Object3D[] = [];
      const originals: Vector3[] = [];
      for (const meshName of catalogPart.meshes) {
        const node = scene.getObjectByName(meshName);
        if (!node) {
          console.warn(
            `[catalog] mesh not found: ${meshName} (part ${catalogPart.id})`,
          );
          continue;
        }
        nodes.push(node);
        originals.push(node.position.clone());
        catalogNodeMap.set(node, catalogPart.id);
      }
      if (nodes.length === 0) continue;
      const overrideMat = createMaterialForType(catalogPart.materialType);
      if (overrideMat) {
        for (const node of nodes) {
          node.traverse((child) => {
            const mesh = child as Mesh;
            if (!mesh.isMesh) return;
            mesh.material = overrideMat;
          });
        }
      }
      catalogStates.push({
        part: catalogPart,
        nodes,
        originalPositions: originals,
      });
    }
    catalogStatesRef.current = catalogStates;
    catalogIdByNodeRef.current = catalogNodeMap;

    const parentInvQuats = new Map<Object3D, Quaternion | null>();
    for (const cs of catalogStates) {
      for (const node of cs.nodes) {
        const parent = node.parent;
        if (!parent) {
          parentInvQuats.set(node, null);
          continue;
        }
        const pq = new Quaternion();
        parent.getWorldQuaternion(pq);
        const isIdentity =
          Math.abs(pq.x) < 1e-6 &&
          Math.abs(pq.y) < 1e-6 &&
          Math.abs(pq.z) < 1e-6 &&
          Math.abs(pq.w - 1) < 1e-6;
        if (isIdentity) {
          parentInvQuats.set(node, null);
        } else {
          pq.invert();
          parentInvQuats.set(node, pq);
        }
      }
    }
    parentInvQuatsRef.current = parentInvQuats;

    const box = new Box3().setFromObject(scene);
    const center = new Vector3();
    box.getCenter(center);
    onOrbitTarget?.(center);
  }, [scene, onOrbitTarget]);

  useFrame(() => {
    if (!readyFiredRef.current) {
      readyFiredRef.current = true;
      onReady?.();
    }
    const t = explodeAmount;
    const catalogById = new Map(PART_CATALOG.map((p) => [p.id, p]));

    const partWorldDelta = (partId: string): Vector3 => {
      const part = catalogById.get(partId);
      const out = new Vector3();
      if (!part) return out;
      const dir = part.offset ?? part.offsets?.[0];
      if (!dir) return out;
      out.set(dir[0] * t, dir[1] * t, dir[2] * t);
      return out;
    };

    const chainDeltaCache = new Map<string, Vector3>();
    const chainDelta = (partId: string, seen?: Set<string>): Vector3 => {
      const cached = chainDeltaCache.get(partId);
      if (cached) return cached;
      const part = catalogById.get(partId);
      if (!part) {
        const z = new Vector3();
        chainDeltaCache.set(partId, z);
        return z;
      }
      const visited = seen ?? new Set<string>();
      visited.add(partId);
      const parentDelta =
        part.parent && !visited.has(part.parent)
          ? chainDelta(part.parent, visited)
          : new Vector3();
      const total = new Vector3()
        .copy(parentDelta)
        .add(partWorldDelta(partId));
      chainDeltaCache.set(partId, total);
      return total;
    };

    for (const cs of catalogStatesRef.current) {
      const parentChain = cs.part.parent
        ? chainDelta(cs.part.parent)
        : new Vector3();
      cs.nodes.forEach((node, i) => {
        const own = cs.part.offsets ? cs.part.offsets[i] : cs.part.offset;
        node.position.copy(cs.originalPositions[i]);
        const inv = parentInvQuatsRef.current.get(node);
        if (parentChain.lengthSq() > 0) {
          transformTmp.copy(parentChain);
          if (inv) transformTmp.applyQuaternion(inv);
          node.position.add(transformTmp);
        }
        if (own && (own[0] !== 0 || own[1] !== 0 || own[2] !== 0)) {
          transformTmp.set(own[0] * t, own[1] * t, own[2] * t);
          if (inv) transformTmp.applyQuaternion(inv);
          node.position.add(transformTmp);
        }
      });
    }
  });

  const findCatalogId = useCallback((obj: Object3D): string | null => {
    let cur: Object3D | null = obj;
    while (cur) {
      const id = catalogIdByNodeRef.current.get(cur);
      if (id) return id;
      cur = cur.parent;
    }
    return null;
  }, []);

  const handleClick = useCallback(
    (e: ThreeEvent<MouseEvent>) => {
      e.stopPropagation();
      const hit = e.intersections[0];
      if (!hit) {
        onPartSelect?.(null);
        return;
      }
      const id = findCatalogId(hit.object);
      onPartSelect?.(id);
    },
    [findCatalogId, onPartSelect],
  );

  const handleMissed = useCallback(() => {
    onPartSelect?.(null);
  }, [onPartSelect]);

  return (
    <primitive
      object={scene}
      onClick={handleClick}
      onPointerMissed={handleMissed}
    />
  );
}

function SceneContent({
  handle,
  zoom,
  viewportInsets,
  explodeAmount,
  onReady,
  onPartSelect,
}: {
  handle: React.Ref<AssemblySceneHandle>;
  zoom: number;
  viewportInsets?: ViewportInsets;
  explodeAmount?: number;
  onReady?: () => void;
  onPartSelect?: (partId: string | null) => void;
}) {
  const [appearance] = useAppearance();
  const controlsRef = useRef<OrbitControlsImpl>(null);
  const resettingRef = useRef(false);
  const autopilotRef = useRef(true);
  const camera = useThree((s) => s.camera);
  const targetRef = useRef(new Vector3());

  const resetStartTimeRef = useRef(0);
  const resetStartPosRef = useRef(new Vector3());
  const resetStartTargetRef = useRef(new Vector3());
  const resetStartZoomRef = useRef(0);
  const resetStartQuatRef = useRef(new Quaternion());

  const beginReset = useCallback(() => {
    resettingRef.current = true;
    resetStartTimeRef.current = 0;
  }, []);

  const isDark = appearance === "dark";

  const goalPosRef = useRef(new Vector3(...INITIAL_CAMERA));
  const goalZoomRef = useRef(zoom);
  const goalTarget = targetRef.current;

  const onOrbitTarget = useCallback((center: Vector3) => {
    targetRef.current.copy(center);
    controlsRef.current?.target.copy(center);
    controlsRef.current?.update();
  }, []);

  useImperativeHandle(handle, () => ({
    resetView: () => {
      autopilotRef.current = true;
      goalPosRef.current.set(...INITIAL_CAMERA);
      goalZoomRef.current = zoom;
      beginReset();
    },
  }));

  const size = useThree((s) => s.size);

  useEffect(() => {
    goalZoomRef.current = zoom;
  }, [zoom]);

  useFrame(({ gl, clock }) => {
    gl.toneMappingExposure = isDark ? 0.75 : 1.2;

    if (
      viewportInsets &&
      viewportInsets.width > 0 &&
      viewportInsets.height > 0
    ) {
      const { top, left, width, height } = viewportInsets;
      const vpCenterX = left + width / 2;
      const vpCenterY = top + height / 2;
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

    if (!autopilotRef.current) return;
    const controls = controlsRef.current;
    if (!controls) return;

    const goalPos = goalPosRef.current;
    const goalZoom = goalZoomRef.current;

    if (resettingRef.current) {
      if (resetStartTimeRef.current === 0) {
        resetStartTimeRef.current = clock.getElapsedTime();
        resetStartPosRef.current.copy(camera.position);
        resetStartTargetRef.current.copy(controls.target);
        resetStartZoomRef.current = camera.zoom;
        resetStartQuatRef.current.copy(camera.quaternion);
      }
      const elapsed = clock.getElapsedTime() - resetStartTimeRef.current;
      const t = Math.min(elapsed / RESET_DURATION, 1);
      const eased = easeOutCubicScalar(t);

      cameraTmpMat.lookAt(goalPos, goalTarget, camera.up);
      cameraEndQuat.setFromRotationMatrix(cameraTmpMat);

      camera.position.copy(resetStartPosRef.current).lerp(goalPos, eased);
      controls.target.copy(resetStartTargetRef.current).lerp(goalTarget, eased);
      camera.quaternion
        .copy(resetStartQuatRef.current)
        .slerp(cameraEndQuat, eased);
      camera.zoom =
        resetStartZoomRef.current +
        (goalZoom - resetStartZoomRef.current) * eased;
      camera.updateProjectionMatrix();
      camera.updateMatrixWorld();

      if (t >= 1) {
        camera.position.copy(goalPos);
        controls.target.copy(goalTarget);
        camera.quaternion.copy(cameraEndQuat);
        camera.zoom = goalZoom;
        camera.updateProjectionMatrix();
        controls.update();
        resettingRef.current = false;
        resetStartTimeRef.current = 0;
      }
    }
  });

  return (
    <>
      <color attach="background" args={[isDark ? "#111113" : "#ffffff"]} />
      <ambientLight intensity={isDark ? 0.4 : 0.8} />
      <directionalLight position={[1, 2, 3]} intensity={isDark ? 0.8 : 1.5} />
      <directionalLight position={[-1, 1, -1]} intensity={isDark ? 0.3 : 0.5} />
      <Environment preset="studio" environmentIntensity={isDark ? 0.4 : 1} />
      <Model
        onOrbitTarget={onOrbitTarget}
        onReady={onReady}
        onPartSelect={onPartSelect}
        explodeAmount={explodeAmount}
      />
      <OrthographicCamera
        makeDefault
        position={INITIAL_CAMERA}
        zoom={zoom}
        near={0.001}
        far={10}
      />
      <OrbitControls
        ref={controlsRef}
        onStart={() => {
          autopilotRef.current = false;
          resettingRef.current = false;
        }}
      />
    </>
  );
}

const AssemblyScene = memo(
  forwardRef<
    AssemblySceneHandle,
    {
      zoom?: number;
      viewportInsets?: ViewportInsets;
      explodeAmount?: number;
      onReady?: () => void;
      onPartSelect?: (partId: string | null) => void;
    }
  >(function AssemblyScene(
    { zoom = 1500, viewportInsets, explodeAmount, onReady, onPartSelect },
    ref,
  ) {
    return (
      <Canvas
        gl={{ toneMapping: ACESFilmicToneMapping, toneMappingExposure: 1.2 }}
        style={{ width: "100%", height: "100%" }}
      >
        <SceneContent
          handle={ref}
          zoom={zoom}
          viewportInsets={viewportInsets}
          explodeAmount={explodeAmount}
          onReady={onReady}
          onPartSelect={onPartSelect}
        />
      </Canvas>
    );
  }),
);

export default AssemblyScene;
