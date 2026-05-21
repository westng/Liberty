import { useEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { shopImageUrl } from "@/features/pet-store/services/petStorePresentation";
import type { PetBlindBoxPoolItem, PetStoreCatalogItem } from "@/shared/types/meeting";

type ThreeStagePhase = "idle" | "drawing" | "result";

type PrizeVisual = {
  itemKey: string;
  assetKey: string;
  color: string;
  isEmpty: boolean;
};

type PrizeCardMesh = THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial> & {
  userData: {
    baseAngle: number;
    baseY: number;
    baseZ: number;
    baseRadius: number;
    floatSpeed: number;
    orbitSpeed: number;
    scale: number;
    texture?: THREE.Texture;
    isEmpty: boolean;
  };
};

type ParticlePoints = THREE.Points<THREE.BufferGeometry, THREE.PointsMaterial> & {
  userData: {
    seeds: Float32Array;
  };
};

const SCENE_CENTER_Y = 0;

export function PetBlindBoxThreeStage({
  pool,
  phase,
  resultPrize,
}: {
  pool: PetBlindBoxPoolItem[];
  phase: ThreeStagePhase;
  resultPrize: PetStoreCatalogItem | null;
}) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const phaseRef = useRef(phase);
  const resultRef = useRef(resultPrize);
  const poolVisuals = useMemo(() => buildPrizeVisuals(pool), [pool]);

  useEffect(() => {
    phaseRef.current = phase;
    resultRef.current = resultPrize;
  }, [phase, resultPrize]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) {
      return;
    }

    let width = host.clientWidth || 720;
    let height = host.clientHeight || 360;
    let animationFrame = 0;
    const clock = new THREE.Clock();
    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(42, width / height, 0.1, 120);
    camera.position.set(0, 0, 10.8);
    camera.lookAt(0, SCENE_CENTER_Y, 0);

    const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.setSize(width, height);
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    host.appendChild(renderer.domElement);

    const root = new THREE.Group();
    scene.add(root);

    const textureLoader = new THREE.TextureLoader();
    const cards = createPrizeCards(poolVisuals, textureLoader);
    cards.forEach((card) => root.add(card));

    const box = createEnergyBox();
    root.add(box);

    const particles = createParticles();
    scene.add(particles);

    const resultCard = createResultCard(textureLoader, resultRef.current);
    resultCard.visible = false;
    scene.add(resultCard);

    const glow = createGlow();
    scene.add(glow);

    let activePhase = phaseRef.current;
    let phaseStartedAt = 0;

    function resize() {
      if (!host) {
        return;
      }
      width = host.clientWidth || width;
      height = host.clientHeight || height;
      camera.aspect = width / height;
      camera.lookAt(0, SCENE_CENTER_Y, 0);
      camera.updateProjectionMatrix();
      renderer.setSize(width, height);
    }

    function animate() {
      const elapsed = clock.getElapsedTime();
      const currentPhase = phaseRef.current;
      if (currentPhase !== activePhase) {
        activePhase = currentPhase;
        phaseStartedAt = elapsed;
      }
      const phaseElapsed = elapsed - phaseStartedAt;
      const isDrawing = currentPhase === "drawing";
      const showPoolCards = currentPhase === "idle" || currentPhase === "drawing";
      const drawCharge = isDrawing ? easeOutCubic(clamp01(phaseElapsed / 2.8)) : 0;
      const drawConverge = isDrawing ? easeInOutCubic(clamp01((phaseElapsed - 3.05) / 1.3)) : 0;
      const drawBurst = isDrawing ? smoothstep(4.32, 4.85, phaseElapsed) : 0;
      const drawFade = isDrawing ? smoothstep(4.55, 4.95, phaseElapsed) : 0;
      const resultPrize = resultRef.current;
      const isEmpty = resultPrize?.itemType === "none";
      root.rotation.y = elapsed * (currentPhase === "drawing" ? 0.22 + drawCharge * 0.2 : 0.12);
      box.rotation.x = elapsed * 0.38;
      box.rotation.y = elapsed * (currentPhase === "drawing" ? 0.72 + drawCharge * 0.6 : 0.62);
      box.scale.setScalar(currentPhase === "drawing" ? 1 + drawCharge * 0.16 + drawBurst * 0.22 + Math.sin(elapsed * 5.5) * 0.035 : 1);
      glow.scale.setScalar(currentPhase === "drawing" ? 1.02 + drawCharge * 0.22 + drawBurst * 0.42 + Math.sin(elapsed * 5.2) * 0.05 : 0.9);
      glow.material.opacity = currentPhase === "drawing" ? 0.28 + drawCharge * 0.18 + drawBurst * 0.22 : 0.18;

      cards.forEach((card, index) => {
        const phaseOffset = index * 0.37;
        const orbitBoost = isDrawing ? 1.1 + drawCharge * 2.8 + drawConverge * 1.7 : 0.55;
        const spiralTwist = drawConverge * (2.6 + (index % 4) * 0.18);
        const angle = card.userData.baseAngle + elapsed * card.userData.orbitSpeed * orbitBoost + spiralTwist;
        const idleRadius = card.userData.baseRadius + Math.sin(elapsed * 0.7 + phaseOffset) * 0.18;
        const radius = isDrawing
          ? THREE.MathUtils.lerp(idleRadius * (1.02 + drawCharge * 0.08), 0.28 + (index % 5) * 0.055, drawConverge)
          : idleRadius;
        const floatY = SCENE_CENTER_Y + card.userData.baseY + Math.sin(elapsed * card.userData.floatSpeed + phaseOffset) * 0.22;
        const cardY = isDrawing
          ? THREE.MathUtils.lerp(floatY, SCENE_CENTER_Y + Math.sin(phaseOffset + elapsed * 2.4) * 0.06, drawConverge)
          : floatY;
        const burstLift = Math.sin(phaseOffset + elapsed * 4.6) * 0.28 * drawBurst;
        card.position.set(
          Math.cos(angle) * radius * (1 + drawBurst * 0.24),
          cardY + burstLift,
          Math.sin(angle) * radius * (1 + drawBurst * 0.24) + card.userData.baseZ * (1 - drawConverge),
        );
        card.rotation.set(
          -0.16 + Math.sin(elapsed * (1.2 + drawCharge * 3.2) + phaseOffset) * (0.16 + drawCharge * 0.1),
          -angle + Math.PI / 2 + Math.sin(elapsed * (0.9 + drawCharge * 2.6) + phaseOffset) * (0.38 + drawCharge * 0.18),
          Math.sin(elapsed * (1.7 + drawCharge * 2.2) + phaseOffset) * (0.16 + drawCharge * 0.08),
        );
        card.scale.setScalar(card.userData.scale * (isDrawing ? 1.02 + drawCharge * 0.08 - drawConverge * 0.12 + drawBurst * 0.1 : 1));
        card.visible = showPoolCards && drawFade < 0.99;
        card.material.opacity = 1;
      });

      animateParticles(particles, elapsed, phaseElapsed, currentPhase, isEmpty);
      updateResultCard(resultCard, textureLoader, resultPrize, currentPhase, elapsed);
      renderer.render(scene, camera);
      animationFrame = window.requestAnimationFrame(animate);
    }

    window.addEventListener("resize", resize);
    resize();
    animate();

    return () => {
      window.cancelAnimationFrame(animationFrame);
      window.removeEventListener("resize", resize);
      cards.forEach((card) => {
        card.geometry.dispose();
        card.material.map?.dispose();
        card.material.dispose();
      });
      box.geometry.dispose();
      box.material.dispose();
      particles.geometry.dispose();
      particles.material.dispose();
      resultCard.geometry.dispose();
      resultCard.material.map?.dispose();
      resultCard.material.dispose();
      glow.geometry.dispose();
      glow.material.dispose();
      renderer.dispose();
      host.removeChild(renderer.domElement);
    };
  }, [poolVisuals]);

  return <div className="pet-blind-box-three-stage" ref={hostRef} aria-hidden="true" />;
}

function buildPrizeVisuals(pool: PetBlindBoxPoolItem[]) {
  return pool.map((poolItem) => ({
    itemKey: poolItem.item.itemKey,
    assetKey: poolItem.item.assetKey,
    color: colorForType(poolItem.item.itemType),
    isEmpty: poolItem.item.itemType === "none",
  }));
}

function createPrizeCards(visuals: PrizeVisual[], textureLoader: THREE.TextureLoader) {
  const geometry = new THREE.PlaneGeometry(0.76, 0.96);
  return visuals.map((visual, index) => {
    const texture = textureLoader.load(shopImageUrl(visual.assetKey));
    texture.colorSpace = THREE.SRGBColorSpace;
    const material = new THREE.MeshBasicMaterial({
      map: texture,
      transparent: true,
      opacity: 1,
      side: THREE.DoubleSide,
      depthWrite: false,
    });
    const mesh = new THREE.Mesh(geometry.clone(), material) as PrizeCardMesh;
    const ring = index % 4;
    const angle = (index / Math.max(visuals.length, 1)) * Math.PI * 2 + ring * 0.22;
    mesh.userData.baseAngle = angle;
    mesh.userData.baseRadius = 2.85 + ring * 0.92 + Math.sin(index * 1.7) * 0.22;
    mesh.userData.baseY = ((index % 7) - 3) * 0.44;
    mesh.userData.baseZ = ((index % 5) - 2) * 0.34;
    mesh.userData.floatSpeed = 0.86 + (index % 6) * 0.1;
    mesh.userData.orbitSpeed = 0.32 + (index % 5) * 0.035;
    mesh.userData.scale = visual.isEmpty ? 0.88 : 1.02 + (index % 3) * 0.05;
    mesh.userData.texture = texture;
    mesh.userData.isEmpty = visual.isEmpty;
    return mesh;
  });
}

function createEnergyBox() {
  const geometry = new THREE.IcosahedronGeometry(1.05, 1);
  const material = new THREE.MeshBasicMaterial({
    color: new THREE.Color("#f6c04f"),
    transparent: true,
    opacity: 0.3,
    wireframe: true,
  });
  return new THREE.Mesh(geometry, material);
}

function createGlow() {
  const geometry = new THREE.SphereGeometry(1.48, 32, 32);
  const material = new THREE.MeshBasicMaterial({
    color: new THREE.Color("#f6c04f"),
    transparent: true,
    opacity: 0.18,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
  });
  return new THREE.Mesh(geometry, material);
}

function createParticles() {
  const count = 1480;
  const positions = new Float32Array(count * 3);
  const colors = new Float32Array(count * 3);
  const seeds = new Float32Array(count * 4);
  for (let index = 0; index < count; index += 1) {
    seeds[index * 4] = Math.random() * Math.PI * 2;
    seeds[index * 4 + 1] = 1.6 + Math.random() * 7.3;
    seeds[index * 4 + 2] = -3.35 + Math.random() * 6.7;
    seeds[index * 4 + 3] = Math.random();
    colors[index * 3] = 1;
    colors[index * 3 + 1] = 0.76 + Math.random() * 0.2;
    colors[index * 3 + 2] = 0.28 + Math.random() * 0.24;
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));
  const material = new THREE.PointsMaterial({
    size: 0.082,
    vertexColors: true,
    transparent: true,
    opacity: 0.84,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
  });
  const points = new THREE.Points(geometry, material) as ParticlePoints;
  points.userData.seeds = seeds;
  return points;
}

function animateParticles(points: ParticlePoints, elapsed: number, phaseElapsed: number, phase: ThreeStagePhase, emptyResult: boolean) {
  const positions = points.geometry.getAttribute("position") as THREE.BufferAttribute;
  const seeds = points.userData.seeds;
  const awaken = phase === "drawing" ? easeOutCubic(Math.min(1, phaseElapsed / 0.9)) : 0;
  for (let index = 0; index < positions.count; index += 1) {
    const angle = seeds[index * 4] + elapsed * (phase === "drawing" ? 2.2 : 0.6);
    const baseRadius = seeds[index * 4 + 1];
    const ySeed = seeds[index * 4 + 2];
    const pulse = (Math.sin(elapsed * 2.6 + seeds[index * 4 + 3] * 9) + 1) / 2;
    const radius = phase === "drawing"
      ? baseRadius * (0.2 + awaken * 0.4 + pulse * 0.28)
      : baseRadius * (0.78 + pulse * 0.18);
    const spread = phase === "result" ? (emptyResult ? 1.95 : 2.85) : 1.2;
    const ySpread = phase === "drawing" ? 0.46 + awaken * 0.22 : 0.86;
    positions.setXYZ(
      index,
      Math.cos(angle) * radius * spread,
      SCENE_CENTER_Y + ySeed * ySpread,
      Math.sin(angle) * radius * spread,
    );
  }
  points.material.color.set(emptyResult ? "#d9dee8" : "#ffd15f");
  points.material.opacity = phase === "idle" ? 0.58 : phase === "drawing" ? 0.98 : 0.82;
  positions.needsUpdate = true;
}

function easeOutCubic(value: number) {
  return 1 - Math.pow(1 - value, 3);
}

function easeInOutCubic(value: number) {
  return value < 0.5 ? 4 * value * value * value : 1 - Math.pow(-2 * value + 2, 3) / 2;
}

function smoothstep(edge0: number, edge1: number, value: number) {
  const nextValue = clamp01((value - edge0) / (edge1 - edge0));
  return nextValue * nextValue * (3 - 2 * nextValue);
}

function clamp01(value: number) {
  return Math.min(1, Math.max(0, value));
}

function createResultCard(textureLoader: THREE.TextureLoader, prize: PetStoreCatalogItem | null) {
  const geometry = new THREE.PlaneGeometry(1.42, 1.72);
  const material = new THREE.MeshBasicMaterial({
    transparent: true,
    opacity: 0,
    side: THREE.DoubleSide,
    depthWrite: false,
  });
  const mesh = new THREE.Mesh(geometry, material);
  applyResultTexture(mesh, textureLoader, prize);
  mesh.position.set(0, SCENE_CENTER_Y + 0.18, 1.25);
  return mesh;
}

function updateResultCard(
  mesh: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>,
  textureLoader: THREE.TextureLoader,
  prize: PetStoreCatalogItem | null,
  phase: ThreeStagePhase,
  elapsed: number,
) {
  applyResultTexture(mesh, textureLoader, prize);
  mesh.visible = Boolean(prize) && phase === "result";
  mesh.material.opacity = mesh.visible ? 0.94 : 0;
  mesh.position.y = SCENE_CENTER_Y + 0.18 + Math.sin(elapsed * 2) * 0.04;
  mesh.rotation.set(Math.sin(elapsed * 1.4) * 0.08, Math.sin(elapsed * 0.8) * 0.18, 0);
  mesh.scale.setScalar(mesh.visible ? 1.06 + Math.sin(elapsed * 3) * 0.03 : 0.7);
}

function applyResultTexture(
  mesh: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>,
  textureLoader: THREE.TextureLoader,
  prize: PetStoreCatalogItem | null,
) {
  if (!prize || mesh.userData.itemKey === prize.itemKey) {
    return;
  }
  mesh.userData.itemKey = prize.itemKey;
  mesh.material.map?.dispose();
  const texture = textureLoader.load(shopImageUrl(prize.assetKey));
  texture.colorSpace = THREE.SRGBColorSpace;
  mesh.material.map = texture;
  mesh.material.needsUpdate = true;
}

function colorForType(itemType: string) {
  if (itemType === "food") {
    return "#f58a4c";
  }
  if (itemType === "tool") {
    return "#f6c04f";
  }
  if (itemType === "cosmetic") {
    return "#ff7aa8";
  }
  if (itemType === "theme") {
    return "#61b86b";
  }
  if (itemType === "badge") {
    return "#5f7dff";
  }
  return "#8f96a3";
}
